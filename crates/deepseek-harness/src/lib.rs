// The DeepSeek Harness service layer.
//
// This is the one harness DeepMate controls. It keeps DeepSeek-specific
// command and path knowledge out of the core and the frontends.
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

use deepmate_core::error::{CoreError, CoreResult};
use deepmate_core::model::{
    CheckStatus, CompatReport, Detection, DisabledPlugin, DoctorCheck, DoctorReport, HarnessInfo,
    MarketEntry, MarketSourceInfo, MarketTrust, Model, Plugin, PluginOpEvent, PluginOpKind,
    Profile, Provider, RuntimeStatus, RuntimeStatusKind,
};
use deepmate_core::snapshot::{Snapshot, SnapshotReport, SNAPSHOT_FORMAT};
use deepmate_platform::PlatformService;
use tokio::io::AsyncBufReadExt;

mod dsh;
mod market;
mod ports;
mod registry;

use deepmate_core::model::{RuntimeInstance, Surface};
use dsh::{
    create_profile, discover_profiles, list_all_plugins, list_models, list_providers,
    profile_surface, read_advanced_file, remove_profile, save_advanced_file,
    set_profile_description, SettingsEditor, BASE_BUNDLE, HEADLESS_BUNDLE, WEB_APP_BUNDLE,
};
// The raw-editor file scopes are part of the harness contract: the desktop
// command layer maps its string arguments onto them.
pub use dsh::AdvancedFileScope;
use ports::PortRegistry;

const HARNESS_ID: &str = "deepseek-harness";
const HARNESS_NAME: &str = "DeepSeek Harness";
// The default scenario. On the engine side `dsh web` is nothing more than a
// hardcoded alias for `dsh --profile web` (a regular scenario backed by the
// `dsh-base` + `dsh-web-app` template); DeepMate keeps it protected from
// rename/remove because it is the compatibility default every existing
// command and integration assumes.
const WEB_PROFILE: &str = "web";
const DEFAULT_UI_URL: &str = "http://127.0.0.1:3080";
const UI_PROBE_TIMEOUT: Duration = Duration::from_millis(500);
// A plugin operation must always terminate: the dsh launcher (or a pnpm
// grandchild) can outlive the actual work and leave stdout open, and an
// unbounded wait would trap the desktop progress dialog with no way to
// close it. On timeout the whole process group is killed and the operation
// reports a bounded failure. Overridable for tests.
const PLUGIN_OP_TIMEOUT: Duration = Duration::from_secs(120);
const PLUGIN_OP_TIMEOUT_ENV: &str = "DEEPMATE_PLUGIN_OP_TIMEOUT_SECS";
// How long `start` waits for the spawned web UI to answer before reporting
// failure. Every caller reports state right after start returns (hero
// status, doctor re-run), so returning at spawn time would make a healthy
// boot look like a failed one.
const WEB_UI_BOOT_TIMEOUT: Duration = Duration::from_secs(15);
const WEB_UI_BOOT_TIMEOUT_ENV: &str = "DEEPMATE_UI_BOOT_TIMEOUT_SECS";
// The stderr tail kept for a failure detail line; older lines are dropped.
const STDERR_TAIL: usize = 8;

fn plugin_op_timeout() -> Duration {
    std::env::var(PLUGIN_OP_TIMEOUT_ENV)
        .ok()
        .and_then(|value| value.parse().ok())
        .map(Duration::from_secs)
        .unwrap_or(PLUGIN_OP_TIMEOUT)
}

fn ui_boot_timeout() -> Duration {
    std::env::var(WEB_UI_BOOT_TIMEOUT_ENV)
        .ok()
        .and_then(|value| value.parse().ok())
        .map(Duration::from_secs)
        .unwrap_or(WEB_UI_BOOT_TIMEOUT)
}

// One completed streamed child run: the pieces a caller needs for its
// operation-specific handling (task-log persistence, bundle cleanup).
struct ChildRun {
    status: std::process::ExitStatus,
    // Collected stdout lines (trimmed, non-empty), in order.
    stdout_lines: Vec<String>,
    // The stderr tail at the moment the run ended.
    stderr_tail: Vec<String>,
}

// How a bounded streamed child run failed; the io::Error is passed through
// so each caller can word its own detail message.
enum ChildRunFailure {
    Spawn(std::io::Error),
    Wait(std::io::Error),
    TimedOut,
}

// Run a piped child under the bounded, streamed discipline shared by plugin
// operations and task runs: stdout/stderr are drained on independent tasks
// (a single select! loop could truncate the other stream's tail on the first
// EOF — or hang forever when a pnpm grandchild keeps a pipe open after the
// child printed its final line), stdout lines are relayed as `Line` events
// and collected in order, stderr is kept as a bounded tail for the failure
// detail, and the run is capped by `timeout`. On expiry the drain tasks are
// aborted and the process group plus the child are killed, then reaped — so
// the channel always gets its final `Finished` event and no process is left
// behind.
async fn run_streamed_child(
    command: &mut tokio::process::Command,
    tx: &tokio::sync::mpsc::Sender<PluginOpEvent>,
    timeout: Duration,
) -> Result<ChildRun, ChildRunFailure> {
    // The child gets its own process group (unix) so a timeout can kill dsh
    // and any pnpm grandchild together instead of orphaning it.
    command
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped());
    #[cfg(unix)]
    command.process_group(0);
    let mut child = command.spawn().map_err(ChildRunFailure::Spawn)?;

    let stdout = child.stdout.take().expect("stdout was piped");
    let stderr = child.stderr.take().expect("stderr was piped");
    let stdout_lines: std::sync::Arc<std::sync::Mutex<Vec<String>>> =
        std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let stdout_tx = tx.clone();
    let collected = std::sync::Arc::clone(&stdout_lines);
    let mut stdout_task = tokio::spawn(async move {
        let mut reader = tokio::io::BufReader::new(stdout).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            let trimmed = line.trim().to_string();
            if !trimmed.is_empty() {
                collected.lock().unwrap().push(trimmed.clone());
                let _ = stdout_tx.send(PluginOpEvent::Line { text: trimmed }).await;
            }
        }
    });
    let stderr_tail: std::sync::Arc<std::sync::Mutex<Vec<String>>> =
        std::sync::Arc::new(std::sync::Mutex::new(Vec::new()));
    let tail = std::sync::Arc::clone(&stderr_tail);
    let mut stderr_task = tokio::spawn(async move {
        let mut reader = tokio::io::BufReader::new(stderr).lines();
        while let Ok(Some(line)) = reader.next_line().await {
            let trimmed = line.trim().to_string();
            if !trimmed.is_empty() {
                let mut tail = tail.lock().unwrap();
                if tail.len() >= STDERR_TAIL {
                    tail.remove(0);
                }
                tail.push(trimmed);
            }
        }
    });

    let outcome = tokio::time::timeout(timeout, async {
        // Drain both streams, then collect the exit status.
        let _ = (&mut stdout_task).await;
        let _ = (&mut stderr_task).await;
        child.wait().await
    })
    .await;
    let stderr_tail = stderr_tail.lock().unwrap().clone();

    match outcome {
        Ok(Ok(status)) => Ok(ChildRun {
            status,
            stdout_lines: std::mem::take(&mut *stdout_lines.lock().unwrap()),
            stderr_tail,
        }),
        Ok(Err(err)) => Err(ChildRunFailure::Wait(err)),
        Err(_elapsed) => {
            // Stop the stream drain tasks first (dropping their channel
            // clones), then kill the child process group (dsh + any pnpm
            // grandchild), then reap. `kill(2)` is used directly because
            // tokio's `Child::kill` did not reliably terminate a stuck child
            // on macOS; a failed group kill still gets the child pid, so no
            // leftover process is ever orphaned.
            tracing::warn!(
                timeout_secs = timeout.as_secs(),
                "streamed child run timed out; killing the process group"
            );
            stdout_task.abort();
            stderr_task.abort();
            #[cfg(unix)]
            {
                let pid = child.id().unwrap_or_default();
                unsafe {
                    libc::kill(-(pid as i32), libc::SIGKILL);
                    libc::kill(pid as i32, libc::SIGKILL);
                }
            }
            let _ = child.kill().await;
            let _ = child.wait().await;
            Err(ChildRunFailure::TimedOut)
        }
    }
}

// The DeepSeek Harness service.
pub struct DeepSeekHarness {
    platform: Arc<dyn PlatformService>,
    ui_url: Option<String>,
    cli_names: Vec<String>,
    data_dir: Option<PathBuf>,
    cli_cache: OnceLock<String>,
    http: reqwest::Client,
    // Market cache freshness; mirrors `Config.market.refresh_interval_seconds`
    // when assembled through `deepmate_app::build_harness`.
    market_ttl: Duration,
}

// One failing loader entry from the runtime bundle probe.
#[derive(Debug, Clone)]
struct BundleLoadFailure {
    profile: String,
    id: String,
    reason: String,
}

impl DeepSeekHarness {
    pub fn new(platform: Arc<dyn PlatformService>) -> Self {
        Self {
            platform,
            ui_url: None,
            cli_names: vec!["dsh".to_string(), "deepseek-harness".to_string()],
            data_dir: None,
            cli_cache: OnceLock::new(),
            http: market::build_http_client(),
            market_ttl: Duration::from_secs(3600),
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

    // The data directory is used for DeepMate-owned runtime state: the pid of
    // a harness started by DeepMate and the harness's own web log.
    pub fn with_data_dir(mut self, dir: impl Into<PathBuf>) -> Self {
        self.data_dir = Some(dir.into());
        self
    }

    // Override the market cache freshness in seconds.
    pub fn with_market_ttl(mut self, seconds: u64) -> Self {
        self.market_ttl = Duration::from_secs(seconds);
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
    // True when the web UI at `url` answers HTTP. A real HTTP response (even
    // a 404) means a web server is serving the endpoint; a refused connection
    // or a probe timeout means nothing is actually there. This is stricter
    // than a raw TCP connect, which would also succeed against a stray
    // socket holding the port.
    async fn ui_reachable_at(&self, url: &str) -> bool {
        let Ok(url) = url.parse::<reqwest::Url>() else {
            return false;
        };
        let probe = self.http.get(url).send();
        matches!(
            tokio::time::timeout(UI_PROBE_TIMEOUT, probe).await,
            Ok(Ok(_response))
        )
    }

    // The legacy single-instance probe: the `web` scenario's URL.
    async fn ui_reachable(&self) -> bool {
        self.ui_reachable_at(&self.ui_url()).await
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

    // Locate the harness CLI, probing each candidate command. Only a
    // successful probe is cached: the desktop app is a long-lived tray
    // process, and a user who installs the harness after launching DeepMate
    // must be picked up without a restart.
    fn find_cli(&self) -> Option<String> {
        if let Some(cli) = self.cli_cache.get() {
            return Some(cli.clone());
        }
        let found = self.candidate_commands().into_iter().find_map(|name| {
            // If the process can be spawned at all, we treat it as present.
            // A non-zero exit may still mean a real CLI exists but uses a
            // different flag.
            Command::new(&name).arg("--version").output().ok()?;
            Some(name)
        })?;
        let _ = self.cli_cache.set(found.clone());
        Some(found)
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

    // The per-scenario pid file: `state/run-<profile>.pid`. The legacy
    // single `harness.pid` is still read for the `web` scenario (and cleared
    // on stop) so a harness started by an older DeepMate stays manageable.
    fn pid_path(&self, profile: &str) -> Option<PathBuf> {
        self.data_dir
            .as_ref()
            .map(|dir| dir.join("state").join(format!("run-{profile}.pid")))
    }

    fn legacy_pid_path(&self) -> Option<PathBuf> {
        self.data_dir
            .as_ref()
            .map(|dir| dir.join("state").join("harness.pid"))
    }

    // The per-scenario log file: `logs/harness-<profile>.log` (the `web`
    // scenario keeps the historical `harness-web.log` name).
    fn log_path(&self, profile: &str) -> Option<PathBuf> {
        self.data_dir
            .as_ref()
            .map(|dir| dir.join("logs").join(format!("harness-{profile}.log")))
    }

    // Resolve the pid that runs a scenario: the pid file records a scenario
    // started by DeepMate itself, a legacy single pid file holds older `web`
    // harnesses, and otherwise whoever serves the scenario's port is the
    // runtime to manage (a scenario started outside DeepMate, e.g. `dsh web`
    // in a terminal, or a process that died without clearing the file). The
    // port fallback spawns `lsof`/`netstat` and blocks, so it must not run
    // on a tokio worker thread; the pid-file checks are cheap file reads
    // and stay inline.
    async fn read_pid_async(&self, profile: &str) -> Option<u32> {
        if let Some(pid) = self.pid_file_pid(profile) {
            return Some(pid);
        }
        if profile == WEB_PROFILE {
            if let Some(pid) = self.legacy_pid_file_pid() {
                return Some(pid);
            }
        }
        let url = self.scenario_url(profile).ok()?;
        let port = url.parse::<reqwest::Url>().ok()?.port_or_known_default()?;
        self.find_listener_pid(port).await
    }

    // `PlatformService::find_listener_pid` off the async worker threads.
    async fn find_listener_pid(&self, port: u16) -> Option<u32> {
        let platform = Arc::clone(&self.platform);
        tokio::task::spawn_blocking(move || platform.find_listener_pid(port))
            .await
            .unwrap_or_default()
    }

    // `PlatformService::kill_process` off the async worker threads.
    async fn kill_process(&self, pid: u32) -> CoreResult<()> {
        let platform = Arc::clone(&self.platform);
        tokio::task::spawn_blocking(move || platform.kill_process(pid))
            .await
            .map_err(|err| CoreError::InvalidState(format!("kill task failed: {err}")))?
            .map_err(|err| CoreError::InvalidState(err.to_string()))
    }

    fn pid_file_pid(&self, profile: &str) -> Option<u32> {
        let path = self.pid_path(profile)?;
        let text = std::fs::read_to_string(path).ok()?;
        text.trim().parse().ok()
    }

    fn legacy_pid_file_pid(&self) -> Option<u32> {
        let path = self.legacy_pid_path()?;
        let text = std::fs::read_to_string(path).ok()?;
        text.trim().parse().ok()
    }

    fn write_pid(&self, profile: &str, pid: u32) -> CoreResult<()> {
        let Some(path) = self.pid_path(profile) else {
            return Ok(());
        };
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, pid.to_string())?;
        Ok(())
    }

    fn clear_pid(&self, profile: &str) -> CoreResult<()> {
        if let Some(path) = self.pid_path(profile) {
            let _ = std::fs::remove_file(path);
        }
        // A legacy `web` harness also recorded under the old file name.
        if profile == WEB_PROFILE {
            if let Some(path) = self.legacy_pid_path() {
                let _ = std::fs::remove_file(path);
            }
        }
        Ok(())
    }

    // The URL a web-surface scenario serves, when it has a port.
    fn scenario_url(&self, profile: &str) -> CoreResult<String> {
        let Some(port) = self.scenario_port(profile)? else {
            return Err(CoreError::InvalidState(format!(
                "scenario {profile} has no assigned port"
            )));
        };
        Ok(format!("http://127.0.0.1:{port}"))
    }

    // The port a scenario serves on. The `web` scenario's address is fixed:
    // the legacy DEEPMATE_HARNESS_UI_URL override, or the engine's default
    // 3080. Other web scenarios get the port the registry assigned them,
    // which is part of why their URL survives DeepMate restarts.
    fn scenario_port(&self, profile: &str) -> CoreResult<Option<u16>> {
        if profile == WEB_PROFILE {
            return Ok(Some(
                self.ui_url()
                    .parse::<reqwest::Url>()
                    .ok()
                    .and_then(|url| url.port_or_known_default())
                    .unwrap_or(ports::DEFAULT_WEB_PORT),
            ));
        }
        PortRegistry::new(self.data_dir.clone()).port(profile)
    }

    // Run one `dsh plugin` forwarding command against the profile's pnpm and
    // fail loudly with the captured output when the command exits non-zero.
    //
    // Commands are always run with an explicit forwarded pnpm verb (`add`,
    // `remove`, `update`); a bare `dsh plugin --profile <name>` would run a
    // full pnpm install, which is a side effect callers here never intend.
    async fn run_plugin(&self, profile: &str, forwarded: &[&str]) -> CoreResult<()> {
        let cli = self
            .find_cli()
            .ok_or_else(|| CoreError::NotFound("harness CLI was not found on PATH".to_string()))?;
        let output = tokio::process::Command::new(&cli)
            .arg("plugin")
            .arg("--profile")
            .arg(profile)
            .args(forwarded)
            .output()
            .await
            .map_err(|err| CoreError::SpawnFailed(format!("failed to run `dsh plugin`: {err}")))?;
        if !output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(CoreError::CommandFailed(format!(
                "`dsh plugin` failed with exit {:?}: {} {}",
                output.status.code(),
                stdout.trim(),
                stderr.trim()
            )));
        }
        tracing::info!(cli = %cli, profile, forwarded = ?forwarded, "plugin command completed");
        Ok(())
    }

    pub async fn detect(&self) -> CoreResult<Detection> {
        let cli = self.find_cli();
        Ok(Detection {
            found: cli.is_some(),
            harness: cli.as_ref().map(|_cli| HarnessInfo {
                id: HARNESS_ID.to_string(),
                name: HARNESS_NAME.to_string(),
                version: self.cli_version(),
            }),
            detail: cli.map(|cli| format!("found CLI: {cli}")),
        })
    }

    // The legacy single-instance status: the `web` scenario.
    pub async fn status(&self) -> CoreResult<RuntimeStatus> {
        self.status_scenario(WEB_PROFILE).await
    }

    pub async fn status_scenario(&self, profile: &str) -> CoreResult<RuntimeStatus> {
        if self.find_cli().is_none() {
            return Ok(RuntimeStatus {
                kind: RuntimeStatusKind::Error,
                pid: None,
                message: Some("harness CLI was not found on PATH".to_string()),
            });
        }
        match profile_surface(profile)? {
            // Task scenarios boot per task and never stay resident.
            Surface::Task => {
                return Ok(RuntimeStatus {
                    kind: RuntimeStatusKind::Installed,
                    pid: None,
                    message: Some(format!(
                        "scenario {profile} is a task scenario: it boots per task and exits"
                    )),
                });
            }
            Surface::Undetermined => {
                return Ok(RuntimeStatus {
                    kind: RuntimeStatusKind::Installed,
                    pid: None,
                    message: Some(format!("scenario {profile} has no surface installed yet")),
                });
            }
            Surface::Web => {}
        }
        if let Ok(url) = self.scenario_url(profile) {
            if self.ui_reachable_at(&url).await {
                return Ok(RuntimeStatus {
                    kind: RuntimeStatusKind::Running,
                    pid: self.read_pid_async(profile).await,
                    message: Some(format!("scenario web UI is reachable at {url}")),
                });
            }
        }
        Ok(RuntimeStatus {
            kind: RuntimeStatusKind::Installed,
            pid: None,
            message: Some(format!("scenario {profile} is not running")),
        })
    }

    // The legacy single-instance start: the `web` scenario.
    pub async fn start(&self) -> CoreResult<()> {
        self.start_scenario(WEB_PROFILE, None).await
    }

    // Start one web-surface scenario as a detached process
    // (`dsh --profile <name> --port <port>`). The port is resolved through
    // the registry (the `web` scenario keeps the legacy URL override / the
    // engine default), assigned and persisted; a start of an already-running
    // scenario is a no-op. Task-surface scenarios have nothing to start.
    pub async fn start_scenario(
        &self,
        profile: &str,
        port_override: Option<u16>,
    ) -> CoreResult<()> {
        // Surface checks come first: a task scenario is not "started" by
        // mistake even when no engine CLI would be found afterwards.
        match profile_surface(profile)? {
            Surface::Web => {}
            Surface::Task => {
                return Err(CoreError::InvalidState(format!(
                    "scenario {profile} is a task scenario: it boots, answers one task and exits; use a task run instead"
                )));
            }
            Surface::Undetermined => {
                return Err(CoreError::InvalidState(format!(
                    "scenario {profile} has no web surface installed yet (missing @deepseek-ai/dsh-web-app)"
                )));
            }
        }
        let cli = self
            .find_cli()
            .ok_or_else(|| CoreError::NotFound("harness CLI was not found on PATH".to_string()))?;
        let url = self.scenario_url_for(profile, port_override).await?;
        if self.ui_reachable_at(&url).await {
            return Ok(());
        }
        let port = url
            .parse::<reqwest::Url>()
            .ok()
            .and_then(|url| url.port_or_known_default())
            .ok_or_else(|| {
                CoreError::InvalidState("failed to resolve the scenario port".to_string())
            })?;
        let mut command = Command::new(&cli);
        command
            .arg("--profile")
            .arg(profile)
            .arg("--port")
            .arg(port.to_string());
        match self.log_path(profile) {
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
        // Spawn and drop the handle: the scenario keeps running independently
        // of the DeepMate process.
        let child = command
            .spawn()
            .map_err(|err| CoreError::SpawnFailed(format!("failed to start scenario: {err}")))?;
        self.write_pid(profile, child.id())?;

        // Wait until the scenario's web UI actually answers. The hero status
        // and the doctor re-run probe immediately after this returns;
        // reporting at spawn time would race the boot and show "not running".
        let timeout = ui_boot_timeout();
        let deadline = tokio::time::Instant::now() + timeout;
        loop {
            if self.ui_reachable_at(&url).await {
                return Ok(());
            }
            if tokio::time::Instant::now() >= deadline {
                break;
            }
            tokio::time::sleep(Duration::from_millis(300)).await;
        }
        // The UI never came up: kill the half-started instance and clear the
        // pid file, otherwise a zombie keeps the scenario marked as running.
        // The port assignment stays: it is the scenario's stable address,
        // not a leak.
        let pid = child.id();
        if let Err(err) = self.kill_process(pid).await {
            tracing::warn!(profile, pid, error = %err, "failed to kill the scenario that missed its boot deadline");
        }
        if let Err(err) = self.clear_pid(profile) {
            tracing::warn!(profile, error = %err, "failed to clear the scenario pid file");
        }
        Err(CoreError::Timeout(format!(
            "scenario web UI did not come up within {timeout:?}; check {}",
            self.log_path(profile)
                .map(|path| path.to_string_lossy().into_owned())
                .unwrap_or_else(|| "the scenario web log".to_string()),
        )))
    }

    // Resolve the URL a scenario starts on, assigning a new port when
    // needed. The `web` scenario without an override uses the legacy URL
    // override or the engine default.
    async fn scenario_url_for(
        &self,
        profile: &str,
        port_override: Option<u16>,
    ) -> CoreResult<String> {
        let registry = PortRegistry::new(self.data_dir.clone());
        let port = if profile == WEB_PROFILE && port_override.is_none() {
            self.scenario_port(profile)?
                .unwrap_or(ports::DEFAULT_WEB_PORT)
        } else {
            // The assignment loop probes listeners with `lsof`/`netstat`,
            // which block; run it off the async worker threads.
            let platform = Arc::clone(&self.platform);
            let profile = profile.to_string();
            tokio::task::spawn_blocking(move || {
                registry.assign(&profile, port_override, |port| {
                    platform.find_listener_pid(port).is_some()
                })
            })
            .await
            .map_err(|err| {
                CoreError::InvalidState(format!("port assignment task failed: {err}"))
            })??
        };
        Ok(format!("http://127.0.0.1:{port}"))
    }

    // The legacy single-instance stop: the `web` scenario.
    pub async fn stop(&self) -> CoreResult<()> {
        self.stop_scenario(WEB_PROFILE).await
    }

    pub async fn stop_scenario(&self, profile: &str) -> CoreResult<()> {
        let Some(pid) = self.read_pid_async(profile).await else {
            tracing::warn!(
                profile,
                "no harness pid recorded; the scenario may have been started manually"
            );
            return Ok(());
        };
        self.platform
            .kill_process(pid)
            .map_err(|err| CoreError::InvalidState(err.to_string()))?;
        self.clear_pid(profile)?;
        // Free the port only for non-default scenarios: the `web` scenario's
        // port is fixed.
        if profile != WEB_PROFILE {
            let _ = PortRegistry::new(self.data_dir.clone()).release(profile);
        }
        Ok(())
    }

    // The legacy single-instance restart: the `web` scenario.
    pub async fn restart(&self) -> CoreResult<()> {
        self.restart_scenario(WEB_PROFILE).await
    }

    pub async fn restart_scenario(&self, profile: &str) -> CoreResult<()> {
        self.stop_scenario(profile).await?;
        self.start_scenario(profile, None).await
    }

    // The legacy single-instance open: the `web` scenario.
    pub async fn open_ui(&self) -> CoreResult<()> {
        self.open_ui_scenario(WEB_PROFILE).await
    }

    pub async fn open_ui_scenario(&self, profile: &str) -> CoreResult<()> {
        let url = self.scenario_url(profile)?;
        if !self.ui_reachable_at(&url).await {
            return Err(CoreError::InvalidState(format!(
                "scenario {profile} is not running; start it first"
            )));
        }
        self.platform
            .open_url(&url)
            .map_err(|err| CoreError::InvalidState(err.to_string()))
    }

    // Every scenario's runtime state as the UI sees it: web-surface
    // scenarios carry their port and (when running) pid and live status;
    // task-surface scenarios are always idle.
    pub async fn instances(&self) -> CoreResult<Vec<RuntimeInstance>> {
        let mut instances = Vec::new();
        for profile in self.profiles().await? {
            let surface = profile_surface(&profile.id)?;
            let mut instance = RuntimeInstance {
                profile: profile.id.clone(),
                surface,
                status: RuntimeStatusKind::Installed,
                pid: None,
                port: None,
                url: None,
                message: None,
            };
            if surface == Surface::Web {
                if let Ok(Some(port)) = self.scenario_port(&profile.id) {
                    let url = format!("http://127.0.0.1:{port}");
                    instance.port = Some(port);
                    let up = self.ui_reachable_at(&url).await;
                    instance.url = Some(url);
                    if up {
                        instance.status = RuntimeStatusKind::Running;
                        instance.pid = self.read_pid_async(&profile.id).await;
                    }
                }
            } else if surface == Surface::Task {
                instance.message = Some("boots per task".to_string());
            }
            instances.push(instance);
        }
        instances.sort_by(|a, b| a.profile.cmp(&b.profile));
        Ok(instances)
    }

    pub async fn profiles(&self) -> CoreResult<Vec<Profile>> {
        discover_profiles()
    }

    // The legacy global provider/model surface reads and writes the default
    // `web` scenario's settings; snapshots keep using it until the schema
    // learns per-scenario sections.
    pub async fn providers(&self) -> CoreResult<Vec<Provider>> {
        self.providers_scenario(WEB_PROFILE).await
    }

    pub async fn providers_scenario(&self, profile: &str) -> CoreResult<Vec<Provider>> {
        // Lazy migration: every read sees the scenario's own settings
        // document, bootstrapped (redirection + legacy carry-over) on first
        // access.
        dsh::ensure_scene_settings(profile)?;
        list_providers(profile)
    }

    pub async fn models(&self) -> CoreResult<Vec<Model>> {
        self.models_scenario(WEB_PROFILE).await
    }

    pub async fn models_scenario(&self, profile: &str) -> CoreResult<Vec<Model>> {
        dsh::ensure_scene_settings(profile)?;
        list_models(profile)
    }

    pub async fn upsert_provider(&self, provider: Provider) -> CoreResult<()> {
        self.upsert_provider_scenario(WEB_PROFILE, provider).await
    }

    pub async fn upsert_provider_scenario(
        &self,
        profile: &str,
        provider: Provider,
    ) -> CoreResult<()> {
        dsh::ensure_scene_settings(profile)?;
        SettingsEditor::upsert_provider(profile, &provider)
    }

    pub async fn remove_provider(&self, id: &str) -> CoreResult<()> {
        self.remove_provider_scenario(WEB_PROFILE, id).await
    }

    pub async fn remove_provider_scenario(&self, profile: &str, id: &str) -> CoreResult<()> {
        SettingsEditor::remove_provider(profile, id)
    }

    pub async fn upsert_model(&self, provider: &str, model: Model) -> CoreResult<()> {
        self.upsert_model_scenario(WEB_PROFILE, provider, model)
            .await
    }

    pub async fn upsert_model_scenario(
        &self,
        profile: &str,
        provider: &str,
        model: Model,
    ) -> CoreResult<()> {
        dsh::ensure_scene_settings(profile)?;
        SettingsEditor::upsert_model(profile, provider, &model)
    }

    pub async fn remove_model(&self, provider: &str, id: &str) -> CoreResult<()> {
        self.remove_model_scenario(WEB_PROFILE, provider, id).await
    }

    pub async fn remove_model_scenario(
        &self,
        profile: &str,
        provider: &str,
        id: &str,
    ) -> CoreResult<()> {
        SettingsEditor::remove_model(profile, provider, id)
    }

    pub async fn create_profile(&self, name: &str) -> CoreResult<()> {
        create_profile(name)
    }

    // Derive a scenario's run surface from its installed bundle set.
    pub async fn surface(&self, profile_id: &str) -> CoreResult<Surface> {
        profile_surface(profile_id)
    }

    // Create a scenario and bootstrap its surface bundles, so the profile is
    // actually runnable: `@deepseek-ai/dsh-base` plus the surface bundle
    // (`dsh-web-app` for the browser console, `dsh-headless` for one-shot
    // tasks). The engine initializes a missing profile during `dsh plugin
    // add`, and the pnpm operation both installs the packages and (on
    // current engines) reconciles the bundle declarations. A failed
    // bootstrap rolls the scaffold back so an unusable scenario never
    // lingers.
    pub async fn create_scenario(&self, name: &str, surface: Surface) -> CoreResult<()> {
        create_profile(name)?;
        let surface_bundle = match surface {
            Surface::Web => WEB_APP_BUNDLE,
            Surface::Task => HEADLESS_BUNDLE,
            Surface::Undetermined => {
                return Err(CoreError::InvalidState(
                    "a scenario surface must be chosen before creation".to_string(),
                ));
            }
        };
        let result = self
            .run_plugin(name, &["add", BASE_BUNDLE, surface_bundle])
            .await;
        if result.is_err() {
            let _ = remove_profile(name);
            return result;
        }
        // The scenario's settings are its own from the start: isolate the
        // settings document so providers/models never leak across scenarios.
        dsh::ensure_scene_settings(name)
    }

    pub async fn remove_profile(&self, name: &str) -> CoreResult<()> {
        if name == WEB_PROFILE {
            return Err(CoreError::InvalidState(format!(
                "the {WEB_PROFILE} scenario is the default runtime and cannot be removed"
            )));
        }
        remove_profile(name)
    }

    // Rename a profile and carry DeepMate-owned references (such as disabled
    // plugin records) over to the new name. The default `web` scenario is
    // the compatibility default and cannot be renamed.
    pub async fn rename_profile(&self, old: &str, new: &str) -> CoreResult<()> {
        if old == WEB_PROFILE {
            return Err(CoreError::InvalidState(format!(
                "the {WEB_PROFILE} scenario is the default runtime and cannot be renamed"
            )));
        }
        dsh::rename_profile(old, new)?;
        self.registry().rename(old, new)?;
        Ok(())
    }

    // Set (or, with `None`, clear) a scenario's manifest description. The
    // default `web` scenario and any other profile may set it; only
    // rename/remove are protected.
    pub async fn set_scenario_description(
        &self,
        name: &str,
        description: Option<String>,
    ) -> CoreResult<()> {
        set_profile_description(name, description)
    }

    // Read one raw advanced document for the desktop raw editor.
    pub async fn advanced_file_read(
        &self,
        scope: AdvancedFileScope,
        name: Option<String>,
    ) -> CoreResult<Option<String>> {
        read_advanced_file(scope, name.as_deref())
    }

    // Write one raw advanced document back (validated and backed up by the
    // harness filesystem contract).
    pub async fn advanced_file_save(
        &self,
        scope: AdvancedFileScope,
        name: Option<String>,
        content: String,
    ) -> CoreResult<()> {
        save_advanced_file(scope, name.as_deref(), &content)
    }

    pub async fn plugins(&self) -> CoreResult<Vec<Plugin>> {
        let mut plugins = list_all_plugins()?;
        // Enrich each plugin with its trust tier from the curated market, so
        // the installed list can show official/vetted provenance without
        // hitting the network: the curated list is cached (1h TTL, offline
        // fallback) and a fetch failure degrades to no trust signal.
        let curated = market::Market::new(self.data_dir.clone(), self.http.clone())
            .with_cache_ttl(self.market_ttl)
            .curated()
            .await
            .unwrap_or_default();
        let trust: std::collections::HashMap<&str, MarketTrust> = curated
            .iter()
            .map(|entry| (entry.id.as_str(), entry.trust))
            .collect();
        for plugin in &mut plugins {
            plugin.trust = trust.get(plugin.id.as_str()).copied();
        }
        Ok(plugins)
    }

    pub async fn install_plugin(&self, profile: &str, spec: &str) -> CoreResult<()> {
        self.run_plugin(profile, &["add", spec]).await?;
        // Installing a previously disabled plugin is a re-enable: clear the
        // registry record so the listing does not show it twice.
        let package = market::spec_package_name(spec);
        self.registry().remove(profile, package)?;
        // `dsh plugin add` installs the dependency but leaves the bundle
        // declaration untouched, and the bundle list is what the harness web
        // UI actually loads at runtime — so an installed plugin that declares
        // harness capabilities must be declared as a bundle too, or the web UI
        // never loads it. `add_bundle` is idempotent; plain library
        // dependencies are intentionally not declared.
        if dsh::declares_dsh_capability(profile, package)?
            && !dsh::bundle_declared(profile, package)?
        {
            dsh::add_bundle(profile, package)?;
        }
        Ok(())
    }

    pub async fn remove_plugin(&self, profile: &str, id: &str) -> CoreResult<()> {
        // A bundle-only declaration with nothing installed is already
        // removed: stripping the declaration IS the removal, and `dsh plugin
        // remove` would only fail on a package it does not manage.
        let declared = dsh::declared_spec(profile, id)?;
        let installed = dsh::list_plugins(profile)?
            .into_iter()
            .any(|plugin| plugin.id == id && plugin.enabled);
        if declared.is_none() && !installed {
            let stripped = dsh::remove_bundle(profile, id)?;
            if !stripped {
                return Err(CoreError::InvalidState(format!(
                    "plugin {id} is not installed in profile {profile}"
                )));
            }
            return Ok(());
        }
        self.run_plugin(profile, &["remove", id]).await?;
        // `dsh plugin remove` drops the dependency but leaves the bundle
        // declaration behind; a leftover declaration makes the harness web
        // UI fail to load the plugin's client bundle on every boot.
        dsh::remove_bundle(profile, id)?;
        Ok(())
    }

    // Disable a plugin: uninstall it (the harness only loads declared,
    // installed dependencies, so removing it is a real off switch) while
    // remembering its package spec so enabling can restore the same range.
    pub async fn disable_plugin(&self, profile: &str, id: &str) -> CoreResult<()> {
        // Resolve a reinstallable spec before removing: the declared range
        // from the manifest, falling back to the actually installed version
        // for plugins that were never declared (e.g. shared-bundle
        // fallbacks). The full `id@range` form is what `dsh plugin add`
        // accepts, so the record can be passed to install verbatim.
        let declared = dsh::declared_spec(profile, id)?;
        let installed_version = if declared.is_none() {
            dsh::list_plugins(profile)?
                .into_iter()
                .find(|plugin| plugin.id == id)
                .and_then(|plugin| plugin.version)
        } else {
            None
        };
        match (declared, installed_version) {
            // A bundle-only leftover with nothing installed: disabling is
            // just stripping the stale declaration; there is no package spec
            // to remember for a re-enable.
            (None, None) => {
                let stripped = dsh::remove_bundle(profile, id)?;
                if !stripped {
                    return Err(CoreError::InvalidState(format!(
                        "plugin {id} is not installed in profile {profile}"
                    )));
                }
                Ok(())
            }
            (declared, installed_version) => {
                let spec = declared
                    .map(|range| format!("{id}@{range}"))
                    .or_else(|| installed_version.map(|version| format!("{id}@{version}")))
                    .expect("at least one spec source");
                self.run_plugin(profile, &["remove", id]).await?;
                self.registry().record(profile, id, &spec)?;
                // Same leftover-bundle cleanup as remove_plugin.
                dsh::remove_bundle(profile, id)?;
                Ok(())
            }
        }
    }

    // Enable a disabled plugin, reinstalling its recorded package spec (or
    // its declared range when it is still declared but was never installed).
    pub async fn enable_plugin(&self, profile: &str, id: &str) -> CoreResult<()> {
        let registry = self.registry();
        let spec = registry
            .spec(profile, id)?
            .or(dsh::declared_spec(profile, id)?);
        if let Some(spec) = spec {
            // install_plugin clears the registry record on success.
            return self.install_plugin(profile, &spec).await;
        }
        // A declared-but-uninstalled bundle (a leftover after a manual
        // removal) is enabled by reinstalling its declared entry; the
        // harness resolves and pins the latest matching version.
        if dsh::bundle_declared(profile, id)? {
            return self.install_plugin(profile, id).await;
        }
        Err(CoreError::InvalidState(format!(
            "plugin {id} is not disabled in profile {profile}"
        )))
    }

    // The disabled-plugin registry, sorted by (profile, id).
    pub fn disabled_plugins(&self) -> CoreResult<Vec<DisabledPlugin>> {
        self.registry().list()
    }

    fn registry(&self) -> registry::DisabledRegistry {
        registry::DisabledRegistry::new(self.data_dir.clone())
    }

    pub async fn update_plugin(&self, profile: &str, id: Option<&str>) -> CoreResult<()> {
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
    pub async fn stream_plugin_op(
        &self,
        profile: &str,
        kind: PluginOpKind,
        target: Option<&str>,
        tx: tokio::sync::mpsc::Sender<PluginOpEvent>,
    ) -> CoreResult<()> {
        let cli = self
            .find_cli()
            .ok_or_else(|| CoreError::NotFound("harness CLI was not found on PATH".to_string()))?;
        let target = target.unwrap_or("").to_string();
        let forwarded: &[&str] = match kind {
            PluginOpKind::Install => &["add", &target],
            PluginOpKind::Remove => &["remove", &target],
            PluginOpKind::Update => &["update", &target],
            // Tasks are not plugin operations; `run_task` streams them.
            PluginOpKind::Task => {
                return Err(CoreError::InvalidState(
                    "task runs are not plugin operations".to_string(),
                ));
            }
        };
        let _ = tx
            .send(PluginOpEvent::Started {
                op: kind,
                target: target.clone(),
            })
            .await;

        // A `remove` of a bundle-only declaration with nothing installed has
        // no package to uninstall: stripping the stale declaration is the
        // whole operation, and a `dsh plugin remove` would only fail on a
        // package the launcher does not manage.
        if kind == PluginOpKind::Remove {
            let declared = dsh::declared_spec(profile, &target)?;
            let installed = dsh::list_plugins(profile)?
                .into_iter()
                .any(|plugin| plugin.id == target && plugin.enabled);
            if declared.is_none() && !installed {
                let stripped = dsh::remove_bundle(profile, &target)?;
                if !stripped {
                    let detail = format!("plugin {target} is not installed in profile {profile}");
                    let _ = tx
                        .send(PluginOpEvent::Finished {
                            ok: false,
                            detail: Some(detail.clone()),
                        })
                        .await;
                    return Err(CoreError::NotFound(detail));
                }
                let _ = tx
                    .send(PluginOpEvent::Finished {
                        ok: true,
                        detail: None,
                    })
                    .await;
                return Ok(());
            }
        }

        // The child gets its own process group (unix) so a timeout can kill dsh
        // and any pnpm grandchild together instead of orphaning it.
        let mut command = tokio::process::Command::new(&cli);
        command
            .arg("plugin")
            .arg("--profile")
            .arg(profile)
            .args(forwarded);

        let timeout = plugin_op_timeout();
        tracing::debug!(
            profile,
            target = %target,
            timeout_secs = timeout.as_secs(),
            "running `dsh plugin`"
        );
        let run = run_streamed_child(&mut command, &tx, timeout).await;

        match run {
            Ok(run) => {
                if run.status.success() {
                    tracing::debug!(profile, target = %target, "`dsh plugin` finished");
                    // `dsh plugin remove` drops the dependency but leaves the
                    // bundle declaration behind; strip it so the web UI stops
                    // trying to load a removed plugin on every boot.
                    if kind == PluginOpKind::Remove {
                        if let Err(err) = dsh::remove_bundle(profile, &target) {
                            tracing::warn!(%err, profile, target = %target, "failed to strip leftover bundle declaration");
                        }
                    }
                    let _ = tx
                        .send(PluginOpEvent::Finished {
                            ok: true,
                            detail: None,
                        })
                        .await;
                    Ok(())
                } else {
                    let detail = if run.stderr_tail.is_empty() {
                        format!("`dsh plugin` failed with exit {:?}", run.status.code())
                    } else {
                        format!(
                            "`dsh plugin` failed with exit {:?}: {}",
                            run.status.code(),
                            run.stderr_tail.join(" ")
                        )
                    };
                    tracing::warn!(%detail, "`dsh plugin` failed");
                    let _ = tx
                        .send(PluginOpEvent::Finished {
                            ok: false,
                            detail: Some(detail.clone()),
                        })
                        .await;
                    Err(CoreError::CommandFailed(detail))
                }
            }
            Err(ChildRunFailure::Spawn(err)) => {
                let detail = format!("failed to run `dsh plugin`: {err}");
                let _ = tx
                    .send(PluginOpEvent::Finished {
                        ok: false,
                        detail: Some(detail.clone()),
                    })
                    .await;
                Err(CoreError::SpawnFailed(detail))
            }
            Err(ChildRunFailure::Wait(err)) => {
                let detail = format!("failed to wait for `dsh plugin`: {err}");
                let _ = tx
                    .send(PluginOpEvent::Finished {
                        ok: false,
                        detail: Some(detail.clone()),
                    })
                    .await;
                Err(CoreError::CommandFailed(detail))
            }
            Err(ChildRunFailure::TimedOut) => {
                let detail = format!(
                    "`dsh plugin` did not finish within {timeout:?} and was stopped. \
                     A leftover pnpm/dsh process may have been stuck; retry the operation."
                );
                let _ = tx
                    .send(PluginOpEvent::Finished {
                        ok: false,
                        detail: Some(detail.clone()),
                    })
                    .await;
                Err(CoreError::Timeout(detail))
            }
        }
    }

    pub async fn search_plugins(&self, query: &str) -> CoreResult<Vec<MarketEntry>> {
        let market = market::Market::new(self.data_dir.clone(), self.http.clone())
            .with_cache_ttl(self.market_ttl);
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

    pub async fn market_sources(&self) -> CoreResult<Vec<MarketSourceInfo>> {
        Ok(market::market_sources())
    } // Run one task against a task-surface scenario (the `dsh-headless`
      // bundle): `dsh --profile <name> "<prompt>"` boots the profile, answers
      // exactly once, prints the final text on stdout and exits. Output is
      // streamed as `Line` events (mirroring plugin operations) and, on
      // success, persisted to `logs/task-<profile>-<timestamp>.log`. The run
      // is always bounded: the same timeout-and-kill discipline as plugin
      // operations applies.
    pub async fn run_task(
        &self,
        profile: &str,
        prompt: &str,
        tx: tokio::sync::mpsc::Sender<PluginOpEvent>,
    ) -> CoreResult<()> {
        if profile_surface(profile)? != Surface::Task {
            return Err(CoreError::InvalidState(format!(
                "scenario {profile} is not a task scenario (missing @deepseek-ai/dsh-headless)"
            )));
        }
        let cli = self
            .find_cli()
            .ok_or_else(|| CoreError::NotFound("harness CLI was not found on PATH".to_string()))?;
        let _ = tx
            .send(PluginOpEvent::Started {
                op: PluginOpKind::Task,
                target: prompt.to_string(),
            })
            .await;

        let mut command = tokio::process::Command::new(&cli);
        command.arg("--profile").arg(profile).arg(prompt);

        let timeout = plugin_op_timeout();
        tracing::debug!(profile, timeout_secs = timeout.as_secs(), "running task");
        let run = run_streamed_child(&mut command, &tx, timeout).await;

        match run {
            Ok(run) if run.status.success() => {
                // Persist the answer so a task result outlives the run.
                if let Some(dir) = self.data_dir.as_ref() {
                    let stamp = chrono::Utc::now().format("%Y%m%d-%H%M%S");
                    let log = dir.join("logs").join(format!("task-{profile}-{stamp}.log"));
                    // The run itself succeeded; a failed log write is worth a
                    // warning, not a task failure.
                    let written = std::fs::create_dir_all(log.parent().unwrap_or(dir))
                        .and_then(|()| std::fs::write(&log, run.stdout_lines.join("\n")));
                    match written {
                        Ok(()) => {
                            tracing::info!(profile, path = %log.display(), "task log written")
                        }
                        Err(err) => tracing::warn!(
                            profile,
                            path = %log.display(),
                            error = %err,
                            "failed to write the task log"
                        ),
                    }
                }
                let _ = tx
                    .send(PluginOpEvent::Finished {
                        ok: true,
                        detail: None,
                    })
                    .await;
                Ok(())
            }
            Ok(run) => {
                let detail = if run.stderr_tail.is_empty() {
                    format!("task failed with exit {:?}", run.status.code())
                } else {
                    format!(
                        "task failed with exit {:?}: {}",
                        run.status.code(),
                        run.stderr_tail.join(" ")
                    )
                };
                tracing::warn!(%detail, "task failed");
                let _ = tx
                    .send(PluginOpEvent::Finished {
                        ok: false,
                        detail: Some(detail.clone()),
                    })
                    .await;
                Err(CoreError::CommandFailed(detail))
            }
            Err(ChildRunFailure::Spawn(err)) => {
                let detail = format!("failed to run task: {err}");
                let _ = tx
                    .send(PluginOpEvent::Finished {
                        ok: false,
                        detail: Some(detail.clone()),
                    })
                    .await;
                Err(CoreError::SpawnFailed(detail))
            }
            Err(ChildRunFailure::Wait(err)) => {
                let detail = format!("failed to wait for the task: {err}");
                let _ = tx
                    .send(PluginOpEvent::Finished {
                        ok: false,
                        detail: Some(detail.clone()),
                    })
                    .await;
                Err(CoreError::CommandFailed(detail))
            }
            Err(ChildRunFailure::TimedOut) => {
                let detail = format!("task timed out after {timeout:?}");
                tracing::warn!(%detail, profile);
                let _ = tx
                    .send(PluginOpEvent::Finished {
                        ok: false,
                        detail: Some(detail.clone()),
                    })
                    .await;
                Err(CoreError::Timeout(detail))
            }
        }
    }

    // Compatibility check: the harness version detected from the CLI is
    // matched against the package's declared `engines` requirement.
    pub async fn plugin_compat(&self, spec: &str) -> CoreResult<CompatReport> {
        let harness_version = self.cli_version();
        market::Market::compat(&self.http, spec, harness_version).await
    }

    // Declared-but-uninstalled bundles: a `dsh.profile.bundles` entry with
    // no installed package and no dependency declaration. The harness web
    // UI fails to load these on every boot, and `dsh plugin remove` leaves
    // the declaration behind, so the leftover must be surfaced rather than
    // stay invisible.
    async fn dangling_bundles(&self) -> CoreResult<Vec<(String, String)>> {
        let mut dangling = Vec::new();
        for profile in self.profiles().await? {
            for plugin in dsh::list_plugins(&profile.id)? {
                if plugin.enabled {
                    continue;
                }
                if dsh::declared_spec(&profile.id, &plugin.id)?.is_some() {
                    continue;
                }
                if dsh::bundle_declared(&profile.id, &plugin.id)? {
                    dangling.push((profile.id.clone(), plugin.id.clone()));
                }
            }
        }
        dangling.sort();
        Ok(dangling)
    }

    // Probe the live web UI's `/plugins/<id>/client.js` loader entries for the
    // running scenario's profile-local bundles that carry a web client
    // (`dsh.client` in their own package.json) and report the failures — the
    // exact "bundle script failed to load" condition at the HTTP layer.
    // Only the running scenario is probed: the console loads exactly one
    // scenario's bundle set (frozen at its start), so any other scenario's
    // bundles would 404 even when healthy. Patch-only bundles (skills/tools,
    // no client) are skipped: the route never serves them even when healthy.
    // Shared-fallback bundles and plain dependencies are not probed either.
    // Empty when the console is not running.
    async fn failing_bundle_loads(&self) -> CoreResult<Vec<BundleLoadFailure>> {
        if !self.ui_reachable().await {
            return Ok(Vec::new());
        }
        self.probe_bundle_loads().await
    }

    // The probe itself, for a caller that already knows the web UI is up
    // (doctor shares one reachability probe across all checks).
    async fn probe_bundle_loads(&self) -> CoreResult<Vec<BundleLoadFailure>> {
        let mut targets = Vec::new();
        for profile in self.profiles().await? {
            // Only the running scenario is probed — see `failing_bundle_loads`.
            if profile.id != WEB_PROFILE {
                continue;
            }
            for plugin in dsh::list_plugins(&profile.id)? {
                if !dsh::bundle_declared(&profile.id, &plugin.id)? {
                    continue;
                }
                if !dsh::installed_in_profile(&profile.id, &plugin.id)? {
                    continue;
                }
                if !dsh::declares_web_client(&profile.id, &plugin.id)? {
                    continue;
                }
                targets.push((profile.id.clone(), plugin.id.clone()));
            }
        }
        let results = futures::future::join_all(targets.into_iter().map(|(profile, id)| {
            let url = format!("{}/plugins/{}/client.js", self.ui_url(), id);
            let http = self.http.clone();
            async move {
                let outcome = http.get(&url).send().await;
                (profile, id, outcome)
            }
        }))
        .await;
        let mut failures = Vec::new();
        for (profile, id, outcome) in results {
            let reason = match outcome {
                Ok(response) if response.status().is_success() => continue,
                Ok(response) => format!("HTTP {}", response.status().as_u16()),
                Err(err) => err.to_string(),
            };
            failures.push(BundleLoadFailure {
                profile,
                id,
                reason,
            });
        }
        Ok(failures)
    }

    pub async fn doctor(&self) -> CoreResult<DoctorReport> {
        let cli = self.find_cli();
        let mut checks = Vec::new();

        if cli.is_some() {
            checks.push(DoctorCheck {
                id: "runtime.installed".to_string(),
                status: CheckStatus::Pass,
                summary: "DeepSeek Harness CLI was found".to_string(),
                details: cli.as_ref().map(|name| format!("command: {name}")),
                suggested_action: None,
                cli: cli.clone(),
                url: None,
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
                cli: None,
                url: None,
            });
        }

        // The configured address must be a usable http(s) URL. Anything else
        // would make every reachability report misleading ("no listener at
        // abc"), so it is flagged here instead of trusted downstream.
        let ui_url = self.ui_url();
        let ui_url_valid = ui_url.parse::<reqwest::Url>().ok().is_some_and(|url| {
            matches!(url.scheme(), "http" | "https") && url.host_str().is_some()
        });
        if ui_url_valid {
            checks.push(DoctorCheck {
                id: "ui.url".to_string(),
                status: CheckStatus::Pass,
                summary: "Harness UI URL is configured".to_string(),
                details: Some(format!("url: {ui_url}")),
                suggested_action: None,
                cli: None,
                url: Some(ui_url.clone()),
            });
        } else {
            checks.push(DoctorCheck {
                id: "ui.url".to_string(),
                status: CheckStatus::Warn,
                summary: "Harness UI URL is not a usable address".to_string(),
                details: Some(format!("unusable url: {ui_url}")),
                suggested_action: Some(
                    "Fix the DEEPMATE_HARNESS_UI_URL override or remove it to restore the default"
                        .to_string(),
                ),
                cli: None,
                url: Some(ui_url.clone()),
            });
        }

        // One shared probe drives both the reachability check and the
        // runtime bundle probe below, so the report can never disagree with
        // itself across the two calls. The probe only runs for a usable
        // address: with a broken URL the "start" fix would boot the server
        // on its own port while every check kept staring at the bad one.
        let ui_up = ui_url_valid && self.ui_reachable().await;
        if ui_up {
            checks.push(DoctorCheck {
                id: "ui.reachable".to_string(),
                status: CheckStatus::Pass,
                summary: "Harness web UI is reachable".to_string(),
                details: Some(format!("url: {ui_url}")),
                suggested_action: None,
                cli: None,
                url: Some(ui_url.clone()),
            });
        } else if !ui_url_valid {
            checks.push(DoctorCheck {
                id: "ui.reachable".to_string(),
                status: CheckStatus::Skip,
                summary: "Web UI check skipped: the configured address is not usable".to_string(),
                details: Some("fix the web UI address first".to_string()),
                suggested_action: None,
                cli: None,
                url: Some(ui_url.clone()),
            });
        } else if cli.is_none() {
            // Without the CLI the web UI cannot be started, so "not
            // running" would be a scolding for something the user cannot
            // fix yet — skip until the engine is installed.
            checks.push(DoctorCheck {
                id: "ui.reachable".to_string(),
                status: CheckStatus::Skip,
                summary: "Web UI check skipped: harness CLI is not installed".to_string(),
                details: Some("install the harness CLI to enable the web UI".to_string()),
                suggested_action: None,
                cli: None,
                url: Some(ui_url.clone()),
            });
        } else {
            checks.push(DoctorCheck {
                id: "ui.reachable".to_string(),
                status: CheckStatus::Warn,
                summary: "Harness web UI is not running".to_string(),
                details: Some(format!("no listener at {ui_url}")),
                suggested_action: Some(
                    "Run `deepmate runtime start` to launch the web UI".to_string(),
                ),
                cli: None,
                url: Some(ui_url.clone()),
            });
        }

        // Declared-but-uninstalled bundles make the harness web UI fail to
        // load the plugin's client bundle on every boot, and `dsh plugin
        // remove` leaves the declaration behind, so the leftover must be
        // surfaced rather than stay invisible.
        let dangling = self.dangling_bundles().await?;
        let dangling: Vec<String> = dangling
            .iter()
            .map(|(profile, id)| format!("{id} ({profile})"))
            .collect();
        if dangling.is_empty() {
            checks.push(DoctorCheck {
                id: "plugins.bundles".to_string(),
                status: CheckStatus::Pass,
                summary: "Declared profile bundles are all installed".to_string(),
                details: None,
                suggested_action: None,
                cli: None,
                url: None,
            });
        } else {
            checks.push(DoctorCheck {
                id: "plugins.bundles".to_string(),
                status: CheckStatus::Warn,
                summary: "Profile declares bundles that are not installed".to_string(),
                details: Some(dangling.join(", ")),
                suggested_action: Some(
                    "Remove the leftover declaration (Plugins page) or reinstall the plugin"
                        .to_string(),
                ),
                cli: None,
                url: None,
            });
        }

        // Runtime load probe: the harness web UI serves one `/plugins/<id>/client.js`
        // loader entry per profile-local bundle. Probing that URL reproduces
        // the exact failure the browser reports ("bundle script failed to
        // load") when a package is installed but cannot be served — a
        // mismatch the static check above cannot see. The probe needs the
        // live UI, so it is skipped (not failed) when the console is off.
        if ui_up {
            let failures = self.probe_bundle_loads().await?;
            let failures: Vec<String> = failures
                .iter()
                .map(|failure| format!("{} ({}) → {}", failure.id, failure.profile, failure.reason))
                .collect();
            if failures.is_empty() {
                checks.push(DoctorCheck {
                    id: "plugins.bundles.load".to_string(),
                    status: CheckStatus::Pass,
                    summary: "Installed bundles are servable by the web UI".to_string(),
                    details: None,
                    suggested_action: None,
                    cli: None,
                    url: None,
                });
            } else {
                checks.push(DoctorCheck {
                    id: "plugins.bundles.load".to_string(),
                    status: CheckStatus::Warn,
                    summary: "Some installed bundles fail to load in the web UI".to_string(),
                    details: Some(failures.join(", ")),
                    // The console keeps the bundle set it started with: a
                    // change to the manifest/install state only takes effect
                    // after a restart, so restarting is the first remedy.
                    suggested_action: Some(
                        "Restart the scenario to load the new bundle set".to_string(),
                    ),
                    cli: None,
                    url: None,
                });
            }
        } else {
            checks.push(DoctorCheck {
                id: "plugins.bundles.load".to_string(),
                status: CheckStatus::Skip,
                summary: "Bundle load probe skipped: web UI is not running".to_string(),
                details: None,
                suggested_action: None,
                cli: None,
                url: None,
            });
        }

        Ok(DoctorReport { checks })
    }

    // One-click repair for a failing doctor check, so diagnostics are more
    // than a report: every actionable check carries a repair the frontend
    // can trigger directly and then re-run the check against.
    //
    //   plugins.bundles        + "reinstall" — reinstall every leftover
    //       bundle at the latest version (restores plugin and declaration).
    //   plugins.bundles        + "clear"     — strip every stale
    //       declaration; the console stops trying to load removed plugins.
    //   plugins.bundles.load   + "restart"   — restart the scenario so the
    //       console picks up the current bundle set (a running console keeps
    //       the set it started with; manifest/install changes need a restart
    //       to take effect).
    //   plugins.bundles.load   + "reinstall" — update plugins the live web
    //       UI fails to serve after a restart (typically a version mismatch).
    //   ui.reachable           + "start"     — boot the harness web UI.
    //
    // Batch repairs never abort on the first failure: every item is
    // attempted and counted, failures are collected. Returns the number of
    // repaired items; a repair that fixed nothing but hit failures returns
    // an Err carrying them.
    pub async fn fix_check(&self, check_id: &str, mode: &str) -> CoreResult<u32> {
        let mut fixed = 0u32;
        let mut failures = Vec::new();
        match (check_id, mode) {
            ("plugins.bundles", "clear") => {
                for (profile, id) in self.dangling_bundles().await? {
                    match self.remove_plugin(&profile, &id).await {
                        Ok(()) => fixed += 1,
                        Err(err) => failures.push(format!("{id} ({profile}): {err}")),
                    }
                }
            }
            ("plugins.bundles", "reinstall") => {
                for (profile, id) in self.dangling_bundles().await? {
                    match self.install_plugin(&profile, &id).await {
                        Ok(()) => fixed += 1,
                        Err(err) => failures.push(format!("{id} ({profile}): {err}")),
                    }
                }
            }
            ("plugins.bundles.load", "restart") => match self.restart().await {
                Ok(()) => fixed = 1,
                Err(err) => failures.push(format!("{err}")),
            },
            ("plugins.bundles.load", "reinstall") => {
                for failing in self.failing_bundle_loads().await? {
                    match self
                        .update_plugin(&failing.profile, Some(&failing.id))
                        .await
                    {
                        Ok(()) => fixed += 1,
                        Err(err) => {
                            failures.push(format!("{} ({}): {err}", failing.id, failing.profile))
                        }
                    }
                }
            }
            ("ui.reachable", "start") => match self.start().await {
                Ok(()) => fixed = 1,
                Err(err) => failures.push(format!("{err}")),
            },
            _ => {
                return Err(CoreError::InvalidState(format!(
                    "unsupported doctor fix: {check_id} + {mode}"
                )));
            }
        }
        for failure in &failures {
            tracing::warn!(%failure, check_id, mode, "doctor fix partially failed");
        }
        if fixed == 0 && !failures.is_empty() {
            return Err(CoreError::InvalidState(failures.join("; ")));
        }
        Ok(fixed)
    }

    // Capture the current harness inventory into a portable snapshot.
    pub async fn capture_snapshot(&self) -> CoreResult<Snapshot> {
        Ok(Snapshot {
            format: SNAPSHOT_FORMAT.to_string(),
            created: chrono::Utc::now().to_rfc3339(),
            profiles: self.profiles().await?,
            providers: self.providers().await?,
            models: self.models().await?,
            plugins: self.plugins().await?,
        })
    }

    // Apply a snapshot to this harness, merge-style. Every profile is
    // created, every provider and model upserted, and every plugin installed.
    // Existing items on the target are overwritten, never removed.
    pub async fn apply_snapshot(&self, snapshot: &Snapshot) -> CoreResult<SnapshotReport> {
        let mut report = SnapshotReport::default();

        for profile in &snapshot.profiles {
            self.create_profile(&profile.id).await?;
            report.profiles += 1;
        }
        // A freshly scaffolded profile has no base bundle, and the base
        // bundle is what composes the provider route into a profile — without
        // it the snapshot's profiles would be unusable shells. Bootstrap it
        // when the snapshot does not already carry it; without the engine CLI
        // the scaffold simply stays a shell (same as a plugin-less import
        // always has).
        for profile in &snapshot.profiles {
            let has_base = snapshot
                .plugins
                .iter()
                .any(|plugin| plugin.profile == profile.id && plugin.id == BASE_BUNDLE);
            if !has_base
                && self.find_cli().is_some()
                && dsh::profile_surface(&profile.id)? == Surface::Undetermined
            {
                self.install_plugin(&profile.id, BASE_BUNDLE).await?;
                report.plugins += 1;
            }
        }
        for provider in &snapshot.providers {
            self.upsert_provider(provider.clone()).await?;
            report.providers += 1;
        }
        for model in &snapshot.models {
            let provider = model
                .provider
                .clone()
                .ok_or_else(|| CoreError::NotFound("model has no provider".to_string()))?;
            self.upsert_model(&provider, model.clone()).await?;
            report.models += 1;
        }
        for plugin in &snapshot.plugins {
            // Plugins without an attributable profile cannot be reinstalled.
            if plugin.profile.is_empty() {
                continue;
            }
            self.install_plugin(&plugin.profile, &plugin.id).await?;
            report.plugins += 1;
        }

        Ok(report)
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use deepmate_platform::SystemPlatform;

    // Serializes DSH_HOME mutation against the dsh.rs tests.
    pub(crate) use crate::dsh::tests::ENV_LOCK as DSH_ENV_LOCK;

    #[tokio::test]
    async fn detect_returns_not_found_in_clean_environment() {
        let harness = DeepSeekHarness::new(Arc::new(SystemPlatform))
            .with_cli_names(vec!["definitely-not-a-real-deepmate-command".to_string()]);
        let detection = harness.detect().await.unwrap();
        assert!(!detection.found);
    }

    // Test platform: find_listener_pid answers with a fixed pid so the
    // pid-file fallback can be exercised without a real socket.
    struct ListenerPlatform(u32);

    impl PlatformService for ListenerPlatform {
        fn name(&self) -> &'static str {
            "test"
        }
        fn open_url(&self, _url: &str) -> deepmate_platform::PlatformResult<()> {
            Ok(())
        }
        fn open_path(&self, _path: &Path) -> deepmate_platform::PlatformResult<()> {
            Ok(())
        }
        fn data_dir(&self) -> deepmate_platform::PlatformResult<PathBuf> {
            Ok(PathBuf::from("/tmp/deepmate-fake-data"))
        }
        fn kill_process(&self, _pid: u32) -> deepmate_platform::PlatformResult<()> {
            Ok(())
        }
        fn find_listener_pid(&self, _port: u16) -> Option<u32> {
            Some(self.0)
        }
    }

    #[tokio::test]
    async fn read_pid_falls_back_to_the_process_owning_the_ui_port() {
        let dir =
            std::env::temp_dir().join(format!("deepmate-pid-fallback-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let harness = DeepSeekHarness::new(Arc::new(ListenerPlatform(4242)))
            .with_data_dir(dir.clone())
            .with_ui_url("http://127.0.0.1:3080");
        assert_eq!(harness.read_pid_async("web").await, Some(4242));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[tokio::test]
    async fn read_pid_prefers_the_pid_file_over_the_port_listener() {
        let dir =
            std::env::temp_dir().join(format!("deepmate-pid-file-test-{}", std::process::id()));
        std::fs::create_dir_all(dir.join("state")).unwrap();
        std::fs::write(dir.join("state/harness.pid"), "9999").unwrap();
        let harness = DeepSeekHarness::new(Arc::new(ListenerPlatform(4242)))
            .with_data_dir(dir.clone())
            .with_ui_url("http://127.0.0.1:3080");
        assert_eq!(harness.read_pid_async("web").await, Some(9999));
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn ui_url_defaults_to_local_web_ui() {
        let harness = DeepSeekHarness::new(Arc::new(SystemPlatform));
        assert_eq!(harness.ui_url(), DEFAULT_UI_URL);
    }

    #[tokio::test]
    async fn ui_unreachable_when_nothing_listens() {
        // Port 1 is never a listening harness; the HTTP probe must report it
        // as unreachable rather than trusting a raw TCP connect.
        let harness =
            DeepSeekHarness::new(Arc::new(SystemPlatform)).with_ui_url("http://127.0.0.1:1");
        assert!(!harness.ui_reachable().await);
    }

    #[tokio::test]
    async fn open_ui_fails_when_not_running() {
        let harness =
            DeepSeekHarness::new(Arc::new(SystemPlatform)).with_ui_url("http://127.0.0.1:1");
        let err = harness.open_ui().await.unwrap_err();
        assert!(err.to_string().contains("not running"));
    }

    // Install a fake `dsh` launcher script that answers `--version` and
    // records the forwarded plugin arguments to a file, so run_plugin can be
    // tested without a real harness.

    // A fake launcher that records every invocation (not just `plugin`
    // subcommands) and exits 0, for start_scenario argument-shape tests.
    #[cfg(unix)]
    fn fake_generic_cli(dir: &std::path::Path) -> std::path::PathBuf {
        use std::os::unix::fs::PermissionsExt;
        std::fs::create_dir_all(dir).unwrap();
        let recorded = dir.join("recorded-args");
        let script = dir.join("fake-dsh-generic");
        std::fs::write(
            &script,
            format!(
                "#!/bin/sh\n[ \"$1\" = \"--version\" ] && exit 0\nprintf '%s\\n' \"$@\" > \"{}\"\nexit 0\n",
                recorded.display()
            ),
        )
        .unwrap();
        let mut perms = std::fs::metadata(&script).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&script, perms).unwrap();
        script
    }

    // A fake launcher that records the invocation, prints a body to stdout
    // and exits with the given code, for task-stream tests.
    #[cfg(unix)]
    fn fake_task_cli(dir: &std::path::Path, body: &str, exit_code: i32) -> std::path::PathBuf {
        use std::os::unix::fs::PermissionsExt;
        std::fs::create_dir_all(dir).unwrap();
        let recorded = dir.join("recorded-args");
        let script = dir.join("fake-dsh-task");
        std::fs::write(
            &script,
            format!(
                "#!/bin/sh\n[ \"$1\" = \"--version\" ] && exit 0\nprintf '%s\\n' \"$@\" > \"{}\"\n{body}\nexit {exit_code}\n",
                recorded.display(),
            ),
        )
        .unwrap();
        let mut perms = std::fs::metadata(&script).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&script, perms).unwrap();
        script
    }
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
        let harness = DeepSeekHarness::new(Arc::new(SystemPlatform))
            .with_cli_names(vec![cli.to_string_lossy().into_owned()]);
        harness.install_plugin("web", "dsh-mnemon").await.unwrap();
        let args = std::fs::read_to_string(&recorded).unwrap();
        assert_eq!(
            args.lines().collect::<Vec<_>>(),
            ["plugin", "--profile", "web", "add", "dsh-mnemon"]
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn remove_and_update_forward_correct_verbs() {
        // remove_plugin consults the profile manifest (to detect leftover
        // bundle declarations), so the test home must be isolated from any
        // real harness installation.
        let _guard = dsh::tests::ENV_LOCK.lock().unwrap();
        let previous = std::env::var_os("DSH_HOME");
        let home = fixture_home(300);
        let dir = home.join("profiles/headless");
        std::fs::create_dir_all(&dir).unwrap();
        let manifest = serde_json::json!({
            "name": "dsh-profile-headless",
            "private": true,
            "dependencies": { "old-pkg": "^1.0.0" },
            "dsh": { "profile": {} },
        });
        std::fs::write(
            dir.join("package.json"),
            serde_json::to_string_pretty(&manifest).unwrap(),
        )
        .unwrap();
        std::env::set_var("DSH_HOME", &home);

        let work = fixture_home(301);
        std::fs::create_dir_all(&work).unwrap();
        let cli = fake_plugin_cli(work.as_path(), 0);
        let recorded = work.join("recorded-args");
        let harness = DeepSeekHarness::new(Arc::new(SystemPlatform))
            .with_cli_names(vec![cli.to_string_lossy().into_owned()])
            .with_data_dir(work.clone());

        harness.remove_plugin("headless", "old-pkg").await.unwrap();
        assert_eq!(
            std::fs::read_to_string(&recorded)
                .unwrap()
                .lines()
                .collect::<Vec<_>>(),
            ["plugin", "--profile", "headless", "remove", "old-pkg"]
        );

        harness.update_plugin("web", None).await.unwrap();
        assert_eq!(
            std::fs::read_to_string(&recorded)
                .unwrap()
                .lines()
                .collect::<Vec<_>>(),
            ["plugin", "--profile", "web", "update"]
        );

        harness
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

        std::fs::remove_dir_all(&home).ok();
        std::fs::remove_dir_all(&work).ok();
        restore_env("DSH_HOME", previous);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn plugin_command_failure_propagates_output() {
        let dir =
            std::env::temp_dir().join(format!("deepmate-plugin-fail-test-{}", std::process::id()));
        let cli = fake_plugin_cli(&dir, 7);
        let recorded = dir.join("recorded-args");
        std::fs::write(&recorded, "").unwrap();
        let harness = DeepSeekHarness::new(Arc::new(SystemPlatform))
            .with_cli_names(vec![cli.to_string_lossy().into_owned()]);
        let err = harness.install_plugin("web", "bad-pkg").await.unwrap_err();
        let message = err.to_string();
        assert!(
            message.contains("Some(7)"),
            "exit code must be surfaced in the error: {message}"
        );

        // A missing CLI surfaces the not-found error before any forwarding.
        let missing = DeepSeekHarness::new(Arc::new(SystemPlatform))
            .with_cli_names(vec!["definitely-not-a-command-xyz".to_string()]);
        assert!(missing.install_plugin("web", "pkg").await.is_err());
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn web_profile_is_protected_from_remove_and_rename() {
        let _guard = dsh::tests::ENV_LOCK.lock().unwrap();
        let previous = std::env::var_os("DSH_HOME");
        let home = fixture_home(302);
        std::env::set_var("DSH_HOME", &home);
        let harness = DeepSeekHarness::new(Arc::new(SystemPlatform));
        let err = harness.remove_profile("web").await.unwrap_err();
        assert!(err.to_string().contains("cannot be removed"), "{err}");
        let err = harness.rename_profile("web", "prod").await.unwrap_err();
        assert!(err.to_string().contains("cannot be renamed"), "{err}");
        // Non-web profiles pass through to the filesystem for real.
        let err = harness.remove_profile("missing").await.unwrap_err();
        assert!(err.to_string().contains("profile not found"), "{err}");
        std::fs::remove_dir_all(&home).ok();
        restore_env("DSH_HOME", previous);
    }

    #[cfg(unix)]
    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn create_scenario_bootstraps_the_surface_bundles() {
        let _guard = dsh::tests::ENV_LOCK.lock().unwrap();
        let previous = std::env::var_os("DSH_HOME");
        let home = fixture_home(303);
        std::env::set_var("DSH_HOME", &home);

        let work = fixture_home(304);
        let cli = fake_plugin_cli(work.as_path(), 0);
        let recorded = work.join("recorded-args");
        let harness = DeepSeekHarness::new(Arc::new(SystemPlatform))
            .with_cli_names(vec![cli.to_string_lossy().into_owned()])
            .with_data_dir(work.clone());

        harness
            .create_scenario("coding", Surface::Web)
            .await
            .unwrap();
        assert_eq!(
            std::fs::read_to_string(&recorded)
                .unwrap()
                .lines()
                .collect::<Vec<_>>(),
            [
                "plugin",
                "--profile",
                "coding",
                "add",
                "@deepseek-ai/dsh-base",
                "@deepseek-ai/dsh-web-app"
            ]
        );
        assert!(home.join("profiles/coding/package.json").is_file());

        harness
            .create_scenario("nightly", Surface::Task)
            .await
            .unwrap();
        let args = std::fs::read_to_string(&recorded).unwrap();
        assert!(
            args.lines().any(|line| line == "@deepseek-ai/dsh-headless"),
            "task scenarios must add the headless surface: {args}"
        );

        std::fs::remove_dir_all(&home).ok();
        std::fs::remove_dir_all(&work).ok();
        restore_env("DSH_HOME", previous);
    }

    #[cfg(unix)]
    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn create_scenario_rolls_back_when_bootstrap_fails() {
        let _guard = dsh::tests::ENV_LOCK.lock().unwrap();
        let previous = std::env::var_os("DSH_HOME");
        let home = fixture_home(305);
        std::env::set_var("DSH_HOME", &home);

        let work = fixture_home(306);
        // An engine that fails the pnpm operation must not leave a broken
        // scenario shell behind.
        let cli = fake_plugin_cli(work.as_path(), 7);
        let harness = DeepSeekHarness::new(Arc::new(SystemPlatform))
            .with_cli_names(vec![cli.to_string_lossy().into_owned()])
            .with_data_dir(work.clone());
        assert!(harness
            .create_scenario("broken", Surface::Web)
            .await
            .is_err());
        assert!(
            !home.join("profiles/broken/package.json").is_file(),
            "a failed bootstrap must roll the scaffold back"
        );

        std::fs::remove_dir_all(&home).ok();
        std::fs::remove_dir_all(&work).ok();
        restore_env("DSH_HOME", previous);
    }

    #[cfg(unix)]
    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn rename_profile_carries_disabled_records_over() {
        let _guard = dsh::tests::ENV_LOCK.lock().unwrap();
        let previous = std::env::var_os("DSH_HOME");
        let home = fixture_home(307);
        let dir = home.join("profiles/daily");
        std::fs::create_dir_all(&dir).unwrap();
        let manifest = serde_json::json!({
            "name": "dsh-profile-daily",
            "private": true,
            "dependencies": { "dsh-mnemon": "^0.2" },
            "dsh": { "profile": {} },
        });
        std::fs::write(
            dir.join("package.json"),
            serde_json::to_string_pretty(&manifest).unwrap(),
        )
        .unwrap();
        std::env::set_var("DSH_HOME", &home);

        let work = fixture_home(308);
        let cli = fake_plugin_cli(work.as_path(), 0);
        let harness = DeepSeekHarness::new(Arc::new(SystemPlatform))
            .with_cli_names(vec![cli.to_string_lossy().into_owned()])
            .with_data_dir(work.clone());
        // disable_plugin resolves the declared spec and uninstalls via the CLI;
        // the fake records the arguments and exits 0.
        harness.disable_plugin("daily", "dsh-mnemon").await.unwrap();
        assert_eq!(harness.disabled_plugins().unwrap().len(), 1);
        harness.rename_profile("daily", "dev").await.unwrap();
        // The disabled record follows the rename, so enable works on the new
        // name and clears the record.
        harness.enable_plugin("dev", "dsh-mnemon").await.unwrap();
        assert!(
            harness.disabled_plugins().unwrap().is_empty(),
            "re-enable must clear the record"
        );

        std::fs::remove_dir_all(&home).ok();
        std::fs::remove_dir_all(&work).ok();
        restore_env("DSH_HOME", previous);
    }

    #[cfg(unix)]
    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn start_scenario_forwards_profile_and_port() {
        let _guard = dsh::tests::ENV_LOCK.lock().unwrap();
        let previous = std::env::var_os("DSH_HOME");
        let previous_boot = std::env::var_os(WEB_UI_BOOT_TIMEOUT_ENV);
        let home = fixture_home(309);
        write_web_surface_fixture(&home);
        // A second web-surface scenario gets the first registry port.
        write_scenario_fixture(&home, "coding");
        std::env::set_var("DSH_HOME", &home);
        // The fake launcher never serves the port, so the boot wait must be
        // bounded for the test to terminate.
        std::env::set_var(WEB_UI_BOOT_TIMEOUT_ENV, "1");

        let work = fixture_home(310);
        std::fs::create_dir_all(&work).unwrap();
        let cli = fake_generic_cli(work.as_path());
        let recorded = work.join("recorded-args");
        let harness = DeepSeekHarness::new(Arc::new(SystemPlatform))
            .with_cli_names(vec![cli.to_string_lossy().into_owned()])
            .with_data_dir(work.clone());

        // The fake launcher never binds the assigned port, so start reports
        // the boot failure — but only after forwarding the right args and
        // recording the pid. The failed instance is then killed and its pid
        // file cleared so no half-started scenario looks running.
        let err = harness.start_scenario("coding", None).await.unwrap_err();
        assert!(err.to_string().contains("did not come up"), "{err}");
        assert_eq!(
            std::fs::read_to_string(&recorded)
                .unwrap()
                .lines()
                .collect::<Vec<_>>(),
            ["--profile", "coding", "--port", "3081"]
        );
        assert!(!work.join("state/run-web.pid").exists());
        assert!(!work.join("state/run-coding.pid").exists());
        // The port assignment is sticky for the next instances() call.
        assert_eq!(
            harness.scenario_port("coding").unwrap(),
            Some(ports::FIRST_SCENARIO_PORT)
        );

        std::fs::remove_dir_all(&home).ok();
        std::fs::remove_dir_all(&work).ok();
        restore_env("DSH_HOME", previous);
        restore_env(WEB_UI_BOOT_TIMEOUT_ENV, previous_boot);
    }

    #[cfg(unix)]
    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn start_scenario_is_idempotent_for_a_running_scenario() {
        let _guard = dsh::tests::ENV_LOCK.lock().unwrap();
        let previous = std::env::var_os("DSH_HOME");
        let home = fixture_home(311);
        write_web_surface_fixture(&home);
        std::env::set_var("DSH_HOME", &home);

        let work = fixture_home(312);
        let cli = fake_generic_cli(work.as_path());
        let recorded = work.join("recorded-args");
        let (addr, server) = probe_server("");
        let harness = DeepSeekHarness::new(Arc::new(SystemPlatform))
            .with_cli_names(vec![cli.to_string_lossy().into_owned()])
            .with_data_dir(work.clone())
            .with_ui_url(format!("http://{addr}"));

        harness.start_scenario("web", None).await.unwrap();
        assert!(
            !recorded.is_file(),
            "an already-running scenario must not be started again"
        );

        stop_probe_server(addr);
        server.join().unwrap();
        std::fs::remove_dir_all(&home).ok();
        std::fs::remove_dir_all(&work).ok();
        restore_env("DSH_HOME", previous);
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn task_scenarios_never_start_and_web_scenarios_never_run_tasks() {
        let _guard = dsh::tests::ENV_LOCK.lock().unwrap();
        let previous = std::env::var_os("DSH_HOME");
        let home = fixture_home(313);
        write_web_surface_fixture(&home);
        write_task_fixture(&home);
        std::env::set_var("DSH_HOME", &home);

        let harness = DeepSeekHarness::new(Arc::new(SystemPlatform))
            .with_cli_names(vec!["definitely-not-a-real-command-zz".to_string()]);
        let err = harness.start_scenario("nightly", None).await.unwrap_err();
        assert!(err.to_string().contains("task scenario"), "{err}");
        let (tx, _rx) = tokio::sync::mpsc::channel(8);
        let err = harness.run_task("web", "hi", tx).await.unwrap_err();
        assert!(err.to_string().contains("not a task scenario"), "{err}");

        std::fs::remove_dir_all(&home).ok();
        restore_env("DSH_HOME", previous);
    }

    #[cfg(unix)]
    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn run_task_streams_output_and_writes_the_log() {
        let _guard = dsh::tests::ENV_LOCK.lock().unwrap();
        let previous = std::env::var_os("DSH_HOME");
        let home = fixture_home(314);
        write_task_fixture(&home);
        std::env::set_var("DSH_HOME", &home);

        let work = fixture_home(315);
        let cli = fake_task_cli(work.as_path(), "echo 'thinking…'\necho 'answer text'", 0);
        let recorded = work.join("recorded-args");
        let harness = DeepSeekHarness::new(Arc::new(SystemPlatform))
            .with_cli_names(vec![cli.to_string_lossy().into_owned()])
            .with_data_dir(work.clone());

        let (tx, mut rx) = tokio::sync::mpsc::channel(16);
        let task =
            tokio::spawn(async move { harness.run_task("nightly", "summarize the doc", tx).await });
        let mut events = Vec::new();
        while let Some(event) = rx.recv().await {
            let done = matches!(event, PluginOpEvent::Finished { .. });
            events.push(event);
            if done {
                break;
            }
        }
        task.await.unwrap().unwrap();
        assert_eq!(
            std::fs::read_to_string(&recorded)
                .unwrap()
                .lines()
                .collect::<Vec<_>>(),
            ["--profile", "nightly", "summarize the doc"]
        );
        assert!(events.iter().any(|e| matches!(
            e,
            PluginOpEvent::Started {
                op: PluginOpKind::Task,
                ..
            }
        )));
        let lines: Vec<&str> = events
            .iter()
            .filter_map(|e| match e {
                PluginOpEvent::Line { text } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(lines, ["thinking…", "answer text"]);
        assert!(matches!(
            events.last(),
            Some(PluginOpEvent::Finished { ok: true, .. })
        ));
        // The answer is persisted under logs/task-nightly-*.log.
        let log_files: Vec<_> = std::fs::read_dir(work.join("logs"))
            .unwrap()
            .flatten()
            .filter(|e| e.file_name().to_string_lossy().starts_with("task-nightly-"))
            .collect();
        assert_eq!(log_files.len(), 1);
        assert_eq!(
            std::fs::read_to_string(log_files[0].path()).unwrap(),
            "thinking…\nanswer text"
        );

        std::fs::remove_dir_all(&home).ok();
        std::fs::remove_dir_all(&work).ok();
        restore_env("DSH_HOME", previous);
    }

    #[cfg(unix)]
    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn run_task_reports_failure_with_exit_code() {
        let _guard = dsh::tests::ENV_LOCK.lock().unwrap();
        let previous = std::env::var_os("DSH_HOME");
        let home = fixture_home(316);
        write_task_fixture(&home);
        std::env::set_var("DSH_HOME", &home);

        let work = fixture_home(317);
        let cli = fake_task_cli(work.as_path(), "echo 'boom' >&2", 7);
        let harness = DeepSeekHarness::new(Arc::new(SystemPlatform))
            .with_cli_names(vec![cli.to_string_lossy().into_owned()])
            .with_data_dir(work.clone());

        let (tx, mut rx) = tokio::sync::mpsc::channel(16);
        let task = tokio::spawn(async move { harness.run_task("nightly", "explode", tx).await });
        let mut events = Vec::new();
        while let Some(event) = rx.recv().await {
            let done = matches!(event, PluginOpEvent::Finished { .. });
            events.push(event);
            if done {
                break;
            }
        }
        assert!(task.await.unwrap().is_err());
        match events.last() {
            Some(PluginOpEvent::Finished {
                ok: false,
                detail: Some(detail),
            }) => {
                assert!(detail.contains("exit"), "{detail}");
            }
            other => panic!("expected a failed finish event, got {other:?}"),
        }

        std::fs::remove_dir_all(&home).ok();
        std::fs::remove_dir_all(&work).ok();
        restore_env("DSH_HOME", previous);
    }

    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn instances_reports_every_scenario_state() {
        let _guard = dsh::tests::ENV_LOCK.lock().unwrap();
        let previous = std::env::var_os("DSH_HOME");
        let home = fixture_home(318);
        // web (with an assigned registry port) is running against a
        // probe server; coding is a web scenario that never started;
        // nightly is a task scenario.
        write_web_surface_fixture(&home);
        write_scenario_fixture(&home, "coding");
        write_task_fixture(&home);
        std::env::set_var("DSH_HOME", &home);

        let work = fixture_home(319);
        std::fs::create_dir_all(work.join("state")).unwrap();
        std::fs::write(
            work.join("state/ports.json"),
            r#"{ "schema": 1, "ports": { "coding": 3081 } }"#,
        )
        .unwrap();
        let (addr, server) = probe_server("");
        let harness = DeepSeekHarness::new(Arc::new(SystemPlatform))
            .with_cli_names(vec!["definitely-not-a-real-command-zz".to_string()])
            .with_data_dir(work.clone())
            .with_ui_url(format!("http://{addr}"));

        let instances = harness.instances().await.unwrap();
        let by_profile: std::collections::HashMap<&str, &RuntimeInstance> = instances
            .iter()
            .map(|instance| (instance.profile.as_str(), instance))
            .collect();
        let web = by_profile["web"];
        assert_eq!(web.status, RuntimeStatusKind::Running);
        assert_eq!(web.port, Some(addr.port()));
        let coding = by_profile["coding"];
        assert_eq!(coding.status, RuntimeStatusKind::Installed);
        assert_eq!(coding.port, Some(3081));
        assert_eq!(coding.url.as_deref(), Some("http://127.0.0.1:3081"));
        let nightly = by_profile["nightly"];
        assert_eq!(nightly.status, RuntimeStatusKind::Installed);
        assert_eq!(nightly.surface, Surface::Task);
        assert_eq!(nightly.port, None);

        stop_probe_server(addr);
        server.join().unwrap();
        std::fs::remove_dir_all(&home).ok();
        std::fs::remove_dir_all(&work).ok();
        restore_env("DSH_HOME", previous);
    }

    #[cfg(unix)]
    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn install_plugin_declares_the_bundle_for_a_plugin_package() {
        let _guard = dsh::tests::ENV_LOCK.lock().unwrap();
        let previous = std::env::var_os("DSH_HOME");
        let home = fixture_home(320);
        // A profile whose manifest declares the dependency but no bundle; the
        // package lands in node_modules as a real `dsh plugin add` would.
        let dir = home.join("profiles/web");
        std::fs::create_dir_all(&dir).unwrap();
        let manifest = serde_json::json!({
            "name": "dsh-profile-web",
            "private": true,
            "dependencies": { "dsh-hook": "^1.0.0" },
            "dsh": { "profile": {} },
        });
        std::fs::write(
            dir.join("package.json"),
            serde_json::to_string_pretty(&manifest).unwrap(),
        )
        .unwrap();
        let dsh_dir = dir.join("node_modules/dsh-hook");
        std::fs::create_dir_all(&dsh_dir).unwrap();
        std::fs::write(
            dsh_dir.join("package.json"),
            r#"{ "name": "dsh-hook", "version": "1.0.0", "dsh": { "bundle": { "patch": "./cordis.patch.yml" } } }"#,
        )
        .unwrap();
        std::env::set_var("DSH_HOME", &home);

        let work = fixture_home(321);
        let cli = fake_plugin_cli(work.as_path(), 0);
        let harness = DeepSeekHarness::new(Arc::new(SystemPlatform))
            .with_cli_names(vec![cli.to_string_lossy().into_owned()])
            .with_data_dir(work.clone());

        assert!(!dsh::bundle_declared("web", "dsh-hook").unwrap());
        harness
            .install_plugin("web", "dsh-hook@^1.0.0")
            .await
            .unwrap();
        assert!(
            dsh::bundle_declared("web", "dsh-hook").unwrap(),
            "a plugin install must declare the bundle so the web UI loads it"
        );

        std::fs::remove_dir_all(&home).ok();
        std::fs::remove_dir_all(&work).ok();
        restore_env("DSH_HOME", previous);
    }

    #[cfg(unix)]
    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn install_plugin_skips_bundle_for_plain_libraries() {
        let _guard = dsh::tests::ENV_LOCK.lock().unwrap();
        let previous = std::env::var_os("DSH_HOME");
        let home = fixture_home(322);
        let dir = home.join("profiles/web");
        std::fs::create_dir_all(&dir).unwrap();
        let manifest = serde_json::json!({
            "name": "dsh-profile-web",
            "private": true,
            "dependencies": { "plain-lib": "2.0.0" },
            "dsh": { "profile": {} },
        });
        std::fs::write(
            dir.join("package.json"),
            serde_json::to_string_pretty(&manifest).unwrap(),
        )
        .unwrap();
        // A plain dependency has no `dsh` section in its manifest.
        std::fs::create_dir_all(dir.join("node_modules/plain-lib")).unwrap();
        std::fs::write(
            dir.join("node_modules/plain-lib/package.json"),
            r#"{ "name": "plain-lib", "version": "2.0.0" }"#,
        )
        .unwrap();
        std::env::set_var("DSH_HOME", &home);

        let work = fixture_home(323);
        let cli = fake_plugin_cli(work.as_path(), 0);
        let harness = DeepSeekHarness::new(Arc::new(SystemPlatform))
            .with_cli_names(vec![cli.to_string_lossy().into_owned()])
            .with_data_dir(work.clone());

        harness.install_plugin("web", "plain-lib").await.unwrap();
        assert!(
            !dsh::bundle_declared("web", "plain-lib").unwrap(),
            "plain libraries must not become bundle declarations"
        );

        std::fs::remove_dir_all(&home).ok();
        std::fs::remove_dir_all(&work).ok();
        restore_env("DSH_HOME", previous);
    }

    // A fake `dsh` script that prints the given shell body, for exercising
    // the streamed plugin operation (lines, exit codes, timeouts).
    #[cfg(unix)]
    fn stream_op_cli(dir: &std::path::Path, body: &str) -> std::path::PathBuf {
        use std::os::unix::fs::PermissionsExt;
        std::fs::create_dir_all(dir).unwrap();
        let script = dir.join("fake-dsh-stream");
        std::fs::write(&script, format!("#!/bin/sh\n{body}\n")).unwrap();
        let mut perms = std::fs::metadata(&script).unwrap().permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&script, perms).unwrap();
        script
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn stream_op_forwards_lines_then_finishes() {
        let dir = std::env::temp_dir().join(format!("deepmate-stream-test-{}", std::process::id()));
        let cli = stream_op_cli(&dir, "echo hello\nprintf 'warn line\\n' >&2\nexit 0");
        let harness = DeepSeekHarness::new(Arc::new(SystemPlatform))
            .with_cli_names(vec![cli.to_string_lossy().into_owned()]);
        let (tx, mut rx) = tokio::sync::mpsc::channel(16);
        let task = tokio::spawn(async move {
            harness
                .stream_plugin_op("web", PluginOpKind::Install, Some("dsh-mnemon"), tx)
                .await
        });
        let mut events = Vec::new();
        while let Some(event) = rx.recv().await {
            let done = matches!(event, PluginOpEvent::Finished { .. });
            events.push(event);
            if done {
                break;
            }
        }
        assert!(task.await.unwrap().is_ok());
        assert_eq!(
            events.first(),
            Some(&PluginOpEvent::Started {
                op: PluginOpKind::Install,
                target: "dsh-mnemon".to_string(),
            })
        );
        let lines: Vec<&str> = events
            .iter()
            .filter_map(|event| match event {
                PluginOpEvent::Line { text } => Some(text.as_str()),
                _ => None,
            })
            .collect();
        assert!(lines.contains(&"hello"), "stdout lines must stream");
        assert!(
            matches!(
                events.last(),
                Some(PluginOpEvent::Finished {
                    ok: true,
                    detail: None
                })
            ),
            "a successful op must end with Finished(ok)"
        );
        std::fs::remove_dir_all(&dir).ok();
    }

    // A stuck child (a `sleep` standing in for a pnpm grandchild holding the
    // pipe) must not trap the operation forever: the timeout kills the
    // process group and reports a bounded failure.
    #[cfg(unix)]
    #[tokio::test(flavor = "multi_thread")]
    #[allow(clippy::await_holding_lock)]
    async fn stream_op_times_out_instead_of_hanging() {
        let _guard = dsh::tests::ENV_LOCK.lock().unwrap();
        let previous = std::env::var_os("DSH_HOME");
        let previous_timeout = std::env::var_os("DEEPMATE_PLUGIN_OP_TIMEOUT_SECS");
        std::env::set_var("DEEPMATE_PLUGIN_OP_TIMEOUT_SECS", "1");
        // The Remove op now consults the profile manifest before spawning the
        // launcher, so "pkg" must be a declared dependency for the remove to
        // reach the stuck-child path it is testing.
        let home = fixture_home(324);
        let dir = home.join("profiles/web");
        std::fs::create_dir_all(&dir).unwrap();
        let manifest = serde_json::json!({
            "name": "dsh-profile-web",
            "private": true,
            "dependencies": { "pkg": "^1.0.0" },
            "dsh": { "profile": {} },
        });
        std::fs::write(
            dir.join("package.json"),
            serde_json::to_string_pretty(&manifest).unwrap(),
        )
        .unwrap();
        std::env::set_var("DSH_HOME", &home);

        let work = fixture_home(325);
        std::fs::create_dir_all(&work).unwrap();
        let cli = stream_op_cli(work.as_path(), "echo hi\nsleep 2\nexit 0");
        let harness = DeepSeekHarness::new(Arc::new(SystemPlatform))
            .with_cli_names(vec![cli.to_string_lossy().into_owned()])
            .with_data_dir(work.clone());
        let (tx, mut rx) = tokio::sync::mpsc::channel(16);
        let err = harness
            .stream_plugin_op("web", PluginOpKind::Remove, Some("pkg"), tx)
            .await
            .unwrap_err();
        assert!(
            err.to_string().contains("did not finish"),
            "the timeout detail must explain the abort: {err}"
        );
        let mut last = None;
        while let Some(event) = rx.recv().await {
            last = Some(event);
        }
        assert!(
            matches!(last, Some(PluginOpEvent::Finished { ok: false, .. })),
            "a timed-out op must still emit Finished(ok=false)"
        );
        std::fs::remove_dir_all(&home).ok();
        std::fs::remove_dir_all(&work).ok();
        restore_env("DSH_HOME", previous);
        restore_env("DEEPMATE_PLUGIN_OP_TIMEOUT_SECS", previous_timeout);
    }

    // Disable/enable reads the profile manifest (via DSH_HOME) and the
    // DeepMate state directory, so these tests share the env lock with the
    // dsh module tests. `dsh::tests` is pub(crate) under cfg(test) and its
    // ENV_LOCK serializes every env-var test in this crate.
    fn write_disabled_fixture(home: &std::path::Path, profile: &str) {
        let dir = home.join("profiles").join(profile);
        std::fs::create_dir_all(&dir).unwrap();
        let manifest = serde_json::json!({
            "name": format!("dsh-profile-{profile}"),
            "private": true,
            "dependencies": { "dsh-mnemon": "^0.2" },
            "dsh": { "profile": {} },
        });
        std::fs::write(
            dir.join("package.json"),
            serde_json::to_string_pretty(&manifest).unwrap(),
        )
        .unwrap();
    }

    // ---- "dangling bundle declarations ----
    //
    // `dsh plugin remove` drops the dependency but keeps the bundle
    // declaration, and a bundle-only entry with nothing installed has no
    // package for the launcher to manage. DeepMate compensates: remove and
    // disable strip the leftover declaration, enable reinstalls the entry.

    // A web-surface scenario under a custom name (base + web-app installed).
    fn write_scenario_fixture(home: &std::path::Path, name: &str) {
        let dir = home.join("profiles").join(name);
        std::fs::create_dir_all(&dir).unwrap();
        let manifest = serde_json::json!({
            "name": format!("dsh-profile-{name}"),
            "private": true,
            "dependencies": {
                "@deepseek-ai/dsh-base": "^0.1",
                "@deepseek-ai/dsh-web-app": "^0.1",
            },
            "dsh": { "profile": { "bundles": ["@deepseek-ai/dsh-base", "@deepseek-ai/dsh-web-app"] } },
        });
        std::fs::write(
            dir.join("package.json"),
            serde_json::to_string_pretty(&manifest).unwrap(),
        )
        .unwrap();
        for bundle in ["@deepseek-ai/dsh-base", "@deepseek-ai/dsh-web-app"] {
            let pkg = dir.join("node_modules").join(bundle);
            std::fs::create_dir_all(&pkg).unwrap();
            std::fs::write(
                pkg.join("package.json"),
                format!(r#"{{ "name": "{bundle}", "version": "0.1.0" }}"#),
            )
            .unwrap();
        }
    }

    // A task-surface scenario (base + headless installed).
    fn write_task_fixture(home: &std::path::Path) {
        let dir = home.join("profiles/nightly");
        std::fs::create_dir_all(&dir).unwrap();
        let manifest = serde_json::json!({
            "name": "dsh-profile-nightly",
            "private": true,
            "dependencies": {
                "@deepseek-ai/dsh-base": "^0.1",
                "@deepseek-ai/dsh-headless": "^0.1",
            },
            "dsh": { "profile": { "bundles": ["@deepseek-ai/dsh-base", "@deepseek-ai/dsh-headless"] } },
        });
        std::fs::write(
            dir.join("package.json"),
            serde_json::to_string_pretty(&manifest).unwrap(),
        )
        .unwrap();
        for bundle in ["@deepseek-ai/dsh-base", "@deepseek-ai/dsh-headless"] {
            let pkg = dir.join("node_modules").join(bundle);
            std::fs::create_dir_all(&pkg).unwrap();
            std::fs::write(
                pkg.join("package.json"),
                format!(r#"{{ "name": "{bundle}", "version": "0.1.0" }}"#),
            )
            .unwrap();
        }
    }

    // A web-surface scenario: the base bundle plus the web-app surface are
    // declared and "installed" (a node_modules dir exists), so start_scenario
    // resolves Surface::Web and boots it.
    fn write_web_surface_fixture(home: &std::path::Path) {
        let dir = home.join("profiles/web");
        std::fs::create_dir_all(&dir).unwrap();
        let manifest = serde_json::json!({
            "name": "dsh-profile-web",
            "private": true,
            "dependencies": {
                "@deepseek-ai/dsh-base": "^0.1",
                "@deepseek-ai/dsh-web-app": "^0.1",
            },
            "dsh": { "profile": { "bundles": ["@deepseek-ai/dsh-base", "@deepseek-ai/dsh-web-app"] } },
        });
        std::fs::write(
            dir.join("package.json"),
            serde_json::to_string_pretty(&manifest).unwrap(),
        )
        .unwrap();
        for bundle in ["@deepseek-ai/dsh-base", "@deepseek-ai/dsh-web-app"] {
            let pkg = dir.join("node_modules").join(bundle);
            std::fs::create_dir_all(&pkg).unwrap();
            std::fs::write(
                pkg.join("package.json"),
                format!(r#"{{ "name": "{bundle}", "version": "0.1.0" }}"#),
            )
            .unwrap();
        }
    }

    fn write_bundle_and_dep_fixture(home: &std::path::Path, profile: &str) {
        let dir = home.join("profiles").join(profile);
        std::fs::create_dir_all(&dir).unwrap();
        let manifest = serde_json::json!({
            "name": format!("dsh-profile-{profile}"),
            "private": true,
            "dependencies": { "dsh-mnemon": "^0.2" },
            "dsh": { "profile": { "bundles": ["@deepseek-ai/dsh-base", "dsh-mnemon"] } },
        });
        std::fs::write(
            dir.join("package.json"),
            serde_json::to_string_pretty(&manifest).unwrap(),
        )
        .unwrap();
    }

    fn write_bundle_only_fixture(home: &std::path::Path, profile: &str) {
        let dir = home.join("profiles").join(profile);
        std::fs::create_dir_all(&dir).unwrap();
        let manifest = serde_json::json!({
            "name": format!("dsh-profile-{profile}"),
            "private": true,
            "dependencies": {},
            "dsh": { "profile": { "bundles": ["@deepseek-ai/dsh-base", "dsh-mnemon"] } },
        });
        std::fs::write(
            dir.join("package.json"),
            serde_json::to_string_pretty(&manifest).unwrap(),
        )
        .unwrap();
    }

    fn manifest_bundles(home: &std::path::Path, profile: &str) -> Vec<String> {
        let text =
            std::fs::read_to_string(home.join("profiles").join(profile).join("package.json"))
                .unwrap();
        let value: serde_json::Value = serde_json::from_str(&text).unwrap();
        value["dsh"]["profile"]["bundles"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|v| v.as_str().map(str::to_string))
            .collect()
    }

    #[cfg(unix)]
    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn remove_plugin_strips_leftover_bundle_declaration() {
        let _guard = dsh::tests::ENV_LOCK.lock().unwrap();
        let previous = std::env::var_os("DSH_HOME");
        let home = fixture_home(326);
        write_bundle_and_dep_fixture(&home, "web");
        std::env::set_var("DSH_HOME", &home);

        let work = fixture_home(327);
        std::fs::create_dir_all(&work).unwrap();
        let cli = fake_plugin_cli(work.as_path(), 0);
        let recorded = work.join("recorded-args");
        let harness = DeepSeekHarness::new(Arc::new(SystemPlatform))
            .with_cli_names(vec![cli.to_string_lossy().into_owned()])
            .with_data_dir(work.clone());

        harness.remove_plugin("web", "dsh-mnemon").await.unwrap();
        assert_eq!(
            std::fs::read_to_string(&recorded)
                .unwrap()
                .lines()
                .collect::<Vec<_>>(),
            ["plugin", "--profile", "web", "remove", "dsh-mnemon"]
        );
        // The declaration is gone even though dsh left it behind; the other
        // bundle survives.
        assert!(!manifest_bundles(&home, "web").contains(&"dsh-mnemon".to_string()));
        assert!(manifest_bundles(&home, "web").contains(&"@deepseek-ai/dsh-base".to_string()));

        std::fs::remove_dir_all(&home).ok();
        std::fs::remove_dir_all(&work).ok();
        restore_env("DSH_HOME", previous);
    }

    #[cfg(unix)]
    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn remove_plugin_dangling_bundle_strips_without_dsh() {
        let _guard = dsh::tests::ENV_LOCK.lock().unwrap();
        let previous = std::env::var_os("DSH_HOME");
        let home = fixture_home(328);
        write_bundle_only_fixture(&home, "web");
        std::env::set_var("DSH_HOME", &home);

        let work = fixture_home(329);
        std::fs::create_dir_all(&work).unwrap();
        let cli = fake_plugin_cli(work.as_path(), 0);
        let recorded = work.join("recorded-args");
        let harness = DeepSeekHarness::new(Arc::new(SystemPlatform))
            .with_cli_names(vec![cli.to_string_lossy().into_owned()])
            .with_data_dir(work.clone());

        harness.remove_plugin("web", "dsh-mnemon").await.unwrap();
        // The launcher is never invoked: stripping the stale declaration is
        // the whole removal.
        assert!(!recorded.exists());
        assert!(!manifest_bundles(&home, "web").contains(&"dsh-mnemon".to_string()));

        std::fs::remove_dir_all(&home).ok();
        std::fs::remove_dir_all(&work).ok();
        restore_env("DSH_HOME", previous);
    }

    #[cfg(unix)]
    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn remove_plugin_propagates_dsh_failure_and_keeps_bundle() {
        let _guard = dsh::tests::ENV_LOCK.lock().unwrap();
        let previous = std::env::var_os("DSH_HOME");
        let home = fixture_home(330);
        write_bundle_and_dep_fixture(&home, "web");
        std::env::set_var("DSH_HOME", &home);

        let work = fixture_home(331);
        std::fs::create_dir_all(&work).unwrap();
        let cli = fake_plugin_cli(work.as_path(), 7);
        let harness = DeepSeekHarness::new(Arc::new(SystemPlatform))
            .with_cli_names(vec![cli.to_string_lossy().into_owned()])
            .with_data_dir(work.clone());

        let err = harness
            .remove_plugin("web", "dsh-mnemon")
            .await
            .unwrap_err();
        assert!(err.to_string().contains("Some(7)"));
        // A failed remove leaves the profile untouched: the declaration is
        // still there so the harness keeps loading a plugin that exists.
        assert!(manifest_bundles(&home, "web").contains(&"dsh-mnemon".to_string()));

        std::fs::remove_dir_all(&home).ok();
        std::fs::remove_dir_all(&work).ok();
        restore_env("DSH_HOME", previous);
    }

    #[cfg(unix)]
    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn disable_plugin_strips_leftover_bundle_declaration() {
        let _guard = dsh::tests::ENV_LOCK.lock().unwrap();
        let previous = std::env::var_os("DSH_HOME");
        let home = fixture_home(332);
        write_bundle_and_dep_fixture(&home, "web");
        std::env::set_var("DSH_HOME", &home);

        let work = fixture_home(333);
        std::fs::create_dir_all(&work).unwrap();
        let cli = fake_plugin_cli(work.as_path(), 0);
        let recorded = work.join("recorded-args");
        let harness = DeepSeekHarness::new(Arc::new(SystemPlatform))
            .with_cli_names(vec![cli.to_string_lossy().into_owned()])
            .with_data_dir(work.clone());

        harness.disable_plugin("web", "dsh-mnemon").await.unwrap();
        assert_eq!(
            std::fs::read_to_string(&recorded)
                .unwrap()
                .lines()
                .collect::<Vec<_>>(),
            ["plugin", "--profile", "web", "remove", "dsh-mnemon"]
        );
        // The recorded spec keeps the declared range for a re-enable.
        let text = std::fs::read_to_string(work.join("state/disabled-plugins.json")).unwrap();
        let file: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(file["plugins"][0]["spec"], "dsh-mnemon@^0.2");
        assert!(!manifest_bundles(&home, "web").contains(&"dsh-mnemon".to_string()));

        std::fs::remove_dir_all(&home).ok();
        std::fs::remove_dir_all(&work).ok();
        restore_env("DSH_HOME", previous);
    }

    #[cfg(unix)]
    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn disable_plugin_dangling_bundle_is_cleanup() {
        let _guard = dsh::tests::ENV_LOCK.lock().unwrap();
        let previous = std::env::var_os("DSH_HOME");
        let home = fixture_home(334);
        write_bundle_only_fixture(&home, "web");
        std::env::set_var("DSH_HOME", &home);

        let work = fixture_home(335);
        std::fs::create_dir_all(&work).unwrap();
        let cli = fake_plugin_cli(work.as_path(), 0);
        let recorded = work.join("recorded-args");
        let harness = DeepSeekHarness::new(Arc::new(SystemPlatform))
            .with_cli_names(vec![cli.to_string_lossy().into_owned()])
            .with_data_dir(work.clone());

        // There is no package to uninstall and no spec to remember; the
        // stale declaration is simply dropped.
        harness.disable_plugin("web", "dsh-mnemon").await.unwrap();
        assert!(!recorded.exists());
        assert!(harness.disabled_plugins().unwrap().is_empty());
        assert!(!manifest_bundles(&home, "web").contains(&"dsh-mnemon".to_string()));

        // A genuinely unknown plugin still refuses.
        let err = harness.disable_plugin("web", "nope").await.unwrap_err();
        assert!(err.to_string().contains("not installed"));

        std::fs::remove_dir_all(&home).ok();
        std::fs::remove_dir_all(&work).ok();
        restore_env("DSH_HOME", previous);
    }

    #[cfg(unix)]
    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn enable_plugin_reinstalls_dangling_bundle() {
        let _guard = dsh::tests::ENV_LOCK.lock().unwrap();
        let previous = std::env::var_os("DSH_HOME");
        let home = fixture_home(336);
        write_bundle_only_fixture(&home, "web");
        std::env::set_var("DSH_HOME", &home);

        let work = fixture_home(337);
        std::fs::create_dir_all(&work).unwrap();
        let cli = fake_plugin_cli(work.as_path(), 0);
        let recorded = work.join("recorded-args");
        let harness = DeepSeekHarness::new(Arc::new(SystemPlatform))
            .with_cli_names(vec![cli.to_string_lossy().into_owned()])
            .with_data_dir(work.clone());

        // A leftover bundle with no recorded disable is enabled by
        // reinstalling the declared entry at the latest version.
        harness.enable_plugin("web", "dsh-mnemon").await.unwrap();
        assert_eq!(
            std::fs::read_to_string(&recorded)
                .unwrap()
                .lines()
                .collect::<Vec<_>>(),
            ["plugin", "--profile", "web", "add", "dsh-mnemon"]
        );

        std::fs::remove_dir_all(&home).ok();
        std::fs::remove_dir_all(&work).ok();
        restore_env("DSH_HOME", previous);
    }

    #[cfg(unix)]
    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn stream_remove_dangling_bundle_skips_launcher() {
        let _guard = dsh::tests::ENV_LOCK.lock().unwrap();
        let previous = std::env::var_os("DSH_HOME");
        let home = fixture_home(338);
        write_bundle_only_fixture(&home, "web");
        std::env::set_var("DSH_HOME", &home);

        let work = fixture_home(339);
        std::fs::create_dir_all(&work).unwrap();
        let marker = work.join("spawned");
        // The marker is only touched when the fake CLI is actually invoked
        // as a plugin command, not by the `--version` probe.
        let cli = stream_op_cli(
            work.as_path(),
            &format!("[ \"$1\" = \"plugin\" ] && touch {}", marker.display()),
        );
        let harness = DeepSeekHarness::new(Arc::new(SystemPlatform))
            .with_cli_names(vec![cli.to_string_lossy().into_owned()])
            .with_data_dir(work.clone());

        let (tx, mut rx) = tokio::sync::mpsc::channel(16);
        harness
            .stream_plugin_op("web", PluginOpKind::Remove, Some("dsh-mnemon"), tx)
            .await
            .unwrap();
        // The launcher was never spawned (no marker); the stale declaration
        // was stripped and the op still finished cleanly.
        assert!(!marker.exists());
        assert!(!manifest_bundles(&home, "web").contains(&"dsh-mnemon".to_string()));
        let mut last = None;
        while let Some(event) = rx.recv().await {
            last = Some(event);
        }
        assert!(matches!(
            last,
            Some(PluginOpEvent::Finished { ok: true, .. })
        ));

        std::fs::remove_dir_all(&home).ok();
        std::fs::remove_dir_all(&work).ok();
        restore_env("DSH_HOME", previous);
    }

    #[cfg(unix)]
    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn doctor_reports_invalid_ui_url() {
        let _guard = dsh::tests::ENV_LOCK.lock().unwrap();
        let previous = std::env::var_os("DSH_HOME");
        let home = fixture_home(308);
        std::env::set_var("DSH_HOME", &home);

        // An override that is not a usable http(s) address must be flagged,
        // not reported as "configured".
        let harness = DeepSeekHarness::new(Arc::new(SystemPlatform))
            .with_cli_names(vec!["definitely-not-a-real-deepmate-command".to_string()])
            .with_ui_url("not-a-url");
        let report = harness.doctor().await.unwrap();
        let check = report
            .checks
            .iter()
            .find(|check| check.id == "ui.url")
            .unwrap();
        assert_eq!(check.status, CheckStatus::Warn);
        assert!(check.details.as_deref().unwrap().contains("not-a-url"));
        // The reachability check must skip (not warn+fix) while the address
        // itself is broken: starting the server could never satisfy an
        // unparseable URL target.
        let reachable = report
            .checks
            .iter()
            .find(|check| check.id == "ui.reachable")
            .unwrap();
        assert_eq!(reachable.status, CheckStatus::Skip);
        std::fs::remove_dir_all(&home).ok();
        restore_env("DSH_HOME", previous);
    }

    #[cfg(unix)]
    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn doctor_skips_web_ui_check_when_cli_missing() {
        let _guard = dsh::tests::ENV_LOCK.lock().unwrap();
        let previous = std::env::var_os("DSH_HOME");
        let home = fixture_home(320);
        std::env::set_var("DSH_HOME", &home);

        // Without the CLI the web UI cannot be started, so the reachability
        // check skips instead of warning about something unfixable.
        let harness = DeepSeekHarness::new(Arc::new(SystemPlatform))
            .with_cli_names(vec!["definitely-not-a-real-deepmate-command".to_string()])
            .with_ui_url("http://127.0.0.1:1");
        let report = harness.doctor().await.unwrap();
        let installed = report
            .checks
            .iter()
            .find(|check| check.id == "runtime.installed")
            .unwrap();
        assert_eq!(installed.status, CheckStatus::Fail);
        let reachable = report
            .checks
            .iter()
            .find(|check| check.id == "ui.reachable")
            .unwrap();
        assert_eq!(reachable.status, CheckStatus::Skip);
        std::fs::remove_dir_all(&home).ok();
        restore_env("DSH_HOME", previous);
    }

    #[cfg(unix)]
    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn doctor_flags_dangling_bundle_declarations() {
        let _guard = dsh::tests::ENV_LOCK.lock().unwrap();
        let previous = std::env::var_os("DSH_HOME");
        let home = fixture_home(340);
        write_bundle_only_fixture(&home, "web");
        std::env::set_var("DSH_HOME", &home);

        let harness = DeepSeekHarness::new(Arc::new(SystemPlatform))
            .with_cli_names(vec!["definitely-not-a-real-deepmate-command".to_string()]);
        let report = harness.doctor().await.unwrap();
        let check = report
            .checks
            .iter()
            .find(|check| check.id == "plugins.bundles")
            .unwrap();
        assert_eq!(check.status, CheckStatus::Warn);
        assert!(check.details.as_deref().unwrap().contains("dsh-mnemon"));
        assert!(check.suggested_action.is_some());

        std::fs::remove_dir_all(&home).ok();
        restore_env("DSH_HOME", previous);
    }

    #[cfg(unix)]
    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn doctor_passes_when_bundles_are_installed() {
        let _guard = dsh::tests::ENV_LOCK.lock().unwrap();
        let previous = std::env::var_os("DSH_HOME");
        let home = fixture_home(341);
        write_bundle_only_fixture(&home, "web");
        // Installed into the shared launcher fallback, like the official
        // core bundles: declared, present, and therefore healthy.
        for name in ["dsh-mnemon", "@deepseek-ai/dsh-base"] {
            let bundle_dir = home.join("profiles/node_modules").join(name);
            std::fs::create_dir_all(&bundle_dir).unwrap();
            std::fs::write(
                bundle_dir.join("package.json"),
                r#"{"name": "placeholder", "version": "0.2.14"}"#,
            )
            .unwrap();
        }
        std::env::set_var("DSH_HOME", &home);

        let harness = DeepSeekHarness::new(Arc::new(SystemPlatform))
            .with_cli_names(vec!["definitely-not-a-real-deepmate-command".to_string()]);
        let report = harness.doctor().await.unwrap();
        let check = report
            .checks
            .iter()
            .find(|check| check.id == "plugins.bundles")
            .unwrap();
        assert_eq!(check.status, CheckStatus::Pass);

        std::fs::remove_dir_all(&home).ok();
        restore_env("DSH_HOME", previous);
    }

    fn fixture_home(seq: u64) -> std::path::PathBuf {
        std::env::temp_dir().join(format!(
            "deepmate-disable-test-{}-{seq}",
            std::process::id()
        ))
    }

    fn restore_env(key: &str, previous: Option<std::ffi::OsString>) {
        match previous {
            Some(value) => std::env::set_var(key, value),
            None => std::env::remove_var(key),
        }
    }

    // The shared env lock serializes every DSH_HOME mutation in this crate's
    // tests. Holding it across `.await` is deliberate: the harness methods
    // read DSH_HOME at call time, so the whole async body must stay under
    // the lock. Test-only, so the lint is scoped out.
    #[cfg(unix)]
    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn disable_removes_and_records_then_enable_restores() {
        let _guard = dsh::tests::ENV_LOCK.lock().unwrap();
        let previous = std::env::var_os("DSH_HOME");
        let home = fixture_home(342);
        write_disabled_fixture(&home, "web");
        std::env::set_var("DSH_HOME", &home);

        let work = fixture_home(343);
        std::fs::create_dir_all(&work).unwrap();
        let cli = fake_plugin_cli(work.as_path(), 0);
        let recorded = work.join("recorded-args");
        let harness = DeepSeekHarness::new(Arc::new(SystemPlatform))
            .with_cli_names(vec![cli.to_string_lossy().into_owned()])
            .with_data_dir(work.clone());

        // Disabling uninstalls through the harness CLI and records the
        // declared range for a later re-enable.
        harness.disable_plugin("web", "dsh-mnemon").await.unwrap();
        assert_eq!(
            std::fs::read_to_string(&recorded)
                .unwrap()
                .lines()
                .collect::<Vec<_>>(),
            ["plugin", "--profile", "web", "remove", "dsh-mnemon"]
        );
        let text = std::fs::read_to_string(work.join("state/disabled-plugins.json")).unwrap();
        let file: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(file["plugins"][0]["profile"], "web");
        assert_eq!(file["plugins"][0]["id"], "dsh-mnemon");
        assert_eq!(file["plugins"][0]["spec"], "dsh-mnemon@^0.2");

        // Enabling reinstalls the recorded spec and clears the record.
        harness.enable_plugin("web", "dsh-mnemon").await.unwrap();
        assert_eq!(
            std::fs::read_to_string(&recorded)
                .unwrap()
                .lines()
                .collect::<Vec<_>>(),
            ["plugin", "--profile", "web", "add", "dsh-mnemon@^0.2"]
        );
        let text = std::fs::read_to_string(work.join("state/disabled-plugins.json")).unwrap();
        let file: serde_json::Value = serde_json::from_str(&text).unwrap();
        assert_eq!(file["plugins"].as_array().unwrap().len(), 0);

        std::fs::remove_dir_all(&home).ok();
        std::fs::remove_dir_all(&work).ok();
        restore_env("DSH_HOME", previous);
    }

    #[cfg(unix)]
    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn reinstall_clears_a_stale_disable_record() {
        let _guard = dsh::tests::ENV_LOCK.lock().unwrap();
        let previous = std::env::var_os("DSH_HOME");
        let home = fixture_home(344);
        write_disabled_fixture(&home, "web");
        std::env::set_var("DSH_HOME", &home);

        let work = fixture_home(345);
        std::fs::create_dir_all(&work).unwrap();
        let cli = fake_plugin_cli(work.as_path(), 0);
        let harness = DeepSeekHarness::new(Arc::new(SystemPlatform))
            .with_cli_names(vec![cli.to_string_lossy().into_owned()])
            .with_data_dir(work.clone());

        harness.disable_plugin("web", "dsh-mnemon").await.unwrap();
        assert_eq!(
            harness
                .disabled_plugins()
                .unwrap()
                .iter()
                .map(|plugin| plugin.spec.clone())
                .collect::<Vec<_>>(),
            ["dsh-mnemon@^0.2"]
        );

        // Installing the same package again (e.g. from the market) is a
        // re-enable: the record must not leave a ghost row in the listing.
        harness
            .install_plugin("web", "dsh-mnemon@^0.2")
            .await
            .unwrap();
        assert!(harness.disabled_plugins().unwrap().is_empty());

        std::fs::remove_dir_all(&home).ok();
        std::fs::remove_dir_all(&work).ok();
        restore_env("DSH_HOME", previous);
    }

    #[cfg(unix)]
    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn enable_refuses_when_nothing_is_disabled() {
        let _guard = dsh::tests::ENV_LOCK.lock().unwrap();
        let previous = std::env::var_os("DSH_HOME");
        let home = fixture_home(346);
        write_disabled_fixture(&home, "web");
        std::env::set_var("DSH_HOME", &home);
        let cli = fake_plugin_cli(home.as_path(), 0);
        let harness = DeepSeekHarness::new(Arc::new(SystemPlatform))
            .with_cli_names(vec![cli.to_string_lossy().into_owned()])
            .with_data_dir(home.clone());
        let err = harness
            .enable_plugin("web", "absent-pkg")
            .await
            .unwrap_err();
        assert!(err.to_string().contains("not disabled"));
        std::fs::remove_dir_all(&home).ok();
        restore_env("DSH_HOME", previous);
    }

    // Plugins listed from a profile carry their curated trust tier when the
    // curated market (here: a fresh cache file under the data dir) has a
    // record for them, and None otherwise.
    #[cfg(unix)]
    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn plugins_enrich_trust_from_the_curated_list() {
        let _guard = dsh::tests::ENV_LOCK.lock().unwrap();
        let previous = std::env::var_os("DSH_HOME");
        let home = fixture_home(347);
        let dir = home.join("profiles/web");
        std::fs::create_dir_all(&dir).unwrap();
        let manifest = serde_json::json!({
            "name": "dsh-profile-web",
            "private": true,
            "dependencies": {
                "@deepseek-ai/dsh-base": "^0.3",
                "dsh-mnemon": "^0.2",
                "plain-lib": "1.0.0"
            },
            "dsh": { "profile": {} },
        });
        std::fs::write(
            dir.join("package.json"),
            serde_json::to_string_pretty(&manifest).unwrap(),
        )
        .unwrap();
        std::env::set_var("DSH_HOME", &home);

        let cache = home.join("cache/curated.json");
        std::fs::create_dir_all(cache.parent().unwrap()).unwrap();
        std::fs::write(
            &cache,
            serde_json::json!({
                "updated": chrono::Utc::now().to_rfc3339(),
                "entries": [
                    {
                        "id": "@deepseek-ai/dsh-base",
                        "name": "@deepseek-ai/dsh-base",
                        "description": "official base",
                        "version": "^0.3",
                        "source": "curated",
                        "trust": "official",
                        "category": "official"
                    },
                    {
                        "id": "dsh-mnemon",
                        "name": "dsh-mnemon",
                        "description": "memory",
                        "version": "^0.2",
                        "source": "curated",
                        "trust": "vetted"
                    }
                ]
            })
            .to_string(),
        )
        .unwrap();

        let cli = fake_plugin_cli(home.as_path(), 0);
        let harness = DeepSeekHarness::new(Arc::new(SystemPlatform))
            .with_cli_names(vec![cli.to_string_lossy().into_owned()])
            .with_data_dir(home.clone());
        let plugins = harness.plugins().await.unwrap();
        let by_id: std::collections::HashMap<&str, deepmate_core::model::MarketTrust> = plugins
            .iter()
            .map(|plugin| {
                (
                    plugin.id.as_str(),
                    plugin.trust.unwrap_or(MarketTrust::Community),
                )
            })
            .collect();
        assert_eq!(by_id["@deepseek-ai/dsh-base"], MarketTrust::Official);
        assert_eq!(by_id["dsh-mnemon"], MarketTrust::Vetted);
        // A plugin with no curated record stays unmarked (mapped to the
        // "no signal" tier here only for the comparison).
        assert_eq!(by_id["plain-lib"], MarketTrust::Community);

        std::fs::remove_dir_all(&home).ok();
        restore_env("DSH_HOME", previous);
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
        let harness = DeepSeekHarness::new(Arc::new(SystemPlatform))
            .with_cli_names(vec![script.to_string_lossy().into_owned()]);
        let detection = harness.detect().await.unwrap();
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
        let harness = DeepSeekHarness::new(Arc::new(SystemPlatform))
            .with_cli_names(vec![script.to_string_lossy().into_owned()]);
        let detection = harness.detect().await.unwrap();
        assert!(detection.found);
        assert_eq!(
            detection.harness.unwrap().version.as_deref(),
            Some("0.1.0-rc.6")
        );
    }

    // ---- snapshot capture / apply ----

    fn snapshot_test_home(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "deepmate-snapshot-harness-test-{tag}-{}",
            std::process::id()
        ))
    }

    #[cfg(unix)]
    fn write_test_profile(home: &Path) {
        let dir = home.join("profiles").join("web");
        std::fs::create_dir_all(&dir).unwrap();
        let manifest = serde_json::json!({
            "name": "dsh-profile-web",
            "private": true,
            "dependencies": { "dsh-mnemon": "1.0.0" },
            "dsh": { "profile": { "bundles": ["coding"] } },
        });
        std::fs::write(
            dir.join("package.json"),
            serde_json::to_string_pretty(&manifest).unwrap(),
        )
        .unwrap();
    }

    #[cfg(unix)]
    fn write_test_settings(home: &Path) {
        let settings = "\
llm-pi-ai:
  providers:
    my-provider:
      displayName: My Provider
      baseURL: https://api.example.com/v1
      apiKeyEnv: MY_PROVIDER_KEY
      models:
        - id: model-a
          name: Model A
          contextWindow: 128000
";
        std::fs::write(home.join("settings.yaml"), settings).unwrap();
    }

    // The env lock is deliberately held across the await: #[tokio::test] runs
    // on a single-threaded runtime, so holding the std Mutex cannot deadlock.
    #[cfg(unix)]
    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn capture_snapshot_collects_inventory() {
        let _guard = DSH_ENV_LOCK.lock().unwrap();
        let previous = std::env::var_os("DSH_HOME");
        let home = snapshot_test_home("capture");
        let _ = std::fs::remove_dir_all(&home);
        write_test_profile(&home);
        write_test_settings(&home);
        std::env::set_var("DSH_HOME", &home);

        let harness = DeepSeekHarness::new(Arc::new(SystemPlatform))
            .with_cli_names(vec!["definitely-not-a-real-deepmate-command".to_string()]);
        let snapshot = harness.capture_snapshot().await.unwrap();

        match previous {
            Some(value) => std::env::set_var("DSH_HOME", value),
            None => std::env::remove_var("DSH_HOME"),
        }

        assert_eq!(snapshot.format, SNAPSHOT_FORMAT);
        // The always-composed deepseek route plus the seeded pi-ai provider.
        assert_eq!(snapshot.profiles.len(), 1);
        assert_eq!(snapshot.providers.len(), 2);
        // Two built-in deepseek catalog models plus the seeded pi-ai model.
        assert_eq!(snapshot.models.len(), 3);
        // One base bundle plus one seeded dependency on the profile.
        assert_eq!(snapshot.plugins.len(), 2);
        let _ = std::fs::remove_dir_all(&home);
    }

    #[cfg(unix)]
    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn apply_snapshot_upserts_inventory() {
        let _guard = DSH_ENV_LOCK.lock().unwrap();
        let previous = std::env::var_os("DSH_HOME");
        let home = snapshot_test_home("apply");
        let _ = std::fs::remove_dir_all(&home);
        std::env::set_var("DSH_HOME", &home);

        let snapshot = Snapshot {
            format: SNAPSHOT_FORMAT.to_string(),
            created: chrono::Utc::now().to_rfc3339(),
            profiles: vec![Profile {
                id: "coding".to_string(),
                name: "coding".to_string(),
                description: None,
            }],
            providers: vec![Provider {
                id: "imported".to_string(),
                name: "Imported".to_string(),
                kind: "pi-ai".to_string(),
                api: None,
                base_url: Some("https://api.example.com/v1".to_string()),
                api_key_env: Some("IMPORTED_KEY".to_string()),
                compat: None,
            }],
            models: vec![Model {
                id: "model-a".to_string(),
                name: "Model A".to_string(),
                provider: Some("imported".to_string()),
                context_window: None,
                max_tokens: None,
                input: None,
                reasoning_efforts: None,
                compat: None,
            }],
            plugins: vec![],
        };

        let harness = DeepSeekHarness::new(Arc::new(SystemPlatform))
            .with_cli_names(vec!["definitely-not-a-real-deepmate-command".to_string()]);
        let report = harness.apply_snapshot(&snapshot).await.unwrap();

        match previous {
            Some(value) => std::env::set_var("DSH_HOME", value),
            None => std::env::remove_var("DSH_HOME"),
        }

        assert_eq!(report.profiles, 1);
        assert_eq!(report.providers, 1);
        assert_eq!(report.models, 1);
        assert_eq!(report.plugins, 0);
        let _ = std::fs::remove_dir_all(&home);
    }

    // ---- runtime bundle load probe ----
    //
    // The probe asks the live harness web UI for each profile-local bundle's
    // `/plugins/<id>/client.js` URL, reproducing the browser-side "bundle
    // script failed to load" failure at the HTTP layer.

    // A minimal HTTP server answering every request with 200 — or 404 for
    // paths containing the `broken` segment. A `/quit` request ends the
    // accept loop so the test can join the thread.
    #[cfg(unix)]
    fn probe_server(broken: &str) -> (std::net::SocketAddr, std::thread::JoinHandle<()>) {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let broken = broken.to_string();
        let handle = std::thread::spawn(move || {
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { break };
                let mut buf = [0u8; 4096];
                let _ = stream.read(&mut buf);
                let request = String::from_utf8_lossy(&buf[..]);
                let path = request.split_whitespace().nth(1).unwrap_or("/");
                if path == "/quit" {
                    break;
                }
                let broken_hit = path.contains(&broken);
                let status = if broken_hit {
                    "404 Not Found"
                } else {
                    "200 OK"
                };
                let body = if broken_hit {
                    "not found"
                } else {
                    "console.log('ok');"
                };
                let _ = write!(
                    stream,
                    "HTTP/1.1 {status}\r\nContent-Type: text/javascript; charset=utf-8\r\n\
                     Content-Length: {}\r\nConnection: close\r\n\r\n{body}",
                    body.len()
                );
            }
        });
        (addr, handle)
    }

    // Signal a probe server to shut down, letting the test join it.
    #[cfg(unix)]
    fn stop_probe_server(addr: std::net::SocketAddr) {
        use std::io::Write;
        if let Ok(mut stream) = std::net::TcpStream::connect(addr) {
            let _ = write!(stream, "GET /quit HTTP/1.1\r\n\r\n");
        }
    }

    // A probe server whose acceptor wakes after `delay`: the port is bound
    // from the start (so the address is known and connects queue up), but
    // nothing answers until then — exactly the window `start`'s boot wait
    // must bridge.
    #[cfg(unix)]
    fn delayed_probe_server(
        delay: std::time::Duration,
    ) -> (std::net::SocketAddr, std::thread::JoinHandle<()>) {
        use std::io::{Read, Write};
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let addr = listener.local_addr().unwrap();
        let handle = std::thread::spawn(move || {
            std::thread::sleep(delay);
            for stream in listener.incoming() {
                let Ok(mut stream) = stream else { break };
                let mut buf = [0u8; 4096];
                let _ = stream.read(&mut buf);
                let request = String::from_utf8_lossy(&buf[..]);
                let path = request.split_whitespace().nth(1).unwrap_or("/");
                if path == "/quit" {
                    break;
                }
                let _ = write!(
                    stream,
                    "HTTP/1.1 200 OK\r\nContent-Type: text/javascript; charset=utf-8\r\n\
                     Content-Length: {}\r\nConnection: close\r\n\r\nconsole.log('ok');",
                    "console.log('ok');".len()
                );
            }
        });
        (addr, handle)
    }

    fn write_local_bundle_fixture(home: &std::path::Path) {
        let dir = home.join("profiles/web");
        std::fs::create_dir_all(&dir).unwrap();
        let manifest = serde_json::json!({
            "name": "dsh-profile-web",
            "private": true,
            "dependencies": {},
            "dsh": { "profile": { "bundles": ["dsh-mnemon", "@deepseek-ai/dsh-base"] } },
        });
        std::fs::write(
            dir.join("package.json"),
            serde_json::to_string_pretty(&manifest).unwrap(),
        )
        .unwrap();
        // dsh-mnemon is installed profile-local (probeable, declares a web
        // client); the base bundle lives in the shared launcher fallback
        // (not probeable).
        let local = dir.join("node_modules/dsh-mnemon");
        std::fs::create_dir_all(&local).unwrap();
        std::fs::write(
            local.join("package.json"),
            r#"{"name": "dsh-mnemon", "version": "0.2.14", "dsh": {"client": {"platform": "web", "inject": ["@deepseek-ai/dsh-client-runtime"]}}}"#,
        )
        .unwrap();
        let shared = home.join("profiles/node_modules/@deepseek-ai/dsh-base");
        std::fs::create_dir_all(&shared).unwrap();
        std::fs::write(
            shared.join("package.json"),
            r#"{"name": "@deepseek-ai/dsh-base", "version": "0.3.1"}"#,
        )
        .unwrap();
    }

    #[cfg(unix)]
    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn doctor_bundle_probe_passes_when_all_bundles_are_servable() {
        let _guard = dsh::tests::ENV_LOCK.lock().unwrap();
        let previous = std::env::var_os("DSH_HOME");
        let home = fixture_home(348);
        write_local_bundle_fixture(&home);
        std::env::set_var("DSH_HOME", &home);

        let (addr, server) = probe_server("never-404s-this-segment");
        let harness = DeepSeekHarness::new(Arc::new(SystemPlatform))
            .with_cli_names(vec!["definitely-not-a-real-deepmate-command".to_string()])
            .with_ui_url(format!("http://{addr}"));
        let report = harness.doctor().await.unwrap();
        let check = report
            .checks
            .iter()
            .find(|check| check.id == "plugins.bundles.load")
            .unwrap();
        assert_eq!(check.status, CheckStatus::Pass);
        // The shared-fallback bundle was not probed: the server 404s any
        // path mentioning it, and the check still passes.
        stop_probe_server(addr);
        server.join().unwrap();
        std::fs::remove_dir_all(&home).ok();
        restore_env("DSH_HOME", previous);
    }

    #[cfg(unix)]
    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn doctor_bundle_probe_warns_on_serving_failure() {
        let _guard = dsh::tests::ENV_LOCK.lock().unwrap();
        let previous = std::env::var_os("DSH_HOME");
        let home = fixture_home(349);
        write_local_bundle_fixture(&home);
        std::env::set_var("DSH_HOME", &home);

        // Installed, but the web UI answers 404 for its loader entry — the
        // exact "bundle script failed to load" condition.
        let (addr, server) = probe_server("dsh-mnemon");
        let harness = DeepSeekHarness::new(Arc::new(SystemPlatform))
            .with_cli_names(vec!["definitely-not-a-real-deepmate-command".to_string()])
            .with_ui_url(format!("http://{addr}"));
        let report = harness.doctor().await.unwrap();
        let check = report
            .checks
            .iter()
            .find(|check| check.id == "plugins.bundles.load")
            .unwrap();
        assert_eq!(check.status, CheckStatus::Warn);
        let details = check.details.as_deref().unwrap();
        assert!(
            details.contains("dsh-mnemon"),
            "details must name the plugin: {details}"
        );
        assert!(
            details.contains("404"),
            "details must carry the status: {details}"
        );
        assert!(check.suggested_action.is_some());
        stop_probe_server(addr);
        server.join().unwrap();
        std::fs::remove_dir_all(&home).ok();
        restore_env("DSH_HOME", previous);
    }

    #[cfg(unix)]
    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn doctor_bundle_probe_skips_clientless_bundles() {
        // A patch-only bundle (skills/tools, `dsh.bundle` without
        // `dsh.client`) has no client.js route even when perfectly healthy;
        // probing it would flag every clean install, like the real
        // @huiliyi37/dsh-office case.
        let _guard = dsh::tests::ENV_LOCK.lock().unwrap();
        let previous = std::env::var_os("DSH_HOME");
        let home = fixture_home(307);
        let dir = home.join("profiles/web");
        std::fs::create_dir_all(&dir).unwrap();
        let manifest = serde_json::json!({
            "name": "dsh-profile-web",
            "private": true,
            "dependencies": {},
            "dsh": { "profile": { "bundles": ["@huiliyi37/dsh-office"] } },
        });
        std::fs::write(
            dir.join("package.json"),
            serde_json::to_string_pretty(&manifest).unwrap(),
        )
        .unwrap();
        let local = dir.join("node_modules/@huiliyi37/dsh-office");
        std::fs::create_dir_all(&local).unwrap();
        std::fs::write(
            local.join("package.json"),
            r#"{"name": "@huiliyi37/dsh-office", "version": "0.2.2", "dsh": {"bundle": {"patch": "./cordis.patch.yml"}}}"#,
        )
        .unwrap();
        std::env::set_var("DSH_HOME", &home);

        // The server 404s every office path; the check must still pass
        // because a client-less bundle is never probed.
        let (addr, server) = probe_server("dsh-office");
        let harness = DeepSeekHarness::new(Arc::new(SystemPlatform))
            .with_cli_names(vec!["definitely-not-a-real-deepmate-command".to_string()])
            .with_ui_url(format!("http://{addr}"));
        let report = harness.doctor().await.unwrap();
        let check = report
            .checks
            .iter()
            .find(|check| check.id == "plugins.bundles.load")
            .unwrap();
        assert_eq!(check.status, CheckStatus::Pass);
        stop_probe_server(addr);
        server.join().unwrap();
        std::fs::remove_dir_all(&home).ok();
        restore_env("DSH_HOME", previous);
    }

    #[cfg(unix)]
    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn doctor_bundle_probe_skips_when_ui_is_down() {
        let _guard = dsh::tests::ENV_LOCK.lock().unwrap();
        let previous = std::env::var_os("DSH_HOME");
        let home = fixture_home(302);
        write_local_bundle_fixture(&home);
        std::env::set_var("DSH_HOME", &home);

        let harness = DeepSeekHarness::new(Arc::new(SystemPlatform))
            .with_cli_names(vec!["definitely-not-a-real-deepmate-command".to_string()])
            .with_ui_url("http://127.0.0.1:1");
        let report = harness.doctor().await.unwrap();
        let check = report
            .checks
            .iter()
            .find(|check| check.id == "plugins.bundles.load")
            .unwrap();
        assert_eq!(check.status, CheckStatus::Skip);
        std::fs::remove_dir_all(&home).ok();
        restore_env("DSH_HOME", previous);
    }

    // ---- one-click doctor fixes ----
    //
    // Every actionable diagnostic carries a repair the frontend can trigger
    // directly: clear or reinstall leftover bundles, update plugins the web
    // UI fails to serve, start the console.

    #[cfg(unix)]
    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn fix_check_clears_leftover_bundle_declarations() {
        let _guard = dsh::tests::ENV_LOCK.lock().unwrap();
        let previous = std::env::var_os("DSH_HOME");
        let home = fixture_home(307);
        write_bundle_only_fixture(&home, "web");
        std::env::set_var("DSH_HOME", &home);

        let work = fixture_home(308);
        std::fs::create_dir_all(&work).unwrap();
        let cli = fake_plugin_cli(work.as_path(), 0);
        let recorded = work.join("recorded-args");
        let harness = DeepSeekHarness::new(Arc::new(SystemPlatform))
            .with_cli_names(vec![cli.to_string_lossy().into_owned()])
            .with_data_dir(work.clone());

        let fixed = harness.fix_check("plugins.bundles", "clear").await.unwrap();
        assert_eq!(fixed, 2);
        // Stripping a stale declaration never needs the launcher.
        assert!(!recorded.exists());
        assert!(manifest_bundles(&home, "web").is_empty());

        std::fs::remove_dir_all(&home).ok();
        std::fs::remove_dir_all(&work).ok();
        restore_env("DSH_HOME", previous);
    }

    #[cfg(unix)]
    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn fix_check_reinstalls_leftover_bundles() {
        let _guard = dsh::tests::ENV_LOCK.lock().unwrap();
        let previous = std::env::var_os("DSH_HOME");
        let home = fixture_home(320);
        write_bundle_only_fixture(&home, "web");
        std::env::set_var("DSH_HOME", &home);

        let work = fixture_home(321);
        std::fs::create_dir_all(&work).unwrap();
        let cli = fake_plugin_cli(work.as_path(), 0);
        let recorded = work.join("recorded-args");
        let harness = DeepSeekHarness::new(Arc::new(SystemPlatform))
            .with_cli_names(vec![cli.to_string_lossy().into_owned()])
            .with_data_dir(work.clone());

        let fixed = harness
            .fix_check("plugins.bundles", "reinstall")
            .await
            .unwrap();
        assert_eq!(fixed, 2);
        // The fake CLI overwrites its record per invocation, so the file
        // holds the last forwarded add (sorted order: base first, mnemon
        // last); the count proves both ran.
        assert_eq!(
            std::fs::read_to_string(&recorded)
                .unwrap()
                .lines()
                .collect::<Vec<_>>(),
            ["plugin", "--profile", "web", "add", "dsh-mnemon"]
        );

        std::fs::remove_dir_all(&home).ok();
        std::fs::remove_dir_all(&work).ok();
        restore_env("DSH_HOME", previous);
    }

    #[cfg(unix)]
    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn fix_check_updates_bundles_the_web_ui_fails_to_serve() {
        let _guard = dsh::tests::ENV_LOCK.lock().unwrap();
        let previous = std::env::var_os("DSH_HOME");
        let home = fixture_home(322);
        write_local_bundle_fixture(&home);
        std::env::set_var("DSH_HOME", &home);

        let work = fixture_home(323);
        std::fs::create_dir_all(&work).unwrap();
        let cli = fake_plugin_cli(work.as_path(), 0);
        let recorded = work.join("recorded-args");
        let (addr, server) = probe_server("dsh-mnemon");
        let harness = DeepSeekHarness::new(Arc::new(SystemPlatform))
            .with_cli_names(vec![cli.to_string_lossy().into_owned()])
            .with_data_dir(work.clone())
            .with_ui_url(format!("http://{addr}"));

        let fixed = harness
            .fix_check("plugins.bundles.load", "reinstall")
            .await
            .unwrap();
        assert_eq!(fixed, 1);
        assert_eq!(
            std::fs::read_to_string(&recorded)
                .unwrap()
                .lines()
                .collect::<Vec<_>>(),
            ["plugin", "--profile", "web", "update", "dsh-mnemon"]
        );
        stop_probe_server(addr);
        server.join().unwrap();
        std::fs::remove_dir_all(&home).ok();
        std::fs::remove_dir_all(&work).ok();
        restore_env("DSH_HOME", previous);
    }

    #[cfg(unix)]
    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn fix_check_starts_the_console() {
        let _guard = dsh::tests::ENV_LOCK.lock().unwrap();
        let previous = std::env::var_os("DSH_HOME");
        let previous_boot = std::env::var_os(WEB_UI_BOOT_TIMEOUT_ENV);
        let home = fixture_home(303);
        // start_scenario derives the surface from the installed bundles, so
        // the test home needs a web-surface scenario to start.
        write_web_surface_fixture(&home);
        std::env::set_var("DSH_HOME", &home);
        // Bound the boot wait so a failure cannot stall the suite.
        std::env::set_var(WEB_UI_BOOT_TIMEOUT_ENV, "5");

        let work = fixture_home(304);
        std::fs::create_dir_all(&work).unwrap();
        let cli = fake_plugin_cli(work.as_path(), 0);
        // The web UI starts answering after a window longer than the probe
        // timeout, so the entry probe must miss it and start() must wait
        // out the boot instead of reporting at spawn time.
        let (addr, server) = delayed_probe_server(std::time::Duration::from_millis(1200));
        let harness = DeepSeekHarness::new(Arc::new(SystemPlatform))
            .with_cli_names(vec![cli.to_string_lossy().into_owned()])
            .with_data_dir(work.clone())
            .with_ui_url(format!("http://{addr}"));

        let fixed = harness.fix_check("ui.reachable", "start").await.unwrap();
        assert_eq!(fixed, 1);
        assert!(work.join("state/run-web.pid").is_file());

        stop_probe_server(addr);
        server.join().unwrap();
        std::fs::remove_dir_all(&home).ok();
        std::fs::remove_dir_all(&work).ok();
        restore_env("DSH_HOME", previous);
        restore_env(WEB_UI_BOOT_TIMEOUT_ENV, previous_boot);
    }

    #[cfg(unix)]
    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn start_fails_when_the_web_ui_never_comes_up() {
        let _guard = dsh::tests::ENV_LOCK.lock().unwrap();
        let previous = std::env::var_os("DSH_HOME");
        let previous_boot = std::env::var_os(WEB_UI_BOOT_TIMEOUT_ENV);
        let home = fixture_home(350);
        // A web-surface scenario must exist for start_scenario to boot it.
        write_web_surface_fixture(&home);
        std::env::set_var("DSH_HOME", &home);
        std::env::set_var(WEB_UI_BOOT_TIMEOUT_ENV, "1");

        let work = fixture_home(351);
        std::fs::create_dir_all(&work).unwrap();
        let cli = fake_plugin_cli(work.as_path(), 0);
        let harness = DeepSeekHarness::new(Arc::new(SystemPlatform))
            .with_cli_names(vec![cli.to_string_lossy().into_owned()])
            .with_data_dir(work.clone())
            .with_ui_url("http://127.0.0.1:1");

        let err = harness.start().await.unwrap_err();
        assert!(
            err.to_string().contains("did not come up"),
            "unexpected error: {err}"
        );

        std::fs::remove_dir_all(&home).ok();
        std::fs::remove_dir_all(&work).ok();
        restore_env("DSH_HOME", previous);
        restore_env(WEB_UI_BOOT_TIMEOUT_ENV, previous_boot);
    }

    #[cfg(unix)]
    #[tokio::test]
    #[allow(clippy::await_holding_lock)]
    async fn fix_check_rejects_unknown_combinations_and_reports_nothing_to_fix() {
        let _guard = dsh::tests::ENV_LOCK.lock().unwrap();
        let previous = std::env::var_os("DSH_HOME");
        let home = fixture_home(305);
        write_local_bundle_fixture(&home);
        std::env::set_var("DSH_HOME", &home);

        let work = fixture_home(306);
        std::fs::create_dir_all(&work).unwrap();
        let cli = fake_plugin_cli(work.as_path(), 0);
        let harness = DeepSeekHarness::new(Arc::new(SystemPlatform))
            .with_cli_names(vec![cli.to_string_lossy().into_owned()])
            .with_data_dir(work.clone())
            .with_ui_url("http://127.0.0.1:1");

        let err = harness
            .fix_check("plugins.bundles", "explode")
            .await
            .unwrap_err();
        assert!(err.to_string().contains("unsupported"));
        // Everything healthy: a clear fixes nothing and stays Ok.
        let fixed = harness.fix_check("plugins.bundles", "clear").await.unwrap();
        assert_eq!(fixed, 0);

        std::fs::remove_dir_all(&home).ok();
        std::fs::remove_dir_all(&work).ok();
        restore_env("DSH_HOME", previous);
    }
}
