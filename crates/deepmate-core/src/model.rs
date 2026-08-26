use serde::{Deserialize, Serialize};

// Basic information about a detected harness.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HarnessInfo {
    pub id: String,
    pub name: String,
    pub version: Option<String>,
    pub adapter_version: String,
}

// Coarse runtime status.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeStatusKind {
    Unknown,
    Installed,
    Running,
    Stopped,
    Error,
}

// Runtime status returned by an adapter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DoctorCheck {
    pub id: String,
    pub status: CheckStatus,
    pub summary: String,
    pub details: Option<String>,
    pub suggested_action: Option<String>,
}

// Status for a Doctor check.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CheckStatus {
    Pass,
    Warn,
    Fail,
    Skip,
}

// A full Doctor report.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DoctorReport {
    pub adapter_id: String,
    pub checks: Vec<DoctorCheck>,
}

// Harness profile (normalized representation).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Model {
    pub id: String,
    pub name: String,
    pub provider: Option<String>,
    pub context_window: Option<u64>,
    pub max_tokens: Option<u64>,
    // The input modalities the model accepts, e.g. `["text", "image"]`.
    pub input: Option<Vec<String>>,
    pub reasoning_efforts: Option<String>,
    pub compat: Option<String>,
}

// Plugin entry (normalized representation).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
}

// Where a market entry came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MarketSource {
    // Official, vetted sources (e.g. the harness vendor's npm scope).
    Curated,
    // Any other community-published source.
    Community,
}

// A market source DeepMate knows about (normalized representation).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
