// DeepMate desktop control center (Tauri).
//
// A thin Tauri shell over the shared core: the React frontend renders state
// delivered by the commands and forwards user intent back as command calls.
// Business rules live in the core and the commands, never in the frontend.

mod commands;

use std::sync::Arc;

use deepmate_app::{build_registry, init_tracing, load_config_or_default};
use deepmate_platform::{PlatformService, SystemPlatform};
use tauri::Manager;

use commands::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let cli = parse_cli();

    let platform = SystemPlatform;
    let data_dir = match &cli.data_dir {
        Some(dir) => dir.clone(),
        None => platform.data_dir().expect("failed to resolve data dir"),
    };
    let layout = deepmate_core::data::DataLayout::new(data_dir);
    layout
        .ensure()
        .expect("failed to initialize the DeepMate data directory");

    let config = load_config_or_default(&layout);
    let _guard = init_tracing(&layout.logs_dir());

    let registry = build_registry(&cli.adapter, &layout).expect("failed to build adapter registry");
    let adapter = registry
        .into_adapter(&cli.adapter)
        .unwrap_or_else(|| panic!("adapter not found: {}", cli.adapter));

    let state = AppState {
        adapter: Arc::from(adapter),
        layout,
    };
    let close_to_tray = config.ui.close_to_tray;

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            commands::refresh_all,
            commands::runtime_start,
            commands::runtime_stop,
            commands::runtime_restart,
            commands::open_harness,
            commands::run_doctor,
            commands::list_profiles,
            commands::list_providers,
            commands::list_models,
            commands::list_plugins,
            commands::plugin_install,
            commands::plugin_remove,
            commands::plugin_update,
            commands::list_market_sources,
            commands::market_search,
            commands::set_language,
            commands::set_theme,
            commands::get_config,
        ])
        .setup(move |app| {
            // Close-to-tray: hide the window on close requests.
            if close_to_tray {
                if let Some(window) = app.get_webview_window("main") {
                    window.on_window_event(|event| {
                        if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                            api.prevent_close();
                        }
                    });
                }
            }
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running the DeepMate application");
}

// The desktop app accepts the same --adapter / --data-dir flags as the CLI.
struct Cli {
    adapter: String,
    data_dir: Option<std::path::PathBuf>,
}

fn parse_cli() -> Cli {
    let mut adapter = "deepseek-harness".to_string();
    let mut data_dir = None;
    let mut args = std::env::args().skip(1);
    while let Some(arg) = args.next() {
        match arg.as_str() {
            "--adapter" => {
                if let Some(v) = args.next() {
                    adapter = v;
                }
            }
            "--data-dir" => {
                if let Some(v) = args.next() {
                    data_dir = Some(std::path::PathBuf::from(v));
                }
            }
            _ => {}
        }
    }
    Cli { adapter, data_dir }
}
