use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{anyhow, Context};
use clap::{Parser, Subcommand};
use deepmate_app::{build_registry, init_tracing, load_config_or_default, record_action};
use deepmate_core::adapter::HarnessAdapter;
use deepmate_core::model::{
    is_outdated, MarketEntry, MarketSourceInfo, Model, Plugin, Profile, Provider,
};
use deepmate_core::registry::AdapterRegistry;
use deepmate_core::DataLayout;
use deepmate_platform::{PlatformService, SystemPlatform};

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
    /// Manage harness plugins.
    Plugin {
        #[command(subcommand)]
        action: PluginAction,
    },
    /// Discover plugins from market sources.
    Market {
        #[command(subcommand)]
        action: MarketAction,
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
    /// List installed plugins. Append --check-updates to consult the market
    /// for newer versions and mark outdated plugins.
    List {
        /// Check the market for newer versions and mark outdated plugins.
        #[arg(long)]
        check_updates: bool,
    },
    /// Install a plugin into a profile
    /// (e.g. `deepmate plugin install dsh-mnemon --profile web`).
    Install {
        /// Package name, optionally with a version range.
        spec: String,
        /// Target profile.
        #[arg(long, default_value = "web")]
        profile: String,
    },
    /// Remove a plugin from a profile.
    Remove {
        /// Installed package name.
        id: String,
        /// Target profile.
        #[arg(long, default_value = "web")]
        profile: String,
    },
    /// Update the plugins of a profile, or a single plugin when named.
    Update {
        /// Package name to update; every plugin when omitted.
        id: Option<String>,
        /// Target profile.
        #[arg(long, default_value = "web")]
        profile: String,
    },
}

#[derive(Debug, Subcommand)]
enum MarketAction {
    /// List the market sources DeepMate knows about.
    List,
    /// Search the market for plugins.
    Search {
        /// Search terms, matched against package names and descriptions.
        query: String,
    },
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

    let config = load_config_or_default(&layout);

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
    record_action(&layout, &cli.adapter, action);
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
                PluginAction::List { check_updates } => {
                    let mut plugins = adapter.plugins().await?;
                    if *check_updates {
                        require_capability(
                            adapter,
                            adapter.capabilities().marketplace,
                            "marketplace",
                        )?;
                        plugins = check_for_updates(adapter, plugins).await;
                    }
                    print_list("plugins", plugins, cli.json)?;
                    "cli.plugin.list".to_string()
                }
                PluginAction::Install { spec, profile } => {
                    adapter.install_plugin(profile, spec).await?;
                    if cli.json {
                        println!("{}", serde_json::json!({ "ok": true }));
                    } else {
                        println!("installed {spec} into profile {profile}");
                    }
                    "cli.plugin.install".to_string()
                }
                PluginAction::Remove { id, profile } => {
                    adapter.remove_plugin(profile, id).await?;
                    if cli.json {
                        println!("{}", serde_json::json!({ "ok": true }));
                    } else {
                        println!("removed {id} from profile {profile}");
                    }
                    "cli.plugin.remove".to_string()
                }
                PluginAction::Update { id, profile } => {
                    adapter.update_plugin(profile, id.as_deref()).await?;
                    if cli.json {
                        println!("{}", serde_json::json!({ "ok": true }));
                    } else {
                        match id {
                            Some(id) => println!("updated {id} in profile {profile}"),
                            None => println!("updated all plugins in profile {profile}"),
                        }
                    }
                    "cli.plugin.update".to_string()
                }
            }
        }
        Command::Market { action } => {
            require_capability(adapter, adapter.capabilities().marketplace, "marketplace")?;
            match action {
                MarketAction::List => {
                    let sources = adapter.market_sources().await?;
                    print_list("sources", sources, cli.json)?;
                    "cli.market.list".to_string()
                }
                MarketAction::Search { query } => {
                    let entries = adapter.search_plugins(query).await?;
                    print_list("market", entries, cli.json)?;
                    "cli.market.search".to_string()
                }
            }
        }
    };
    Ok(action)
}

// Ask the market for the latest version of every listed plugin and mark the
// outdated ones. Failures are per-plugin: a network error for one package
// must not fail the whole listing.
//
// The market lookups run concurrently so checking N plugins costs one round
// trip rather than N sequential ones.
async fn check_for_updates(adapter: &dyn HarnessAdapter, plugins: Vec<Plugin>) -> Vec<Plugin> {
    let lookups = plugins.iter().map(|plugin| {
        let id = plugin.id.clone();
        let installed = plugin.version.clone();
        async move {
            match adapter.search_plugins(&id).await {
                Ok(entries) => {
                    let latest = entries
                        .iter()
                        .find(|entry| entry.id == id)
                        .or_else(|| entries.first())
                        .and_then(|entry| entry.version.clone())?;
                    let outdated = installed
                        .as_deref()
                        .is_some_and(|installed| is_outdated(installed, &latest));
                    Some((latest, outdated))
                }
                Err(err) => {
                    tracing::warn!(plugin = %id, error = %err, "update check failed");
                    None
                }
            }
        }
    });
    let results: Vec<Option<(String, bool)>> = futures::future::join_all(lookups).await;
    let mut enriched = plugins;
    for (plugin, result) in enriched.iter_mut().zip(results) {
        if let Some((latest, outdated)) = result {
            plugin.latest = Some(latest);
            plugin.outdated = outdated;
        }
    }
    enriched
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
        let version = match &self.version {
            Some(version) => format!("v{version}"),
            None => "not installed".to_string(),
        };
        let mut line = format!(
            "{}/{} — {} ({version}, {state})",
            self.profile, self.id, self.name
        );
        if self.outdated {
            let latest = self.latest.as_deref().unwrap_or("?");
            line.push_str(&format!(" [outdated: latest {latest}]"));
        }
        line
    }
}

impl HumanLine for MarketEntry {
    fn line(&self) -> String {
        let source = match self.source {
            deepmate_core::model::MarketSource::Curated => "curated",
            deepmate_core::model::MarketSource::Community => "community",
        };
        let version = self.version.as_deref().unwrap_or("?");
        let mut line = match &self.description {
            Some(description) => {
                format!("{} — {description} (v{version}, {source})", self.id)
            }
            None => format!("{} (v{version}, {source})", self.id),
        };
        if let Some(publisher) = &self.publisher {
            line.push_str(&format!(" [by {publisher}]"));
        }
        if let Some(repository) = &self.repository {
            line.push_str(&format!(" [repo {repository}]"));
        }
        line
    }
}

impl HumanLine for MarketSourceInfo {
    fn line(&self) -> String {
        format!("{} — {}", self.id, self.description)
    }
}
