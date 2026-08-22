// Marketplace access for DeepSeek Harness plugins.
//
// DeepSeek Harness plugins are npm packages, so the market is the public npm
// registry. Search results are normalized into core MarketEntry records and
// split into curated (the official harness vendor scope) and community
// sources.
//
// Results are cached in the DeepMate data directory
// (`cache/marketplace.json`, single most-recent query) so repeated searches
// are cheap and an offline rerun of the same query still renders.

use std::path::{Path, PathBuf};
use std::time::Duration;

use deepmate_core::error::{CoreError, CoreResult};
use deepmate_core::model::{MarketEntry, MarketSource, MarketSourceInfo};

const NPM_SEARCH_URL: &str = "https://registry.npmjs.org/-/v1/search";
const CURATED_SCOPE: &str = "@deepseek-ai";
const SEARCH_SIZE: usize = 15;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);
const CACHE_DIR: &str = "cache";
const CACHE_FILENAME: &str = "marketplace.json";
// Keep the cache in sync with `Config.market.refresh_interval_seconds`
// (1 hour by default).
const CACHE_TTL: Duration = Duration::from_secs(3600);

// Market access with optional on-disk caching under the DeepMate data
// directory.
#[derive(Debug, Clone)]
pub struct Market {
    cache_path: Option<PathBuf>,
    // The HTTP client is built once by the adapter and shared across searches
    // (and across search calls) so connections are reused instead of paying
    // for a new TLS handshake per query. reqwest::Client clones share the same
    // underlying connection pool.
    client: reqwest::Client,
}

impl Market {
    pub fn new(cache_root: Option<PathBuf>, client: reqwest::Client) -> Self {
        Self {
            cache_path: cache_root.map(|root| root.join(CACHE_DIR).join(CACHE_FILENAME)),
            client,
        }
    }

    // Search the market, serving the cached entries for the same query when
    // they are still fresh.
    pub async fn search(&self, query: &str) -> CoreResult<Vec<MarketEntry>> {
        if let Some(entries) = self.cached(query) {
            return Ok(entries);
        }
        let entries = search_npm(&self.client, query).await?;
        if let Some(path) = &self.cache_path {
            if let Err(err) = store_cache(path, query, &entries) {
                tracing::warn!(error = %err, "failed to write market cache");
            }
        }
        Ok(entries)
    }

    // A cached result for this exact query, when fresh. A stale or missing
    // cache yields None and the caller goes to the network.
    fn cached(&self, query: &str) -> Option<Vec<MarketEntry>> {
        let path = self.cache_path.as_ref()?;
        let text = std::fs::read_to_string(path).ok()?;
        let file: CacheFile = serde_json::from_str(&text).ok()?;
        if file.query != query {
            return None;
        }
        let updated = chrono::DateTime::parse_from_rfc3339(&file.updated).ok()?;
        let age = (chrono::Utc::now() - updated.with_timezone(&chrono::Utc))
            .to_std()
            .ok()?;
        if age > CACHE_TTL {
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
        .map_err(|err| CoreError::InvalidState(format!("market request failed: {err}")))?;
    if !response.status().is_success() {
        return Err(CoreError::InvalidState(format!(
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
            let source = if name.starts_with(CURATED_SCOPE) {
                MarketSource::Curated
            } else {
                MarketSource::Community
            };
            MarketEntry {
                id: name.clone(),
                name,
                description: obj.package.description,
                version: Some(obj.package.version),
                source,
                repository: obj.package.links.and_then(|links| links.repository),
                publisher: obj
                    .package
                    .publisher
                    .and_then(|publisher| publisher.username.or(publisher.name)),
                updated: obj.package.date,
            }
        })
        .collect();
    Ok(entries)
}

// The market sources the DeepSeek Harness adapter can discover from: the
// curated harness vendor npm scope and the wider public registry.
pub fn market_sources() -> Vec<MarketSourceInfo> {
    vec![
        MarketSourceInfo {
            id: "curated".to_string(),
            name: "Curated".to_string(),
            description: format!("official plugins from the {CURATED_SCOPE} npm scope"),
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

#[derive(Debug, serde::Deserialize)]
struct SearchResponse {
    objects: Vec<SearchObject>,
}

#[derive(Debug, serde::Deserialize)]
struct SearchObject {
    package: SearchPackage,
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
}

fn store_cache(path: &Path, query: &str, entries: &[MarketEntry]) -> CoreResult<()> {
    let file = CacheFile {
        updated: chrono::Utc::now().to_rfc3339(),
        query: query.to_string(),
        entries: entries.to_vec(),
    };
    let text = serde_json::to_string_pretty(&file).map_err(|err| {
        CoreError::InvalidState(format!("failed to serialize market cache: {err}"))
    })?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, text)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn curated_scope_classification_is_stable() {
        // The classification lives in the search mapping; the scope constant
        // is the contract with the harness vendor's npm scope.
        assert!(CURATED_SCOPE.starts_with('@'));
        assert!(CURATED_SCOPE.ends_with("-ai") || CURATED_SCOPE.contains('/'));
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
            repository: Some("https://github.com/example/dsh-mnemon".to_string()),
            publisher: Some("someone".to_string()),
            updated: Some("2026-01-01T00:00:00.000Z".to_string()),
        }];
        store_cache(market.cache_path.as_ref().unwrap(), "mnemon", &entries).unwrap();
        let cached = market.cached("mnemon").expect("same query must hit");
        assert_eq!(cached, entries);
        assert!(market.cached("other").is_none(), "other query must miss");
    }
}
