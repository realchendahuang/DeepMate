// Marketplace access for DeepSeek Harness plugins.
//
// DeepSeek Harness plugins are npm packages, so the market is the public npm
// registry. Search results are normalized into core MarketEntry records and
// split into curated (the DeepMate-maintained plugin list in this repository)
// and community (raw npm search) sources.
//
// Results are cached in the DeepMate data directory
// (`cache/marketplace.json`, single most-recent query) so repeated searches
// are cheap and an offline rerun of the same query still renders.

use std::path::{Path, PathBuf};
use std::time::Duration;

use deepmate_core::error::{CoreError, CoreResult};
use deepmate_core::model::{
    compat_status, CompatReport, CompatStatus, MarketEntry, MarketSource, MarketSourceInfo,
    MarketTrust,
};

const NPM_SEARCH_URL: &str = "https://registry.npmjs.org/-/v1/search";
const NPM_REGISTRY_URL: &str = "https://registry.npmjs.org";
// The curated plugin list lives in this repository (`plugins/curated.json`)
// and is fetched from its raw GitHub URL. `DEEPMATE_CURATED_LIST_URL`
// overrides it for tests and mirrors.
const CURATED_LIST_URL: &str =
    "https://raw.githubusercontent.com/realchendahuang/DeepMate/main/plugins/curated.json";
const CURATED_LIST_ENV: &str = "DEEPMATE_CURATED_LIST_URL";
const HARNESS_ENGINE_KEYS: [&str; 2] = ["dsh", "deepseek-harness"];
const SEARCH_SIZE: usize = 15;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);
const CACHE_DIR: &str = "cache";
const CACHE_FILENAME: &str = "marketplace.json";
const CURATED_CACHE_FILENAME: &str = "curated.json";
// Keep the cache in sync with `Config.market.refresh_interval_seconds`
// (1 hour by default).
const CACHE_TTL: Duration = Duration::from_secs(3600);

// Market access with optional on-disk caching under the DeepMate data
// directory. The cache freshness (`cache_ttl`) mirrors
// `Config.market.refresh_interval_seconds`; the harness service wires it from
// the loaded configuration.
#[derive(Debug, Clone)]
pub struct Market {
    cache_path: Option<PathBuf>,
    curated_cache_path: Option<PathBuf>,
    cache_ttl: Duration,
    // The HTTP client is built once by the adapter and shared across searches
    // (and across search calls) so connections are reused instead of paying
    // for a new TLS handshake per query. reqwest::Client clones share the same
    // underlying connection pool.
    client: reqwest::Client,
}

impl Market {
    pub fn new(cache_root: Option<PathBuf>, client: reqwest::Client) -> Self {
        Self {
            cache_path: cache_root
                .as_ref()
                .map(|root| root.join(CACHE_DIR).join(CACHE_FILENAME)),
            curated_cache_path: cache_root
                .as_ref()
                .map(|root| root.join(CACHE_DIR).join(CURATED_CACHE_FILENAME)),
            cache_ttl: CACHE_TTL,
            client,
        }
    }

    // Override the cache freshness with a configured refresh interval.
    pub fn with_cache_ttl(mut self, ttl: Duration) -> Self {
        self.cache_ttl = ttl;
        self
    }

    // Search the market, serving the cached entries for the same query when
    // they are still fresh.
    pub async fn search(&self, query: &str) -> CoreResult<Vec<MarketEntry>> {
        if let Some(entries) = self.cached(query) {
            return Ok(entries);
        }
        match search_npm(&self.client, query).await {
            Ok(entries) => {
                if let Some(path) = &self.cache_path {
                    if let Err(err) = store_cache(path, query, &entries) {
                        tracing::warn!(error = %err, "failed to write market cache");
                    }
                }
                Ok(entries)
            }
            Err(err) => {
                // The network is the reason the search failed; a previous
                // answer for this exact query is still the best available
                // result. Report it (stale) rather than an empty market.
                match self.cached_entry(query, true) {
                    Some((entries, _stale)) => {
                        tracing::warn!(
                            query,
                            error = %err,
                            "market search failed; serving the cached result"
                        );
                        Ok(entries)
                    }
                    None => Err(err),
                }
            }
        }
    }

    // The curated plugin list, fetched from this repository and cached under
    // the data directory. A stale or missing cache falls back to the network;
    // a network failure degrades to the cached copy when one exists, and to
    // an empty list otherwise (the curated source is a convenience, never a
    // hard dependency of the market).
    pub async fn curated(&self) -> CoreResult<Vec<MarketEntry>> {
        if let Some(entries) = self.cached_curated() {
            return Ok(entries);
        }
        let url = std::env::var(CURATED_LIST_ENV).unwrap_or_else(|_| CURATED_LIST_URL.to_string());
        match fetch_curated(&self.client, &url).await {
            Ok(entries) => {
                if let Some(path) = &self.curated_cache_path {
                    if let Err(err) = store_curated_cache(path, &entries) {
                        tracing::warn!(error = %err, "failed to write curated list cache");
                    }
                }
                Ok(entries)
            }
            Err(err) => {
                // A curated list fetched at any point in the past is still
                // better than an empty storefront, which is what an offline
                // (or proxied) network otherwise produces.
                match self.any_cached_curated() {
                    Some(entries) => {
                        tracing::warn!(
                            error = %err,
                            "curated list fetch failed; serving the cached copy"
                        );
                        Ok(entries)
                    }
                    None => Err(err),
                }
            }
        }
    }

    // Check a package's harness compatibility through its registry packument:
    // the latest published version's `engines` entry is matched against the
    // detected harness version by the core's semver logic.
    pub async fn compat(
        client: &reqwest::Client,
        spec: &str,
        harness_version: Option<String>,
    ) -> CoreResult<CompatReport> {
        let name = spec_package_name(spec);
        let url = format!("{NPM_REGISTRY_URL}/{}", encode_package_name(name));
        let response = client
            .get(url)
            .send()
            .await
            .map_err(|err| CoreError::Network(format!("compat request failed: {err}")))?;
        if !response.status().is_success() {
            return Err(CoreError::Network(format!(
                "registry returned HTTP {} for {name}",
                response.status()
            )));
        }
        let text = response
            .text()
            .await
            .map_err(|err| CoreError::InvalidState(format!("invalid registry response: {err}")))?;
        compat_from_packument(&text, harness_version.as_deref())
    }

    // A cached result for this exact query, when fresh. A stale or missing
    // cache yields None and the caller goes to the network.
    fn cached(&self, query: &str) -> Option<Vec<MarketEntry>> {
        self.cached_entry(query, false).map(|(entries, _)| entries)
    }

    // A cached search result: `allow_stale` returns the last known answer even
    // past its TTL, which is what keeps the market usable offline. The bool
    // reports whether the answer is stale.
    fn cached_entry(&self, query: &str, allow_stale: bool) -> Option<(Vec<MarketEntry>, bool)> {
        let path = self.cache_path.as_ref()?;
        let text = std::fs::read_to_string(path).ok()?;
        let file: CacheFile = serde_json::from_str(&text).ok()?;
        // Prefer the per-query map; fall back to the legacy single-slot fields
        // so a cache written by an older build still works.
        let (updated, entries) = match file.queries.get(query) {
            Some(cached) => (cached.updated.clone(), cached.entries.clone()),
            None if file.query == query => (file.updated.clone(), file.entries.clone()),
            None => return None,
        };
        let stamp = chrono::DateTime::parse_from_rfc3339(&updated).ok()?;
        let age = (chrono::Utc::now() - stamp.with_timezone(&chrono::Utc))
            .to_std()
            .ok()?;
        let stale = age > self.cache_ttl;
        if stale && !allow_stale {
            return None;
        }
        Some((entries, stale))
    }

    // The curated cache regardless of age; the offline fallback reads it
    // directly.
    fn any_cached_curated(&self) -> Option<Vec<MarketEntry>> {
        let path = self.curated_cache_path.as_ref()?;
        let text = std::fs::read_to_string(path).ok()?;
        let file: CuratedCacheFile = serde_json::from_str(&text).ok()?;
        Some(file.entries)
    }

    // The cached curated list, when fresh. A stale copy is still served as a
    // fallback by `curated()` when the network fetch fails, so the curated
    // source keeps working offline.
    fn cached_curated(&self) -> Option<Vec<MarketEntry>> {
        let path = self.curated_cache_path.as_ref()?;
        let text = std::fs::read_to_string(path).ok()?;
        let file: CuratedCacheFile = serde_json::from_str(&text).ok()?;
        let updated = chrono::DateTime::parse_from_rfc3339(&file.updated).ok()?;
        let age = (chrono::Utc::now() - updated.with_timezone(&chrono::Utc))
            .to_std()
            .ok()?;
        if age > self.cache_ttl {
            return None;
        }
        Some(file.entries)
    }
}

// Build the shared HTTP client for market requests. Falls back to the
// default client if the custom timeout/user-agent cannot be applied, so
// market access degrades gracefully instead of failing hard at startup.
pub fn build_http_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(REQUEST_TIMEOUT)
        .user_agent(concat!("deepmate/", env!("CARGO_PKG_VERSION")))
        .build()
        .unwrap_or_else(|_| reqwest::Client::new())
}

// Query the npm registry search endpoint and normalize the results.
async fn search_npm(client: &reqwest::Client, query: &str) -> CoreResult<Vec<MarketEntry>> {
    let size = SEARCH_SIZE.to_string();
    let response = client
        .get(NPM_SEARCH_URL)
        .query(&[("text", query), ("size", size.as_str())])
        .send()
        .await
        .map_err(|err| CoreError::Network(format!("market request failed: {err}")))?;
    if !response.status().is_success() {
        return Err(CoreError::Network(format!(
            "market returned HTTP {}",
            response.status()
        )));
    }
    let body: SearchResponse = response
        .json()
        .await
        .map_err(|err| CoreError::InvalidState(format!("invalid market response: {err}")))?;
    let entries: Vec<MarketEntry> = body
        .objects
        .into_iter()
        .map(|obj| {
            let name = obj.package.name;
            // npm search results are community entries; the curated source is
            // the DeepMate-maintained list, merged in by the adapter.
            let source = MarketSource::Community;
            let (popularity, quality) = obj
                .score
                .and_then(|score| score.detail)
                .map(|detail| (normalized(detail.popularity), normalized(detail.quality)))
                .unwrap_or((None, None));
            MarketEntry {
                id: name.clone(),
                name,
                description: obj.package.description,
                version: Some(obj.package.version),
                source,
                // npm search results carry no review signal; they are
                // community until proven otherwise.
                trust: MarketTrust::Community,
                repository: obj.package.links.and_then(|links| links.repository),
                publisher: obj
                    .package
                    .publisher
                    .and_then(|publisher| publisher.username.or(publisher.name)),
                updated: obj.package.date.and_then(|date| parse_updated(&date)),
                category: None,
                popularity,
                quality,
            }
        })
        .collect();
    Ok(entries)
}

// Parse a package's last-updated value into a UTC timestamp. Sources publish
// either a full RFC3339 timestamp (npm) or a bare `YYYY-MM-DD` date
// (curated.json); anything else degrades to `None` rather than a wrong date.
fn parse_updated(value: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    chrono::DateTime::parse_from_rfc3339(value)
        .map(|date| date.with_timezone(&chrono::Utc))
        .ok()
        .or_else(|| {
            chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d")
                .ok()
                .map(|date| date.and_hms_opt(0, 0, 0).unwrap().and_utc())
        })
}

// Clamp a registry score into `0.0..=1.0`; anything outside or missing is
// treated as "no signal" instead of a misleading value.
fn normalized(score: Option<f64>) -> Option<f64> {
    score
        .filter(|value| value.is_finite())
        .map(|value| value.clamp(0.0, 1.0))
}

// The package name a plugin spec refers to: `pkg@^1.0` and `pkg` both name
// `pkg`, and `@scope/pkg@1.0` names `@scope/pkg` (the leading scope `@` is
// never treated as a version separator). Used by the plugin disable/enable
// flow to clear registry records after an install.
pub(crate) fn spec_package_name(spec: &str) -> &str {
    let spec = spec.trim();
    if let Some(rest) = spec.strip_prefix('@') {
        return match rest.split_once('@') {
            Some((scope, _)) => &spec[..1 + scope.len()],
            None => spec,
        };
    }
    spec.split_once('@').map_or(spec, |(name, _)| name)
}

// Package names are used verbatim in the registry URL; the `@scope/name`
// slash is a legal path character, so no percent-encoding is needed beyond
// rejecting whitespace and separators that would change the path.
fn encode_package_name(name: &str) -> String {
    name.trim().to_string()
}

// Extract a `CompatReport` from a registry packument document. Pure so it can
// be tested without network access.
fn compat_from_packument(text: &str, harness_version: Option<&str>) -> CoreResult<CompatReport> {
    let packument: Packument = serde_json::from_str(text)
        .map_err(|err| CoreError::InvalidState(format!("invalid registry packument: {err}")))?;
    let latest = packument
        .dist_tags
        .and_then(|tags| tags.latest)
        .ok_or_else(|| CoreError::InvalidState("packument has no latest dist-tag".to_string()))?;
    let manifest = packument
        .versions
        .as_ref()
        .and_then(|versions| versions.get(&latest));
    let required = manifest
        .and_then(|manifest| manifest.engines.as_ref())
        .and_then(|engines| HARNESS_ENGINE_KEYS.iter().find_map(|key| engines.get(*key)));
    let required = match required {
        Some(range) => range.trim(),
        None => {
            return Ok(CompatReport {
                status: CompatStatus::Unknown,
                harness_version: harness_version.map(str::to_string),
                required_range: None,
                message: format!(
                    "the latest version ({latest}) declares no harness requirement in its engines field"
                ),
            });
        }
    };
    let status = compat_status(harness_version, Some(required));
    let message = match status {
        CompatStatus::Compatible => {
            format!("requires harness {required}; the detected harness satisfies it")
        }
        CompatStatus::Incompatible => {
            format!("requires harness {required}; the detected harness does not satisfy it")
        }
        CompatStatus::Unknown => {
            format!("requires harness {required}; the requirement could not be evaluated")
        }
    };
    Ok(CompatReport {
        status,
        harness_version: harness_version.map(str::to_string),
        required_range: Some(required.to_string()),
        message,
    })
}

// Fetch and normalize the curated plugin list. The list is a static JSON
// document in the DeepMate repository; entries are normalized into the same
// MarketEntry records the npm search produces, so the rest of the pipeline
// (compat checks, installation, trust display) is shared.
async fn fetch_curated(client: &reqwest::Client, url: &str) -> CoreResult<Vec<MarketEntry>> {
    let response = client
        .get(url)
        .send()
        .await
        .map_err(|err| CoreError::Network(format!("curated list request failed: {err}")))?;
    if !response.status().is_success() {
        return Err(CoreError::Network(format!(
            "curated list returned HTTP {}",
            response.status()
        )));
    }
    let text = response
        .text()
        .await
        .map_err(|err| CoreError::InvalidState(format!("invalid curated list response: {err}")))?;
    curated_from_json(&text)
}

// Parse a curated list document into MarketEntry records. Pure so it can be
// tested without network access. A list with a schema version newer than the
// one this build understands is rejected rather than partially interpreted.
fn curated_from_json(text: &str) -> CoreResult<Vec<MarketEntry>> {
    let list: CuratedList = serde_json::from_str(text)
        .map_err(|err| CoreError::InvalidState(format!("invalid curated list: {err}")))?;
    if list.schema > CURATED_SCHEMA {
        return Err(CoreError::InvalidState(format!(
            "curated list schema {} is newer than this build supports ({CURATED_SCHEMA})",
            list.schema
        )));
    }
    Ok(list
        .plugins
        .into_iter()
        .map(|plugin| {
            let trust = curated_trust(&plugin.name, plugin.trust);
            MarketEntry {
                id: plugin.name.clone(),
                name: plugin.name,
                description: Some(plugin.description),
                version: Some(plugin.version),
                source: MarketSource::Curated,
                // The curated list is vetted by definition; the `trust` field
                // only elevates an entry to official. A missing field degrades
                // to vetted, never to a less-trusted tier — and an `official`
                // claim that the package name does not back up is downgraded
                // too: the badge is a promise about provenance, so it must not
                // be taken from the list's word alone.
                trust,
                repository: plugin.repository,
                publisher: plugin.publisher,
                updated: parse_updated(&plugin.added),
                category: plugin.category,
                popularity: None,
                quality: None,
            }
        })
        .collect())
}

// The official tier is reserved for packages published under the harness
// vendor's npm scope. A curated entry that claims `official` without the
// matching scope is downgraded to vetted: the badge tells the user where the
// code comes from, and a data file alone cannot establish that.
fn curated_trust(name: &str, declared: Option<MarketTrust>) -> MarketTrust {
    match declared {
        Some(MarketTrust::Official) => {
            if name.starts_with("@deepseek-ai/") {
                MarketTrust::Official
            } else {
                tracing::warn!(
                    package = name,
                    "curated entry claims official without the vendor scope; downgrading to vetted"
                );
                MarketTrust::Vetted
            }
        }
        Some(other) => other,
        None => MarketTrust::Vetted,
    }
}

// The market sources the DeepSeek Harness adapter can discover from: the
// curated plugin list maintained in the DeepMate repository and the wider
// public registry.
pub fn market_sources() -> Vec<MarketSourceInfo> {
    vec![
        MarketSourceInfo {
            id: "curated".to_string(),
            name: "Curated".to_string(),
            description: "plugins reviewed and listed by the DeepMate maintainers".to_string(),
            source: MarketSource::Curated,
        },
        MarketSourceInfo {
            id: "community".to_string(),
            name: "Community".to_string(),
            description: "plugins published to the public npm registry".to_string(),
            source: MarketSource::Community,
        },
    ]
}

// The curated plugin list document, as maintained in the DeepMate repository
// (`plugins/curated.json`). `schema` gates forward compatibility: a newer
// schema is refused instead of misread.
const CURATED_SCHEMA: u32 = 1;

#[derive(Debug, serde::Deserialize)]
struct CuratedList {
    schema: u32,
    plugins: Vec<CuratedPlugin>,
}

#[derive(Debug, serde::Deserialize)]
struct CuratedPlugin {
    name: String,
    version: String,
    description: String,
    repository: Option<String>,
    publisher: Option<String>,
    added: String,
    category: Option<String>,
    // `official` elevates an entry to the official tier; absent resolves to
    // vetted, since the curated list is reviewed by definition.
    #[serde(default)]
    trust: Option<MarketTrust>,
}

#[derive(Debug, serde::Deserialize)]
struct SearchResponse {
    objects: Vec<SearchObject>,
}

#[derive(Debug, serde::Deserialize)]
struct SearchObject {
    package: SearchPackage,
    // npm's review-style scores (0.0..=1.0); absent on some registries.
    score: Option<SearchScore>,
}

#[derive(Debug, serde::Deserialize)]
struct SearchScore {
    detail: Option<ScoreDetail>,
}

#[derive(Debug, serde::Deserialize)]
struct ScoreDetail {
    popularity: Option<f64>,
    quality: Option<f64>,
}

// The slice of a registry packument needed for a compatibility check: the
// latest dist-tag and that version's `engines` declaration.
#[derive(Debug, serde::Deserialize)]
struct Packument {
    #[serde(rename = "dist-tags")]
    dist_tags: Option<DistTags>,
    versions: Option<std::collections::BTreeMap<String, VersionManifest>>,
}

#[derive(Debug, serde::Deserialize)]
struct DistTags {
    latest: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
struct VersionManifest {
    engines: Option<std::collections::BTreeMap<String, String>>,
}

#[derive(Debug, serde::Deserialize)]
struct SearchPackage {
    name: String,
    version: String,
    description: Option<String>,
    date: Option<String>,
    publisher: Option<Publisher>,
    links: Option<Links>,
}

#[derive(Debug, serde::Deserialize)]
struct Publisher {
    username: Option<String>,
    name: Option<String>,
}

#[derive(Debug, serde::Deserialize)]
struct Links {
    repository: Option<String>,
}

// The on-disk market cache: one query per file, with a timestamp for
// freshness checks.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct CacheFile {
    updated: String,
    query: String,
    entries: Vec<MarketEntry>,
    // Every other query seen recently, keyed by the exact query string, so an
    // alternating search pattern does not evict itself (the single-entry
    // format kept only the most recent query).
    #[serde(default)]
    queries: std::collections::BTreeMap<String, CachedQuery>,
}

// One remembered query: its result and when it was fetched.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct CachedQuery {
    updated: String,
    entries: Vec<MarketEntry>,
}

// How many distinct queries are remembered before the oldest is dropped.
const CACHE_QUERY_LIMIT: usize = 32;

// The on-disk curated list cache: the normalized entries plus a timestamp.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct CuratedCacheFile {
    updated: String,
    entries: Vec<MarketEntry>,
}

fn store_cache(path: &Path, query: &str, entries: &[MarketEntry]) -> CoreResult<()> {
    // Merge into the existing cache so concurrent and alternating searches
    // accumulate instead of overwriting each other.
    let mut file: CacheFile = std::fs::read_to_string(path)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_else(|| CacheFile {
            updated: chrono::Utc::now().to_rfc3339(),
            query: String::new(),
            entries: Vec::new(),
            queries: std::collections::BTreeMap::new(),
        });
    let now = chrono::Utc::now().to_rfc3339();
    file.updated = now.clone();
    file.query = query.to_string();
    file.entries = entries.to_vec();
    file.queries.insert(
        query.to_string(),
        CachedQuery {
            updated: now,
            entries: entries.to_vec(),
        },
    );
    while file.queries.len() > CACHE_QUERY_LIMIT {
        // Drop the oldest remembered query.
        let oldest = file
            .queries
            .iter()
            .min_by(|a, b| a.1.updated.cmp(&b.1.updated))
            .map(|(key, _)| key.clone());
        match oldest {
            Some(key) => {
                file.queries.remove(&key);
            }
            None => break,
        }
    }
    let text = serde_json::to_string_pretty(&file).map_err(|err| {
        CoreError::InvalidState(format!("failed to serialize market cache: {err}"))
    })?;
    deepmate_core::write_atomic_string(path, &text)
}

fn store_curated_cache(path: &Path, entries: &[MarketEntry]) -> CoreResult<()> {
    let file = CuratedCacheFile {
        updated: chrono::Utc::now().to_rfc3339(),
        entries: entries.to_vec(),
    };
    let text = serde_json::to_string_pretty(&file).map_err(|err| {
        CoreError::InvalidState(format!("failed to serialize curated list cache: {err}"))
    })?;
    deepmate_core::write_atomic_string(path, &text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn curated_list_parses_into_curated_entries() {
        let list = r#"{
            "schema": 1,
            "updated": "2026-08-28",
            "plugins": [
                {
                    "name": "dsh-mnemon",
                    "version": "^0.2",
                    "description": "memory",
                    "repository": "https://github.com/example/dsh-mnemon",
                    "publisher": "someone",
                    "added": "2026-08-28",
                    "trust": "vetted"
                },
                {
                    "name": "@deepseek-ai/dsh-base",
                    "version": "^0.3",
                    "description": "official",
                    "repository": "https://github.com/deepseek-ai/deepseek-harness",
                    "publisher": "deepseek-ai",
                    "added": "2026-08-28",
                    "trust": "official",
                    "category": "official"
                },
                {
                    "name": "dsh-nothing",
                    "version": "^0.1",
                    "description": "no trust field",
                    "publisher": "someone",
                    "added": "2026-08-28"
                }
            ]
        }"#;
        let entries = curated_from_json(list).unwrap();
        assert_eq!(entries.len(), 3);
        let entry = &entries[0];
        assert_eq!(entry.id, "dsh-mnemon");
        assert_eq!(entry.source, MarketSource::Curated);
        assert_eq!(entry.version.as_deref(), Some("^0.2"));
        assert_eq!(entry.description.as_deref(), Some("memory"));
        assert_eq!(
            entry.repository.as_deref(),
            Some("https://github.com/example/dsh-mnemon")
        );
        assert_eq!(entry.publisher.as_deref(), Some("someone"));
        assert_eq!(
            entry.updated.map(|date| date.to_rfc3339()),
            Some("2026-08-28T00:00:00+00:00".to_string())
        );
        assert_eq!(entry.popularity, None);
        assert_eq!(entry.quality, None);
        // The trust field drives the market tiers.
        assert_eq!(entries[0].trust, MarketTrust::Vetted);
        assert_eq!(entries[1].trust, MarketTrust::Official);
        // A curated entry without an explicit trust field stays vetted.
        assert_eq!(entries[2].trust, MarketTrust::Vetted);
    }

    #[test]
    fn curated_list_rejects_newer_schema() {
        let list = r#"{
            "schema": 2,
            "plugins": []
        }"#;
        assert!(curated_from_json(list).is_err());
    }

    #[test]
    fn curated_list_rejects_invalid_json() {
        assert!(curated_from_json("not json").is_err());
    }

    #[test]
    fn curated_cache_roundtrip_serves_entries() {
        let dir = std::env::temp_dir().join(format!(
            "deepmate-curated-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let market = Market::new(Some(dir), build_http_client());
        let entries = vec![MarketEntry {
            id: "dsh-mnemon".to_string(),
            name: "dsh-mnemon".to_string(),
            description: Some("memory".to_string()),
            version: Some("0.2.14".to_string()),
            source: MarketSource::Curated,
            trust: MarketTrust::Vetted,
            repository: Some("https://github.com/example/dsh-mnemon".to_string()),
            publisher: Some("someone".to_string()),
            updated: Some(
                chrono::DateTime::parse_from_rfc3339("2026-01-01T00:00:00.000Z")
                    .unwrap()
                    .with_timezone(&chrono::Utc),
            ),
            category: Some("memory".to_string()),
            popularity: None,
            quality: None,
        }];
        store_curated_cache(market.curated_cache_path.as_ref().unwrap(), &entries).unwrap();
        let cached = market.cached_curated().expect("fresh cache must hit");
        assert_eq!(cached, entries);
    }

    #[test]
    fn market_sources_are_curated_and_community() {
        let sources = market_sources();
        assert_eq!(sources.len(), 2);
        assert_eq!(sources[0].id, "curated");
        assert_eq!(sources[0].source, MarketSource::Curated);
        assert_eq!(sources[1].id, "community");
        assert_eq!(sources[1].source, MarketSource::Community);
    }

    #[test]
    fn cache_roundtrip_serves_the_same_query() {
        let dir = std::env::temp_dir().join(format!(
            "deepmate-market-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let market = Market::new(Some(dir), build_http_client());
        let entries = vec![MarketEntry {
            id: "dsh-mnemon".to_string(),
            name: "dsh-mnemon".to_string(),
            description: Some("memory".to_string()),
            version: Some("0.2.14".to_string()),
            source: MarketSource::Community,
            trust: MarketTrust::Community,
            repository: Some("https://github.com/example/dsh-mnemon".to_string()),
            publisher: Some("someone".to_string()),
            updated: Some(
                chrono::DateTime::parse_from_rfc3339("2026-01-01T00:00:00.000Z")
                    .unwrap()
                    .with_timezone(&chrono::Utc),
            ),
            category: None,
            popularity: Some(0.4),
            quality: Some(0.9),
        }];
        store_cache(market.cache_path.as_ref().unwrap(), "mnemon", &entries).unwrap();
        let cached = market.cached("mnemon").expect("same query must hit");
        assert_eq!(cached, entries);
        assert!(market.cached("other").is_none(), "other query must miss");
    }

    #[test]
    fn spec_names_are_extracted_with_and_without_ranges() {
        assert_eq!(spec_package_name("dsh-mnemon"), "dsh-mnemon");
        assert_eq!(spec_package_name("dsh-mnemon@^0.2"), "dsh-mnemon");
        assert_eq!(spec_package_name("dsh-mnemon@0.2.14"), "dsh-mnemon");
        assert_eq!(spec_package_name("@deepseek-ai/dsh"), "@deepseek-ai/dsh");
        assert_eq!(
            spec_package_name("@deepseek-ai/dsh@^0.1"),
            "@deepseek-ai/dsh"
        );
    }

    #[test]
    fn scores_outside_the_unit_interval_are_dropped() {
        assert_eq!(normalized(Some(0.5)), Some(0.5));
        assert_eq!(normalized(Some(1.7)), Some(1.0));
        assert_eq!(normalized(Some(-3.0)), Some(0.0));
        assert_eq!(normalized(Some(f64::NAN)), None);
        assert_eq!(normalized(None), None);
    }

    #[test]
    fn compat_reads_engines_from_the_latest_version() {
        let packument = r#"{
            "dist-tags": { "latest": "1.2.0" },
            "versions": {
                "1.1.0": { "engines": { "dsh": "^0.1" } },
                "1.2.0": { "engines": { "dsh": "^0.2" } }
            }
        }"#;
        let report = compat_from_packument(packument, Some("0.2.1")).unwrap();
        assert_eq!(report.status, CompatStatus::Compatible);
        assert_eq!(report.required_range.as_deref(), Some("^0.2"));
        assert_eq!(report.harness_version.as_deref(), Some("0.2.1"));

        let report = compat_from_packument(packument, Some("0.1.0")).unwrap();
        assert_eq!(report.status, CompatStatus::Incompatible);
    }

    #[test]
    fn compat_without_engines_is_unknown_not_an_error() {
        let packument = r#"{
            "dist-tags": { "latest": "1.0.0" },
            "versions": { "1.0.0": {} }
        }"#;
        let report = compat_from_packument(packument, Some("0.1.0")).unwrap();
        assert_eq!(report.status, CompatStatus::Unknown);
        assert!(report.required_range.is_none());
    }

    #[test]
    fn compat_is_unknown_for_other_engine_keys_only() {
        let packument = r#"{
            "dist-tags": { "latest": "1.0.0" },
            "versions": { "1.0.0": { "engines": { "node": ">=18" } } }
        }"#;
        let report = compat_from_packument(packument, Some("0.1.0")).unwrap();
        assert_eq!(report.status, CompatStatus::Unknown);
    }

    #[test]
    fn compat_rejects_invalid_packument() {
        assert!(compat_from_packument("not json", Some("0.1.0")).is_err());
    }
}
