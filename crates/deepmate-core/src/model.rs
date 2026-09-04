use serde::{Deserialize, Serialize};
use specta::Type;

// Basic information about a detected harness.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct HarnessInfo {
    pub id: String,
    pub name: String,
    pub version: Option<String>,
    pub adapter_version: String,
}

// Coarse runtime status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeStatusKind {
    Unknown,
    Installed,
    Running,
    Stopped,
    Error,
}

// Runtime status returned by an adapter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct RuntimeStatus {
    pub kind: RuntimeStatusKind,
    pub pid: Option<u32>,
    pub message: Option<String>,
}

impl RuntimeStatus {
    pub fn unknown() -> Self {
        Self {
            kind: RuntimeStatusKind::Unknown,
            pid: None,
            message: None,
        }
    }

    pub fn installed() -> Self {
        Self {
            kind: RuntimeStatusKind::Installed,
            pid: None,
            message: None,
        }
    }

    pub fn running(pid: u32) -> Self {
        Self {
            kind: RuntimeStatusKind::Running,
            pid: Some(pid),
            message: None,
        }
    }

    pub fn stopped() -> Self {
        Self {
            kind: RuntimeStatusKind::Stopped,
            pid: None,
            message: None,
        }
    }
}

// A single Doctor check result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct DoctorCheck {
    pub id: String,
    pub status: CheckStatus,
    pub summary: String,
    pub details: Option<String>,
    pub suggested_action: Option<String>,
}

// Status for a Doctor check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum CheckStatus {
    Pass,
    Warn,
    Fail,
    Skip,
}

// A full Doctor report.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct DoctorReport {
    pub adapter_id: String,
    pub checks: Vec<DoctorCheck>,
}

// Harness profile (normalized representation).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct Profile {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
}

// Provider configuration (normalized representation).
//
// `compat` is an opaque capability block (e.g. `supportsStore`,
// `thinkingFormat`, `supportsReasoningEffort`) carried through untouched.
// It is serialized as a JSON string so the UI can offer a raw advanced
// editor without the core knowing every possible harness-specific flag.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct Provider {
    pub id: String,
    pub name: String,
    pub kind: String,
    // The wire protocol (`openai-responses`, `openai-completions`, ...).
    pub api: Option<String>,
    pub base_url: Option<String>,
    // The environment variable that holds the provider's secret. Only the
    // variable name is stored here; the secret itself lives in harness-owned
    // storage and is never read or written by DeepMate.
    pub api_key_env: Option<String>,
    pub compat: Option<String>,
}

// Model entry (normalized representation).
//
// `reasoning_efforts` and `compat` are opaque capability blocks (e.g.
// `{ "max": "max" }` / `{ "supportsStore": false, "thinkingFormat": ... }`)
// carried through as raw JSON strings for the same reason as `Provider::compat`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct Model {
    pub id: String,
    pub name: String,
    pub provider: Option<String>,
    // Exported to TypeScript as u32: token counts never approach 2^53, and
    // specta forbids u64 in bindings to avoid JS precision loss.
    #[specta(type = Option<u32>)]
    pub context_window: Option<u64>,
    #[specta(type = Option<u32>)]
    pub max_tokens: Option<u64>,
    // The input modalities the model accepts, e.g. `["text", "image"]`.
    pub input: Option<Vec<String>>,
    pub reasoning_efforts: Option<String>,
    pub compat: Option<String>,
}

// Plugin entry (normalized representation).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct Plugin {
    pub id: String,
    pub name: String,
    pub version: Option<String>,
    pub enabled: bool,
    // The harness profile the plugin belongs to. Empty when the adapter
    // cannot attribute the plugin to a profile.
    pub profile: String,
    // Latest version known from a marketplace check; `None` when no check
    // has been performed for this plugin.
    pub latest: Option<String>,
    // True when a marketplace check found a newer version than the one
    // currently installed.
    pub outdated: bool,
}

// A marketplace search result (normalized representation).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub struct MarketEntry {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub version: Option<String>,
    pub source: MarketSource,
    // Trust/provenance signals surfaced before installation. All optional:
    // a source may not expose any of them.
    pub repository: Option<String>,
    pub publisher: Option<String>,
    pub updated: Option<String>,
    // Functional category (e.g. "memory", "vision", "mcp"), when the source
    // publishes one. Absent for raw npm search results.
    #[serde(default)]
    pub category: Option<String>,
    // Normalized trust scores in `0.0..=1.0`, when the source publishes them.
    // Popularity proxies adoption; quality proxies engineering hygiene.
    #[serde(default)]
    pub popularity: Option<f64>,
    #[serde(default)]
    pub quality: Option<f64>,
}

impl Eq for MarketEntry {}

// Where a market entry came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum MarketSource {
    // Official, vetted sources (e.g. the harness vendor's npm scope).
    Curated,
    // Any other community-published source.
    Community,
}

// Outcome of a plugin compatibility check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum CompatStatus {
    // The plugin declares a requirement the active harness satisfies.
    Compatible,
    // The plugin declares a requirement the active harness does not satisfy.
    Incompatible,
    // Either side declares nothing, or a value could not be parsed. An
    // unknown verdict must never block an installation by itself.
    Unknown,
}

// The result of checking one plugin against the active harness.
//
// `message` is a human-readable detail line in English (like Doctor check
// summaries); the UI surfaces `status` through its own translations and uses
// `message` as supporting detail.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
pub struct CompatReport {
    pub status: CompatStatus,
    // The detected harness version the check ran against.
    pub harness_version: Option<String>,
    // The requirement the plugin declares, verbatim.
    pub required_range: Option<String>,
    pub message: String,
}

impl Eq for CompatReport {}

impl CompatReport {
    pub fn unknown(harness_version: Option<String>) -> Self {
        Self {
            status: CompatStatus::Unknown,
            harness_version,
            required_range: None,
            message: "no harness requirement is declared for this plugin".to_string(),
        }
    }
}

// Compare a plugin's declared harness requirement against the active harness
// version.
//
// `required` is a semver range (e.g. `^0.1`, `>=0.1.0-rc <0.2`). The
// harness's prerelease/build suffix is stripped before matching: a running
// `0.1.0-rc.6` counts as the `0.1` line, which is what a plugin author means
// by `^0.1` — strict semver would otherwise report every prerelease harness
// as incompatible with its own release range. Anything unparsable on either
// side — or absent — yields `Unknown` so a malformed declaration degrades to
// a non-blocking signal instead of a false refusal.
pub fn compat_status(harness: Option<&str>, required: Option<&str>) -> CompatStatus {
    let (Some(harness), Some(required)) = (harness, required) else {
        return CompatStatus::Unknown;
    };
    let harness = harness.trim().trim_start_matches('v');
    let required = required.trim();
    let harness = semver::Version::parse(harness).map(|version| semver::Version {
        pre: semver::Prerelease::EMPTY,
        build: semver::BuildMetadata::EMPTY,
        ..version
    });
    match (harness, semver::VersionReq::parse(required)) {
        (Ok(harness), Ok(req)) if req.matches(&harness) => CompatStatus::Compatible,
        (Ok(_), Ok(_)) => CompatStatus::Incompatible,
        _ => CompatStatus::Unknown,
    }
}

// A market source DeepMate knows about (normalized representation).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct MarketSourceInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub source: MarketSource,
}

// Compare an installed version against a latest version.
//
// Falls back to plain string inequality when either side is not a valid
// semantic version, so prerelease and loose ranges still get a conservative
// signal instead of an error.
pub fn is_outdated(installed: &str, latest: &str) -> bool {
    match (
        semver::Version::parse(installed),
        semver::Version::parse(latest),
    ) {
        (Ok(a), Ok(b)) => b > a,
        _ => installed != latest,
    }
}

#[cfg(test)]
mod tests {
    use super::{compat_status, CompatStatus};

    #[test]
    fn satisfied_range_is_compatible() {
        assert_eq!(
            compat_status(Some("0.1.0-rc.6"), Some("^0.1")),
            CompatStatus::Compatible
        );
        assert_eq!(
            compat_status(Some("1.4.2"), Some(">=1.0, <2.0")),
            CompatStatus::Compatible
        );
    }

    #[test]
    fn prerelease_counts_as_its_release_line() {
        // The real launcher reports prereleases like 0.1.0-rc.6; a plugin
        // declaring ^0.1 means the 0.1 line, not "no prereleases".
        assert_eq!(
            compat_status(Some("0.1.0-rc.6"), Some(">=0.1.0")),
            CompatStatus::Compatible
        );
        // A different line stays incompatible.
        assert_eq!(
            compat_status(Some("0.1.0-rc.6"), Some("^0.2")),
            CompatStatus::Incompatible
        );
    }

    #[test]
    fn missing_sides_are_unknown() {
        assert_eq!(compat_status(None, Some("^0.1")), CompatStatus::Unknown);
        assert_eq!(compat_status(Some("0.1.0"), None), CompatStatus::Unknown);
        assert_eq!(compat_status(None, None), CompatStatus::Unknown);
    }

    #[test]
    fn unparsable_values_are_unknown_never_incompatible() {
        assert_eq!(
            compat_status(Some("garbage"), Some("^0.1")),
            CompatStatus::Unknown
        );
        assert_eq!(
            compat_status(Some("0.1.0"), Some("latest")),
            CompatStatus::Unknown
        );
    }

    #[test]
    fn leading_v_is_tolerated() {
        assert_eq!(
            compat_status(Some("v0.1.0"), Some("^0.1")),
            CompatStatus::Compatible
        );
    }
}
