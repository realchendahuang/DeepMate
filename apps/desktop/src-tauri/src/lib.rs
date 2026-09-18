// DeepMate desktop control center (Tauri).
//
// A thin Tauri shell over the shared core: the React frontend renders state
// delivered by the commands and forwards user intent back as command calls.
// Business rules live in the core and the commands, never in the frontend.

mod commands;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use deepmate_app::{build_harness, init_tracing, load_config_or_default};
use deepmate_platform::{PlatformService, SystemPlatform};
use tauri::menu::{Menu, MenuItem};
use tauri::tray::TrayIconBuilder;
use tauri::Manager;
#[cfg(target_os = "macos")]
use tauri::RunEvent;

use commands::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let cli = parse_cli();

    let platform = SystemPlatform;
    let data_dir = match &cli.data_dir {
        Some(dir) => dir.clone(),
        None => match platform.data_dir() {
            Ok(dir) => dir,
            Err(err) => {
                return fatal_startup(
                    "Cannot determine the DeepMate data folder",
                    &err.to_string(),
                )
            }
        },
    };
    let layout = deepmate_core::data::DataLayout::new(data_dir);
    if let Err(err) = layout.ensure() {
        return fatal_startup(
            "Cannot create the DeepMate data folder",
            &format!("{err}\n\nData folder: {}", layout.root().display()),
        );
    }

    let config = load_config_or_default(&layout);
    let _guard = init_tracing(&layout.logs_dir());

    let harness = Arc::new(build_harness(&layout));

    // Close-to-tray lives behind an atomic so the window close handler always
    // reads the current preference, not the value captured at startup.
    let close_to_tray = Arc::new(AtomicBool::new(config.ui.close_to_tray));
    let language = config.general.language.clone();
    let check_updates_on_start = config.general.check_updates && config.general.notify_updates;

    let state = AppState {
        harness,
        layout,
        close_to_tray: Arc::clone(&close_to_tray),
    };

    // Regenerate the frontend's typed bindings on every debug build, so the
    // React side can never drift from the Rust command surface. Release
    // builds skip the export and use the last generated file.
    #[cfg(debug_assertions)]
    export_bindings();

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
        // Remember the window size and position across sessions.
        .plugin(tauri_plugin_window_state::Builder::default().build())
        // A second instance focuses the existing window instead of opening
        // a duplicate control center.
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            show_main_window(app);
        }))
        // Auto-start registers the app with the OS login items; carry the
        // data-dir override so a login-started instance manages the same
        // data directory the user chose in the desktop session.
        .plugin({
            let mut builder = tauri_plugin_autostart::Builder::new();
            if let Some(dir) = &cli.data_dir {
                builder =
                    builder.args(["--data-dir".to_string(), dir.to_string_lossy().into_owned()]);
            }
            builder.build()
        })
        .manage(state)
        .invoke_handler(specta_builder().invoke_handler())
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
            if check_updates_on_start {
                request_notification_permission(&app.handle().clone());
            }

            // Announce a newer release when the automatic check is on, so a
            // resident (tray-hidden) DeepMate still surfaces it. The webview
            // keeps showing the banner on Overview either way.
            if check_updates_on_start {
                let handle = app.handle().clone();
                let language = language.clone();
                tauri::async_runtime::spawn(async move {
                    // The automatic check speaks only when a release
                    // exists: errors stay in the log, and "up to date" needs
                    // no announcement.
                    if let Ok(Some(info)) = commands::latest_release().await {
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

    // The handler body is macOS-only (a dock click while the window is hidden
    // restores it), so both bindings are unused on the other platforms.
    app.run(|_app_handle, _event| {
        #[cfg(target_os = "macos")]
        if let RunEvent::Reopen { .. } = _event {
            show_main_window(_app_handle);
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
        ("显示 DeepMate", "打开 Harness", "检查更新", "退出")
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
                        let harness = handle.state::<AppState>().harness.clone();
                        if let Err(err) = harness.open_ui().await {
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
                        // regardless of the notification preference — and it
                        // says "could not check" when that is what happened,
                        // instead of claiming the app is up to date.
                        let zh = language == "zh";
                        match commands::latest_release().await {
                            Ok(Some(info)) => {
                                show_main_window(&handle);
                                let title = format!("DeepMate v{}", info.latest_version);
                                let body = if zh {
                                    "新版本已发布。".to_string()
                                } else {
                                    "A new release is available.".to_string()
                                };
                                notify(&handle, &title, &body);
                            }
                            Ok(None) => {
                                let body = if zh {
                                    "已是最新版本。".to_string()
                                } else {
                                    "You're on the latest version.".to_string()
                                };
                                notify(&handle, "DeepMate", &body);
                            }
                            Err(message) => {
                                let body = if zh {
                                    format!("检查更新失败：{message}")
                                } else {
                                    format!("Update check failed: {message}")
                                };
                                notify(&handle, "DeepMate", &body);
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
// platform must never take the app down. The permission is requested once at
// startup, so by the time anything is shown the grant (or the refusal) is
// already settled.
fn notify(app: &tauri::AppHandle, title: &str, body: &str) {
    use tauri_plugin_notification::NotificationExt;
    if let Err(err) = app.notification().builder().title(title).body(body).show() {
        tracing::warn!(error = %err, "failed to show a notification");
    }
}

// Ask for the notification permission up front, so a later update
// notification is not silently dropped. A refusal is recorded and the user
// keeps a working app.
fn request_notification_permission(app: &tauri::AppHandle) {
    use tauri_plugin_notification::NotificationExt;
    match app.notification().request_permission() {
        Ok(tauri_plugin_notification::PermissionState::Granted) => {}
        Ok(state) => tracing::info!(?state, "notification permission not granted"),
        Err(err) => tracing::warn!(error = %err, "failed to request the notification permission"),
    }
}

// An unrecoverable startup problem (no data directory, unwritable disk) is
// reported in a native dialog before the app exits.
//
// This runs before the Tauri application exists (there is no window to host a
// dialog yet), so the message goes through the OS: `osascript` on macOS,
// `zenity`/`kdialog` on Linux, a message box on Windows. A panic here would
// make the window flash and vanish, which reads as "the app is broken" with
// no explanation at all.
fn fatal_startup(title: &str, detail: &str) {
    let message = format!("{title}\n\n{detail}");
    #[cfg(target_os = "macos")]
    {
        let script = format!(
            "display dialog {} with title {} buttons {{\"OK\"}} default button 1 with icon stop",
            applescript_quote(&message),
            applescript_quote("DeepMate"),
        );
        let _ = std::process::Command::new("osascript")
            .args(["-e", &script])
            .status();
    }
    #[cfg(target_os = "windows")]
    {
        let script = format!(
            "[System.Reflection.Assembly]::LoadWithPartialName('PresentationFramework') | Out-Null; [System.Windows.MessageBox]::Show({}, 'DeepMate')",
            powershell_quote(&message),
        );
        let _ = std::process::Command::new("powershell")
            .args(["-NoProfile", "-Command", &script])
            .status();
    }
    #[cfg(all(unix, not(target_os = "macos")))]
    {
        if std::process::Command::new("zenity")
            .args(["--error", "--title", "DeepMate", "--text", &message])
            .status()
            .is_err()
        {
            let _ = std::process::Command::new("kdialog")
                .args(["--error", &message, "--title", "DeepMate"])
                .status();
        }
    }
    eprintln!("{title}: {detail}");
    std::process::exit(1);
}

// Escape a string for embedding in an AppleScript literal.
#[cfg(target_os = "macos")]
fn applescript_quote(text: &str) -> String {
    format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
}

// Escape a string for embedding in a PowerShell single-quoted literal.
#[cfg(target_os = "windows")]
fn powershell_quote(text: &str) -> String {
    format!("'{}'", text.replace('\'', "''"))
}

fn show_main_window<R: tauri::Runtime>(app: &impl tauri::Manager<R>) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

// The command bridge, shared by the invoke handler and the build-time
// TypeScript export (see build.rs). The command list lives here so the
// runtime registration and the generated bindings can never disagree.
fn specta_builder() -> tauri_specta::Builder<tauri::Wry> {
    tauri_specta::Builder::<tauri::Wry>::new()
        // Map transport-shaped types to their real JS runtime shapes: chrono
        // DateTime becomes `Date`, bytes become `Uint8Array`, etc. The
        // generated bindings then carry the conversion code.
        .semantic_types(specta_typescript::semantic::Configuration::default())
        // The app version, exported as a constant so the UI never hard-codes
        // or re-derives it.
        .constant("appVersion", env!("CARGO_PKG_VERSION"))
        .commands(tauri_specta::collect_commands![
            commands::refresh_all,
            commands::runtime_start,
            commands::runtime_stop,
            commands::runtime_restart,
            commands::runtime_list,
            commands::task_run,
            commands::open_harness,
            commands::run_doctor,
            commands::doctor_fix,
            commands::list_profiles,
            commands::list_providers,
            commands::list_models,
            commands::upsert_provider,
            commands::remove_provider,
            commands::upsert_model,
            commands::remove_model,
            commands::create_scenario,
            commands::remove_profile,
            commands::rename_profile,
            commands::set_scenario_description,
            commands::list_plugins,
            commands::plugin_install,
            commands::plugin_disable,
            commands::plugin_enable,
            commands::list_disabled_plugins,
            commands::plugin_op_stream,
            commands::market_search,
            commands::plugin_check,
            commands::snapshot_export,
            commands::snapshot_import,
            commands::snapshot_list,
            commands::snapshot_delete,
            commands::config_export,
            commands::config_import,
            commands::set_language,
            commands::set_theme,
            commands::set_close_to_tray,
            commands::set_check_updates,
            commands::set_notify_updates,
            commands::autostart_get,
            commands::autostart_set,
            commands::set_market_default_source,
            commands::set_market_refresh_interval,
            commands::advanced_file_read,
            commands::advanced_file_save,
            commands::check_update,
            commands::update_install,
            commands::open_url,
            commands::get_config,
        ])
}

// The desktop app accepts the same --data-dir flag as the CLI.
#[derive(Debug, Default, PartialEq, Eq)]
struct Cli {
    data_dir: Option<std::path::PathBuf>,
}

// Parse the desktop app's arguments.
//
// Both spellings are accepted (`--data-dir <path>` and `--data-dir=<path>`)
// because the autostart registration writes the flag itself and users copy
// either form. Anything else is reported instead of ignored: a mistyped flag
// silently starting against the default data directory is how a user ends up
// with two sets of settings.
fn parse_cli() -> Cli {
    parse_cli_from(std::env::args().skip(1))
}

fn parse_cli_from(args: impl IntoIterator<Item = String>) -> Cli {
    let mut cli = Cli::default();
    let mut args = args.into_iter();
    while let Some(arg) = args.next() {
        if let Some(value) = arg.strip_prefix("--data-dir=") {
            if !value.is_empty() {
                cli.data_dir = Some(value.into());
            }
            continue;
        }
        if arg == "--data-dir" {
            match args.next() {
                Some(value) if !value.is_empty() => cli.data_dir = Some(value.into()),
                _ => {
                    eprintln!("warning: --data-dir needs a path; using the default data directory")
                }
            }
            continue;
        }
        // macOS passes `-psn_...` when an app is launched from Finder.
        if arg.starts_with("-psn_") {
            continue;
        }
        eprintln!("warning: ignoring unknown argument {arg:?}");
    }
    cli
}

#[cfg(test)]
mod cli_tests {
    use super::parse_cli_from;
    use std::path::PathBuf;

    fn parse(args: &[&str]) -> super::Cli {
        parse_cli_from(args.iter().map(|arg| arg.to_string()))
    }

    #[test]
    fn both_data_dir_spellings_are_accepted() {
        assert_eq!(
            parse(&["--data-dir", "/tmp/x"]).data_dir,
            Some(PathBuf::from("/tmp/x"))
        );
        assert_eq!(
            parse(&["--data-dir=/tmp/x"]).data_dir,
            Some(PathBuf::from("/tmp/x"))
        );
    }

    #[test]
    fn a_missing_or_empty_value_falls_back_to_the_default() {
        assert_eq!(parse(&["--data-dir"]).data_dir, None);
        assert_eq!(parse(&["--data-dir="]).data_dir, None);
    }

    #[test]
    fn the_finder_process_serial_number_is_ignored() {
        let cli = parse(&["-psn_0_123456", "--data-dir", "/tmp/y"]);
        assert_eq!(cli.data_dir, Some(PathBuf::from("/tmp/y")));
    }
}

// Export the frontend's typed bindings. Runs on every debug launch, and can
// also be triggered headlessly (no window) via:
//   cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml --lib export_bindings
#[cfg(debug_assertions)]
fn export_bindings() {
    specta_builder()
        .export(
            specta_typescript::Typescript::default(),
            "../src/shared/api/bindings.ts",
        )
        .expect("failed to export TypeScript bindings");
}

#[cfg(debug_assertions)]
#[test]
fn export_bindings_headless() {
    export_bindings();
}

// The capability grant and the command surface must agree.
//
// Declaring an app ACL manifest turns on enforcement: from then on a command
// the webview invokes is rejected unless a capability grants it. A name that
// drifts (a rename on one side, a forgotten entry on the other) would show up
// as a runtime "command not allowed" in the UI, so it is checked here
// instead.
#[test]
fn capability_grants_every_registered_command() {
    use std::collections::BTreeSet;

    // The commands the invoke handler registers, taken from the same source
    // the builder uses.
    let registered = registered_command_names();
    assert!(!registered.is_empty());

    let permissions = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/permissions/deepmate.toml"
    ))
    .expect("permissions/deepmate.toml must exist");

    let granted: BTreeSet<String> = permissions
        .lines()
        .skip_while(|line| !line.starts_with("commands.allow"))
        .skip(1)
        .take_while(|line| !line.trim_start().starts_with(']'))
        .filter_map(|line| {
            let name = line.trim().trim_end_matches(',').trim_matches('"');
            (!name.is_empty()).then(|| name.to_string())
        })
        .collect();
    assert!(
        !granted.is_empty(),
        "no commands are granted by the capability"
    );

    let missing: Vec<&String> = registered.difference(&granted).collect();
    assert!(
        missing.is_empty(),
        "these commands are registered but not granted by permissions/deepmate.toml: {missing:?}"
    );

    let stale: Vec<&String> = granted.difference(&registered).collect();
    assert!(
        stale.is_empty(),
        "these commands are granted but no longer registered: {stale:?}"
    );
}

// The registered command names, read from this file's own `collect_commands!`
// invocation so the list cannot drift from the real registration.
#[cfg(test)]
fn registered_command_names() -> std::collections::BTreeSet<String> {
    let source = include_str!("lib.rs");
    let body = source
        .split("tauri_specta::collect_commands![")
        .nth(1)
        .and_then(|rest| rest.split("])").next())
        .expect("the command list must exist");
    body.split(',')
        .filter_map(|entry| entry.trim().split("::").last())
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_string)
        .collect()
}
