// DeepSeek Harness adapter.
//
// This is the first concrete adapter. It keeps DeepSeek-specific command and
// path knowledge behind the HarnessAdapter trait.
//
// The real harness CLI is `dsh` (npm package @deepseek-ai/dsh):
//   - `dsh web` boots the web profile (default UI: http://127.0.0.1:3080)
//   - `dsh --profile headless "job"` runs one headless task and exits
//   - `dsh plugin --profile <name> <pnpm args>` manages profile plugins
//   - launcher flags must come before the first app argument
//   - invalid commands and startup failures exit non-zero
//
// The launcher has no `--version` flag, so version detection is best-effort
// and usually yields None for the real CLI.

use std::net::ToSocketAddrs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use async_trait::async_trait;
use deepmate_core::adapter::{AdapterCapabilities, AdapterMetadata, Detection, HarnessAdapter};
use deepmate_core::error::{CoreError, CoreResult};
use deepmate_core::model::{
    CheckStatus, DoctorCheck, DoctorReport, HarnessInfo, Model, Plugin, Profile, Provider,
    RuntimeStatus, RuntimeStatusKind,
};
use deepmate_platform::PlatformService;

mod dsh;

use dsh::{discover_profiles, list_all_plugins, list_models, list_providers};

const ADAPTER_ID: &str = "deepseek-harness";
const ADAPTER_NAME: &str = "DeepSeek Harness";
const ADAPTER_VERSION: &str = "0.2.0";
const DEFAULT_UI_URL: &str = "http://127.0.0.1:3080";
const UI_PROBE_TIMEOUT: Duration = Duration::from_millis(500);

// The adapter for DeepSeek Harness.
pub struct DeepSeekHarnessAdapter {
    platform: Arc<dyn PlatformService>,
    ui_url: Option<String>,
    cli_names: Vec<String>,
    data_dir: Option<PathBuf>,
    cli_cache: OnceLock<Option<String>>,
}

impl DeepSeekHarnessAdapter {
    pub fn new(platform: Arc<dyn PlatformService>) -> Self {
        Self {
            platform,
            ui_url: None,
            cli_names: vec!["dsh".to_string(), "deepseek-harness".to_string()],
            data_dir: None,
            cli_cache: OnceLock::new(),
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

    // The data directory is used for adapter-owned runtime state: the pid of
    // a harness started by DeepMate and the harness's own web log.
    pub fn with_data_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.data_dir = Some(dir.into());
        self
    }

    fn ui_url(&self) -> String {
        self.ui_url
            .clone()
            .unwrap_or_else(|| DEFAULT_UI_URL.to_string())
    }

    // Parse "http://host:port" into (host, port). Returns None for
    // unparseable URLs so reachability checks can be skipped.
    fn ui_endpoint(&self) -> Option<(String, u16)> {
        let url = self.ui_url();
        let rest = url
            .strip_prefix("http://")
            .or_else(|| url.strip_prefix("https://"))?;
        let rest = rest.trim_end_matches('/');
        let (host, port) = match rest.split_once(':') {
            Some((host, port)) => (host.to_string(), port.parse().ok()?),
            None => (rest.to_string(), 80),
        };
        Some((host, port))
    }

    // True when the harness web UI accepts TCP connections.
    fn ui_reachable(&self) -> bool {
        let Some((host, port)) = self.ui_endpoint() else {
            return false;
        };
        let Ok(mut addrs) = (host.as_str(), port).to_socket_addrs() else {
            return false;
        };
        let Some(addr) = addrs.next() else {
            return false;
        };
        std::net::TcpStream::connect_timeout(&addr, UI_PROBE_TIMEOUT).is_ok()
    }

    // Candidate launcher commands, most specific first:
    // 1. the explicit DEEPMATE_DSH_BIN override
    // 2. the configured names resolved on PATH
    // 3. launcher bins found in npm's npx cache, a common install location
    //    (`npx @deepseek-ai/dsh`) that is not on PATH
    fn candidate_commands(&self) -> Vec<String> {
        let mut candidates = Vec::new();
        if let Ok(bin) = std::env::var("DEEPMATE_DSH_BIN") {
            if !bin.is_empty() {
                candidates.push(bin);
            }
        }
        candidates.extend(self.cli_names.iter().cloned());
        candidates.extend(self.npx_candidates());
        candidates
    }

    // Absolute paths to `<name>` launcher bins inside npm's npx cache. The
    // cache holds one directory per `npx` install; only entries that actually
    // exist are returned, in deterministic order.
    fn npx_candidates(&self) -> Vec<String> {
        let Some(root) = Self::npm_npx_root() else {
            return Vec::new();
        };
        let Ok(entries) = std::fs::read_dir(&root) else {
            return Vec::new();
        };
        let mut bins: Vec<String> = entries
            .flatten()
            .filter(|entry| entry.file_type().is_ok_and(|t| t.is_dir()))
            .flat_map(|entry| {
                self.cli_names.iter().map(move |name| {
                    entry
                        .path()
                        .join("node_modules")
                        .join(".bin")
                        .join(name)
                        .to_string_lossy()
                        .into_owned()
                })
            })
            .filter(|path| Path::new(path).is_file())
            .collect();
        bins.sort();
        bins.dedup();
        bins
    }

    fn npm_npx_root() -> Option<PathBuf> {
        #[cfg(target_os = "windows")]
        {
            if let Some(local) = std::env::var_os("LOCALAPPDATA") {
                let root = PathBuf::from(local).join("npm-cache").join("_npx");
                if root.is_dir() {
                    return Some(root);
                }
            }
        }
        #[cfg(not(target_os = "windows"))]
        {
            if let Some(home) = std::env::var_os("HOME") {
                let root = PathBuf::from(home).join(".npm").join("_npx");
                if root.is_dir() {
                    return Some(root);
                }
            }
        }
        None
    }

    // Locate the harness CLI, probing each candidate command once and caching
    // the result.
    fn find_cli(&self) -> Option<String> {
        self.cli_cache
            .get_or_init(|| {
                self.candidate_commands().into_iter().find_map(|name| {
                    // If the process can be spawned at all, we treat it as
                    // present. A non-zero exit may still mean a real CLI
                    // exists but uses a different flag.
                    Command::new(&name).arg("--version").output().ok()?;
                    Some(name)
                })
            })
            .clone()
    }

    // Best-effort version from `--version` output. The real `dsh` launcher
    // reports a prerelease like `0.1.0-rc.6`; it stays None only when no
    // token looks like a version at all.
    fn cli_version(&self) -> Option<String> {
        let cli = self.find_cli()?;
        let output = Command::new(&cli).arg("--version").output().ok()?;
        if !output.status.success() {
            return None;
        }
        let text = String::from_utf8_lossy(&output.stdout);
        text.lines().next()?.split_whitespace().find_map(|token| {
            let candidate = token.trim_start_matches('v');
            let looks_like_version = candidate.chars().next().is_some_and(|c| c.is_ascii_digit())
                && candidate.split('.').count() >= 2
                && candidate
                    .chars()
                    .all(|c| c.is_ascii_digit() || c == '.' || c == '-' || c.is_ascii_alphabetic());
            looks_like_version.then(|| candidate.to_string())
        })
    }

    fn pid_path(&self) -> Option<PathBuf> {
        self.data_dir
            .as_ref()
            .map(|dir| dir.join("state").join("harness.pid"))
    }

    fn web_log_path(&self) -> Option<PathBuf> {
        self.data_dir
            .as_ref()
            .map(|dir| dir.join("logs").join("harness-web.log"))
    }

    fn read_pid(&self) -> Option<u32> {
        let path = self.pid_path()?;
        let text = std::fs::read_to_string(path).ok()?;
        text.trim().parse().ok()
    }

    fn write_pid(&self, pid: u32) -> CoreResult<()> {
        let Some(path) = self.pid_path() else {
            return Ok(());
        };
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, pid.to_string())?;
        Ok(())
    }

    fn clear_pid(&self) -> CoreResult<()> {
        if let Some(path) = self.pid_path() {
            let _ = std::fs::remove_file(path);
        }
        Ok(())
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
            profiles: true,
            providers: true,
            models: true,
            plugins: true,
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
                message: Some("harness CLI was not found on PATH".to_string()),
            });
        }
        if self.ui_reachable() {
            return Ok(RuntimeStatus {
                kind: RuntimeStatusKind::Running,
                pid: self.read_pid(),
                message: Some(format!("harness web UI is reachable at {}", self.ui_url())),
            });
        }
        Ok(RuntimeStatus {
            kind: RuntimeStatusKind::Installed,
            pid: None,
            message: Some("harness CLI detected; web UI is not running".to_string()),
        })
    }

    async fn start(&self) -> CoreResult<()> {
        let cli = self.find_cli().ok_or_else(|| {
            CoreError::InvalidState("harness CLI was not found on PATH".to_string())
        })?;
        if self.ui_reachable() {
            return Ok(());
        }
        let mut command = Command::new(&cli);
        command.arg("web");
        match self.web_log_path() {
            Some(log_path) => {
                if let Some(parent) = log_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }
                let file = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(&log_path)?;
                command.stdout(file.try_clone()?).stderr(file);
            }
            None => {
                command.stdout(std::process::Stdio::null());
                command.stderr(std::process::Stdio::null());
            }
        }
        // Spawn and drop the handle: the harness keeps running independently
        // of the DeepMate process.
        let child = command
            .spawn()
            .map_err(|err| CoreError::InvalidState(format!("failed to start harness: {err}")))?;
        self.write_pid(child.id())?;
        Ok(())
    }

    async fn stop(&self) -> CoreResult<()> {
        let Some(pid) = self.read_pid() else {
            tracing::warn!("no harness pid recorded; the harness may have been started manually");
            return Ok(());
        };
        self.platform
            .kill_process(pid)
            .map_err(|err| CoreError::InvalidState(err.to_string()))?;
        self.clear_pid()?;
        Ok(())
    }

    async fn restart(&self) -> CoreResult<()> {
        self.stop().await?;
        self.start().await
    }

    async fn open_ui(&self) -> CoreResult<()> {
        let url = self.ui_url();
        self.platform
            .open_url(&url)
            .map_err(|err| CoreError::InvalidState(err.to_string()))
    }

    async fn profiles(&self) -> CoreResult<Vec<Profile>> {
        discover_profiles()
    }

    async fn providers(&self) -> CoreResult<Vec<Provider>> {
        list_providers()
    }

    async fn models(&self) -> CoreResult<Vec<Model>> {
        list_models()
    }

    async fn plugins(&self) -> CoreResult<Vec<Plugin>> {
        list_all_plugins()
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
                details: Some("Checked PATH for: dsh, deepseek-harness".to_string()),
                suggested_action: Some(
                    "Install DeepSeek Harness (npm i -g @deepseek-ai/dsh) or add it to PATH"
                        .to_string(),
                ),
            });
        }

        checks.push(DoctorCheck {
            id: "ui.url".to_string(),
            status: CheckStatus::Pass,
            summary: "Harness UI URL is configured".to_string(),
            details: Some(format!("url: {}", self.ui_url())),
            suggested_action: None,
        });

        if self.ui_reachable() {
            checks.push(DoctorCheck {
                id: "ui.reachable".to_string(),
                status: CheckStatus::Pass,
                summary: "Harness web UI is reachable".to_string(),
                details: Some(format!("url: {}", self.ui_url())),
                suggested_action: None,
            });
        } else {
            checks.push(DoctorCheck {
                id: "ui.reachable".to_string(),
                status: CheckStatus::Warn,
                summary: "Harness web UI is not running".to_string(),
                details: Some(format!("no listener at {}", self.ui_url())),
                suggested_action: Some(
                    "Run `deepmate runtime start` to launch the web UI".to_string(),
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

    #[test]
    fn ui_url_defaults_to_local_web_ui() {
        let adapter = DeepSeekHarnessAdapter::new(Arc::new(SystemPlatform));
        assert_eq!(adapter.ui_url(), DEFAULT_UI_URL);
        assert_eq!(adapter.ui_endpoint(), Some(("127.0.0.1".to_string(), 3080)));
    }

    #[test]
    fn ui_endpoint_parses_custom_url() {
        let adapter = DeepSeekHarnessAdapter::new(Arc::new(SystemPlatform))
            .with_ui_url("http://localhost:8080");
        assert_eq!(adapter.ui_endpoint(), Some(("localhost".to_string(), 8080)));
    }

    #[test]
    fn ui_endpoint_rejects_unparseable_url() {
        let adapter =
            DeepSeekHarnessAdapter::new(Arc::new(SystemPlatform)).with_ui_url("not a url");
        assert_eq!(adapter.ui_endpoint(), None);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn detect_parses_version_from_cli_output() {
        let dir =
            std::env::temp_dir().join(format!("deepmate-adapter-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let script = dir.join("fake-dsh");
        std::fs::write(&script, "#!/bin/sh\necho 'deepseek-harness 1.2.3'\n").unwrap();
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&script).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&script, perms).unwrap();
        let adapter = DeepSeekHarnessAdapter::new(Arc::new(SystemPlatform))
            .with_cli_names(vec![script.to_string_lossy().into_owned()]);
        let detection = adapter.detect().await.unwrap();
        assert!(detection.found);
        assert_eq!(detection.harness.unwrap().version.as_deref(), Some("1.2.3"));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn detect_parses_prerelease_version() {
        // The real dsh launcher reports a prerelease such as 0.1.0-rc.6.
        let dir = std::env::temp_dir().join(format!(
            "deepmate-adapter-prerelease-test-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let script = dir.join("fake-dsh");
        std::fs::write(
            &script,
            "#!/bin/sh\n[ \"$1\" = \"--version\" ] && echo '0.1.0-rc.6'\n",
        )
        .unwrap();
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&script).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&script, perms).unwrap();
        let adapter = DeepSeekHarnessAdapter::new(Arc::new(SystemPlatform))
            .with_cli_names(vec![script.to_string_lossy().into_owned()]);
        let detection = adapter.detect().await.unwrap();
        assert!(detection.found);
        assert_eq!(
            detection.harness.unwrap().version.as_deref(),
            Some("0.1.0-rc.6")
        );
    }
}
