// Tauri command surface for the DeepMate desktop shell.
//
// These commands mirror the old Slint bridge's UiCommand/UiEvent surface
// (see git history: apps/desktop/src/bridge.rs). The core models are already
// Serialize/Deserialize, so they flow straight back to the React frontend as
// JSON. Action-history recording mirrors the CLI.
//
// Every command carries `#[specta::specta]` so the build generates the
// frontend's typed bindings (src/bindings.ts) from these signatures — the
// React side can never drift from the Rust command surface. Commands take
// `AppHandle` instead of `State` because specta cannot inspect `State`
// parameters; the shared state is reached through `handle.state::<AppState>()`.
//
// Tauri 2 requires async commands that take references to return `Result`, so
// every command returns `Result<T, String>` where the error is a human-readable
// message the frontend surfaces.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use deepmate_app::record_action;
use deepmate_core::data::DataLayout;
use deepmate_core::model::{
    CompatReport, Detection, DisabledPlugin, DoctorReport, MarketEntry, Model, Plugin,
    PluginOpEvent, PluginOpKind, Profile, Provider, RuntimeInstance, RuntimeStatus, Surface,
};
use deepmate_core::CoreResult;
use deepmate_platform::{PlatformService, SystemPlatform};
use deepseek_harness::{AdvancedFileScope, DeepSeekHarness};
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_dialog::DialogExt;

// Shared application state: the harness service and the data layout. The
// service is Send + Sync, so it can live behind an Arc in Tauri state and be
// called from async commands.
pub struct AppState {
    pub harness: Arc<DeepSeekHarness>,
    pub layout: DataLayout,
    // Mirrors config.ui.close_to_tray and is updated when the preference
    // changes, so the window close handler always reads the current value.
    pub close_to_tray: Arc<AtomicBool>,
}

// The overview payload: harness detection, runtime status and inventory
// counts.
#[derive(Serialize, Type)]
pub struct Overview {
    pub detection: Detection,
    pub status: RuntimeStatus,
    pub counts: InventoryCounts,
}

#[derive(Serialize, Default, Type)]
pub struct InventoryCounts {
    pub profiles: u32,
    pub providers: u32,
    pub models: u32,
    pub plugins: u32,
}

// The persisted preferences so the frontend can restore them on startup and
// keep the UI in sync after a change.
#[derive(Serialize, Type)]
pub struct UiPrefs {
    pub language: String,
    pub theme: String,
    pub check_updates: bool,
    pub notify_updates: bool,
    pub close_to_tray: bool,
    pub market_default_source: String,
    // Reflects `Config.market.refresh_interval_seconds`; exported as u32
    // because specta forbids exporting u64.
    #[specta(type = u32)]
    pub market_refresh_interval_seconds: u64,
}

// A newer DeepMate release found on GitHub, if any.
#[derive(Serialize, Type)]
pub struct UpdateInfo {
    pub current_version: String,
    pub latest_version: String,
    pub url: String,
    pub published_at: String,
}

// What an update check concluded. "Could not check" is a distinct answer from
// "you are up to date": reporting a failed check as `up_to_date` told offline
// users they were current, which is the opposite of the truth.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(rename_all = "snake_case")]
pub enum UpdateCheckStatus {
    UpToDate,
    Available,
    Failed,
}

#[derive(Serialize, Type)]
pub struct UpdateCheck {
    pub status: UpdateCheckStatus,
    pub info: Option<UpdateInfo>,
    // A short, user-presentable reason when the check failed.
    pub message: Option<String>,
}

// What an install attempt ended up doing.
#[derive(Serialize, Type)]
pub struct UpdateInstallOutcome {
    // "up_to_date" or "downloaded".
    pub status: String,
    // The DMG that was verified and handed to the platform installer.
    pub path: Option<String>,
    pub version: Option<String>,
}

// Progress for a running update install, forwarded to the webview channel:
// a multi-hundred-megabyte download with no feedback looks like a hang.
#[derive(Debug, Clone, Serialize, Type)]
pub struct UpdateProgressEvent {
    pub phase: String,
    // Exported as u32 because specta forbids BigInt-style widths; byte counts
    // beyond 4 GiB are not a case an installer download reaches.
    #[specta(type = u32)]
    pub received: u64,
    #[specta(type = Option<u32>)]
    pub total: Option<u64>,
    pub message: Option<String>,
}

// ---- Command plumbing ----

// The IPC error contract: commands still resolve errors to a `String`, but
// core errors are serialized as a JSON envelope
// `{"code":"<stable code>","message":"..."}` built from the core error's
// classification (`CoreError::code`), so the frontend maps exact codes to
// localized text instead of pattern-matching prose. Errors that do not come
// from core (update plumbing) stay plain strings and fall back to the
// frontend's regex mapping.
fn command_error(e: deepmate_core::CoreError) -> String {
    serde_json::json!({ "code": e.code(), "message": e.to_string() }).to_string()
}

// `command_error` with a context prefix kept in the message.
fn command_error_ctx(context: &str, e: deepmate_core::CoreError) -> String {
    serde_json::json!({ "code": e.code(), "message": format!("{context}: {e}") }).to_string()
}

// Run a harness operation, record it in the action history and normalize the
// error to the human-readable string the frontend surfaces.
async fn run_action(
    state: &AppState,
    action: &'static str,
    op: impl std::future::Future<Output = CoreResult<()>>,
) -> Result<(), String> {
    match op.await {
        Ok(()) => {
            record_action(&state.layout, action.to_string());
            Ok(())
        }
        Err(e) => Err(command_error(e)),
    }
}

// ---- Overview ----

#[tauri::command]
#[specta::specta]
pub async fn refresh_all(app: AppHandle) -> Result<Overview, String> {
    let state = app.state::<AppState>();
    let harness = state.harness.as_ref();

    let detection = harness
        .detect()
        .await
        .map_err(|e| command_error_ctx("detect failed", e))?;
    let status = harness
        .status()
        .await
        .map_err(|e| command_error_ctx("status failed", e))?;
    let counts = InventoryCounts {
        profiles: count(harness.profiles().await)?,
        providers: count(harness.providers().await)?,
        models: count(harness.models().await)?,
        plugins: count(harness.plugins().await)?,
    };
    Ok(Overview {
        detection,
        status,
        counts,
    })
}

// A list result reduced to its item count for the overview payload.
fn count<T>(items: CoreResult<Vec<T>>) -> Result<u32, String> {
    items.map(|items| items.len() as u32).map_err(command_error)
}

// ---- Runtime ----

#[tauri::command]
#[specta::specta]
pub async fn runtime_start(app: AppHandle, profile: String) -> Result<(), String> {
    runtime_op(&app, RuntimeOp::Start, profile, "desktop.runtime.start").await
}

#[tauri::command]
#[specta::specta]
pub async fn runtime_stop(app: AppHandle, profile: String) -> Result<(), String> {
    runtime_op(&app, RuntimeOp::Stop, profile, "desktop.runtime.stop").await
}

#[tauri::command]
#[specta::specta]
pub async fn runtime_restart(app: AppHandle, profile: String) -> Result<(), String> {
    runtime_op(&app, RuntimeOp::Restart, profile, "desktop.runtime.restart").await
}

#[tauri::command]
#[specta::specta]
pub async fn runtime_list(app: AppHandle) -> Result<Vec<RuntimeInstance>, String> {
    let state = app.state::<AppState>();
    state.harness.instances().await.map_err(command_error)
}

enum RuntimeOp {
    Start,
    Stop,
    Restart,
}

async fn runtime_op(
    app: &AppHandle,
    op: RuntimeOp,
    profile: String,
    action: &'static str,
) -> Result<(), String> {
    let state = app.state::<AppState>();
    let harness = state.harness.as_ref();
    let result = match op {
        RuntimeOp::Start => harness.start_scenario(&profile, None).await,
        RuntimeOp::Stop => harness.stop_scenario(&profile).await,
        RuntimeOp::Restart => harness.restart_scenario(&profile).await,
    };
    match result {
        Ok(()) => {
            record_action(&state.layout, action.to_string());
            Ok(())
        }
        Err(e) => Err(command_error(e)),
    }
}

#[tauri::command]
#[specta::specta]
pub async fn open_harness(app: AppHandle, profile: String) -> Result<(), String> {
    let state = app.state::<AppState>();
    run_action(
        &state,
        "desktop.open",
        state.harness.open_ui_scenario(&profile),
    )
    .await
}

// Stream one task run against a task-surface scenario: events (Started /
// Line / Finished) are forwarded to the webview channel as they happen.
#[tauri::command]
#[specta::specta]
pub async fn task_run(
    app: AppHandle,
    channel: tauri::ipc::Channel<PluginOpEvent>,
    profile: String,
    prompt: String,
) -> Result<(), String> {
    let state = app.state::<AppState>();
    let (tx, mut rx) = tokio::sync::mpsc::channel::<PluginOpEvent>(64);
    let harness = Arc::clone(&state.harness);

    let forwarder = tauri::async_runtime::spawn(async move {
        while let Some(event) = rx.recv().await {
            if channel.send(event).is_err() {
                break;
            }
        }
    });

    let result = harness.run_task(&profile, &prompt, tx).await;
    std::mem::drop(forwarder);
    match result {
        Ok(()) => {
            record_action(&state.layout, "desktop.task.run".to_string());
            Ok(())
        }
        Err(e) => Err(command_error(e)),
    }
}

#[tauri::command]
#[specta::specta]
pub async fn run_doctor(app: AppHandle) -> Result<DoctorReport, String> {
    let state = app.state::<AppState>();
    match state.harness.doctor().await {
        Ok(report) => {
            record_action(&state.layout, "desktop.doctor".to_string());
            Ok(report)
        }
        Err(e) => Err(command_error(e)),
    }
}

// The result of a one-click doctor fix: how many items were repaired, and
// which ones could not be. A partial repair is a real outcome the UI must be
// able to show instead of a bare success count.
#[derive(Debug, Clone, Serialize, Type)]
pub struct DoctorFixReport {
    pub fixed: u32,
    pub failures: Vec<String>,
}

// One-click repair for a failing doctor check. `mode` selects the repair:
// "clear" strips stale bundle declarations, "reinstall" reinstalls or
// updates the affected plugins, "start" boots the harness web UI.
#[tauri::command]
#[specta::specta]
pub async fn doctor_fix(
    app: AppHandle,
    check_id: String,
    mode: String,
) -> Result<DoctorFixReport, String> {
    let state = app.state::<AppState>();
    match state.harness.fix_check(&check_id, &mode).await {
        Ok(report) => {
            record_action(&state.layout, "desktop.doctor.fix".to_string());
            Ok(DoctorFixReport {
                fixed: report.fixed,
                failures: report.failures,
            })
        }
        Err(e) => Err(command_error(e)),
    }
}

// ---- Inventory ----

#[tauri::command]
#[specta::specta]
pub async fn list_profiles(app: AppHandle) -> Result<Vec<Profile>, String> {
    let state = app.state::<AppState>();
    state.harness.profiles().await.map_err(command_error)
}

#[tauri::command]
#[specta::specta]
pub async fn list_providers(app: AppHandle, profile: String) -> Result<Vec<Provider>, String> {
    let state = app.state::<AppState>();
    state
        .harness
        .providers_scenario(&profile)
        .await
        .map_err(command_error)
}

#[tauri::command]
#[specta::specta]
pub async fn list_models(app: AppHandle, profile: String) -> Result<Vec<Model>, String> {
    let state = app.state::<AppState>();
    state
        .harness
        .models_scenario(&profile)
        .await
        .map_err(command_error)
}

// ---- Configuration editing (per-scenario) ----

#[tauri::command]
#[specta::specta]
pub async fn upsert_provider(
    app: AppHandle,
    profile: String,
    provider: Provider,
) -> Result<(), String> {
    let state = app.state::<AppState>();
    run_action(
        &state,
        "desktop.provider.set",
        state.harness.upsert_provider_scenario(&profile, provider),
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn remove_provider(app: AppHandle, profile: String, id: String) -> Result<(), String> {
    let state = app.state::<AppState>();
    run_action(
        &state,
        "desktop.provider.remove",
        state.harness.remove_provider_scenario(&profile, &id),
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn upsert_model(
    app: AppHandle,
    profile: String,
    provider: String,
    model: Model,
) -> Result<(), String> {
    let state = app.state::<AppState>();
    run_action(
        &state,
        "desktop.model.set",
        state
            .harness
            .upsert_model_scenario(&profile, &provider, model),
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn remove_model(
    app: AppHandle,
    profile: String,
    provider: String,
    id: String,
) -> Result<(), String> {
    let state = app.state::<AppState>();
    run_action(
        &state,
        "desktop.model.remove",
        state
            .harness
            .remove_model_scenario(&profile, &provider, &id),
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn create_scenario(app: AppHandle, name: String, surface: Surface) -> Result<(), String> {
    let state = app.state::<AppState>();
    run_action(
        &state,
        "desktop.profile.create",
        state.harness.create_scenario(&name, surface),
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn remove_profile(app: AppHandle, name: String) -> Result<(), String> {
    let state = app.state::<AppState>();
    run_action(
        &state,
        "desktop.profile.remove",
        state.harness.remove_profile(&name),
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn rename_profile(app: AppHandle, old: String, new_name: String) -> Result<(), String> {
    let state = app.state::<AppState>();
    run_action(
        &state,
        "desktop.profile.rename",
        state.harness.rename_profile(&old, &new_name),
    )
    .await
}

// Set (or clear) a scenario's manifest description. `None` restores the
// engine's bundles fallback description.
#[tauri::command]
#[specta::specta]
pub async fn set_scenario_description(
    app: AppHandle,
    name: String,
    description: Option<String>,
) -> Result<(), String> {
    let state = app.state::<AppState>();
    run_action(
        &state,
        "desktop.profile.description",
        state.harness.set_scenario_description(&name, description),
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn list_plugins(app: AppHandle) -> Result<Vec<Plugin>, String> {
    let state = app.state::<AppState>();
    state.harness.plugins().await.map_err(command_error)
}

// ---- Plugins ----

#[tauri::command]
#[specta::specta]
pub async fn plugin_install(app: AppHandle, profile: String, spec: String) -> Result<(), String> {
    let state = app.state::<AppState>();
    run_action(
        &state,
        "desktop.plugin.install",
        state.harness.install_plugin(&profile, &spec),
    )
    .await
}

// Disable a plugin: uninstall it while remembering its package spec, so
// enabling restores the same range later.
#[tauri::command]
#[specta::specta]
pub async fn plugin_disable(app: AppHandle, profile: String, id: String) -> Result<(), String> {
    let state = app.state::<AppState>();
    run_action(
        &state,
        "desktop.plugin.disable",
        state.harness.disable_plugin(&profile, &id),
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn plugin_enable(app: AppHandle, profile: String, id: String) -> Result<(), String> {
    let state = app.state::<AppState>();
    run_action(
        &state,
        "desktop.plugin.enable",
        state.harness.enable_plugin(&profile, &id),
    )
    .await
}

// The disabled-plugin registry: packages the user turned off, kept so the UI
// can offer a one-click re-enable.
#[tauri::command]
#[specta::specta]
pub async fn list_disabled_plugins(app: AppHandle) -> Result<Vec<DisabledPlugin>, String> {
    let state = app.state::<AppState>();
    state.harness.disabled_plugins().map_err(command_error)
}

// Run a plugin operation while streaming progress events to the frontend
// over a Tauri channel. The command returns once the operation finishes; the
// events (Started / Line / Finished) arrive on `channel` as they happen, so
// the UI can render a live progress log. The blocking plugin commands above
// stay for callers that only need the outcome.
#[tauri::command]
#[specta::specta]
pub async fn plugin_op_stream(
    app: AppHandle,
    channel: tauri::ipc::Channel<PluginOpEvent>,
    profile: String,
    kind: PluginOpKind,
    target: String,
) -> Result<(), String> {
    let state = app.state::<AppState>();

    let (tx, mut rx) = tokio::sync::mpsc::channel::<PluginOpEvent>(64);
    let harness = Arc::clone(&state.harness);
    let action = match kind {
        PluginOpKind::Install => "desktop.plugin.install",
        PluginOpKind::Remove => "desktop.plugin.remove",
        PluginOpKind::Update => "desktop.plugin.update",
        // Tasks stream through their own command; the action is not recorded.
        PluginOpKind::Task => "desktop.task.run",
    };

    // Forward events from the harness's stream to the webview channel.
    let forwarder = tauri::async_runtime::spawn(async move {
        while let Some(event) = rx.recv().await {
            if channel.send(event).is_err() {
                break;
            }
        }
    });

    let result = harness
        .stream_plugin_op(&profile, kind, Some(&target), tx)
        .await;
    // The forwarder drains the channel and exits on its own once the sender
    // is dropped; no explicit join needed.
    std::mem::drop(forwarder);
    match result {
        Ok(()) => {
            record_action(&state.layout, action.to_string());
            Ok(())
        }
        Err(e) => Err(command_error(e)),
    }
}

// ---- Market ----

#[tauri::command]
#[specta::specta]
pub async fn market_search(app: AppHandle, query: String) -> Result<Vec<MarketEntry>, String> {
    let state = app.state::<AppState>();
    state
        .harness
        .search_plugins(&query)
        .await
        .map_err(command_error)
}

// Check a market package's compatibility with the detected harness before an
// installation, like the CLI's `deepmate plugin check`.
#[tauri::command]
#[specta::specta]
pub async fn plugin_check(app: AppHandle, spec: String) -> Result<CompatReport, String> {
    let state = app.state::<AppState>();
    state
        .harness
        .plugin_compat(&spec)
        .await
        .map_err(command_error)
}

// ---- Snapshots ----

#[tauri::command]
#[specta::specta]
pub async fn snapshot_export(app: AppHandle, name: String) -> Result<(), String> {
    let state = app.state::<AppState>();
    let store = deepmate_core::SnapshotStore::new(state.layout.snapshots_dir());
    match state.harness.capture_snapshot().await {
        Ok(snapshot) => match store.save(&name, &snapshot) {
            Ok(()) => {
                record_action(&state.layout, "desktop.snapshot.export".to_string());
                Ok(())
            }
            Err(e) => Err(command_error(e)),
        },
        Err(e) => Err(command_error(e)),
    }
}

#[tauri::command]
#[specta::specta]
pub async fn snapshot_import(app: AppHandle, name: String) -> Result<(), String> {
    let state = app.state::<AppState>();
    let store = deepmate_core::SnapshotStore::new(state.layout.snapshots_dir());
    let snapshot = store.load(&name).map_err(command_error)?;
    // Import overwrites the current setup, so a restore point is written
    // first: a mistaken import stays undoable by hand (the file is named
    // `state/pre-snapshot-import-<stamp>.json` and applies like any other
    // snapshot).
    match state.harness.write_restore_point("snapshot-import") {
        Ok(Some(path)) => {
            tracing::info!(path = %path.display(), "pre-import restore point written")
        }
        Ok(None) => {}
        Err(err) => {
            // No restore point means no way back: refuse rather than destroy
            // the user's setup with no undo.
            return Err(command_error_ctx(
                "refusing to import without a restore point",
                err,
            ));
        }
    }
    match state.harness.apply_snapshot(&snapshot).await {
        Ok(_) => {
            record_action(&state.layout, "desktop.snapshot.import".to_string());
            Ok(())
        }
        Err(e) => Err(command_error(e)),
    }
}

#[tauri::command]
#[specta::specta]
pub async fn snapshot_list(app: AppHandle) -> Result<Vec<String>, String> {
    let state = app.state::<AppState>();
    let store = deepmate_core::SnapshotStore::new(state.layout.snapshots_dir());
    store.list().map_err(command_error)
}

#[tauri::command]
#[specta::specta]
pub async fn snapshot_delete(app: AppHandle, name: String) -> Result<(), String> {
    let state = app.state::<AppState>();
    let store = deepmate_core::SnapshotStore::new(state.layout.snapshots_dir());
    store.delete(&name).map_err(command_error)
}

// ---- Config (language / theme / preferences) ----

#[tauri::command]
#[specta::specta]
pub async fn set_language(app: AppHandle, language: String) -> Result<(), String> {
    if !matches!(language.as_str(), "en" | "zh") {
        return Err(format!("unsupported language: {language}"));
    }
    let state = app.state::<AppState>();
    let mut config =
        deepmate_core::data::Config::load(&state.layout.config_path()).map_err(command_error)?;
    config.general.language = language;
    config
        .save(&state.layout.config_path())
        .map_err(command_error)
}

#[tauri::command]
#[specta::specta]
pub async fn set_notify_updates(app: AppHandle, enabled: bool) -> Result<(), String> {
    let state = app.state::<AppState>();
    let mut config =
        deepmate_core::data::Config::load(&state.layout.config_path()).map_err(command_error)?;
    config.general.notify_updates = enabled;
    config
        .save(&state.layout.config_path())
        .map_err(command_error)
}

#[tauri::command]
#[specta::specta]
pub async fn set_theme(app: AppHandle, theme: String) -> Result<(), String> {
    if !matches!(theme.as_str(), "system" | "light" | "dark") {
        return Err(format!("unsupported theme: {theme}"));
    }
    let state = app.state::<AppState>();
    let mut config =
        deepmate_core::data::Config::load(&state.layout.config_path()).map_err(command_error)?;
    config.ui.theme = theme;
    config
        .save(&state.layout.config_path())
        .map_err(command_error)
}

#[tauri::command]
#[specta::specta]
pub async fn set_close_to_tray(app: AppHandle, enabled: bool) -> Result<(), String> {
    let state = app.state::<AppState>();
    let mut config =
        deepmate_core::data::Config::load(&state.layout.config_path()).map_err(command_error)?;
    config.ui.close_to_tray = enabled;
    config
        .save(&state.layout.config_path())
        .map_err(command_error)?;
    state.close_to_tray.store(enabled, Ordering::Relaxed);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn set_check_updates(app: AppHandle, enabled: bool) -> Result<(), String> {
    let state = app.state::<AppState>();
    let mut config =
        deepmate_core::data::Config::load(&state.layout.config_path()).map_err(command_error)?;
    config.general.check_updates = enabled;
    config
        .save(&state.layout.config_path())
        .map_err(command_error)
}

#[tauri::command]
#[specta::specta]
pub async fn get_config(app: AppHandle) -> Result<UiPrefs, String> {
    let state = app.state::<AppState>();
    let config =
        deepmate_core::data::Config::load(&state.layout.config_path()).map_err(command_error)?;
    Ok(UiPrefs {
        language: config.general.language,
        theme: config.ui.theme,
        check_updates: config.general.check_updates,
        notify_updates: config.general.notify_updates,
        close_to_tray: config.ui.close_to_tray,
        market_default_source: config.market.default_source,
        market_refresh_interval_seconds: config.market.refresh_interval_seconds,
    })
}

// ---- Market settings ----

#[tauri::command]
#[specta::specta]
pub async fn set_market_default_source(app: AppHandle, source: String) -> Result<(), String> {
    if !matches!(source.as_str(), "curated" | "community") {
        return Err(format!("unsupported market source: {source}"));
    }
    let state = app.state::<AppState>();
    let mut config =
        deepmate_core::data::Config::load(&state.layout.config_path()).map_err(command_error)?;
    config.market.default_source = source;
    config
        .save(&state.layout.config_path())
        .map_err(command_error)
}

// The market cache freshness, in seconds. The lower bound keeps a mis-set
// value from hammering the registry; the upper bound is a week.
// `u32`, not `u64`: Specta refuses BigInt-style types in bindings exports.
#[tauri::command]
#[specta::specta]
pub async fn set_market_refresh_interval(app: AppHandle, seconds: u32) -> Result<(), String> {
    let seconds = u64::from(seconds);
    if !(60..=604800).contains(&seconds) {
        return Err("market refresh interval must be between 60 and 604800 seconds".to_string());
    }
    let state = app.state::<AppState>();
    let mut config =
        deepmate_core::data::Config::load(&state.layout.config_path()).map_err(command_error)?;
    config.market.refresh_interval_seconds = seconds;
    config
        .save(&state.layout.config_path())
        .map_err(command_error)
}

// ---- Advanced raw configuration files ----

// Map the frontend's string scope onto the harness's file-scope enum. The
// mapping is the only place the wire names for these files are defined.
fn parse_advanced_scope(scope: &str) -> Result<AdvancedFileScope, String> {
    match scope {
        "global-settings" => Ok(AdvancedFileScope::GlobalSettings),
        "scene-settings" => Ok(AdvancedFileScope::SceneSettings),
        "scene-cordis" => Ok(AdvancedFileScope::SceneCordis),
        _ => Err(format!("unknown advanced file scope: {scope}")),
    }
}

#[tauri::command]
#[specta::specta]
pub async fn advanced_file_read(
    app: AppHandle,
    scope: String,
    name: Option<String>,
) -> Result<Option<String>, String> {
    let state = app.state::<AppState>();
    let scope = parse_advanced_scope(&scope)?;
    state
        .harness
        .advanced_file_read(scope, name)
        .await
        .map_err(command_error)
}

#[tauri::command]
#[specta::specta]
pub async fn advanced_file_save(
    app: AppHandle,
    scope: String,
    name: Option<String>,
    content: String,
) -> Result<(), String> {
    let state = app.state::<AppState>();
    let scope = parse_advanced_scope(&scope)?;
    run_action(
        &state,
        "desktop.advanced.save",
        state.harness.advanced_file_save(scope, name, content),
    )
    .await
}

// ---- Own-settings backup (export / import) ----

// `Ok(None)` means the dialog was dismissed; both commands are no-ops then.
#[tauri::command]
#[specta::specta]
pub async fn config_export(app: AppHandle) -> Result<Option<String>, String> {
    let picked = app
        .dialog()
        .file()
        .add_filter("JSON", &["json"])
        .set_file_name("deepmate-config.json")
        .blocking_save_file();
    let Some(path) = picked.and_then(|file| file.into_path().ok()) else {
        return Ok(None);
    };
    let state = app.state::<AppState>();
    let config =
        deepmate_core::data::Config::load(&state.layout.config_path()).map_err(command_error)?;
    deepmate_core::ConfigBackup::capture(&config)
        .save(&path)
        .map_err(command_error)?;
    record_action(&state.layout, "desktop.config.export".to_string());
    Ok(Some(path.display().to_string()))
}

#[tauri::command]
#[specta::specta]
pub async fn config_import(app: AppHandle) -> Result<Option<String>, String> {
    let picked = app
        .dialog()
        .file()
        .add_filter("JSON", &["json"])
        .blocking_pick_file();
    let Some(path) = picked.and_then(|file| file.into_path().ok()) else {
        return Ok(None);
    };
    let state = app.state::<AppState>();
    let backup = deepmate_core::ConfigBackup::load(&path).map_err(command_error)?;
    // Keep the current settings so an unwanted import can be reversed with
    // the same command pointed at the saved copy.
    let current = state.layout.config_path();
    if current.is_file() {
        let stamp = chrono::Utc::now().format("%Y%m%d-%H%M%S");
        let saved = state
            .layout
            .state_dir()
            .join(format!("pre-config-import-{stamp}.json"));
        let previous = deepmate_core::Config::load(&current).map_err(command_error)?;
        deepmate_core::ConfigBackup::capture(&previous)
            .save(&saved)
            .map_err(command_error)?;
    }
    backup.config.save(&current).map_err(command_error)?;
    record_action(&state.layout, "desktop.config.import".to_string());
    Ok(Some(path.display().to_string()))
}

// ---- Auto-start ----

// Whether the OS is registered to start DeepMate at login. The operating
// system is the source of truth here; config.general.auto_start only mirrors
// it after a toggle.
#[tauri::command]
#[specta::specta]
pub async fn autostart_get(app: AppHandle) -> Result<bool, String> {
    app.autolaunch()
        .is_enabled()
        .map_err(|e| format!("failed to read auto-start state: {e}"))
}

#[tauri::command]
#[specta::specta]
pub async fn autostart_set(app: AppHandle, enabled: bool) -> Result<(), String> {
    let manager = app.autolaunch();
    let result = if enabled {
        manager.enable()
    } else {
        manager.disable()
    };
    result.map_err(|e| format!("failed to update auto-start state: {e}"))?;

    let state = app.state::<AppState>();
    let mut config =
        deepmate_core::data::Config::load(&state.layout.config_path()).map_err(command_error)?;
    config.general.auto_start = enabled;
    config
        .save(&state.layout.config_path())
        .map_err(command_error)
}

// ---- Update check ----

const UPDATE_API_URL: &str =
    "https://api.github.com/repos/realchendahuang/DeepMate/releases/latest";
const UPDATE_TIMEOUT_SECS: u64 = 10;

// Query the GitHub releases API for a newer DeepMate release. Shared by the
// `check_update` command, the tray/startup notification and the install
// flow. `Ok(None)` means the current version is the latest; `Err` means the
// check could not complete (offline, rate limited, a bad response) and the
// caller must say so rather than claiming the app is current.
//
// `DEEPMATE_UPDATE_API_URL` overrides the endpoint so the flow can be
// exercised against a test release.
pub(crate) async fn latest_release() -> Result<Option<UpdateInfo>, String> {
    let client = reqwest::Client::builder()
        .user_agent(format!("DeepMate/{}", env!("CARGO_PKG_VERSION")))
        .timeout(std::time::Duration::from_secs(UPDATE_TIMEOUT_SECS))
        .build()
        .map_err(|err| format!("failed to build the HTTP client: {err}"))?;

    #[derive(Deserialize)]
    struct Release {
        tag_name: String,
        html_url: String,
        published_at: String,
    }

    let api =
        std::env::var("DEEPMATE_UPDATE_API_URL").unwrap_or_else(|_| UPDATE_API_URL.to_string());
    let response = client.get(api).send().await.map_err(|err| {
        tracing::warn!("update check failed: {err}");
        format!("could not reach the release server: {err}")
    })?;
    if !response.status().is_success() {
        let status = response.status();
        tracing::warn!("update check: the release server returned {status}");
        return Err(format!("the release server returned HTTP {status}"));
    }
    let release: Release = response.json().await.map_err(|err| {
        tracing::warn!("update check: invalid response: {err}");
        format!("the release server returned an unreadable response: {err}")
    })?;

    let current = env!("CARGO_PKG_VERSION");
    if !deepmate_core::is_newer_version(&release.tag_name, current) {
        return Ok(None);
    }
    Ok(Some(UpdateInfo {
        current_version: current.to_string(),
        latest_version: release.tag_name.trim_start_matches('v').to_string(),
        url: release.html_url,
        published_at: release.published_at,
    }))
}

// The release document with its assets, for the install flow (richer than
// `UpdateInfo`, which only feeds the banner).
#[derive(Deserialize)]
struct FullRelease {
    tag_name: String,
    assets: Vec<ReleaseAssetResponse>,
}

#[derive(Deserialize)]
struct ReleaseAssetResponse {
    name: String,
    browser_download_url: String,
}

// The full install half of the update loop: fetch the latest release, pick
// the DMG for this platform, download it, verify the published sha256 and
// hand it to the OS installer. Downloaded installers are not run silently:
// the DMG opens so the drag-to-install step stays in the user's hands.
#[tauri::command]
#[specta::specta]
pub async fn update_install(
    app: AppHandle,
    channel: tauri::ipc::Channel<UpdateProgressEvent>,
) -> Result<UpdateInstallOutcome, String> {
    let result = update_install_inner(&app, &channel).await;
    if let Err(err) = &result {
        let _ = channel.send(UpdateProgressEvent {
            phase: "failed".to_string(),
            received: 0,
            total: None,
            message: Some(err.clone()),
        });
    }
    result
}

async fn update_install_inner(
    app: &AppHandle,
    channel: &tauri::ipc::Channel<UpdateProgressEvent>,
) -> Result<UpdateInstallOutcome, String> {
    let client = reqwest::Client::builder()
        .user_agent(format!("DeepMate/{}", env!("CARGO_PKG_VERSION")))
        .timeout(std::time::Duration::from_secs(600))
        .build()
        .map_err(|e| format!("failed to build HTTP client: {e}"))?;
    let api =
        std::env::var("DEEPMATE_UPDATE_API_URL").unwrap_or_else(|_| UPDATE_API_URL.to_string());

    let release: FullRelease = client
        .get(&api)
        .send()
        .await
        .map_err(|e| format!("update check failed: {e}"))?
        .error_for_status()
        .map_err(|e| format!("update check failed: {e}"))?
        .json()
        .await
        .map_err(|e| format!("invalid release response: {e}"))?;

    let current = env!("CARGO_PKG_VERSION");
    let version = release.tag_name.trim_start_matches('v').to_string();
    if !deepmate_core::is_newer_version(&release.tag_name, current) {
        return Ok(UpdateInstallOutcome {
            status: "up_to_date".to_string(),
            path: None,
            version: None,
        });
    }

    let assets: Vec<deepmate_core::ReleaseAsset> = release
        .assets
        .iter()
        .map(|asset| deepmate_core::ReleaseAsset {
            name: asset.name.clone(),
            url: asset.browser_download_url.clone(),
        })
        .collect();
    let triple = deepmate_core::target_triple();
    let (dmg, checksum) =
        deepmate_core::pick_release_asset(&assets, &triple, deepmate_core::AssetKind::Dmg)
            .ok_or_else(|| format!("release {} has no desktop bundle for {triple}", version))?;

    let work = std::env::temp_dir().join(format!("deepmate-update-{}", std::process::id()));
    std::fs::create_dir_all(&work)
        .map_err(|e| format!("failed to create the work directory: {e}"))?;
    let dmg_path = work.join(&dmg.name);

    // Stream to disk and report progress: a release bundle is hundreds of
    // megabytes, and buffering it whole in memory (the previous shape) both
    // spiked RAM and left the UI with nothing to show.
    let _ = channel.send(UpdateProgressEvent {
        phase: "downloading".to_string(),
        received: 0,
        total: None,
        message: None,
    });
    let response = client
        .get(&dmg.url)
        .send()
        .await
        .map_err(|e| format!("failed to download the update: {e}"))?
        .error_for_status()
        .map_err(|e| format!("failed to download the update: {e}"))?;
    let total = response.content_length();
    let mut file = tokio::fs::File::create(&dmg_path)
        .await
        .map_err(|e| format!("failed to write the update: {e}"))?;
    let mut received: u64 = 0;
    let mut last_report: u64 = 0;
    let mut stream = response.bytes_stream();
    {
        use futures::StreamExt as _;
        use tokio::io::AsyncWriteExt as _;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|e| format!("failed to download the update: {e}"))?;
            file.write_all(&chunk)
                .await
                .map_err(|e| format!("failed to write the update: {e}"))?;
            received += chunk.len() as u64;
            // Throttle the events: one per megabyte is plenty for a progress
            // bar and keeps the channel from flooding.
            if received - last_report >= 1_048_576 {
                last_report = received;
                let _ = channel.send(UpdateProgressEvent {
                    phase: "downloading".to_string(),
                    received,
                    total,
                    message: None,
                });
            }
        }
        file.flush()
            .await
            .map_err(|e| format!("failed to write the update: {e}"))?;
        file.sync_all()
            .await
            .map_err(|e| format!("failed to write the update: {e}"))?;
    }

    let _ = channel.send(UpdateProgressEvent {
        phase: "verifying".to_string(),
        received,
        total,
        message: None,
    });
    let sum_text = client
        .get(&checksum.url)
        .send()
        .await
        .map_err(|e| format!("failed to download the checksum: {e}"))?
        .error_for_status()
        .map_err(|e| format!("failed to download the checksum: {e}"))?
        .text()
        .await
        .map_err(|e| format!("failed to download the checksum: {e}"))?;
    // The checksum must name the artifact it is supposed to vouch for, so a
    // stale or swapped sidecar cannot certify a different download.
    let expected = deepmate_core::parse_checksum_for(&sum_text, &dmg.name)
        .ok_or_else(|| "the published checksum file does not match the update".to_string())?;
    let verify_path = dmg_path.clone();
    tokio::task::spawn_blocking(move || deepmate_core::verify_sha256(&verify_path, &expected))
        .await
        .map_err(|e| format!("checksum task failed: {e}"))?
        .map_err(|e| format!("the downloaded update failed verification: {e}"))?;

    // The checksum only proves the bytes arrived intact from the same
    // channel; asking the OS to assess the code signature is the check that
    // actually speaks to who produced the bundle.
    verify_installer_signature(&dmg_path).map_err(|e| {
        format!("the downloaded update is not correctly signed and was not opened: {e}")
    })?;

    let _ = channel.send(UpdateProgressEvent {
        phase: "opening".to_string(),
        received,
        total,
        message: None,
    });
    open_with_platform(&dmg_path).map_err(|e| format!("failed to open the installer: {e}"))?;
    let state = app.state::<AppState>();
    record_action(&state.layout, "desktop.update.install".to_string());
    Ok(UpdateInstallOutcome {
        status: "downloaded".to_string(),
        path: Some(dmg_path.display().to_string()),
        version: Some(version),
    })
}

// Have the OS validate the installer's code signature before it is opened.
// Only macOS has a cheap CLI for this; other platforms pass through.
fn verify_installer_signature(path: &std::path::Path) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        let output = std::process::Command::new("spctl")
            .args(["--assess", "--type", "install"])
            .arg(path)
            .output()
            .map_err(|e| format!("could not run the signature check: {e}"))?;
        if !output.status.success() {
            let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
            return Err(if detail.is_empty() {
                "the installer failed the system signature assessment".to_string()
            } else {
                detail
            });
        }
    }
    let _ = path;
    Ok(())
}

// Hand a downloaded file to the platform's installer association.
fn open_with_platform(path: &std::path::Path) -> Result<(), String> {
    #[cfg(target_os = "macos")]
    let mut command = {
        let mut command = std::process::Command::new("open");
        command.arg(path);
        command
    };
    #[cfg(target_os = "windows")]
    let mut command = {
        let mut command = std::process::Command::new("explorer");
        command.arg(path);
        command
    };
    #[cfg(all(unix, not(target_os = "macos")))]
    let mut command = {
        let mut command = std::process::Command::new("xdg-open");
        command.arg(path);
        command
    };
    command.status().map(|_| ()).map_err(|e| format!("{e}"))
}

#[tauri::command]
#[specta::specta]
pub async fn check_update() -> Result<UpdateCheck, String> {
    Ok(match latest_release().await {
        Ok(Some(info)) => UpdateCheck {
            status: UpdateCheckStatus::Available,
            info: Some(info),
            message: None,
        },
        Ok(None) => UpdateCheck {
            status: UpdateCheckStatus::UpToDate,
            info: None,
            message: None,
        },
        Err(message) => UpdateCheck {
            status: UpdateCheckStatus::Failed,
            info: None,
            message: Some(message),
        },
    })
}

// Open a URL in the system browser (used by the update banner). Only http(s)
// is accepted: this command is reachable from the webview.
#[tauri::command]
#[specta::specta]
pub async fn open_url(url: String) -> Result<(), String> {
    if !url.starts_with("https://") && !url.starts_with("http://") {
        return Err("refusing to open non-http URL".to_string());
    }
    SystemPlatform.open_url(&url).map_err(|e| format!("{e}"))
}
