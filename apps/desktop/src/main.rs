// DeepMate desktop control center.
//
// A thin Slint shell over the shared core: the UI renders state delivered by
// the bridge and forwards user intent back as bridge commands. Business rules
// live in the core and the bridge, never in UI callbacks.
//
// History action names follow the CLI convention with a `desktop.` prefix:
// desktop.runtime.start, desktop.runtime.stop, desktop.runtime.restart,
// desktop.open, desktop.doctor. Like the CLI, only successful actions are
// recorded.

// The tray app must not pop a console window on Windows release builds.
#![cfg_attr(
    all(target_os = "windows", not(debug_assertions)),
    windows_subsystem = "windows"
)]

use std::path::PathBuf;
use std::time::Duration;

use anyhow::Context;
use clap::Parser;
use deepmate_app::{build_registry, init_tracing, load_config_or_default, record_action};
use deepmate_core::adapter::AdapterCapabilities;
use deepmate_core::model::MarketSource;
use deepmate_core::{CheckStatus, DataLayout, RuntimeStatus, RuntimeStatusKind};
use deepmate_platform::{PlatformService, SystemPlatform};
use slint::{ComponentHandle, ModelRc, SharedString, VecModel};

mod bridge;
mod tray;

use bridge::{CapabilityCounts, UiCommand, UiEvent};

slint::include_modules!();

#[derive(Debug, Parser)]
#[command(
    name = "deepmate-desktop",
    version,
    about = "DeepMate desktop control center"
)]
struct Cli {
    /// Adapter to use. Use "test" for a deterministic fake adapter.
    #[arg(long, default_value = "deepseek-harness")]
    adapter: String,

    /// Override the DeepMate data directory (default: OS application-data convention).
    #[arg(long)]
    data_dir: Option<PathBuf>,
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let platform = SystemPlatform;
    let data_dir = match &cli.data_dir {
        Some(dir) => dir.clone(),
        None => platform.data_dir()?,
    };
    let layout = DataLayout::new(data_dir);
    layout
        .ensure()
        .context("failed to initialize the DeepMate data directory")?;

    let config = load_config_or_default(&layout);
    let _guard = init_tracing(&layout.logs_dir());

    let registry = build_registry(&cli.adapter, &layout)?;
    let adapter = registry
        .into_adapter(&cli.adapter)
        .with_context(|| format!("adapter not found: {}", cli.adapter))?;

    // Capture identity and capabilities before the adapter moves into the
    // bridge thread.
    let metadata = adapter.metadata();
    let capabilities = adapter.capabilities();
    tracing::debug!(
        adapter = %cli.adapter,
        data_dir = %layout.root().display(),
        "deepmate-desktop startup"
    );

    let bridge = bridge::spawn(adapter);

    let window = AppWindow::new()?;
    window.set_adapter_id(metadata.id.clone().into());
    window.set_adapter_name(metadata.name.clone().into());
    window.set_adapter_version(metadata.version.clone().into());
    window.set_capabilities(ModelRc::new(VecModel::from(capability_names(
        &capabilities,
    ))));

    wire_commands(&window, &bridge);
    let _events_timer = wire_events(&window, bridge.events, layout.clone(), cli.adapter.clone());
    wire_close_behavior(&window, config.ui.close_to_tray);

    tray::install(&window)?;

    // Populate the UI as soon as the bridge is ready.
    let _ = bridge.cmd.send(UiCommand::RefreshAll);

    window.run().context("the Slint event loop failed")?;
    Ok(())
}

// Forward UI callbacks to the bridge as commands. Action history is recorded
// later, when the bridge reports the action as completed (UiEvent::
// ActionCompleted), mirroring the CLI which records only successful commands.
fn wire_commands(window: &AppWindow, bridge: &bridge::Bridge) {
    // `UiCommand` is Clone but not Copy (some variants carry owned strings),
    // so the command is cloned on each invocation to keep the callback FnMut.
    let sender = |command: UiCommand| {
        let cmd = bridge.cmd.clone();
        move || {
            let _ = cmd.send(command.clone());
        }
    };
    window.on_refresh_all(sender(UiCommand::RefreshAll));
    window.on_runtime_start(sender(UiCommand::RuntimeStart));
    window.on_runtime_stop(sender(UiCommand::RuntimeStop));
    window.on_runtime_restart(sender(UiCommand::RuntimeRestart));
    window.on_open_harness(sender(UiCommand::OpenHarness));
    window.on_run_doctor(sender(UiCommand::RunDoctor));
    window.on_refresh_profiles(sender(UiCommand::ListProfiles));
    window.on_refresh_providers(sender(UiCommand::ListProviders));
    window.on_refresh_models(sender(UiCommand::ListModels));
    window.on_refresh_plugins(sender(UiCommand::ListPlugins));
    window.on_refresh_market_sources(sender(UiCommand::ListMarketSources));

    let cmd = bridge.cmd.clone();
    window.on_plugin_install(move |profile, spec| {
        let _ = cmd.send(UiCommand::PluginInstall {
            profile: profile.to_string(),
            spec: spec.to_string(),
        });
    });
    let cmd = bridge.cmd.clone();
    window.on_plugin_remove(move |profile, id| {
        let _ = cmd.send(UiCommand::PluginRemove {
            profile: profile.to_string(),
            id: id.to_string(),
        });
    });
    let cmd = bridge.cmd.clone();
    window.on_plugin_update(move |profile, id| {
        let _ = cmd.send(UiCommand::PluginUpdate {
            profile: profile.to_string(),
            id: Some(id.to_string()),
        });
    });
    let cmd = bridge.cmd.clone();
    window.on_market_search(move |query| {
        let _ = cmd.send(UiCommand::MarketSearch {
            query: query.to_string(),
        });
    });
}

// Drain bridge events on the UI thread with a repeating timer and apply them
// to the window properties and models. The returned timer must be kept alive
// for the duration of the event loop.
fn wire_events(
    window: &AppWindow,
    events: std::sync::mpsc::Receiver<UiEvent>,
    layout: DataLayout,
    adapter_id: String,
) -> slint::Timer {
    let weak = window.as_weak();
    let timer = slint::Timer::default();
    timer.start(
        slint::TimerMode::Repeated,
        Duration::from_millis(100),
        move || {
            let Some(window) = weak.upgrade() else {
                return;
            };
            while let Ok(event) = events.try_recv() {
                apply_event(&window, event, &layout, &adapter_id);
            }
        },
    );
    timer
}

// Close-to-tray: hide the window on close requests. When disabled, closing
// the window quits the event loop (and with it the process, since hiding the
// last window does not terminate the Slint winit loop on its own).
fn wire_close_behavior(window: &AppWindow, close_to_tray: bool) {
    if close_to_tray {
        window
            .window()
            .on_close_requested(|| slint::CloseRequestResponse::HideWindow);
    } else {
        window.window().on_close_requested(|| {
            let _ = slint::quit_event_loop();
            slint::CloseRequestResponse::HideWindow
        });
    }
}

fn apply_event(window: &AppWindow, event: UiEvent, layout: &DataLayout, adapter_id: &str) {
    match event {
        UiEvent::Busy(busy) => {
            // A new command supersedes any previous error: clear the error
            // bar when a command starts, so errors raised later in the same
            // batch (for example by a partial refresh) stay visible.
            if busy {
                window.set_error_text("".into());
            }
            window.set_busy(busy);
        }
        UiEvent::ActionCompleted(action) => record_action(layout, adapter_id, action.to_string()),
        UiEvent::Error(message) => {
            tracing::warn!(%message, "bridge error");
            window.set_error_text(message.into());
        }
        UiEvent::Status(status) => set_status(window, &status),
        UiEvent::Overview {
            detection,
            status,
            counts,
        } => {
            window.set_harness_found(detection.found);
            match &detection.harness {
                Some(harness) => {
                    window.set_harness_name(harness.name.clone().into());
                    window.set_harness_version(harness.version.clone().unwrap_or_default().into());
                }
                None => {
                    window.set_harness_name("".into());
                    window.set_harness_version("".into());
                }
            }
            window.set_detection_detail(detection.detail.unwrap_or_default().into());
            set_status(window, &status);
            set_counts(window, &counts);
        }
        UiEvent::Doctor(report) => {
            let rows: Vec<CheckRow> = report
                .checks
                .iter()
                .map(|check| CheckRow {
                    status: check_status_text(check.status).into(),
                    summary: check.summary.clone().into(),
                    details: check.details.clone().unwrap_or_default().into(),
                    action: check.suggested_action.clone().unwrap_or_default().into(),
                })
                .collect();
            window.set_doctor_checks(ModelRc::new(VecModel::from(rows)));
        }
        UiEvent::Profiles(items) => {
            let rows: Vec<ProfileRow> = items
                .iter()
                .map(|item| ProfileRow {
                    id: item.id.clone().into(),
                    name: item.name.clone().into(),
                    description: item.description.clone().unwrap_or_default().into(),
                })
                .collect();
            window.set_profiles(ModelRc::new(VecModel::from(rows)));
        }
        UiEvent::Providers(items) => {
            let rows: Vec<ProviderRow> = items
                .iter()
                .map(|item| ProviderRow {
                    id: item.id.clone().into(),
                    name: item.name.clone().into(),
                    kind: item.kind.clone().into(),
                })
                .collect();
            window.set_providers(ModelRc::new(VecModel::from(rows)));
        }
        UiEvent::Models(items) => {
            let rows: Vec<ModelRow> = items
                .iter()
                .map(|item| ModelRow {
                    id: item.id.clone().into(),
                    name: item.name.clone().into(),
                    provider: item.provider.clone().unwrap_or_default().into(),
                })
                .collect();
            window.set_models(ModelRc::new(VecModel::from(rows)));
        }
        UiEvent::Plugins(items) => {
            let rows: Vec<PluginRow> = items
                .iter()
                .map(|item| PluginRow {
                    id: item.id.clone().into(),
                    name: item.name.clone().into(),
                    version: item.version.clone().unwrap_or_default().into(),
                    enabled: item.enabled,
                    profile: item.profile.clone().into(),
                    outdated: item.outdated,
                    latest: item.latest.clone().unwrap_or_default().into(),
                })
                .collect();
            window.set_plugins(ModelRc::new(VecModel::from(rows)));
        }
        UiEvent::MarketSources(items) => {
            let rows: Vec<MarketSourceRow> = items
                .iter()
                .map(|item| MarketSourceRow {
                    id: item.id.clone().into(),
                    description: item.description.clone().into(),
                })
                .collect();
            window.set_market_sources(ModelRc::new(VecModel::from(rows)));
        }
        UiEvent::MarketEntries(items) => {
            let rows: Vec<MarketEntryRow> = items
                .iter()
                .map(|item| MarketEntryRow {
                    id: item.id.clone().into(),
                    description: item.description.clone().unwrap_or_default().into(),
                    version: item.version.clone().unwrap_or_default().into(),
                    source: market_source_text(&item.source).into(),
                    publisher: item.publisher.clone().unwrap_or_default().into(),
                    repository: item.repository.clone().unwrap_or_default().into(),
                })
                .collect();
            window.set_market_entries(ModelRc::new(VecModel::from(rows)));
        }
    }
}

fn market_source_text(source: &MarketSource) -> &'static str {
    match source {
        MarketSource::Curated => "curated",
        MarketSource::Community => "community",
    }
}

fn set_status(window: &AppWindow, status: &RuntimeStatus) {
    let (kind, tone) = match status.kind {
        RuntimeStatusKind::Running => ("running", "pass"),
        RuntimeStatusKind::Installed => ("installed", "accent"),
        RuntimeStatusKind::Stopped => ("stopped", "neutral"),
        RuntimeStatusKind::Unknown => ("unknown", "skip"),
        RuntimeStatusKind::Error => ("error", "fail"),
    };
    window.set_status_kind(kind.into());
    window.set_status_tone(tone.into());
    window.set_status_pid(
        status
            .pid
            .map(|pid| pid.to_string())
            .unwrap_or_default()
            .into(),
    );
    window.set_status_message(status.message.clone().unwrap_or_default().into());
}

fn set_counts(window: &AppWindow, counts: &CapabilityCounts) {
    // The UI uses -1 for "unsupported by this adapter".
    fn count(value: Option<usize>) -> i32 {
        value.map(|count| count as i32).unwrap_or(-1)
    }
    window.set_counts_profiles(count(counts.profiles));
    window.set_counts_providers(count(counts.providers));
    window.set_counts_models(count(counts.models));
    window.set_counts_plugins(count(counts.plugins));
}

fn check_status_text(status: CheckStatus) -> &'static str {
    match status {
        CheckStatus::Pass => "pass",
        CheckStatus::Warn => "warn",
        CheckStatus::Fail => "fail",
        CheckStatus::Skip => "skip",
    }
}

fn capability_names(capabilities: &AdapterCapabilities) -> Vec<SharedString> {
    let mut names = Vec::new();
    if capabilities.runtime {
        names.push("runtime");
    }
    if capabilities.profiles {
        names.push("profiles");
    }
    if capabilities.providers {
        names.push("providers");
    }
    if capabilities.models {
        names.push("models");
    }
    if capabilities.plugins {
        names.push("plugins");
    }
    if capabilities.marketplace {
        names.push("marketplace");
    }
    if capabilities.skills {
        names.push("skills");
    }
    if capabilities.mcp {
        names.push("mcp");
    }
    if capabilities.snapshots {
        names.push("snapshots");
    }
    names.into_iter().map(SharedString::from).collect()
}
