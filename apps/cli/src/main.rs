use std::sync::Arc;

use anyhow::{anyhow, Context};
use clap::{Parser, Subcommand};
use deepmate_core::model::{Model, Plugin, Profile, Provider};
use deepmate_core::registry::AdapterRegistry;
use deepmate_core::testkit::FakeAdapter;
use deepmate_platform::SystemPlatform;
use deepseek_harness::DeepSeekHarnessAdapter;
use tracing_subscriber::EnvFilter;

#[derive(Debug, Parser)]
#[command(name = "deepmate", version, about = "DeepMate control plane CLI")]
struct Cli {
    /// Adapter to use. Use "test" for a deterministic fake adapter.
    #[arg(long, global = true, default_value = "deepseek-harness")]
    adapter: String,

    /// Print machine-readable JSON output.
    #[arg(long, global = true)]
    json: bool,

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
    init_tracing();

    let cli = Cli::parse();
    tracing::debug!(adapter = %cli.adapter, json = cli.json, "deepmate startup");

    let registry = build_registry(&cli.adapter)?;

    if matches!(cli.command, Command::Adapters) {
        print_adapters(&registry, cli.json)?;
        return Ok(());
    }

    let adapter = registry
        .get(&cli.adapter)
        .with_context(|| format!("adapter not found: {}", cli.adapter))?;

    match cli.command {
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
        }
        Command::Open => {
            adapter.open_ui().await?;
            if cli.json {
                println!("{}", serde_json::json!({ "opened": true }));
            } else {
                println!("opened harness UI");
            }
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
        }
        Command::Runtime { action } => {
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
        }
        Command::Profile { action } => match action {
            ProfileAction::List => print_list("profiles", adapter.profiles().await?, cli.json)?,
        },
        Command::Provider { action } => match action {
            ProviderAction::List => print_list("providers", adapter.providers().await?, cli.json)?,
        },
        Command::Model { action } => match action {
            ModelAction::List => print_list("models", adapter.models().await?, cli.json)?,
        },
        Command::Plugin { action } => match action {
            PluginAction::List => print_list("plugins", adapter.plugins().await?, cli.json)?,
        },
    }

    Ok(())
}

fn init_tracing() {
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info"));
    tracing_subscriber::fmt()
        .with_env_filter(filter)
        // Keep stdout clean so machine-readable output (--json) is never
        // polluted by log lines.
        .with_writer(std::io::stderr)
        .init();
}

fn build_registry(adapter_id: &str) -> anyhow::Result<AdapterRegistry> {
    let mut registry = AdapterRegistry::new();
    match adapter_id {
        "test" => {
            registry.register(Box::new(FakeAdapter::healthy()));
        }
        "deepseek-harness" => {
            let platform = Arc::new(SystemPlatform);
            let mut adapter = DeepSeekHarnessAdapter::new(platform);
            if let Ok(url) = std::env::var("DEEPMATE_HARNESS_UI_URL") {
                adapter = adapter.with_ui_url(url);
            }
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
