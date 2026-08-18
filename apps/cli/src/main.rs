use std::path::{Path, PathBuf};
use std::sync::Arc;

use anyhow::{anyhow, Context};
use clap::{Parser, Subcommand};
use deepmate_core::adapter::{AdapterCapabilities, HarnessAdapter};
use deepmate_core::model::{Model, Plugin, Profile, Provider};
use deepmate_core::registry::AdapterRegistry;
use deepmate_core::testkit::FakeAdapter;
use deepmate_core::{ActionRecord, Config, DataLayout};
use deepmate_platform::{PlatformService, SystemPlatform};
use deepseek_harness::DeepSeekHarnessAdapter;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;
use tracing_subscriber::{EnvFilter, Layer};

#[derive(Debug, Parser)]
#[command(name = "deepmate", version, about = "DeepMate control plane CLI")]
struct Cli {
    /// Adapter to use. Use "test" for a deterministic fake adapter.
    #[arg(long, global = true, default_value = "deepseek-harness")]
    adapter: String,

    /// Print machine-readable JSON output.
    #[arg(long, global = true)]
    json: bool,

    /// Override the DeepMate data directory (default: OS application-data convention).
    #[arg(long, global = true)]
    data_dir: Option<PathBuf>,

    #[command(subcommand)]
    command: Command,
}

#[derive(Debug, Subcommand)]
enum Command {
    /// List registered adapters.
    Adapters,
    /// Detect the active harness.
    Detect,
    /// Show the active harness runtime status.
    Status,
    /// Open the harness UI in the system browser.
    Open,
    /// Run environment diagnostics.
    Doctor,
    /// Control the harness runtime.
    Runtime {
        #[command(subcommand)]
        action: RuntimeAction,
    },
    /// List harness profiles.
    Profile {
        #[command(subcommand)]
        action: ProfileAction,
    },
    /// List configured providers.
    Provider {
        #[command(subcommand)]
        action: ProviderAction,
    },
    /// List available models.
    Model {
        #[command(subcommand)]
        action: ModelAction,
    },
    /// List installed plugins.
    Plugin {
        #[command(subcommand)]
        action: PluginAction,
    },
}

#[derive(Debug, Subcommand)]
enum RuntimeAction {
    Start,
    Stop,
    Restart,
}

#[derive(Debug, Subcommand)]
enum ProfileAction {
    List,
}

#[derive(Debug, Subcommand)]
enum ProviderAction {
    List,
}

#[derive(Debug, Subcommand)]
enum ModelAction {
    List,
}

#[derive(Debug, Subcommand)]
enum PluginAction {
    List,
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    let platform = Arc::new(SystemPlatform);
    let data_dir = match &cli.data_dir {
        Some(dir) => dir.clone(),
        None => platform.data_dir()?,
    };
    let layout = DataLayout::new(data_dir);
    layout
        .ensure()
        .context("failed to initialize the DeepMate data directory")?;

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

    let _guard = init_tracing(&layout.logs_dir());
    tracing::debug!(
        adapter = %cli.adapter,
        json = cli.json,
        data_dir = %layout.root().display(),
        ?config,
        "deepmate startup"
    );

    let registry = build_registry(&cli.adapter, &layout)?;
    let action = run(&cli, &registry).await?;

    // History recording is best-effort: a read-only data directory must not
    // break the command itself.
    let record = ActionRecord::new(action).with_adapter(cli.adapter.clone());
    if let Err(err) = layout.history().record(&record) {
        tracing::warn!(error = %err, "failed to record action history");
    }
    Ok(())
}

// Dispatch the command and return the history action name on success.
async fn run(cli: &Cli, registry: &AdapterRegistry) -> anyhow::Result<String> {
    if matches!(cli.command, Command::Adapters) {
        print_adapters(registry, cli.json)?;
        return Ok("cli.adapters".to_string());
    }

    let adapter = registry
        .get(&cli.adapter)
        .with_context(|| format!("adapter not found: {}", cli.adapter))?;

    let action = match &cli.command {
        Command::Adapters => unreachable!(),
        Command::Detect => {
            let detection = adapter.detect().await?;
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&detection)?);
            } else {
                println!("found: {}", detection.found);
                if let Some(harness) = &detection.harness {
                    println!("harness: {} ({})", harness.id, harness.name);
                    if let Some(version) = &harness.version {
                        println!("version: {version}");
                    }
                    println!("adapter version: {}", harness.adapter_version);
                }
                if let Some(detail) = &detection.detail {
                    println!("detail: {detail}");
                }
            }
            "cli.detect".to_string()
        }
        Command::Status => {
            let status = adapter.status().await?;
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&status)?);
            } else {
                println!("adapter: {}", adapter.metadata().id);
                println!("status: {:?}", status.kind);
                if let Some(pid) = status.pid {
                    println!("pid: {pid}");
                }
                if let Some(message) = status.message {
                    println!("message: {message}");
                }
            }
            "cli.status".to_string()
        }
        Command::Open => {
            adapter.open_ui().await?;
            if cli.json {
                println!("{}", serde_json::json!({ "opened": true }));
            } else {
                println!("opened harness UI");
            }
            "cli.open".to_string()
        }
        Command::Doctor => {
            let report = adapter.doctor().await?;
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
                println!("adapter: {}", report.adapter_id);
                for check in report.checks {
                    let status = format!("{:?}", check.status).to_lowercase();
                    println!("- [{}] {} ({})", status, check.summary, check.id);
                    if let Some(details) = check.details {
                        println!("    details: {details}");
                    }
                    if let Some(action) = check.suggested_action {
                        println!("    action: {action}");
                    }
                }
            }
            "cli.doctor".to_string()
        }
        Command::Runtime { action } => {
            require_capability(adapter, adapter.capabilities().runtime, "runtime control")?;
            match action {
                RuntimeAction::Start => adapter.start().await?,
                RuntimeAction::Stop => adapter.stop().await?,
                RuntimeAction::Restart => adapter.restart().await?,
            }
            if cli.json {
                println!("{}", serde_json::json!({ "ok": true }));
            } else {
                println!("runtime command completed");
            }
            let name = match action {
                RuntimeAction::Start => "start",
                RuntimeAction::Stop => "stop",
                RuntimeAction::Restart => "restart",
            };
            format!("cli.runtime.{name}")
        }
        Command::Profile { action } => {
            require_capability(adapter, adapter.capabilities().profiles, "profiles")?;
            match action {
                ProfileAction::List => print_list("profiles", adapter.profiles().await?, cli.json)?,
            }
            "cli.profile.list".to_string()
        }
        Command::Provider { action } => {
            require_capability(adapter, adapter.capabilities().providers, "providers")?;
            match action {
                ProviderAction::List => {
                    print_list("providers", adapter.providers().await?, cli.json)?
                }
            }
            "cli.provider.list".to_string()
        }
        Command::Model { action } => {
            require_capability(adapter, adapter.capabilities().models, "models")?;
            match action {
                ModelAction::List => print_list("models", adapter.models().await?, cli.json)?,
            }
            "cli.model.list".to_string()
        }
        Command::Plugin { action } => {
            require_capability(adapter, adapter.capabilities().plugins, "plugins")?;
            match action {
                PluginAction::List => print_list("plugins", adapter.plugins().await?, cli.json)?,
            }
            "cli.plugin.list".to_string()
        }
    };
    Ok(action)
}

// Reject commands the active adapter does not declare support for, instead
// of silently returning empty results.
fn require_capability(
    adapter: &dyn HarnessAdapter,
    supported: bool,
    what: &str,
) -> anyhow::Result<()> {
    if supported {
        Ok(())
    } else {
        Err(anyhow!(
            "adapter '{}' does not support {what}",
            adapter.metadata().id
        ))
    }
}

fn init_tracing(logs_dir: &Path) -> tracing_appender::non_blocking::WorkerGuard {
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

fn build_registry(adapter_id: &str, layout: &DataLayout) -> anyhow::Result<AdapterRegistry> {
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

fn print_adapters(registry: &AdapterRegistry, json: bool) -> anyhow::Result<()> {
    if json {
        println!("{}", serde_json::to_string_pretty(&registry.list())?);
    } else {
        for metadata in registry.list() {
            println!("{} ({})", metadata.id, metadata.name);
        }
    }
    Ok(())
}

// Print a normalized entity list in JSON or human-readable form.
//
// `what` names the entity kind for the JSON envelope; items must serialize
// with serde and implement HumanLine for the plain-text form.
fn print_list<T>(what: &str, items: Vec<T>, json: bool) -> anyhow::Result<()>
where
    T: serde::Serialize + HumanLine,
{
    if json {
        println!(
            "{}",
            serde_json::to_string_pretty(&serde_json::json!({ what: items }))?
        );
    } else if items.is_empty() {
        println!("no {what} found");
    } else {
        for item in &items {
            println!("{}", item.line());
        }
    }
    Ok(())
}

// Human-readable one-line rendering for normalized domain entities.
trait HumanLine {
    fn line(&self) -> String;
}

impl HumanLine for Profile {
    fn line(&self) -> String {
        match &self.description {
            Some(description) => format!("{} — {} ({})", self.id, self.name, description),
            None => format!("{} — {}", self.id, self.name),
        }
    }
}

impl HumanLine for Provider {
    fn line(&self) -> String {
        format!("{} — {} ({})", self.id, self.name, self.kind)
    }
}

impl HumanLine for Model {
    fn line(&self) -> String {
        match &self.provider {
            Some(provider) => format!("{} — {} (provider: {})", self.id, self.name, provider),
            None => format!("{} — {}", self.id, self.name),
        }
    }
}

impl HumanLine for Plugin {
    fn line(&self) -> String {
        let state = if self.enabled { "enabled" } else { "disabled" };
        match &self.version {
            Some(version) => format!("{} — {} (v{version}, {state})", self.id, self.name),
            None => format!("{} — {} ({state})", self.id, self.name),
        }
    }
}
