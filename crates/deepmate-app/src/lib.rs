// Application-level services shared by the DeepMate frontends.
//
// The CLI and the desktop app are both consumers of the same core; this crate
// holds the small service layer they have in common: adapter registry
// assembly, configuration loading, logging setup and action history. It has
// no UI or command-line knowledge of its own.

use std::path::Path;
use std::sync::Arc;

use anyhow::anyhow;
use deepmate_core::adapter::AdapterCapabilities;
use deepmate_core::registry::AdapterRegistry;
use deepmate_core::testkit::FakeAdapter;
use deepmate_core::{ActionRecord, Config, DataLayout};
use deepmate_platform::SystemPlatform;
use deepseek_harness::DeepSeekHarnessAdapter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer};

// Assemble the adapter registry for the given adapter id.
//
// Supported ids: `test` (deterministic fake adapter), `minimal` (fake adapter
// with only runtime support, for exercising the capability gate) and
// `deepseek-harness` (the real adapter; `DEEPMATE_HARNESS_UI_URL` overrides
// the harness UI URL).
pub fn build_registry(adapter_id: &str, layout: &DataLayout) -> anyhow::Result<AdapterRegistry> {
    let mut registry = AdapterRegistry::new();
    match adapter_id {
        "test" => {
            registry.register(Box::new(FakeAdapter::healthy()));
        }
        // A fake adapter with only runtime support, for exercising the
        // capability gate without a real harness.
        "minimal" => {
            let mut adapter = FakeAdapter::new("minimal");
            adapter.capabilities = AdapterCapabilities {
                runtime: true,
                ..Default::default()
            };
            registry.register(Box::new(adapter));
        }
        "deepseek-harness" => {
            let platform = Arc::new(SystemPlatform);
            let mut adapter = DeepSeekHarnessAdapter::new(platform);
            if let Ok(url) = std::env::var("DEEPMATE_HARNESS_UI_URL") {
                adapter = adapter.with_ui_url(url);
            }
            adapter = adapter.with_data_dir(layout.root().to_path_buf());
            registry.register(Box::new(adapter));
        }
        other => return Err(anyhow!("unknown adapter: {other}")),
    }
    Ok(registry)
}

// Load the DeepMate configuration, falling back to defaults.
//
// An unreadable or invalid config file warns and yields defaults; a missing
// config file is seeded with the defaults on a best-effort basis.
pub fn load_config_or_default(layout: &DataLayout) -> Config {
    let config = match Config::load(&layout.config_path()) {
        Ok(config) => config,
        Err(err) => {
            eprintln!("warning: {err}; using default configuration");
            Config::default()
        }
    };
    if !layout.config_path().exists() {
        if let Err(err) = config.save(&layout.config_path()) {
            eprintln!("warning: failed to write default config: {err}");
        }
    }
    config
}

// Initialize structured logging to stderr and to a rolling file under
// `logs_dir`. The returned guard must be kept alive for the duration of the
// process so buffered file logs are flushed.
pub fn init_tracing(logs_dir: &Path) -> tracing_appender::non_blocking::WorkerGuard {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    let file_appender = tracing_appender::rolling::daily(logs_dir, "deepmate.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(std::io::stderr)
                .with_filter(filter.clone()),
        )
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(non_blocking)
                .with_ansi(false)
                .with_filter(filter),
        )
        .init();
    guard
}

// Append an action record to the JSONL history.
//
// History recording is best-effort: a read-only data directory must not
// break the caller, so failures are logged instead of propagated.
pub fn record_action(layout: &DataLayout, adapter_id: &str, action: String) {
    let record = ActionRecord::new(action).with_adapter(adapter_id.to_string());
    if let Err(err) = layout.history().record(&record) {
        tracing::warn!(error = %err, "failed to record action history");
    }
}
