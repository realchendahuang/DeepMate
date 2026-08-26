// Tauri command surface for the DeepMate desktop shell.
//
// These commands mirror the old Slint bridge's UiCommand/UiEvent surface
// (see git history: apps/desktop/src/bridge.rs). The core models are already
// Serialize/Deserialize, so they flow straight back to the React frontend as
// JSON. Capability gating and action-history recording mirror the CLI.
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
    DoctorReport, MarketEntry, MarketSourceInfo, Model, Plugin, Profile, Provider, RuntimeStatus,
};
use deepmate_core::CoreResult;
use deepmate_platform::{PlatformService, SystemPlatform};
use serde::{Deserialize, Serialize};
use tauri::State;
use tauri_plugin_autostart::ManagerExt;

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
#[derive(Serialize)]
pub struct Overview {
    pub detection: deepmate_core::adapter::Detection,
    pub status: RuntimeStatus,
    pub counts: CapabilityCounts,
}

#[derive(Serialize, Default)]
pub struct CapabilityCounts {
    pub profiles: Option<usize>,
    pub providers: Option<usize>,
    pub models: Option<usize>,
    pub plugins: Option<usize>,
}

// ---- Overview ----

#[tauri::command]
pub async fn refresh_all(state: State<'_, AppState>) -> Result<Overview, String> {
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
) -> Option<usize> {
    if !supported {
        return None;
    }
    list.await.ok().map(|items| items.len())
}

// ---- Runtime ----

#[tauri::command]
pub async fn runtime_start(state: State<'_, AppState>) -> Result<(), String> {
    runtime_op(&state, RuntimeOp::Start, "desktop.runtime.start").await
}

#[tauri::command]
pub async fn runtime_stop(state: State<'_, AppState>) -> Result<(), String> {
    runtime_op(&state, RuntimeOp::Stop, "desktop.runtime.stop").await
}

#[tauri::command]
pub async fn runtime_restart(state: State<'_, AppState>) -> Result<(), String> {
    runtime_op(&state, RuntimeOp::Restart, "desktop.runtime.restart").await
}

enum RuntimeOp {
    Start,
    Stop,
    Restart,
}

async fn runtime_op(state: &AppState, op: RuntimeOp, action: &'static str) -> Result<(), String> {
    let adapter = state.adapter.as_ref();
    if !adapter.capabilities().runtime {
        return Err(format!(
            "adapter '{}' does not support runtime control",
            adapter.metadata().id
        ));
    }
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
pub async fn open_harness(state: State<'_, AppState>) -> Result<(), String> {
    let adapter = state.adapter.as_ref();
    match adapter.open_ui().await {
        Ok(()) => {
            record_action(
                &state.layout,
                &adapter.metadata().id,
                "desktop.open".to_string(),
            );
            Ok(())
        }
        Err(e) => Err(format!("{e}")),
    }
}

#[tauri::command]
pub async fn run_doctor(state: State<'_, AppState>) -> Result<DoctorReport, String> {
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
pub async fn list_profiles(state: State<'_, AppState>) -> Result<Vec<Profile>, String> {
    let adapter = state.adapter.as_ref();
    if !adapter.capabilities().profiles {
        return Err(format!(
            "adapter '{}' does not support profiles",
            adapter.metadata().id
        ));
    }
    adapter.profiles().await.map_err(|e| format!("{e}"))
}

#[tauri::command]
pub async fn list_providers(state: State<'_, AppState>) -> Result<Vec<Provider>, String> {
    let adapter = state.adapter.as_ref();
    if !adapter.capabilities().providers {
        return Err(format!(
            "adapter '{}' does not support providers",
            adapter.metadata().id
        ));
    }
    adapter.providers().await.map_err(|e| format!("{e}"))
}

#[tauri::command]
pub async fn list_models(state: State<'_, AppState>) -> Result<Vec<Model>, String> {
    let adapter = state.adapter.as_ref();
    if !adapter.capabilities().models {
        return Err(format!(
            "adapter '{}' does not support models",
            adapter.metadata().id
        ));
    }
    adapter.models().await.map_err(|e| format!("{e}"))
}

// ---- Configuration editing ----

#[tauri::command]
pub async fn upsert_provider(state: State<'_, AppState>, provider: Provider) -> Result<(), String> {
    let adapter = state.adapter.as_ref();
    if !adapter.capabilities().providers {
        return Err(format!(
            "adapter '{}' does not support providers",
            adapter.metadata().id
        ));
    }
    match adapter.upsert_provider(provider).await {
        Ok(()) => {
            record_action(
                &state.layout,
                &adapter.metadata().id,
                "desktop.provider.set".to_string(),
            );
            Ok(())
        }
        Err(e) => Err(format!("{e}")),
    }
}

#[tauri::command]
pub async fn remove_provider(state: State<'_, AppState>, id: String) -> Result<(), String> {
    let adapter = state.adapter.as_ref();
    if !adapter.capabilities().providers {
        return Err(format!(
            "adapter '{}' does not support providers",
            adapter.metadata().id
        ));
    }
    match adapter.remove_provider(&id).await {
        Ok(()) => {
            record_action(
                &state.layout,
                &adapter.metadata().id,
                "desktop.provider.remove".to_string(),
            );
            Ok(())
        }
        Err(e) => Err(format!("{e}")),
    }
}

#[tauri::command]
pub async fn upsert_model(
    state: State<'_, AppState>,
    provider: String,
    model: Model,
) -> Result<(), String> {
    let adapter = state.adapter.as_ref();
    if !adapter.capabilities().models {
        return Err(format!(
            "adapter '{}' does not support models",
            adapter.metadata().id
        ));
    }
    match adapter.upsert_model(&provider, model).await {
        Ok(()) => {
            record_action(
                &state.layout,
                &adapter.metadata().id,
                "desktop.model.set".to_string(),
            );
            Ok(())
        }
        Err(e) => Err(format!("{e}")),
    }
}

#[tauri::command]
pub async fn remove_model(
    state: State<'_, AppState>,
    provider: String,
    id: String,
) -> Result<(), String> {
    let adapter = state.adapter.as_ref();
    if !adapter.capabilities().models {
        return Err(format!(
            "adapter '{}' does not support models",
            adapter.metadata().id
        ));
    }
    match adapter.remove_model(&provider, &id).await {
        Ok(()) => {
            record_action(
                &state.layout,
                &adapter.metadata().id,
                "desktop.model.remove".to_string(),
            );
            Ok(())
        }
        Err(e) => Err(format!("{e}")),
    }
}

#[tauri::command]
pub async fn create_profile(state: State<'_, AppState>, name: String) -> Result<(), String> {
    let adapter = state.adapter.as_ref();
    if !adapter.capabilities().profiles {
        return Err(format!(
            "adapter '{}' does not support profiles",
            adapter.metadata().id
        ));
    }
    match adapter.create_profile(&name).await {
        Ok(()) => {
            record_action(
                &state.layout,
                &adapter.metadata().id,
                "desktop.profile.create".to_string(),
            );
            Ok(())
        }
        Err(e) => Err(format!("{e}")),
    }
}

#[tauri::command]
pub async fn remove_profile(state: State<'_, AppState>, name: String) -> Result<(), String> {
    let adapter = state.adapter.as_ref();
    if !adapter.capabilities().profiles {
        return Err(format!(
            "adapter '{}' does not support profiles",
            adapter.metadata().id
        ));
    }
    match adapter.remove_profile(&name).await {
        Ok(()) => {
            record_action(
                &state.layout,
                &adapter.metadata().id,
                "desktop.profile.remove".to_string(),
            );
            Ok(())
        }
        Err(e) => Err(format!("{e}")),
    }
}

#[tauri::command]
pub async fn list_plugins(state: State<'_, AppState>) -> Result<Vec<Plugin>, String> {
    let adapter = state.adapter.as_ref();
    if !adapter.capabilities().plugins {
        return Err(format!(
            "adapter '{}' does not support plugins",
            adapter.metadata().id
        ));
    }
    adapter.plugins().await.map_err(|e| format!("{e}"))
}

// ---- Plugins ----

#[tauri::command]
pub async fn plugin_install(
    state: State<'_, AppState>,
    profile: String,
    spec: String,
) -> Result<(), String> {
    let adapter = state.adapter.as_ref();
    if !adapter.capabilities().plugins {
        return Err(format!(
            "adapter '{}' does not support plugins",
            adapter.metadata().id
        ));
    }
    match adapter.install_plugin(&profile, &spec).await {
        Ok(()) => {
            record_action(
                &state.layout,
                &adapter.metadata().id,
                "desktop.plugin.install".to_string(),
            );
            Ok(())
        }
        Err(e) => Err(format!("{e}")),
    }
}

#[tauri::command]
pub async fn plugin_remove(
    state: State<'_, AppState>,
    profile: String,
    id: String,
) -> Result<(), String> {
    let adapter = state.adapter.as_ref();
    if !adapter.capabilities().plugins {
        return Err(format!(
            "adapter '{}' does not support plugins",
            adapter.metadata().id
        ));
    }
    match adapter.remove_plugin(&profile, &id).await {
        Ok(()) => {
            record_action(
                &state.layout,
                &adapter.metadata().id,
                "desktop.plugin.remove".to_string(),
            );
            Ok(())
        }
        Err(e) => Err(format!("{e}")),
    }
}

#[tauri::command]
pub async fn plugin_update(
    state: State<'_, AppState>,
    profile: String,
    id: String,
) -> Result<(), String> {
    let adapter = state.adapter.as_ref();
    if !adapter.capabilities().plugins {
        return Err(format!(
            "adapter '{}' does not support plugins",
            adapter.metadata().id
        ));
    }
    match adapter.update_plugin(&profile, Some(&id)).await {
        Ok(()) => {
            record_action(
                &state.layout,
                &adapter.metadata().id,
                "desktop.plugin.update".to_string(),
            );
            Ok(())
        }
        Err(e) => Err(format!("{e}")),
    }
}

// ---- Market ----

#[tauri::command]
pub async fn list_market_sources(
    state: State<'_, AppState>,
) -> Result<Vec<MarketSourceInfo>, String> {
    let adapter = state.adapter.as_ref();
    if !adapter.capabilities().marketplace {
        return Err(format!(
            "adapter '{}' does not support marketplace",
            adapter.metadata().id
        ));
    }
    adapter.market_sources().await.map_err(|e| format!("{e}"))
}

#[tauri::command]
pub async fn market_search(
    state: State<'_, AppState>,
    query: String,
) -> Result<Vec<MarketEntry>, String> {
    let adapter = state.adapter.as_ref();
    if !adapter.capabilities().marketplace {
        return Err(format!(
            "adapter '{}' does not support marketplace",
            adapter.metadata().id
        ));
    }
    adapter
        .search_plugins(&query)
        .await
        .map_err(|e| format!("{e}"))
}

// ---- Snapshots ----

#[tauri::command]
pub async fn snapshot_export(state: State<'_, AppState>, name: String) -> Result<(), String> {
    let adapter = state.adapter.as_ref();
    if !adapter.capabilities().snapshots {
        return Err(format!(
            "adapter '{}' does not support snapshots",
            adapter.metadata().id
        ));
    }
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
pub async fn snapshot_import(state: State<'_, AppState>, name: String) -> Result<(), String> {
    let adapter = state.adapter.as_ref();
    if !adapter.capabilities().snapshots {
        return Err(format!(
            "adapter '{}' does not support snapshots",
            adapter.metadata().id
        ));
    }
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
pub async fn snapshot_list(state: State<'_, AppState>) -> Result<Vec<String>, String> {
    let adapter = state.adapter.as_ref();
    if !adapter.capabilities().snapshots {
        return Err(format!(
            "adapter '{}' does not support snapshots",
            adapter.metadata().id
        ));
    }
    let store = deepmate_core::SnapshotStore::new(state.layout.snapshots_dir());
    store.list().map_err(|e| format!("{e}"))
}

// ---- Config (language / theme / preferences) ----

#[tauri::command]
pub async fn set_language(state: State<'_, AppState>, language: String) -> Result<(), String> {
    if !matches!(language.as_str(), "en" | "zh") {
        return Err(format!("unsupported language: {language}"));
    }
    let mut config = deepmate_core::data::Config::load(&state.layout.config_path())
        .map_err(|e| format!("{e}"))?;
    config.general.language = language;
    config
        .save(&state.layout.config_path())
        .map_err(|e| format!("{e}"))
}

#[tauri::command]
pub async fn set_theme(state: State<'_, AppState>, theme: String) -> Result<(), String> {
    if !matches!(theme.as_str(), "system" | "light" | "dark") {
        return Err(format!("unsupported theme: {theme}"));
    }
    let mut config = deepmate_core::data::Config::load(&state.layout.config_path())
        .map_err(|e| format!("{e}"))?;
    config.ui.theme = theme;
    config
        .save(&state.layout.config_path())
        .map_err(|e| format!("{e}"))
}

#[tauri::command]
pub async fn set_close_to_tray(state: State<'_, AppState>, enabled: bool) -> Result<(), String> {
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
pub async fn set_check_updates(state: State<'_, AppState>, enabled: bool) -> Result<(), String> {
    let mut config = deepmate_core::data::Config::load(&state.layout.config_path())
        .map_err(|e| format!("{e}"))?;
    config.general.check_updates = enabled;
    config
        .save(&state.layout.config_path())
        .map_err(|e| format!("{e}"))
}

// The persisted preferences so the frontend can restore them on startup and
// keep the UI in sync after a change.
#[derive(Serialize)]
pub struct UiPrefs {
    pub language: String,
    pub theme: String,
    pub check_updates: bool,
    pub close_to_tray: bool,
}

#[tauri::command]
pub async fn get_config(state: State<'_, AppState>) -> Result<UiPrefs, String> {
    let config = deepmate_core::data::Config::load(&state.layout.config_path())
        .map_err(|e| format!("{e}"))?;
    Ok(UiPrefs {
        language: config.general.language,
        theme: config.ui.theme,
        check_updates: config.general.check_updates,
        close_to_tray: config.ui.close_to_tray,
    })
}

// ---- Auto-start ----

// Whether the OS is registered to start DeepMate at login. The operating
// system is the source of truth here; config.general.auto_start only mirrors
// it after a toggle.
#[tauri::command]
pub async fn autostart_get(app: tauri::AppHandle) -> Result<bool, String> {
    app.autolaunch()
        .is_enabled()
        .map_err(|e| format!("failed to read auto-start state: {e}"))
}

#[tauri::command]
pub async fn autostart_set(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    enabled: bool,
) -> Result<(), String> {
    let manager = app.autolaunch();
    let result = if enabled {
        manager.enable()
    } else {
        manager.disable()
    };
    result.map_err(|e| format!("failed to update auto-start state: {e}"))?;

    let mut config = deepmate_core::data::Config::load(&state.layout.config_path())
        .map_err(|e| format!("{e}"))?;
    config.general.auto_start = enabled;
    config
        .save(&state.layout.config_path())
        .map_err(|e| format!("{e}"))
}

// ---- Update check ----

// A newer DeepMate release found on GitHub, if any. `None` means the current
// version is the latest, or the check could not complete (offline, rate
// limited, ...) — an update check must fail quietly, never block the UI.
#[derive(Serialize)]
pub struct UpdateInfo {
    pub current_version: String,
    pub latest_version: String,
    pub url: String,
    pub published_at: String,
}

const UPDATE_API_URL: &str =
    "https://api.github.com/repos/realchendahuang/DeepMate/releases/latest";
const UPDATE_TIMEOUT_SECS: u64 = 10;

#[tauri::command]
pub async fn check_update() -> Result<Option<UpdateInfo>, String> {
    let client = reqwest::Client::builder()
        .user_agent(format!("DeepMate/{}", env!("CARGO_PKG_VERSION")))
        .timeout(std::time::Duration::from_secs(UPDATE_TIMEOUT_SECS))
        .build()
        .map_err(|e| format!("failed to build HTTP client: {e}"))?;

    #[derive(Deserialize)]
    struct Release {
        tag_name: String,
        html_url: String,
        published_at: String,
    }

    let release: Release = match client.get(UPDATE_API_URL).send().await {
        Ok(response) if response.status().is_success() => match response.json().await {
            Ok(release) => release,
            Err(err) => {
                tracing::warn!("update check: invalid response: {err}");
                return Ok(None);
            }
        },
        Ok(response) => {
            tracing::warn!("update check: GitHub returned {}", response.status());
            return Ok(None);
        }
        Err(err) => {
            tracing::warn!("update check failed: {err}");
            return Ok(None);
        }
    };

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

// Open a URL in the system browser (used by the update banner). Only http(s)
// is accepted: this command is reachable from the webview.
#[tauri::command]
pub async fn open_url(url: String) -> Result<(), String> {
    if !url.starts_with("https://") && !url.starts_with("http://") {
        return Err("refusing to open non-http URL".to_string());
    }
    SystemPlatform.open_url(&url).map_err(|e| format!("{e}"))
}
