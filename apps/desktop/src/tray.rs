// System tray integration via the tray-icon crate.
//
// The tray icon carries the DeepMate logo (bundled, decoded and resized to
// 32x32 RGBA at startup) and a small context menu. Left-clicking the icon
// toggles the control-center window (Windows/macOS; the Linux
// StatusNotifierItem backend does not deliver click events, so Linux users
// use the menu); the menu offers explicit Show and Quit entries on every
// platform. Tray and menu events arrive on global channels, so a polling
// thread forwards them into the Slint UI thread.

use std::cell::RefCell;
use std::time::Duration;

use anyhow::{Context, Result};
use slint::{ComponentHandle, Weak};
use tray_icon::menu::{Menu, MenuEvent, MenuId, MenuItem};
use tray_icon::{Icon, MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};

use crate::AppWindow;

// The bundled application logo, decoded and resized for the tray at startup.
const LOGO_PNG: &[u8] = include_bytes!("../../../logo/logo.png");
const TRAY_ICON_SIZE: u32 = 32;
const POLL_INTERVAL: Duration = Duration::from_millis(50);

thread_local! {
    // The tray icon must be created on and owned by the UI thread (macOS
    // requirement) and must outlive the whole process.
    static TRAY_ICON: RefCell<Option<TrayIcon>> = const { RefCell::new(None) };
}

// Decode the bundled logo and resize it to 32x32 RGBA.
fn load_icon() -> Result<Icon> {
    let image = image::load_from_memory(LOGO_PNG)
        .context("failed to decode the bundled logo")?
        .resize_exact(
            TRAY_ICON_SIZE,
            TRAY_ICON_SIZE,
            image::imageops::FilterType::Lanczos3,
        )
        .to_rgba8();
    let (width, height) = image.dimensions();
    Icon::from_rgba(image.into_raw(), width, height).context("failed to build the tray icon")
}

// Install the system tray icon and start forwarding its events to the UI.
//
// tray-icon requires an already-running event loop on macOS, so the actual
// creation is deferred onto the Slint event loop: this function queues the
// setup and returns immediately. Creation failures are reported through the
// window's error bar instead of aborting startup.
pub fn install(window: &AppWindow) -> Result<()> {
    let weak = window.as_weak();
    slint::invoke_from_event_loop(move || {
        if let Err(err) = build(weak.clone()) {
            tracing::error!(error = %err, "failed to set up the system tray");
            if let Some(window) = weak.upgrade() {
                window.set_error_text(format!("system tray unavailable: {err:#}").into());
            }
        }
    })
    .context("failed to queue tray setup on the Slint event loop")
}

// Build the tray icon, keep it alive on this (UI) thread and spawn the event
// polling thread.
fn build(window: Weak<AppWindow>) -> Result<()> {
    let icon = load_icon()?;

    let menu = Menu::new();
    let show_item = MenuItem::new("Show DeepMate", true, None);
    let quit_item = MenuItem::new("Quit", true, None);
    menu.append(&show_item)?;
    menu.append(&quit_item)?;
    let show_id = show_item.id().clone();
    let quit_id = quit_item.id().clone();

    let tray = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        // Left-click toggles the window (where the platform delivers click
        // events); the menu opens on right-click.
        .with_menu_on_left_click(false)
        .with_tooltip("DeepMate")
        .with_icon(icon)
        .build()
        .context("failed to create the tray icon")?;

    TRAY_ICON.with(|slot| *slot.borrow_mut() = Some(tray));
    std::thread::Builder::new()
        .name("deepmate-tray".to_string())
        .spawn(move || poll_events(window, show_id, quit_id))
        .context("failed to spawn the tray event thread")?;
    Ok(())
}

// Poll the global tray-icon and menu event channels and marshal each event
// into the UI thread. Exits when the UI thread is gone.
fn poll_events(window: Weak<AppWindow>, show_id: MenuId, quit_id: MenuId) {
    loop {
        // The receivers are static channels that never disconnect, so any
        // recv_timeout error is a timeout and polling continues.
        if let Ok(event) = TrayIconEvent::receiver().recv_timeout(POLL_INTERVAL) {
            if !forward_tray_event(&window, event) {
                return;
            }
        }
        if let Ok(event) = MenuEvent::receiver().recv_timeout(POLL_INTERVAL) {
            if !forward_menu_event(&window, &show_id, &quit_id, event) {
                return;
            }
        }
    }
}

// Returns false when the UI thread is gone and the polling loop should stop.
fn dispatch(window: &Weak<AppWindow>, action: impl FnOnce(&AppWindow) + Send + 'static) -> bool {
    let weak = window.clone();
    slint::invoke_from_event_loop(move || {
        if let Some(window) = weak.upgrade() {
            action(&window);
        }
    })
    .is_ok()
}

fn forward_tray_event(window: &Weak<AppWindow>, event: TrayIconEvent) -> bool {
    // Clicks fire for both press and release; react to the release only.
    // DoubleClick is deliberately ignored: on Windows a double-click also
    // fires Click{Up}, so handling both would toggle the window twice.
    let toggle = matches!(
        &event,
        TrayIconEvent::Click {
            button: MouseButton::Left,
            button_state: MouseButtonState::Up,
            ..
        }
    );
    if !toggle {
        return true;
    }
    dispatch(window, |window| toggle_window(window.window()))
}

fn forward_menu_event(
    window: &Weak<AppWindow>,
    show_id: &MenuId,
    quit_id: &MenuId,
    event: MenuEvent,
) -> bool {
    if event.id == *show_id {
        dispatch(window, |window| show_window(window.window()))
    } else if event.id == *quit_id {
        dispatch(window, |_| {
            if let Err(err) = slint::quit_event_loop() {
                tracing::warn!(error = %err, "failed to quit the event loop");
            }
        })
    } else {
        true
    }
}

fn toggle_window(window: &slint::Window) {
    if window.is_visible() {
        if let Err(err) = window.hide() {
            tracing::warn!(error = %err, "failed to hide the window");
        }
    } else {
        show_window(window);
    }
}

fn show_window(window: &slint::Window) {
    if let Err(err) = window.show() {
        tracing::warn!(error = %err, "failed to show the window");
    }
}
