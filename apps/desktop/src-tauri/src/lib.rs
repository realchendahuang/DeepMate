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
    let check_updates_on_start = config.general.check_updates && config.general.notify_updates;

    let state = AppState {
        adapter: Arc::from(adapter),
        layout,
        close_to_tray: Arc::clone(&close_to_tray),
    };

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
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
            commands::plugin_check,
            commands::snapshot_export,
            commands::snapshot_import,
            commands::snapshot_list,
            commands::config_export,
            commands::config_import,
            commands::set_language,
            commands::set_theme,
            commands::set_close_to_tray,
            commands::set_check_updates,
            commands::set_notify_updates,
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

            // Announce a newer release when the automatic check is on, so a
            // resident (tray-hidden) DeepMate still surfaces it. The webview
            // keeps showing the banner on Overview either way.
            if check_updates_on_start {
                let handle = app.handle().clone();
                let language = language.clone();
                tauri::async_runtime::spawn(async move {
                    if let Some(info) = commands::latest_release().await {
                        let zh = language == "zh";
                        let title = format!("DeepMate v{}", info.latest_version);
                        let body = if zh {
                            "新版本已发布，点击查看。".to_string()
                        } else {
                            "A new release is available; open DeepMate for details.".to_string()
                        };
                        notify(&handle, &title, &body);
                    }
                });
            }
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building the DeepMate application");

    app.run(|app_handle, event| {
        // macOS dock click while the window is hidden restores it.
        match event {
            #[cfg(target_os = "macos")]
            RunEvent::Reopen { .. } => show_main_window(app_handle),
            _ => {}
        }
    });
}

// The tray is the app's home while the window is hidden. The menu labels
// follow the same language preference that drives the UI. Async work from
// the menu (harness UI, update check) runs on the Tauri runtime so the
// handler never blocks the tray.
fn setup_tray(app: &tauri::App, language: &str) -> tauri::Result<()> {
    let zh = language == "zh";
    let (show_label, harness_label, updates_label, quit_label) = if zh {
        (
            "显示 DeepMate",
            "打开 Harness",
            "检查更新",
            "退出",
        )
    } else {
        ("Show DeepMate", "Open Harness", "Check Updates", "Quit")
    };

    let show = MenuItem::with_id(app, "show", show_label, true, None::<&str>)?;
    let harness = MenuItem::with_id(app, "harness", harness_label, true, None::<&str>)?;
    let updates = MenuItem::with_id(app, "updates", updates_label, true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", quit_label, true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show, &harness, &updates, &quit])?;

    let language = language.to_string();
    let mut builder = TrayIconBuilder::with_id("main-tray")
        .menu(&menu)
        .show_menu_on_left_click(true)
        .on_menu_event(move |app, event| {
            // Cloned per invocation so the spawned handlers localize their
            // notifications like the menu labels.
            let language = language.clone();
            match event.id.as_ref() {
                "show" => show_main_window(app),
                "harness" => {
                    let handle = app.clone();
                    tauri::async_runtime::spawn(async move {
                        let adapter = handle.state::<AppState>().adapter.clone();
                        if let Err(err) = adapter.open_ui().await {
                            let zh = language == "zh";
                            let title = if zh {
                                "无法打开 Harness"
                            } else {
                                "Cannot open Harness"
                            };
                            notify(&handle, title, &err.to_string());
                        }
                    });
                }
                "updates" => {
                    let handle = app.clone();
                    tauri::async_runtime::spawn(async move {
                        // An explicit tray check always reports its outcome,
                        // regardless of the notification preference.
                        match commands::latest_release().await {
                            Some(info) => {
                                show_main_window(&handle);
                                let zh = language == "zh";
                                let title = format!("DeepMate v{}", info.latest_version);
                                let body = if zh {
                                    "新版本已发布。".to_string()
                                } else {
                                    "A new release is available.".to_string()
                                };
                                notify(&handle, &title, &body);
                            }
                            None => {
                                let zh = language == "zh";
                                let title = "DeepMate";
                                let body = if zh {
                                    "已是最新版本。".to_string()
                                } else {
                                    "You're on the latest version.".to_string()
                                };
                                notify(&handle, title, &body);
                            }
                        }
                    });
                }
                "quit" => app.exit(0),
                _ => {}
            }
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

// Best-effort system notification; a missing permission or unsupported
// platform must never take the app down.
fn notify(app: &tauri::AppHandle, title: &str, body: &str) {
    use tauri_plugin_notification::NotificationExt;
    if let Err(err) = app.notification().builder().title(title).body(body).show() {
        tracing::warn!(error = %err, "failed to show a notification");
    }
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
