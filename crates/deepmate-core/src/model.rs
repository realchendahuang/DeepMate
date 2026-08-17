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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Provider {
    pub id: String,
    pub name: String,
    pub kind: String,
}

// Model entry (normalized representation).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Model {
    pub id: String,
    pub name: String,
    pub provider: Option<String>,
}

// Plugin entry (normalized representation).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Plugin {
    pub id: String,
    pub name: String,
    pub version: Option<String>,
    pub enabled: bool,
}
