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

use std::sync::Arc;

use deepmate_app::record_action;
use deepmate_core::adapter::HarnessAdapter;
use deepmate_core::data::DataLayout;
use deepmate_core::model::{
    DoctorReport, MarketEntry, MarketSourceInfo, Model, Plugin, Profile, Provider, RuntimeStatus,
};
use deepmate_core::CoreResult;
use serde::Serialize;
use tauri::State;

// Shared application state: the active adapter and the data layout. The
// adapter is Send + Sync, so it can live behind an Arc in Tauri state and be
// called from async commands.
pub struct AppState {
    pub adapter: Arc<dyn HarnessAdapter>,
    pub layout: DataLayout,
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

    let detection = adapter.detect().await.map_err(|e| format!("detect failed: {e}"))?;
    let status = adapter.status().await.map_err(|e| format!("status failed: {e}"))?;
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

async fn gated_count<T>(supported: bool, list: impl std::future::Future<Output = CoreResult<Vec<T>>>) -> Option<usize> {
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
            record_action(&state.layout, &adapter.metadata().id, "desktop.open".to_string());
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
            record_action(&state.layout, &adapter.metadata().id, "desktop.doctor".to_string());
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
        return Err(format!("adapter '{}' does not support profiles", adapter.metadata().id));
    }
    adapter.profiles().await.map_err(|e| format!("{e}"))
}

#[tauri::command]
pub async fn list_providers(state: State<'_, AppState>) -> Result<Vec<Provider>, String> {
    let adapter = state.adapter.as_ref();
    if !adapter.capabilities().providers {
        return Err(format!("adapter '{}' does not support providers", adapter.metadata().id));
    }
    adapter.providers().await.map_err(|e| format!("{e}"))
}

#[tauri::command]
pub async fn list_models(state: State<'_, AppState>) -> Result<Vec<Model>, String> {
    let adapter = state.adapter.as_ref();
    if !adapter.capabilities().models {
        return Err(format!("adapter '{}' does not support models", adapter.metadata().id));
    }
    adapter.models().await.map_err(|e| format!("{e}"))
}

#[tauri::command]
pub async fn list_plugins(state: State<'_, AppState>) -> Result<Vec<Plugin>, String> {
    let adapter = state.adapter.as_ref();
    if !adapter.capabilities().plugins {
        return Err(format!("adapter '{}' does not support plugins", adapter.metadata().id));
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
        return Err(format!("adapter '{}' does not support plugins", adapter.metadata().id));
    }
    match adapter.install_plugin(&profile, &spec).await {
        Ok(()) => {
            record_action(&state.layout, &adapter.metadata().id, "desktop.plugin.install".to_string());
            Ok(())
        }
        Err(e) => Err(format!("{e}")),
    }
}

#[tauri::command]
pub async fn plugin_remove(state: State<'_, AppState>, profile: String, id: String) -> Result<(), String> {
    let adapter = state.adapter.as_ref();
    if !adapter.capabilities().plugins {
        return Err(format!("adapter '{}' does not support plugins", adapter.metadata().id));
    }
    match adapter.remove_plugin(&profile, &id).await {
        Ok(()) => {
            record_action(&state.layout, &adapter.metadata().id, "desktop.plugin.remove".to_string());
            Ok(())
        }
        Err(e) => Err(format!("{e}")),
    }
}

#[tauri::command]
pub async fn plugin_update(state: State<'_, AppState>, profile: String, id: String) -> Result<(), String> {
    let adapter = state.adapter.as_ref();
    if !adapter.capabilities().plugins {
        return Err(format!("adapter '{}' does not support plugins", adapter.metadata().id));
    }
    match adapter.update_plugin(&profile, Some(&id)).await {
        Ok(()) => {
            record_action(&state.layout, &adapter.metadata().id, "desktop.plugin.update".to_string());
            Ok(())
        }
        Err(e) => Err(format!("{e}")),
    }
}

// ---- Market ----

#[tauri::command]
pub async fn list_market_sources(state: State<'_, AppState>) -> Result<Vec<MarketSourceInfo>, String> {
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
pub async fn market_search(state: State<'_, AppState>, query: String) -> Result<Vec<MarketEntry>, String> {
    let adapter = state.adapter.as_ref();
    if !adapter.capabilities().marketplace {
        return Err(format!(
            "adapter '{}' does not support marketplace",
            adapter.metadata().id
        ));
    }
    adapter.search_plugins(&query).await.map_err(|e| format!("{e}"))
}

// ---- Config (language / theme) ----

#[tauri::command]
pub async fn set_language(state: State<'_, AppState>, language: String) -> Result<(), String> {
    if !matches!(language.as_str(), "en" | "zh") {
        return Err(format!("unsupported language: {language}"));
    }
    let mut config = deepmate_core::data::Config::load(&state.layout.config_path())
        .map_err(|e| format!("{e}"))?;
    config.general.language = language;
    config.save(&state.layout.config_path()).map_err(|e| format!("{e}"))
}

#[tauri::command]
pub async fn set_theme(state: State<'_, AppState>, theme: String) -> Result<(), String> {
    if !matches!(theme.as_str(), "system" | "light" | "dark") {
        return Err(format!("unsupported theme: {theme}"));
    }
    let mut config = deepmate_core::data::Config::load(&state.layout.config_path())
        .map_err(|e| format!("{e}"))?;
    config.ui.theme = theme;
    config.save(&state.layout.config_path()).map_err(|e| format!("{e}"))
}

// The persisted preferences (language / theme) so the frontend can restore them
// on startup and keep the UI in sync after a change.
#[derive(Serialize)]
pub struct UiPrefs {
    pub language: String,
    pub theme: String,
}

#[tauri::command]
pub async fn get_config(state: State<'_, AppState>) -> Result<UiPrefs, String> {
    let config = deepmate_core::data::Config::load(&state.layout.config_path())
        .map_err(|e| format!("{e}"))?;
    Ok(UiPrefs {
        language: config.general.language,
        theme: config.ui.theme,
    })
}
