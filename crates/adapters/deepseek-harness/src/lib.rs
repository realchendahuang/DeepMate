// DeepSeek Harness adapter.
//
// This is the first concrete adapter. It keeps DeepSeek-specific command and
// path knowledge behind the HarnessAdapter trait.

use std::process::Command;
use std::sync::Arc;

use async_trait::async_trait;
use deepmate_core::adapter::{AdapterCapabilities, AdapterMetadata, Detection, HarnessAdapter};
use deepmate_core::error::{CoreError, CoreResult};
use deepmate_core::model::{
    CheckStatus, DoctorCheck, DoctorReport, HarnessInfo, RuntimeStatus, RuntimeStatusKind,
};
use deepmate_platform::PlatformService;

const ADAPTER_ID: &str = "deepseek-harness";
const ADAPTER_NAME: &str = "DeepSeek Harness";
const ADAPTER_VERSION: &str = "0.1.0";

// The adapter for DeepSeek Harness.
pub struct DeepSeekHarnessAdapter {
    platform: Arc<dyn PlatformService>,
    ui_url: Option<String>,
    cli_names: Vec<String>,
}

impl DeepSeekHarnessAdapter {
    pub fn new(platform: Arc<dyn PlatformService>) -> Self {
        Self {
            platform,
            ui_url: None,
            cli_names: vec!["deepseek-harness".to_string(), "dsh".to_string()],
        }
    }

    pub fn with_ui_url(mut self, url: impl Into<String>) -> Self {
        self.ui_url = Some(url.into());
        self
    }

    pub fn with_cli_names(mut self, names: Vec<String>) -> Self {
        self.cli_names = names;
        self
    }

    fn find_cli(&self) -> Option<String> {
        self.cli_names.iter().find_map(|name| {
            // If the process can be spawned at all, we treat it as present.
            // A non-zero exit may still mean a real CLI exists but uses a
            // different flag.
            Command::new(name).arg("--version").output().ok()?;
            Some(name.clone())
        })
    }
}

#[async_trait]
impl HarnessAdapter for DeepSeekHarnessAdapter {
    fn metadata(&self) -> AdapterMetadata {
        AdapterMetadata {
            id: ADAPTER_ID.to_string(),
            name: ADAPTER_NAME.to_string(),
            version: ADAPTER_VERSION.to_string(),
        }
    }

    fn capabilities(&self) -> AdapterCapabilities {
        AdapterCapabilities {
            runtime: true,
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
                version: None,
                adapter_version: ADAPTER_VERSION.to_string(),
            }),
            detail: cli.map(|cli| format!("found CLI: {cli}")),
        })
    }

    async fn status(&self) -> CoreResult<RuntimeStatus> {
        if self.find_cli().is_some() {
            Ok(RuntimeStatus {
                kind: RuntimeStatusKind::Installed,
                pid: None,
                message: Some(
                    "harness CLI detected; detailed runtime status is not implemented yet"
                        .to_string(),
                ),
            })
        } else {
            Ok(RuntimeStatus {
                kind: RuntimeStatusKind::Error,
                pid: None,
                message: Some("harness CLI was not found on PATH".to_string()),
            })
        }
    }

    async fn open_ui(&self) -> CoreResult<()> {
        match &self.ui_url {
            Some(url) => self
                .platform
                .open_url(url)
                .map_err(|err| CoreError::InvalidState(err.to_string())),
            None => Err(CoreError::Unsupported(
                "DeepSeek Harness UI URL is not configured; set DEEPMATE_HARNESS_UI_URL or use with_ui_url".to_string(),
            )),
        }
    }

    async fn doctor(&self) -> CoreResult<DoctorReport> {
        let cli = self.find_cli();
        let mut checks = Vec::new();

        if cli.is_some() {
            checks.push(DoctorCheck {
                id: "runtime.installed".to_string(),
                status: CheckStatus::Pass,
                summary: "DeepSeek Harness CLI was found".to_string(),
                details: cli.map(|name| format!("command: {name}")),
                suggested_action: None,
            });
        } else {
            checks.push(DoctorCheck {
                id: "runtime.installed".to_string(),
                status: CheckStatus::Fail,
                summary: "DeepSeek Harness CLI was not found".to_string(),
                details: Some("Checked PATH for: deepseek-harness, dsh".to_string()),
                suggested_action: Some("Install DeepSeek Harness or add it to PATH".to_string()),
            });
        }

        if self.ui_url.is_some() {
            checks.push(DoctorCheck {
                id: "ui.url".to_string(),
                status: CheckStatus::Pass,
                summary: "Harness UI URL is configured".to_string(),
                details: None,
                suggested_action: None,
            });
        } else {
            checks.push(DoctorCheck {
                id: "ui.url".to_string(),
                status: CheckStatus::Warn,
                summary: "Harness UI URL is not configured".to_string(),
                details: Some("Set DEEPMATE_HARNESS_UI_URL to enable deepmate open".to_string()),
                suggested_action: Some(
                    "Set DEEPMATE_HARNESS_UI_URL or pass a UI URL to the adapter".to_string(),
                ),
            });
        }

        Ok(DoctorReport {
            adapter_id: ADAPTER_ID.to_string(),
            checks,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use deepmate_platform::SystemPlatform;

    #[tokio::test]
    async fn adapter_metadata_is_stable() {
        let adapter = DeepSeekHarnessAdapter::new(Arc::new(SystemPlatform));
        assert_eq!(adapter.metadata().id, "deepseek-harness");
    }

    #[tokio::test]
    async fn detect_returns_not_found_in_clean_environment() {
        let adapter = DeepSeekHarnessAdapter::new(Arc::new(SystemPlatform))
            .with_cli_names(vec!["definitely-not-a-real-deepmate-command".to_string()]);
        let detection = adapter.detect().await.unwrap();
        assert!(!detection.found);
    }
}
