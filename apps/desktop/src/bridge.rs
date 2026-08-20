// The shared application state / event bridge between the Slint UI thread and
// the adapter core.
//
// The UI sends `UiCommand`s over an unbounded Tokio channel; a background
// thread with a current-thread Tokio runtime executes them against the active
// adapter and reports results back as `UiEvent`s over a standard channel,
// which the UI drains with a timer. This module is UI-framework-free and is
// unit-tested with the core FakeAdapter.

use std::sync::mpsc as std_mpsc;

use deepmate_core::adapter::{Detection, HarnessAdapter};
use deepmate_core::model::{DoctorReport, RuntimeStatus};
use deepmate_core::CoreResult;
use tokio::sync::mpsc as tokio_mpsc;

// Commands the UI can send to the core.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiCommand {
    RefreshAll,
    RuntimeStart,
    RuntimeStop,
    RuntimeRestart,
    OpenHarness,
    RunDoctor,
}

// Per-entity inventory counts. `None` means the active adapter does not
// declare the matching capability, so the UI shows the entity as unsupported
// rather than empty.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CapabilityCounts {
    pub profiles: Option<usize>,
    pub providers: Option<usize>,
    pub models: Option<usize>,
    pub plugins: Option<usize>,
}

// Events flowing from the core back to the UI. All payloads are plain core
// model types; the UI layer does the rendering-specific mapping.
#[derive(Debug)]
pub enum UiEvent {
    Overview {
        detection: Detection,
        status: RuntimeStatus,
        counts: CapabilityCounts,
    },
    Status(RuntimeStatus),
    Doctor(Box<DoctorReport>),
    // A user-triggered action completed successfully; carries the history
    // action name so the UI layer can record it (mirroring CLI semantics:
    // only successful actions are recorded).
    ActionCompleted(&'static str),
    Busy(bool),
    Error(String),
}

// Handle to the running bridge: send commands, receive events.
pub struct Bridge {
    pub cmd: tokio_mpsc::UnboundedSender<UiCommand>,
    pub events: std_mpsc::Receiver<UiEvent>,
}

// Spawn the bridge background thread and return its handle.
//
// The thread owns the adapter and runs a current-thread Tokio runtime. It
// exits when the command sender is dropped or the UI stops receiving events.
pub fn spawn(adapter: Box<dyn HarnessAdapter>) -> Bridge {
    let (cmd_tx, mut cmd_rx) = tokio_mpsc::unbounded_channel::<UiCommand>();
    let (event_tx, event_rx) = std_mpsc::channel::<UiEvent>();

    std::thread::Builder::new()
        .name("deepmate-bridge".to_string())
        .spawn(move || {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .expect("failed to build the bridge Tokio runtime");
            runtime.block_on(async move {
                while let Some(cmd) = cmd_rx.recv().await {
                    if event_tx.send(UiEvent::Busy(true)).is_err() {
                        return;
                    }
                    // Sends Busy(false) when dropped, so a panic inside an
                    // adapter future can never leave the UI stuck in the busy
                    // state even though the bridge thread itself dies.
                    let mut busy = BusyGuard::armed(&event_tx);
                    for event in handle(adapter.as_ref(), &cmd).await {
                        if event_tx.send(event).is_err() {
                            return;
                        }
                    }
                    busy.disarm();
                    if event_tx.send(UiEvent::Busy(false)).is_err() {
                        return;
                    }
                }
            });
        })
        .expect("failed to spawn the bridge thread");

    Bridge {
        cmd: cmd_tx,
        events: event_rx,
    }
}

// Sends Busy(false) on drop unless disarmed, closing the busy bracket even
// when `handle` panics.
struct BusyGuard<'a> {
    tx: &'a std_mpsc::Sender<UiEvent>,
    armed: bool,
}

impl<'a> BusyGuard<'a> {
    fn armed(tx: &'a std_mpsc::Sender<UiEvent>) -> Self {
        Self { tx, armed: true }
    }

    fn disarm(&mut self) {
        self.armed = false;
    }
}

impl Drop for BusyGuard<'_> {
    fn drop(&mut self) {
        if self.armed {
            let _ = self.tx.send(UiEvent::Busy(false));
        }
    }
}

// Execute one command against the adapter and return the events to emit.
//
// Every CoreError is converted into a UiEvent::Error; this function never
// panics and never propagates adapter failures.
async fn handle(adapter: &dyn HarnessAdapter, cmd: &UiCommand) -> Vec<UiEvent> {
    match cmd {
        UiCommand::RefreshAll => refresh_all(adapter).await,
        UiCommand::RuntimeStart => runtime_command(adapter, RuntimeOp::Start).await,
        UiCommand::RuntimeStop => runtime_command(adapter, RuntimeOp::Stop).await,
        UiCommand::RuntimeRestart => runtime_command(adapter, RuntimeOp::Restart).await,
        UiCommand::OpenHarness => match adapter.open_ui().await {
            Ok(()) => vec![UiEvent::ActionCompleted("desktop.open")],
            Err(err) => vec![UiEvent::Error(err.to_string())],
        },
        UiCommand::RunDoctor => match adapter.doctor().await {
            Ok(report) => vec![
                UiEvent::Doctor(Box::new(report)),
                UiEvent::ActionCompleted("desktop.doctor"),
            ],
            Err(err) => vec![UiEvent::Error(err.to_string())],
        },
    }
}

enum RuntimeOp {
    Start,
    Stop,
    Restart,
}

// Run a runtime lifecycle command, then refresh and emit the new status.
async fn runtime_command(adapter: &dyn HarnessAdapter, op: RuntimeOp) -> Vec<UiEvent> {
    if !adapter.capabilities().runtime {
        return vec![UiEvent::Error(format!(
            "adapter '{}' does not support runtime control",
            adapter.metadata().id
        ))];
    }
    let action = match op {
        RuntimeOp::Start => "desktop.runtime.start",
        RuntimeOp::Stop => "desktop.runtime.stop",
        RuntimeOp::Restart => "desktop.runtime.restart",
    };
    let result = match op {
        RuntimeOp::Start => adapter.start().await,
        RuntimeOp::Stop => adapter.stop().await,
        RuntimeOp::Restart => adapter.restart().await,
    };
    match result {
        Ok(()) => {
            let mut events = vec![UiEvent::ActionCompleted(action)];
            events.extend(refresh_status(adapter).await);
            events
        }
        Err(err) => vec![UiEvent::Error(err.to_string())],
    }
}

async fn refresh_status(adapter: &dyn HarnessAdapter) -> Vec<UiEvent> {
    match adapter.status().await {
        Ok(status) => vec![UiEvent::Status(status)],
        Err(err) => vec![UiEvent::Error(err.to_string())],
    }
}

// Gather detection, status and capability-gated inventory counts into one
// Overview event. Individual failures surface as Error events while the
// overview itself always completes with fallback values.
async fn refresh_all(adapter: &dyn HarnessAdapter) -> Vec<UiEvent> {
    let mut events = Vec::new();
    let capabilities = adapter.capabilities();

    let detection = match adapter.detect().await {
        Ok(detection) => detection,
        Err(err) => {
            events.push(UiEvent::Error(err.to_string()));
            Detection {
                found: false,
                harness: None,
                detail: None,
            }
        }
    };

    let status = match adapter.status().await {
        Ok(status) => status,
        Err(err) => {
            events.push(UiEvent::Error(err.to_string()));
            RuntimeStatus::unknown()
        }
    };

    let counts = CapabilityCounts {
        profiles: gated_count(capabilities.profiles, adapter.profiles(), &mut events).await,
        providers: gated_count(capabilities.providers, adapter.providers(), &mut events).await,
        models: gated_count(capabilities.models, adapter.models(), &mut events).await,
        plugins: gated_count(capabilities.plugins, adapter.plugins(), &mut events).await,
    };

    events.push(UiEvent::Overview {
        detection,
        status,
        counts,
    });
    events
}

// Count an entity list when the adapter declares the capability; `None` when
// unsupported. Failures become Error events and an unknown count.
async fn gated_count<T>(
    supported: bool,
    list: impl std::future::Future<Output = CoreResult<Vec<T>>>,
    events: &mut Vec<UiEvent>,
) -> Option<usize> {
    if !supported {
        return None;
    }
    match list.await {
        Ok(items) => Some(items.len()),
        Err(err) => {
            events.push(UiEvent::Error(err.to_string()));
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use deepmate_core::adapter::AdapterCapabilities;
    use deepmate_core::model::RuntimeStatusKind;
    use deepmate_core::testkit::FakeAdapter;
    use std::time::Duration;

    const TIMEOUT: Duration = Duration::from_secs(5);

    fn recv(bridge: &Bridge) -> UiEvent {
        bridge
            .events
            .recv_timeout(TIMEOUT)
            .expect("timed out waiting for a bridge event")
    }

    // Drain events for one command, asserting the Busy bracketing.
    fn run_command(bridge: &Bridge, cmd: UiCommand) -> Vec<UiEvent> {
        bridge.cmd.send(cmd).expect("bridge command channel open");
        match recv(bridge) {
            UiEvent::Busy(true) => {}
            other => panic!("expected Busy(true), got {other:?}"),
        }
        let mut events = Vec::new();
        loop {
            match recv(bridge) {
                UiEvent::Busy(false) => break,
                event => events.push(event),
            }
        }
        events
    }

    #[tokio::test]
    async fn refresh_all_on_healthy_adapter_yields_overview() {
        let bridge = spawn(Box::new(FakeAdapter::healthy()));
        let events = run_command(&bridge, UiCommand::RefreshAll);

        let overviews: Vec<_> = events
            .iter()
            .filter_map(|event| match event {
                UiEvent::Overview {
                    detection,
                    status: _,
                    counts,
                } => Some((detection, counts)),
                _ => None,
            })
            .collect();
        assert_eq!(overviews.len(), 1, "expected exactly one Overview event");
        let (detection, counts) = overviews[0];
        assert!(detection.found);
        assert_eq!(counts.profiles, Some(1));
        assert_eq!(counts.providers, Some(1));
        assert_eq!(counts.models, Some(1));
        assert_eq!(counts.plugins, Some(1));
        assert!(
            !events
                .iter()
                .any(|event| matches!(event, UiEvent::Error(_))),
            "healthy adapter must not produce errors: {events:?}"
        );
    }

    #[tokio::test]
    async fn runtime_start_then_reports_running_status() {
        let bridge = spawn(Box::new(FakeAdapter::running()));
        let events = run_command(&bridge, UiCommand::RuntimeStart);

        assert!(
            events
                .iter()
                .any(|event| matches!(event, UiEvent::ActionCompleted("desktop.runtime.start"))),
            "expected ActionCompleted after a successful start: {events:?}"
        );
        let statuses: Vec<_> = events
            .iter()
            .filter_map(|event| match event {
                UiEvent::Status(status) => Some(status),
                _ => None,
            })
            .collect();
        assert_eq!(statuses.len(), 1, "expected a Status event after start");
        assert_eq!(statuses[0].kind, RuntimeStatusKind::Running);
        assert!(statuses[0].pid.is_some());
    }

    #[tokio::test]
    async fn run_doctor_yields_report() {
        let bridge = spawn(Box::new(FakeAdapter::healthy()));
        let events = run_command(&bridge, UiCommand::RunDoctor);

        let reports: Vec<_> = events
            .iter()
            .filter_map(|event| match event {
                UiEvent::Doctor(report) => Some(report),
                _ => None,
            })
            .collect();
        assert_eq!(reports.len(), 1, "expected exactly one Doctor event");
        assert_eq!(reports[0].adapter_id, "test");
        assert_eq!(reports[0].checks.len(), 1);
        assert!(
            events
                .iter()
                .any(|event| matches!(event, UiEvent::ActionCompleted("desktop.doctor"))),
            "expected ActionCompleted after a successful doctor run: {events:?}"
        );
    }

    #[tokio::test]
    async fn minimal_adapter_yields_errors_instead_of_panicking() {
        let mut adapter = FakeAdapter::new("minimal");
        adapter.capabilities = AdapterCapabilities::default();
        let bridge = spawn(Box::new(adapter));

        // A gated runtime operation must surface an Error event and no
        // ActionCompleted.
        let events = run_command(&bridge, UiCommand::RuntimeStart);
        assert!(
            events
                .iter()
                .any(|event| matches!(event, UiEvent::Error(_))),
            "expected an Error event for unsupported runtime control: {events:?}"
        );
        assert!(
            !events
                .iter()
                .any(|event| matches!(event, UiEvent::ActionCompleted(_))),
            "failed operations must not record an action: {events:?}"
        );

        // RefreshAll still completes, with all counts marked unsupported.
        let events = run_command(&bridge, UiCommand::RefreshAll);
        for event in &events {
            if let UiEvent::Overview { counts, .. } = event {
                assert_eq!(counts.profiles, None);
                assert_eq!(counts.providers, None);
                assert_eq!(counts.models, None);
                assert_eq!(counts.plugins, None);
                return;
            }
        }
        panic!("expected an Overview event for the minimal adapter: {events:?}");
    }
}
