// DeepMate desktop control center (Tauri).
//
// A thin Tauri shell over the shared core: the React frontend renders state
// delivered by the commands and forwards user intent back as command calls.
// Business rules live in the core and the commands, never in the frontend.

mod commands;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use deepmate_app::{build_registry, init_tracing, load_config_or_default};
use deepmate_platform::{PlatformService, SystemPlatform};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::{Manager, RunEvent};

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

    // Close-to-tray lives behind an atomic so the window close handler always
    // reads the current preference, not the value captured at startup.
    let close_to_tray = Arc::new(AtomicBool::new(config.ui.close_to_tray));
    let language = config.general.language.clone();

    let state = AppState {
        adapter: Arc::from(adapter),
        layout,
        close_to_tray: Arc::clone(&close_to_tray),
    };

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        // Auto-start registers the app with the OS login items; carry the
        // active adapter so a login-started instance manages the same harness
        // the user chose in the desktop session.
        .plugin(
            tauri_plugin_autostart::Builder::new()
                .args(["--adapter", cli.adapter.as_str()])
                .build(),
        )
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
            commands::upsert_provider,
            commands::remove_provider,
            commands::upsert_model,
            commands::remove_model,
            commands::create_profile,
            commands::remove_profile,
            commands::list_plugins,
            commands::plugin_install,
            commands::plugin_remove,
            commands::plugin_update,
            commands::list_market_sources,
            commands::market_search,
            commands::snapshot_export,
            commands::snapshot_import,
            commands::snapshot_list,
            commands::set_language,
            commands::set_theme,
            commands::set_close_to_tray,
            commands::set_check_updates,
            commands::autostart_get,
            commands::autostart_set,
            commands::check_update,
            commands::open_url,
            commands::get_config,
        ])
        .setup(move |app| {
            if let Some(window) = app.get_webview_window("main") {
                let close_to_tray = Arc::clone(&close_to_tray);
                let closed = window.clone();
                window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        if close_to_tray.load(Ordering::Relaxed) {
                            // Close-to-tray: hide instead of quitting. The
                            // window is restored from the tray menu or (on
                            // macOS) a dock click.
                            api.prevent_close();
                            let _ = closed.hide();
                        } else {
                            // Quit on close: closing the control-center
                            // window ends the app.
                            closed.app_handle().exit(0);
                        }
                    }
                });
            }
            setup_tray(app, &language)?;
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building the DeepMate application");

    app.run(|app_handle, event| {
        // macOS dock click while the window is hidden restores it.
        if let RunEvent::Reopen { .. } = event {
            show_main_window(app_handle);
        }
    });
}

// The tray is the app's home while the window is hidden. The menu labels
// follow the same language preference that drives the UI.
fn setup_tray(app: &tauri::App, language: &str) -> tauri::Result<()> {
    let (show_label, quit_label) = if language == "zh" {
        ("显示 DeepMate", "退出")
    } else {
        ("Show DeepMate", "Quit")
    };

    let show = MenuItem::with_id(app, "show", show_label, true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", quit_label, true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &quit])?;

    let mut builder = TrayIconBuilder::with_id("main-tray")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => show_main_window(app),
            "quit" => app.exit(0),
            _ => {}
        });
    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    #[cfg(target_os = "macos")]
    {
        builder = builder.icon_as_template(true);
    }
    builder.build(app)?;
    Ok(())
}

fn show_main_window<R: tauri::Runtime>(app: &impl tauri::Manager<R>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
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
                    data_dir = Some(v.into());
                }
            }
            _ => {}
        }
    }
    Cli { adapter, data_dir }
}
