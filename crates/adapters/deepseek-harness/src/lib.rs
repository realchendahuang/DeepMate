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

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use async_trait::async_trait;
use deepmate_core::adapter::{AdapterCapabilities, AdapterMetadata, Detection, HarnessAdapter};
use deepmate_core::error::{CoreError, CoreResult};
use deepmate_core::model::{
    CheckStatus, CompatReport, DoctorCheck, DoctorReport, HarnessInfo, MarketEntry,
    MarketSourceInfo, Model, Plugin, PluginOpEvent, PluginOpKind, Profile, Provider, RuntimeStatus,
    RuntimeStatusKind,
};
use deepmate_platform::PlatformService;
use tokio::io::AsyncBufReadExt;

mod dsh;
mod market;

use dsh::{
    create_profile, discover_profiles, list_all_plugins, list_models, list_providers,
    remove_profile, SettingsEditor,
};

const ADAPTER_ID: &str = "deepseek-harness";
const ADAPTER_NAME: &str = "DeepSeek Harness";
const ADAPTER_VERSION: &str = "0.4.0";
const DEFAULT_UI_URL: &str = "http://127.0.0.1:3080";
const UI_PROBE_TIMEOUT: Duration = Duration::from_millis(500);

// The adapter for DeepSeek Harness.
pub struct DeepSeekHarnessAdapter {
    platform: Arc<dyn PlatformService>,
    ui_url: Option<String>,
    cli_names: Vec<String>,
    data_dir: Option<PathBuf>,
    cli_cache: OnceLock<Option<String>>,
    http: reqwest::Client,
}

impl DeepSeekHarnessAdapter {
    pub fn new(platform: Arc<dyn PlatformService>) -> Self {
        Self {
            platform,
            ui_url: None,
            cli_names: vec!["dsh".to_string(), "deepseek-harness".to_string()],
            data_dir: None,
            cli_cache: OnceLock::new(),
            http: market::build_http_client(),
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

    // True when the harness web UI answers HTTP. A real HTTP response (even a
    // 404) means a web server is serving the endpoint; a refused connection or
    // a probe timeout means nothing is actually there. This is stricter than a
    // raw TCP connect, which would also succeed against a stray socket holding
    // the port.
    async fn ui_reachable(&self) -> bool {
        let Ok(url) = self.ui_url().parse::<reqwest::Url>() else {
            return false;
        };
        let probe = self.http.get(url).send();
        matches!(
            tokio::time::timeout(UI_PROBE_TIMEOUT, probe).await,
            Ok(Ok(_response))
        )
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

    // Run one `dsh plugin` forwarding command against the profile's pnpm and
    // fail loudly with the captured output when the command exits non-zero.
    //
    // Commands are always run with an explicit forwarded pnpm verb (`add`,
    // `remove`, `update`); a bare `dsh plugin --profile <name>` would run a
    // full pnpm install, which is a side effect callers here never intend.
    async fn run_plugin(&self, profile: &str, forwarded: &[&str]) -> CoreResult<()> {
        let cli = self.find_cli().ok_or_else(|| {
            CoreError::InvalidState("harness CLI was not found on PATH".to_string())
        })?;
        let output = tokio::process::Command::new(&cli)
            .arg("plugin")
            .arg("--profile")
            .arg(profile)
            .args(forwarded)
            .output()
            .await
            .map_err(|err| CoreError::InvalidState(format!("failed to run `dsh plugin`: {err}")))?;
        if !output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(CoreError::InvalidState(format!(
                "`dsh plugin` failed with exit {:?}: {} {}",
                output.status.code(),
                stdout.trim(),
                stderr.trim()
            )));
        }
        tracing::info!(cli = %cli, profile, forwarded = ?forwarded, "plugin command completed");
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
            marketplace: true,
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
                message: Some("harness CLI was not found on PATH".to_string()),
            });
        }
        if self.ui_reachable().await {
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
        if self.ui_reachable().await {
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
        if !self.ui_reachable().await {
            return Err(CoreError::InvalidState(
                "harness web UI is not running; start it first".to_string(),
            ));
        }
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

    async fn upsert_provider(&self, provider: Provider) -> CoreResult<()> {
        SettingsEditor::upsert_provider(&provider)
    }

    async fn remove_provider(&self, id: &str) -> CoreResult<()> {
        SettingsEditor::remove_provider(id)
    }

    async fn upsert_model(&self, provider: &str, model: Model) -> CoreResult<()> {
        SettingsEditor::upsert_model(provider, &model)
    }

    async fn remove_model(&self, provider: &str, id: &str) -> CoreResult<()> {
        SettingsEditor::remove_model(provider, id)
    }

    async fn create_profile(&self, name: &str) -> CoreResult<()> {
        create_profile(name)
    }

    async fn remove_profile(&self, name: &str) -> CoreResult<()> {
        remove_profile(name)
    }

    async fn plugins(&self) -> CoreResult<Vec<Plugin>> {
        list_all_plugins()
    }

    async fn install_plugin(&self, profile: &str, spec: &str) -> CoreResult<()> {
        self.run_plugin(profile, &["add", spec]).await
    }

    async fn remove_plugin(&self, profile: &str, id: &str) -> CoreResult<()> {
        self.run_plugin(profile, &["remove", id]).await
    }

    async fn update_plugin(&self, profile: &str, id: Option<&str>) -> CoreResult<()> {
        match id {
            Some(id) => self.run_plugin(profile, &["update", id]).await,
            None => self.run_plugin(profile, &["update"]).await,
        }
    }

    // Stream a plugin operation with live output: spawn `dsh plugin` with
    // piped stdout/stderr and forward each line as a `Line` event, so the
    // desktop UI can show a real progress log instead of a spinner. The
    // forwarded pnpm verb is derived from the operation kind, mirroring
    // `run_plugin`'s "never run a bare pnpm install" rule.
    async fn stream_plugin_op(
        &self,
        profile: &str,
        kind: PluginOpKind,
        target: Option<&str>,
        tx: tokio::sync::mpsc::Sender<PluginOpEvent>,
    ) -> CoreResult<()> {
        let cli = self.find_cli().ok_or_else(|| {
            CoreError::InvalidState("harness CLI was not found on PATH".to_string())
        })?;
        let target = target.unwrap_or("").to_string();
        let forwarded: &[&str] = match kind {
            PluginOpKind::Install => &["add", &target],
            PluginOpKind::Remove => &["remove", &target],
            PluginOpKind::Update => &["update", &target],
        };
        let _ = tx
            .send(PluginOpEvent::Started {
                op: kind,
                target: target.clone(),
            })
            .await;

        let mut child = tokio::process::Command::new(&cli)
            .arg("plugin")
            .arg("--profile")
            .arg(profile)
            .args(forwarded)
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .spawn()
            .map_err(|err| CoreError::InvalidState(format!("failed to run `dsh plugin`: {err}")))?;

        let stdout = child.stdout.take().expect("stdout was piped");
        let stderr = child.stderr.take().expect("stderr was piped");
        let mut lines = tokio::io::BufReader::new(stdout).lines();
        let mut err_lines = tokio::io::BufReader::new(stderr).lines();
        let mut stderr_tail: Vec<String> = Vec::new();
        loop {
            tokio::select! {
                line = lines.next_line() => match line {
                    Ok(Some(line)) => {
                        let trimmed = line.trim().to_string();
                        if !trimmed.is_empty() {
                            let _ = tx.send(PluginOpEvent::Line { text: trimmed }).await;
                        }
                    }
                    _ => break,
                },
                line = err_lines.next_line() => match line {
                    Ok(Some(line)) => {
                        let trimmed = line.trim().to_string();
                        if !trimmed.is_empty() {
                            stderr_tail.push(trimmed);
                            if stderr_tail.len() > 8 {
                                stderr_tail.remove(0);
                            }
                        }
                    }
                    _ => break,
                },
            }
        }
        let status = child.wait().await.map_err(|err| {
            CoreError::InvalidState(format!("failed to wait for `dsh plugin`: {err}"))
        })?;
        if status.success() {
            let _ = tx
                .send(PluginOpEvent::Finished {
                    ok: true,
                    detail: None,
                })
                .await;
            Ok(())
        } else {
            let detail = if stderr_tail.is_empty() {
                format!("`dsh plugin` failed with exit {:?}", status.code())
            } else {
                format!(
                    "`dsh plugin` failed with exit {:?}: {}",
                    status.code(),
                    stderr_tail.join(" ")
                )
            };
            let _ = tx
                .send(PluginOpEvent::Finished {
                    ok: false,
                    detail: Some(detail.clone()),
                })
                .await;
            Err(CoreError::InvalidState(detail))
        }
    }

    async fn search_plugins(&self, query: &str) -> CoreResult<Vec<MarketEntry>> {
        let market = market::Market::new(self.data_dir.clone(), self.http.clone());
        // The curated list is merged into every search so the curated source
        // is visible without a dedicated query. An empty query skips the npm
        // search entirely and shows the curated list alone — that is the
        // storefront the desktop market tab opens on. A curated entry that
        // also matches the npm results keeps its curated source (and its
        // curated metadata) by taking precedence.
        let curated = market.curated().await.unwrap_or_default();
        if query.trim().is_empty() {
            return Ok(curated);
        }
        let mut entries = market.search(query).await?;
        let curated_ids: std::collections::HashSet<&str> =
            curated.iter().map(|entry| entry.id.as_str()).collect();
        entries.retain(|entry| !curated_ids.contains(entry.id.as_str()));
        entries.splice(0..0, curated);
        Ok(entries)
    }

    async fn market_sources(&self) -> CoreResult<Vec<MarketSourceInfo>> {
        Ok(market::market_sources())
    }

    // Compatibility is a registry contract: the harness version detected from
    // the CLI is matched against the package's declared `engines` requirement.
    async fn plugin_compat(&self, spec: &str) -> CoreResult<CompatReport> {
        let harness_version = self.cli_version();
        market::Market::compat(&self.http, spec, harness_version).await
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

        if self.ui_reachable().await {
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
    }

    #[tokio::test]
    async fn ui_unreachable_when_nothing_listens() {
        // Port 1 is never a listening harness; the HTTP probe must report it
        // as unreachable rather than trusting a raw TCP connect.
        let adapter =
            DeepSeekHarnessAdapter::new(Arc::new(SystemPlatform)).with_ui_url("http://127.0.0.1:1");
        assert!(!adapter.ui_reachable().await);
    }

    #[tokio::test]
    async fn open_ui_fails_when_not_running() {
        let adapter =
            DeepSeekHarnessAdapter::new(Arc::new(SystemPlatform)).with_ui_url("http://127.0.0.1:1");
        let err = adapter.open_ui().await.unwrap_err();
        assert!(err.to_string().contains("not running"));
    }

    // Install a fake `dsh` launcher script that answers `--version` and
    // records the forwarded plugin arguments to a file, so run_plugin can be
    // tested without a real harness.
    #[cfg(unix)]
    fn fake_plugin_cli(dir: &std::path::Path, exit_code: i32) -> std::path::PathBuf {
        use std::os::unix::fs::PermissionsExt;
        std::fs::create_dir_all(dir).unwrap();
        let recorded = dir.join("recorded-args");
        let script = dir.join("fake-dsh-plugin");
        std::fs::write(
            &script,
            format!(
                "#!/bin/sh\n\
                 [ \"$1\" = \"--version\" ] && exit 0\n\
                 if [ \"$1\" = \"plugin\" ]; then\n\
                   printf '%s\\n' \"$@\" > \"{}\"\n\
                   exit {}\n\
                 fi\n\
                 exit 1\n",
                recorded.display(),
                exit_code
            ),
        )
        .unwrap();
        let mut perms = std::fs::metadata(&script).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&script, perms).unwrap();
        script
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn install_plugin_forwards_pnpm_add() {
        let dir = std::env::temp_dir().join(format!(
            "deepmate-plugin-forward-test-{}",
            std::process::id()
        ));
        let cli = fake_plugin_cli(&dir, 0);
        let recorded = dir.join("recorded-args");
        let adapter = DeepSeekHarnessAdapter::new(Arc::new(SystemPlatform))
            .with_cli_names(vec![cli.to_string_lossy().into_owned()]);
        adapter.install_plugin("web", "dsh-mnemon").await.unwrap();
        let args = std::fs::read_to_string(&recorded).unwrap();
        assert_eq!(
            args.lines().collect::<Vec<_>>(),
            ["plugin", "--profile", "web", "add", "dsh-mnemon"]
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn remove_and_update_forward_correct_verbs() {
        let dir =
            std::env::temp_dir().join(format!("deepmate-plugin-verbs-test-{}", std::process::id()));
        let cli = fake_plugin_cli(&dir, 0);
        let recorded = dir.join("recorded-args");
        let adapter = DeepSeekHarnessAdapter::new(Arc::new(SystemPlatform))
            .with_cli_names(vec![cli.to_string_lossy().into_owned()]);

        adapter.remove_plugin("headless", "old-pkg").await.unwrap();
        assert_eq!(
            std::fs::read_to_string(&recorded)
                .unwrap()
                .lines()
                .collect::<Vec<_>>(),
            ["plugin", "--profile", "headless", "remove", "old-pkg"]
        );

        adapter.update_plugin("web", None).await.unwrap();
        assert_eq!(
            std::fs::read_to_string(&recorded)
                .unwrap()
                .lines()
                .collect::<Vec<_>>(),
            ["plugin", "--profile", "web", "update"]
        );

        adapter
            .update_plugin("web", Some("dsh-mnemon"))
            .await
            .unwrap();
        assert_eq!(
            std::fs::read_to_string(&recorded)
                .unwrap()
                .lines()
                .collect::<Vec<_>>(),
            ["plugin", "--profile", "web", "update", "dsh-mnemon"]
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn plugin_command_failure_propagates_output() {
        let dir =
            std::env::temp_dir().join(format!("deepmate-plugin-fail-test-{}", std::process::id()));
        let cli = fake_plugin_cli(&dir, 7);
        let recorded = dir.join("recorded-args");
        std::fs::write(&recorded, "").unwrap();
        let adapter = DeepSeekHarnessAdapter::new(Arc::new(SystemPlatform))
            .with_cli_names(vec![cli.to_string_lossy().into_owned()]);
        let err = adapter.install_plugin("web", "bad-pkg").await.unwrap_err();
        let message = err.to_string();
        assert!(
            message.contains("Some(7)"),
            "exit code must be surfaced in the error: {message}"
        );

        // A missing CLI surfaces the not-found error before any forwarding.
        let missing = DeepSeekHarnessAdapter::new(Arc::new(SystemPlatform))
            .with_cli_names(vec!["definitely-not-a-command-xyz".to_string()]);
        assert!(missing.install_plugin("web", "pkg").await.is_err());
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
