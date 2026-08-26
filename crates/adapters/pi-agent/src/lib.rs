// Pi Agent adapter.
//
// The second concrete adapter. It keeps Pi Agent's file and CLI knowledge
// behind the HarnessAdapter trait, and demonstrates the capability gate: Pi
// has providers, models and extensions, but no profiles, web runtime or
// marketplace, so those capabilities are reported as unsupported and the CLI
// and desktop UI reject them instead of showing empty lists.

use std::process::Command;
use std::sync::OnceLock;

use async_trait::async_trait;
use deepmate_core::adapter::{AdapterCapabilities, AdapterMetadata, Detection, HarnessAdapter};
use deepmate_core::error::CoreResult;
use deepmate_core::model::{
    CheckStatus, DoctorCheck, DoctorReport, HarnessInfo, Model, Plugin, Provider, RuntimeStatus,
    RuntimeStatusKind,
};

mod pi;

use pi::{list_models, list_plugins, list_providers};

const ADAPTER_ID: &str = "pi-agent";
const ADAPTER_NAME: &str = "Pi Agent";
const ADAPTER_VERSION: &str = "0.1.0";

// The adapter for Pi Agent.
pub struct PiAgentAdapter {
    cli_names: Vec<String>,
    cli_cache: OnceLock<Option<String>>,
}

impl Default for PiAgentAdapter {
    fn default() -> Self {
        Self::new()
    }
}

impl PiAgentAdapter {
    pub fn new() -> Self {
        Self {
            cli_names: vec!["pi".to_string()],
            cli_cache: OnceLock::new(),
        }
    }

    pub fn with_cli_names(mut self, names: Vec<String>) -> Self {
        self.cli_names = names;
        self
    }

    // Locate the `pi` CLI, probing each candidate once and caching the result.
    fn find_cli(&self) -> Option<String> {
        self.cli_cache
            .get_or_init(|| {
                self.cli_names.iter().find_map(|name| {
                    Command::new(name).arg("--version").output().ok()?;
                    Some(name.clone())
                })
            })
            .clone()
    }

    // Best-effort version from `--version` output.
    fn cli_version(&self) -> Option<String> {
        let cli = self.find_cli()?;
        let output = Command::new(&cli).arg("--version").output().ok()?;
        if !output.status.success() {
            return None;
        }
        let text = String::from_utf8_lossy(&output.stdout);
        text.lines().next().map(|line| line.trim().to_string())
    }
}

#[async_trait]
impl HarnessAdapter for PiAgentAdapter {
    fn metadata(&self) -> AdapterMetadata {
        AdapterMetadata {
            id: ADAPTER_ID.to_string(),
            name: ADAPTER_NAME.to_string(),
            version: ADAPTER_VERSION.to_string(),
        }
    }

    fn capabilities(&self) -> AdapterCapabilities {
        AdapterCapabilities {
            providers: true,
            models: true,
            plugins: true,
            snapshots: true,
            ..Default::default()
        }
    }

    async fn detect(&self) -> CoreResult<Detection> {
        let cli = self.find_cli();
        Ok(Detection {
            found: cli.is_some(),
            harness: cli.as_ref().map(|_cli| HarnessInfo {
                id: ADAPTER_ID.to_string(),
                name: ADAPTER_NAME.to_string(),
                version: self.cli_version(),
                adapter_version: ADAPTER_VERSION.to_string(),
            }),
            detail: cli.map(|cli| format!("found CLI: {cli}")),
        })
    }

    async fn status(&self) -> CoreResult<RuntimeStatus> {
        if self.find_cli().is_none() {
            return Ok(RuntimeStatus {
                kind: RuntimeStatusKind::Error,
                pid: None,
                message: Some("pi CLI was not found on PATH".to_string()),
            });
        }
        Ok(RuntimeStatus {
            kind: RuntimeStatusKind::Installed,
            pid: None,
            message: Some("pi CLI detected".to_string()),
        })
    }

    async fn providers(&self) -> CoreResult<Vec<Provider>> {
        list_providers()
    }

    async fn models(&self) -> CoreResult<Vec<Model>> {
        list_models()
    }

    async fn plugins(&self) -> CoreResult<Vec<Plugin>> {
        list_plugins()
    }

    async fn doctor(&self) -> CoreResult<DoctorReport> {
        let cli = self.find_cli();
        let mut checks = Vec::new();

        if cli.is_some() {
            checks.push(DoctorCheck {
                id: "runtime.installed".to_string(),
                status: CheckStatus::Pass,
                summary: "Pi Agent CLI was found".to_string(),
                details: cli.map(|name| format!("command: {name}")),
                suggested_action: None,
            });
        } else {
            checks.push(DoctorCheck {
                id: "runtime.installed".to_string(),
                status: CheckStatus::Fail,
                summary: "Pi Agent CLI was not found".to_string(),
                details: Some("Checked PATH for: pi".to_string()),
                suggested_action: Some("Install Pi Agent or add it to PATH".to_string()),
            });
        }

        let models_ok = list_models().is_ok();
        checks.push(DoctorCheck {
            id: "models.readable".to_string(),
            status: if models_ok {
                CheckStatus::Pass
            } else {
                CheckStatus::Fail
            },
            summary: if models_ok {
                "models.json is readable".to_string()
            } else {
                "models.json could not be read".to_string()
            },
            details: None,
            suggested_action: None,
        });

        let settings_ok = list_plugins().is_ok();
        checks.push(DoctorCheck {
            id: "settings.readable".to_string(),
            status: if settings_ok {
                CheckStatus::Pass
            } else {
                CheckStatus::Fail
            },
            summary: if settings_ok {
                "settings.json is readable".to_string()
            } else {
                "settings.json could not be read".to_string()
            },
            details: None,
            suggested_action: None,
        });

        Ok(DoctorReport {
            adapter_id: ADAPTER_ID.to_string(),
            checks,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn adapter_metadata_is_stable() {
        let adapter = PiAgentAdapter::new();
        assert_eq!(adapter.metadata().id, "pi-agent");
    }

    #[tokio::test]
    async fn capabilities_exclude_runtime_profiles_marketplace() {
        let adapter = PiAgentAdapter::new();
        let caps = adapter.capabilities();
        assert!(caps.providers);
        assert!(caps.models);
        assert!(caps.plugins);
        assert!(caps.snapshots);
        assert!(!caps.runtime);
        assert!(!caps.profiles);
        assert!(!caps.marketplace);
    }

    #[tokio::test]
    async fn detect_returns_not_found_in_clean_environment() {
        let adapter = PiAgentAdapter::new()
            .with_cli_names(vec!["definitely-not-a-real-pi-command".to_string()]);
        let detection = adapter.detect().await.unwrap();
        assert!(!detection.found);
    }
}
