// Tauri command surface for the DeepMate desktop shell.
//
// These commands mirror the old Slint bridge's UiCommand/UiEvent surface
// (see git history: apps/desktop/src/bridge.rs). The core models are already
// Serialize/Deserialize, so they flow straight back to the React frontend as
// JSON. Capability gating and action-history recording mirror the CLI.
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
use deepmate_core::adapter::HarnessAdapter;
use deepmate_core::data::DataLayout;
use deepmate_core::model::{
    CompatReport, DoctorReport, MarketEntry, MarketSourceInfo, Model, Plugin, PluginOpEvent,
    PluginOpKind, Profile, Provider, RuntimeStatus,
};
use deepmate_core::CoreResult;
use deepmate_platform::{PlatformService, SystemPlatform};
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager};
use tauri_plugin_autostart::ManagerExt;
use tauri_plugin_dialog::DialogExt;

// Shared application state: the active adapter and the data layout. The
// adapter is Send + Sync, so it can live behind an Arc in Tauri state and be
// called from async commands.
pub struct AppState {
    pub adapter: Arc<dyn HarnessAdapter>,
    pub layout: DataLayout,
    // Mirrors config.ui.close_to_tray and is updated when the preference
    // changes, so the window close handler always reads the current value.
    pub close_to_tray: Arc<AtomicBool>,
}

// The overview payload: detection, runtime status, and capability-gated
// inventory counts. `None` means the adapter does not declare the capability.
#[derive(Serialize, Type)]
pub struct Overview {
    pub detection: deepmate_core::adapter::Detection,
    pub status: RuntimeStatus,
    pub counts: CapabilityCounts,
}

#[derive(Serialize, Default, Type)]
pub struct CapabilityCounts {
    pub profiles: Option<u32>,
    pub providers: Option<u32>,
    pub models: Option<u32>,
    pub plugins: Option<u32>,
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
}

// A newer DeepMate release found on GitHub, if any. `None` means the current
// version is the latest, or the check could not complete (offline, rate
// limited, ...) — an update check must fail quietly, never block the UI.
#[derive(Serialize, Type)]
pub struct UpdateInfo {
    pub current_version: String,
    pub latest_version: String,
    pub url: String,
    pub published_at: String,
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

// ---- Command plumbing ----

// The capability gate every adapter-backed command passes through: reject
// with the same wording the CLI uses when the active adapter does not declare
// the capability.
fn require_capability(
    adapter: &dyn HarnessAdapter,
    supported: bool,
    what: &str,
) -> Result<(), String> {
    if supported {
        Ok(())
    } else {
        Err(format!(
            "adapter '{}' does not support {what}",
            adapter.metadata().id
        ))
    }
}

// Run an adapter operation, record it in the action history and normalize the
// error to the human-readable string the frontend surfaces.
async fn run_action(
    state: &AppState,
    action: &'static str,
    op: impl std::future::Future<Output = CoreResult<()>>,
) -> Result<(), String> {
    let adapter = state.adapter.as_ref();
    match op.await {
        Ok(()) => {
            record_action(&state.layout, &adapter.metadata().id, action.to_string());
            Ok(())
        }
        Err(e) => Err(format!("{e}")),
    }
}

// ---- Overview ----

#[tauri::command]
#[specta::specta]
pub async fn refresh_all(app: AppHandle) -> Result<Overview, String> {
    let state = app.state::<AppState>();
    let adapter = state.adapter.as_ref();
    let capabilities = adapter.capabilities();

    let detection = adapter
        .detect()
        .await
        .map_err(|e| format!("detect failed: {e}"))?;
    let status = adapter
        .status()
        .await
        .map_err(|e| format!("status failed: {e}"))?;
    let counts = CapabilityCounts {
        profiles: gated_count(capabilities.profiles, adapter.profiles()).await,
        providers: gated_count(capabilities.providers, adapter.providers()).await,
        models: gated_count(capabilities.models, adapter.models()).await,
        plugins: gated_count(capabilities.plugins, adapter.plugins()).await,
    };
    Ok(Overview {
        detection,
        status,
        counts,
    })
}

async fn gated_count<T>(
    supported: bool,
    list: impl std::future::Future<Output = CoreResult<Vec<T>>>,
) -> Option<u32> {
    if !supported {
        return None;
    }
    list.await.ok().map(|items| items.len() as u32)
}

// ---- Runtime ----

#[tauri::command]
#[specta::specta]
pub async fn runtime_start(app: AppHandle) -> Result<(), String> {
    runtime_op(&app, RuntimeOp::Start, "desktop.runtime.start").await
}

#[tauri::command]
#[specta::specta]
pub async fn runtime_stop(app: AppHandle) -> Result<(), String> {
    runtime_op(&app, RuntimeOp::Stop, "desktop.runtime.stop").await
}

#[tauri::command]
#[specta::specta]
pub async fn runtime_restart(app: AppHandle) -> Result<(), String> {
    runtime_op(&app, RuntimeOp::Restart, "desktop.runtime.restart").await
}

enum RuntimeOp {
    Start,
    Stop,
    Restart,
}

async fn runtime_op(app: &AppHandle, op: RuntimeOp, action: &'static str) -> Result<(), String> {
    let state = app.state::<AppState>();
    let adapter = state.adapter.as_ref();
    require_capability(adapter, adapter.capabilities().runtime, "runtime control")?;
    let result = match op {
        RuntimeOp::Start => adapter.start().await,
        RuntimeOp::Stop => adapter.stop().await,
        RuntimeOp::Restart => adapter.restart().await,
    };
    match result {
        Ok(()) => {
            record_action(&state.layout, &adapter.metadata().id, action.to_string());
            Ok(())
        }
        Err(e) => Err(format!("{e}")),
    }
}

#[tauri::command]
#[specta::specta]
pub async fn open_harness(app: AppHandle) -> Result<(), String> {
    let state = app.state::<AppState>();
    run_action(&state, "desktop.open", state.adapter.open_ui()).await
}

#[tauri::command]
#[specta::specta]
pub async fn run_doctor(app: AppHandle) -> Result<DoctorReport, String> {
    let state = app.state::<AppState>();
    let adapter = state.adapter.as_ref();
    match adapter.doctor().await {
        Ok(report) => {
            record_action(
                &state.layout,
                &adapter.metadata().id,
                "desktop.doctor".to_string(),
            );
            Ok(report)
        }
        Err(e) => Err(format!("{e}")),
    }
}

// ---- Inventory ----

#[tauri::command]
#[specta::specta]
pub async fn list_profiles(app: AppHandle) -> Result<Vec<Profile>, String> {
    let state = app.state::<AppState>();
    let adapter = state.adapter.as_ref();
    require_capability(adapter, adapter.capabilities().profiles, "profiles")?;
    adapter.profiles().await.map_err(|e| format!("{e}"))
}

#[tauri::command]
#[specta::specta]
pub async fn list_providers(app: AppHandle) -> Result<Vec<Provider>, String> {
    let state = app.state::<AppState>();
    let adapter = state.adapter.as_ref();
    require_capability(adapter, adapter.capabilities().providers, "providers")?;
    adapter.providers().await.map_err(|e| format!("{e}"))
}

#[tauri::command]
#[specta::specta]
pub async fn list_models(app: AppHandle) -> Result<Vec<Model>, String> {
    let state = app.state::<AppState>();
    let adapter = state.adapter.as_ref();
    require_capability(adapter, adapter.capabilities().models, "models")?;
    adapter.models().await.map_err(|e| format!("{e}"))
}

// ---- Configuration editing ----

#[tauri::command]
#[specta::specta]
pub async fn upsert_provider(app: AppHandle, provider: Provider) -> Result<(), String> {
    let state = app.state::<AppState>();
    let adapter = state.adapter.as_ref();
    require_capability(adapter, adapter.capabilities().providers, "providers")?;
    run_action(
        &state,
        "desktop.provider.set",
        adapter.upsert_provider(provider),
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn remove_provider(app: AppHandle, id: String) -> Result<(), String> {
    let state = app.state::<AppState>();
    let adapter = state.adapter.as_ref();
    require_capability(adapter, adapter.capabilities().providers, "providers")?;
    run_action(
        &state,
        "desktop.provider.remove",
        adapter.remove_provider(&id),
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn upsert_model(app: AppHandle, provider: String, model: Model) -> Result<(), String> {
    let state = app.state::<AppState>();
    let adapter = state.adapter.as_ref();
    require_capability(adapter, adapter.capabilities().models, "models")?;
    run_action(
        &state,
        "desktop.model.set",
        adapter.upsert_model(&provider, model),
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn remove_model(app: AppHandle, provider: String, id: String) -> Result<(), String> {
    let state = app.state::<AppState>();
    let adapter = state.adapter.as_ref();
    require_capability(adapter, adapter.capabilities().models, "models")?;
    run_action(
        &state,
        "desktop.model.remove",
        adapter.remove_model(&provider, &id),
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn create_profile(app: AppHandle, name: String) -> Result<(), String> {
    let state = app.state::<AppState>();
    let adapter = state.adapter.as_ref();
    require_capability(adapter, adapter.capabilities().profiles, "profiles")?;
    run_action(
        &state,
        "desktop.profile.create",
        adapter.create_profile(&name),
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn remove_profile(app: AppHandle, name: String) -> Result<(), String> {
    let state = app.state::<AppState>();
    let adapter = state.adapter.as_ref();
    require_capability(adapter, adapter.capabilities().profiles, "profiles")?;
    run_action(
        &state,
        "desktop.profile.remove",
        adapter.remove_profile(&name),
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn list_plugins(app: AppHandle) -> Result<Vec<Plugin>, String> {
    let state = app.state::<AppState>();
    let adapter = state.adapter.as_ref();
    require_capability(adapter, adapter.capabilities().plugins, "plugins")?;
    adapter.plugins().await.map_err(|e| format!("{e}"))
}

// ---- Plugins ----

#[tauri::command]
#[specta::specta]
pub async fn plugin_install(app: AppHandle, profile: String, spec: String) -> Result<(), String> {
    let state = app.state::<AppState>();
    let adapter = state.adapter.as_ref();
    require_capability(adapter, adapter.capabilities().plugins, "plugins")?;
    run_action(
        &state,
        "desktop.plugin.install",
        adapter.install_plugin(&profile, &spec),
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn plugin_remove(app: AppHandle, profile: String, id: String) -> Result<(), String> {
    let state = app.state::<AppState>();
    let adapter = state.adapter.as_ref();
    require_capability(adapter, adapter.capabilities().plugins, "plugins")?;
    run_action(
        &state,
        "desktop.plugin.remove",
        adapter.remove_plugin(&profile, &id),
    )
    .await
}

#[tauri::command]
#[specta::specta]
pub async fn plugin_update(app: AppHandle, profile: String, id: String) -> Result<(), String> {
    let state = app.state::<AppState>();
    let adapter = state.adapter.as_ref();
    require_capability(adapter, adapter.capabilities().plugins, "plugins")?;
    run_action(
        &state,
        "desktop.plugin.update",
        adapter.update_plugin(&profile, Some(&id)),
    )
    .await
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
    let adapter = state.adapter.as_ref();
    require_capability(adapter, adapter.capabilities().plugins, "plugins")?;

    let (tx, mut rx) = tokio::sync::mpsc::channel::<PluginOpEvent>(64);
    let adapter = Arc::clone(&state.adapter);
    let layout = state.layout.clone();
    let adapter_id = adapter.metadata().id.clone();
    let action = match kind {
        PluginOpKind::Install => "desktop.plugin.install",
        PluginOpKind::Remove => "desktop.plugin.remove",
        PluginOpKind::Update => "desktop.plugin.update",
    };

    // Forward events from the adapter's stream to the webview channel.
    let forwarder = tauri::async_runtime::spawn(async move {
        while let Some(event) = rx.recv().await {
            if channel.send(event).is_err() {
                break;
            }
        }
    });

    let result = adapter
        .stream_plugin_op(&profile, kind, Some(&target), tx)
        .await;
    // The forwarder drains the channel and exits on its own once the sender
    // is dropped; no explicit join needed.
    std::mem::drop(forwarder);
    match result {
        Ok(()) => {
            record_action(&layout, &adapter_id, action.to_string());
            Ok(())
        }
        Err(e) => Err(format!("{e}")),
    }
}

// ---- Market ----

#[tauri::command]
#[specta::specta]
pub async fn list_market_sources(app: AppHandle) -> Result<Vec<MarketSourceInfo>, String> {
    let state = app.state::<AppState>();
    let adapter = state.adapter.as_ref();
    require_capability(adapter, adapter.capabilities().marketplace, "marketplace")?;
    adapter.market_sources().await.map_err(|e| format!("{e}"))
}

#[tauri::command]
#[specta::specta]
pub async fn market_search(app: AppHandle, query: String) -> Result<Vec<MarketEntry>, String> {
    let state = app.state::<AppState>();
    let adapter = state.adapter.as_ref();
    require_capability(adapter, adapter.capabilities().marketplace, "marketplace")?;
    adapter
        .search_plugins(&query)
        .await
        .map_err(|e| format!("{e}"))
}

// Check a market package's compatibility with the detected harness before an
// installation. Capability-gated on marketplace, like the CLI's
// `deepmate plugin check`.
#[tauri::command]
#[specta::specta]
pub async fn plugin_check(app: AppHandle, spec: String) -> Result<CompatReport, String> {
    let state = app.state::<AppState>();
    let adapter = state.adapter.as_ref();
    require_capability(
        adapter,
        adapter.capabilities().marketplace,
        "compatibility checks",
    )?;
    adapter
        .plugin_compat(&spec)
        .await
        .map_err(|e| format!("{e}"))
}

// ---- Snapshots ----

#[tauri::command]
#[specta::specta]
pub async fn snapshot_export(app: AppHandle, name: String) -> Result<(), String> {
    let state = app.state::<AppState>();
    let adapter = state.adapter.as_ref();
    require_capability(adapter, adapter.capabilities().snapshots, "snapshots")?;
    let store = deepmate_core::SnapshotStore::new(state.layout.snapshots_dir());
    match deepmate_core::Snapshot::capture(adapter).await {
        Ok(snapshot) => match store.save(&name, &snapshot) {
            Ok(()) => {
                record_action(
                    &state.layout,
                    &adapter.metadata().id,
                    "desktop.snapshot.export".to_string(),
                );
                Ok(())
            }
            Err(e) => Err(format!("{e}")),
        },
        Err(e) => Err(format!("{e}")),
    }
}

#[tauri::command]
#[specta::specta]
pub async fn snapshot_import(app: AppHandle, name: String) -> Result<(), String> {
    let state = app.state::<AppState>();
    let adapter = state.adapter.as_ref();
    require_capability(adapter, adapter.capabilities().snapshots, "snapshots")?;
    let store = deepmate_core::SnapshotStore::new(state.layout.snapshots_dir());
    match store.load(&name) {
        Ok(snapshot) => match snapshot.apply(adapter).await {
            Ok(_) => {
                record_action(
                    &state.layout,
                    &adapter.metadata().id,
                    "desktop.snapshot.import".to_string(),
                );
                Ok(())
            }
            Err(e) => Err(format!("{e}")),
        },
        Err(e) => Err(format!("{e}")),
    }
}

#[tauri::command]
#[specta::specta]
pub async fn snapshot_list(app: AppHandle) -> Result<Vec<String>, String> {
    let state = app.state::<AppState>();
    let adapter = state.adapter.as_ref();
    require_capability(adapter, adapter.capabilities().snapshots, "snapshots")?;
    let store = deepmate_core::SnapshotStore::new(state.layout.snapshots_dir());
    store.list().map_err(|e| format!("{e}"))
}

#[tauri::command]
#[specta::specta]
pub async fn snapshot_delete(app: AppHandle, name: String) -> Result<(), String> {
    let state = app.state::<AppState>();
    let adapter = state.adapter.as_ref();
    require_capability(adapter, adapter.capabilities().snapshots, "snapshots")?;
    let store = deepmate_core::SnapshotStore::new(state.layout.snapshots_dir());
    store.delete(&name).map_err(|e| format!("{e}"))
}

// ---- Config (language / theme / preferences) ----

#[tauri::command]
#[specta::specta]
pub async fn set_language(app: AppHandle, language: String) -> Result<(), String> {
    if !matches!(language.as_str(), "en" | "zh") {
        return Err(format!("unsupported language: {language}"));
    }
    let state = app.state::<AppState>();
    let mut config = deepmate_core::data::Config::load(&state.layout.config_path())
        .map_err(|e| format!("{e}"))?;
    config.general.language = language;
    config
        .save(&state.layout.config_path())
        .map_err(|e| format!("{e}"))
}

#[tauri::command]
#[specta::specta]
pub async fn set_notify_updates(app: AppHandle, enabled: bool) -> Result<(), String> {
    let state = app.state::<AppState>();
    let mut config = deepmate_core::data::Config::load(&state.layout.config_path())
        .map_err(|e| format!("{e}"))?;
    config.general.notify_updates = enabled;
    config
        .save(&state.layout.config_path())
        .map_err(|e| format!("{e}"))
}

#[tauri::command]
#[specta::specta]
pub async fn set_theme(app: AppHandle, theme: String) -> Result<(), String> {
    if !matches!(theme.as_str(), "system" | "light" | "dark") {
        return Err(format!("unsupported theme: {theme}"));
    }
    let state = app.state::<AppState>();
    let mut config = deepmate_core::data::Config::load(&state.layout.config_path())
        .map_err(|e| format!("{e}"))?;
    config.ui.theme = theme;
    config
        .save(&state.layout.config_path())
        .map_err(|e| format!("{e}"))
}

#[tauri::command]
#[specta::specta]
pub async fn set_close_to_tray(app: AppHandle, enabled: bool) -> Result<(), String> {
    let state = app.state::<AppState>();
    let mut config = deepmate_core::data::Config::load(&state.layout.config_path())
        .map_err(|e| format!("{e}"))?;
    config.ui.close_to_tray = enabled;
    config
        .save(&state.layout.config_path())
        .map_err(|e| format!("{e}"))?;
    state.close_to_tray.store(enabled, Ordering::Relaxed);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn set_check_updates(app: AppHandle, enabled: bool) -> Result<(), String> {
    let state = app.state::<AppState>();
    let mut config = deepmate_core::data::Config::load(&state.layout.config_path())
        .map_err(|e| format!("{e}"))?;
    config.general.check_updates = enabled;
    config
        .save(&state.layout.config_path())
        .map_err(|e| format!("{e}"))
}

#[tauri::command]
#[specta::specta]
pub async fn get_config(app: AppHandle) -> Result<UiPrefs, String> {
    let state = app.state::<AppState>();
    let config = deepmate_core::data::Config::load(&state.layout.config_path())
        .map_err(|e| format!("{e}"))?;
    Ok(UiPrefs {
        language: config.general.language,
        theme: config.ui.theme,
        check_updates: config.general.check_updates,
        notify_updates: config.general.notify_updates,
        close_to_tray: config.ui.close_to_tray,
    })
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
    let config = deepmate_core::data::Config::load(&state.layout.config_path())
        .map_err(|e| format!("{e}"))?;
    deepmate_core::ConfigBackup::capture(&config)
        .save(&path)
        .map_err(|e| format!("{e}"))?;
    record_action(&state.layout, "app", "desktop.config.export".to_string());
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
    let backup = deepmate_core::ConfigBackup::load(&path).map_err(|e| format!("{e}"))?;
    backup
        .config
        .save(&state.layout.config_path())
        .map_err(|e| format!("{e}"))?;
    record_action(&state.layout, "app", "desktop.config.import".to_string());
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
    let mut config = deepmate_core::data::Config::load(&state.layout.config_path())
        .map_err(|e| format!("{e}"))?;
    config.general.auto_start = enabled;
    config
        .save(&state.layout.config_path())
        .map_err(|e| format!("{e}"))
}

// ---- Update check ----

const UPDATE_API_URL: &str =
    "https://api.github.com/repos/realchendahuang/DeepMate/releases/latest";
const UPDATE_TIMEOUT_SECS: u64 = 10;

// Query the GitHub releases API for a newer DeepMate release. Shared by the
// `check_update` command, the tray/startup notification and the install
// flow. `None` means the current version is the latest, or the check could
// not complete (offline, rate limited, ...) — an update check must fail
// quietly, never block the UI.
//
// `DEEPMATE_UPDATE_API_URL` overrides the endpoint so the flow can be
// exercised against a test release.
pub(crate) async fn latest_release() -> Option<UpdateInfo> {
    let client = reqwest::Client::builder()
        .user_agent(format!("DeepMate/{}", env!("CARGO_PKG_VERSION")))
        .timeout(std::time::Duration::from_secs(UPDATE_TIMEOUT_SECS))
        .build()
        .ok()?;

    #[derive(Deserialize)]
    struct Release {
        tag_name: String,
        html_url: String,
        published_at: String,
    }

    let api =
        std::env::var("DEEPMATE_UPDATE_API_URL").unwrap_or_else(|_| UPDATE_API_URL.to_string());
    let release: Release = match client.get(api).send().await {
        Ok(response) if response.status().is_success() => match response.json().await {
            Ok(release) => release,
            Err(err) => {
                tracing::warn!("update check: invalid response: {err}");
                return None;
            }
        },
        Ok(response) => {
            tracing::warn!("update check: GitHub returned {}", response.status());
            return None;
        }
        Err(err) => {
            tracing::warn!("update check failed: {err}");
            return None;
        }
    };

    let current = env!("CARGO_PKG_VERSION");
    if !deepmate_core::is_newer_version(&release.tag_name, current) {
        return None;
    }
    Some(UpdateInfo {
        current_version: current.to_string(),
        latest_version: release.tag_name.trim_start_matches('v').to_string(),
        url: release.html_url,
        published_at: release.published_at,
    })
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
pub async fn update_install(app: AppHandle) -> Result<UpdateInstallOutcome, String> {
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
    let bytes = client
        .get(&dmg.url)
        .send()
        .await
        .map_err(|e| format!("failed to download the update: {e}"))?
        .error_for_status()
        .map_err(|e| format!("failed to download the update: {e}"))?
        .bytes()
        .await
        .map_err(|e| format!("failed to download the update: {e}"))?;
    std::fs::write(&dmg_path, &bytes).map_err(|e| format!("failed to write the update: {e}"))?;

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
    let expected = deepmate_core::parse_checksum_file(&sum_text)
        .ok_or_else(|| "the published checksum file is invalid".to_string())?;
    deepmate_core::verify_sha256(&dmg_path, &expected)
        .map_err(|e| format!("the downloaded update failed verification: {e}"))?;

    open_with_platform(&dmg_path).map_err(|e| format!("failed to open the installer: {e}"))?;
    let state = app.state::<AppState>();
    record_action(&state.layout, "app", "desktop.update.install".to_string());
    Ok(UpdateInstallOutcome {
        status: "downloaded".to_string(),
        path: Some(dmg_path.display().to_string()),
        version: Some(version),
    })
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
pub async fn check_update() -> Result<Option<UpdateInfo>, String> {
    Ok(latest_release().await)
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
