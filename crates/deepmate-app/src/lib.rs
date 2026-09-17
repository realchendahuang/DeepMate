// Application-level services shared by the DeepMate frontends.
//
// The CLI and the desktop app are both consumers of the same core; this crate
// holds the small service layer they have in common: harness assembly,
// configuration loading, logging setup and action history. It has no UI or
// command-line knowledge of its own.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use deepmate_core::{ActionRecord, Config, DataLayout};
use deepmate_platform::SystemPlatform;
use deepseek_harness::DeepSeekHarness;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer};

// Assemble the DeepSeek Harness service for the given data layout.
//
// `DEEPMATE_HARNESS_UI_URL` overrides the harness UI URL. The market cache
// freshness follows `Config.market.refresh_interval_seconds`, so a change to
// the refresh interval takes effect on the next launch.
pub fn build_harness(layout: &DataLayout) -> DeepSeekHarness {
    let platform = Arc::new(SystemPlatform);
    let mut harness = DeepSeekHarness::new(platform);
    if let Ok(url) = std::env::var("DEEPMATE_HARNESS_UI_URL") {
        harness = harness.with_ui_url(url);
    }
    harness = harness.with_market_ttl(
        load_config_or_default(layout)
            .market
            .refresh_interval_seconds,
    );
    harness.with_data_dir(layout.root().to_path_buf())
}

// Load the DeepMate configuration, recovering from a corrupt file.
//
// A missing config file is seeded with the defaults. A corrupt one is
// quarantined next to the config (the old bytes are preserved as
// `config.toml.invalid-<stamp>`) and replaced with defaults, so the app can
// always start and every later `Config::load`/`save` succeeds. When that
// happened, the returned path names the quarantined file so the caller can
// tell the user.
pub fn load_config_recovering(layout: &DataLayout) -> (Config, Option<PathBuf>) {
    let path = layout.config_path();
    // Seed a first run with the defaults so the file exists to be edited (and
    // so an operator can see what the app is configured with).
    if !path.exists() {
        if let Err(err) = Config::default().save(&path) {
            tracing::warn!(error = %err, "failed to write the default configuration");
        }
    }
    match Config::load_recovering(&path) {
        Ok(result) => result,
        Err(err) => {
            // Even recovery failed (unwritable directory, say). Report and run
            // on defaults; the caller surfaces the warning.
            tracing::error!(error = %err, "failed to load or recover configuration");
            eprintln!("warning: {err}; using default configuration");
            (Config::default(), None)
        }
    }
}

// Load the DeepMate configuration, falling back to defaults.
//
// An unreadable or invalid config file warns and yields defaults; a missing
// config file is seeded with the defaults on a best-effort basis.
pub fn load_config_or_default(layout: &DataLayout) -> Config {
    let (config, quarantined) = load_config_recovering(layout);
    if let Some(path) = quarantined {
        eprintln!(
            "warning: configuration file was invalid and has been reset (previous contents kept at {})",
            path.display()
        );
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
pub fn record_action(layout: &DataLayout, action: String) {
    let record = ActionRecord::new(action);
    if let Err(err) = layout.history().record(&record) {
        tracing::warn!(error = %err, "failed to record action history");
    }
}
