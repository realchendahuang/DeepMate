use std::io::IsTerminal;
use std::path::PathBuf;
use std::sync::Arc;

use anyhow::{anyhow, Context};
use clap::{Parser, Subcommand};
use deepmate_app::{build_harness, init_tracing, load_config_or_default, record_action};
use deepmate_core::model::{
    is_outdated, CompatStatus, MarketEntry, MarketSourceInfo, Model, Plugin, PluginOpEvent,
    PluginOpKind, Profile, Provider, RuntimeStatusKind, Surface,
};
use deepmate_core::DataLayout;
use deepmate_platform::{PlatformService, SystemPlatform};
use deepseek_harness::DeepSeekHarness;

#[derive(Debug, Parser)]
#[command(name = "deepmate", version, about = "DeepMate control plane CLI")]
struct Cli {
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
    /// Detect the active harness.
    Detect,
    /// Show the active harness runtime status (or one scenario's).
    Status {
        /// Scenario to report on (default: `web`).
        #[arg(long)]
        scenario: Option<String>,
    },
    /// Open the harness UI in the system browser (or one scenario's).
    Open {
        /// Scenario to open (default: `web`).
        #[arg(long)]
        scenario: Option<String>,
    },
    /// Run environment diagnostics.
    Doctor,
    /// Show DeepMate's own action history (most recent first).
    History {
        /// How many entries to show.
        #[arg(long, default_value_t = 20)]
        limit: usize,
    },
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
    /// Export, import and list portable setup snapshots.
    Snapshot {
        #[command(subcommand)]
        action: SnapshotAction,
    },
    /// Export and import DeepMate's own settings.
    Config {
        #[command(subcommand)]
        action: ConfigAction,
    },
    /// Update the CLI to the latest release (download, verify, self-replace).
    Update,
}

#[derive(Debug, Subcommand)]
enum RuntimeAction {
    /// Start a web scenario (default: `web`).
    Start {
        /// Scenario to start (default: `web`).
        #[arg(long)]
        scenario: Option<String>,
        /// Port override for web scenarios.
        #[arg(long)]
        port: Option<u16>,
    },
    /// Stop a scenario (default: `web`), or every running one with --all.
    Stop {
        /// Scenario to stop (default: `web`).
        #[arg(long)]
        scenario: Option<String>,
        /// Stop every running scenario.
        #[arg(long)]
        all: bool,
    },
    /// Restart a web scenario (default: `web`).
    Restart {
        /// Scenario to restart (default: `web`).
        #[arg(long)]
        scenario: Option<String>,
    },
    /// List every scenario's runtime state.
    List,
    /// Run one task against a task scenario.
    Task {
        /// Scenario to run the task against.
        #[arg(long)]
        scenario: Option<String>,
        /// The task prompt (everything after the flags).
        #[arg(trailing_var_arg = true, required = true)]
        prompt: Vec<String>,
    },
}

#[derive(Debug, Subcommand)]
enum ProfileAction {
    List,
    /// Create a new profile.
    Create {
        /// Profile name.
        name: String,
        /// Run surface: `web` (browser console) or `task` (one-shot tasks).
        #[arg(long)]
        surface: Option<String>,
    },
    /// Remove a profile.
    Remove {
        /// Profile name.
        name: String,
    },
}

#[derive(Debug, Subcommand)]
enum ProviderAction {
    List {
        /// Scenario to report on (default: `web`).
        #[arg(long)]
        scenario: Option<String>,
    },
    /// Create or update a provider.
    Set {
        /// Scenario the provider belongs to (default: `web`).
        #[arg(long)]
        scenario: Option<String>,
        /// Provider id (route name). Use `deepseek-official` to edit the
        /// built-in DeepSeek route.
        id: String,
        /// Display name.
        name: Option<String>,
        /// Wire protocol (`openai-responses`, `openai-completions`, ...).
        #[arg(long)]
        api: Option<String>,
        /// Base URL.
        #[arg(long)]
        base_url: Option<String>,
        /// The environment variable that holds the provider's secret.
        #[arg(long)]
        api_key_env: Option<String>,
        /// Raw JSON capability block (e.g. `{"supportsStore":false}`).
        #[arg(long)]
        compat: Option<String>,
    },
    /// Remove a provider.
    Remove {
        /// Scenario the provider belongs to (default: `web`).
        #[arg(long)]
        scenario: Option<String>,
        /// Provider id (route name).
        id: String,
    },
}

#[derive(Debug, Subcommand)]
enum ModelAction {
    List {
        /// Scenario to report on (default: `web`).
        #[arg(long)]
        scenario: Option<String>,
    },
    /// Create or update a model within a provider's catalog.
    Set {
        /// Scenario the model belongs to (default: `web`).
        #[arg(long)]
        scenario: Option<String>,
        /// Provider id (route name) the model belongs to.
        #[arg(long)]
        provider: String,
        /// Model id.
        id: String,
        /// Display name.
        #[arg(long)]
        name: Option<String>,
        /// Context window size.
        #[arg(long)]
        context_window: Option<u64>,
        /// Maximum output tokens.
        #[arg(long)]
        max_tokens: Option<u64>,
        /// Comma-separated input modalities (e.g. `text,image`).
        #[arg(long)]
        input: Option<String>,
        /// Raw JSON reasoning-efforts block (e.g. `{"max":"max"}`).
        #[arg(long)]
        reasoning_efforts: Option<String>,
        /// Raw JSON compat block.
        #[arg(long)]
        compat: Option<String>,
    },
    /// Remove a model from a provider's catalog.
    Remove {
        /// Scenario the model belongs to (default: `web`).
        #[arg(long)]
        scenario: Option<String>,
        /// Provider id (route name).
        #[arg(long)]
        provider: String,
        /// Model id.
        id: String,
    },
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
        /// Install even when the compatibility check reports the plugin as
        /// incompatible with the detected harness.
        #[arg(long)]
        force: bool,
    },
    /// Check a market package's compatibility with the detected harness.
    Check {
        /// Package name, optionally with a version range.
        spec: String,
    },
    /// Remove a plugin from a profile.
    Remove {
        /// Installed package name.
        id: String,
        /// Target profile.
        #[arg(long, default_value = "web")]
        profile: String,
    },
    /// Disable a plugin: uninstall it while remembering its package spec, so
    /// a later `enable` can restore the same version range.
    Disable {
        /// Installed package name.
        id: String,
        /// Target profile.
        #[arg(long, default_value = "web")]
        profile: String,
    },
    /// Enable a disabled plugin, reinstalling its recorded package spec.
    Enable {
        /// Package name to re-enable.
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

#[derive(Debug, Subcommand)]
enum SnapshotAction {
    /// Capture the current setup into a named snapshot.
    Export {
        /// Snapshot name (stored as `snapshots/<name>.json`).
        name: String,
    },
    /// Apply a stored snapshot (merge-style). Overwrites the current setup,
    /// so the current inventory is saved to `state/pre-snapshot-import-*.json`
    /// first and the command asks for confirmation unless `--yes` is given.
    Import {
        /// Snapshot name to import.
        name: String,
        /// Skip the confirmation prompt (required when stdin is not a
        /// terminal).
        #[arg(long)]
        yes: bool,
        /// Show what would be applied without changing anything.
        #[arg(long)]
        dry_run: bool,
    },
    /// List stored snapshots.
    List,
}

#[derive(Debug, Subcommand)]
enum ConfigAction {
    /// Write DeepMate's own settings to a portable JSON file.
    Export {
        /// Destination path for the backup document.
        path: PathBuf,
    },
    /// Replace DeepMate's own settings from a portable JSON file. The
    /// current settings are saved to `state/pre-config-import-*.json` first.
    Import {
        /// Path of the backup document to apply.
        path: PathBuf,
        /// Skip the confirmation prompt (required when stdin is not a
        /// terminal).
        #[arg(long)]
        yes: bool,
    },
}

// Exit codes the CLI contract promises. Anything unmapped still exits 1, but
// scripts can branch on the classifications that have a defined meaning.
const EXIT_FAILURE: i32 = 1;
const EXIT_TIMEOUT: i32 = 2;
const EXIT_DOCTOR_UNHEALTHY: i32 = 3;

#[tokio::main]
async fn main() {
    let cli = Cli::parse();
    match dispatch(cli).await {
        Ok(code) => std::process::exit(code),
        Err(err) => {
            // The failure contract follows `--json`: a script that asked for
            // machine output gets a machine-readable error, not a prose line
            // on stderr it has to parse.
            let core = err.downcast_ref::<deepmate_core::CoreError>();
            if json_requested() {
                let payload = match core {
                    Some(core) => serde_json::json!({
                        "ok": false,
                        "code": core.code(),
                        "message": core.to_string(),
                    }),
                    None => serde_json::json!({
                        "ok": false,
                        "code": "error",
                        "message": format!("{err:#}"),
                    }),
                };
                println!(
                    "{}",
                    serde_json::to_string_pretty(&payload).unwrap_or_default()
                );
            } else {
                eprintln!("error: {err:#}");
            }
            std::process::exit(match core {
                Some(deepmate_core::CoreError::Timeout(_)) => EXIT_TIMEOUT,
                _ => EXIT_FAILURE,
            });
        }
    }
}

// Whether `--json` appeared on the command line (parsed before clap so the
// error path can consult it without a successfully built `Cli`).
fn json_requested() -> bool {
    std::env::args().any(|arg| arg == "--json")
}

async fn dispatch(cli: Cli) -> anyhow::Result<i32> {
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
        json = cli.json,
        data_dir = %layout.root().display(),
        ?config,
        "deepmate startup"
    );

    let harness = build_harness(&layout);
    let (action, exit_code) = run(&cli, &harness, &layout).await?;

    // History recording is best-effort: a read-only data directory must not
    // break the command itself.
    record_action(&layout, action);
    Ok(exit_code)
}

// Dispatch the command, returning the history action name and the process
// exit code. `doctor` is the one command whose result *is* a status: a failing
// check must be visible to a script that only looks at `$?`, which previously
// always saw 0.
async fn run(
    cli: &Cli,
    harness: &DeepSeekHarness,
    layout: &DataLayout,
) -> anyhow::Result<(String, i32)> {
    // Set by commands whose result doubles as a status (doctor today).
    let mut exit_code = 0i32;
    // DeepMate's own settings are harness-independent, so the backup commands
    // run before the harness is consulted. The self-update is the same: it
    // replaces the running binary and has nothing to do with a harness.
    if matches!(cli.command, Command::Update) {
        update_self(cli.json).await?;
        return Ok(("cli.update".to_string(), 0));
    }

    if let Command::Config { action } = &cli.command {
        return match action {
            ConfigAction::Export { path } => {
                let config = deepmate_core::Config::load(&layout.config_path())?;
                let backup = deepmate_core::ConfigBackup::capture(&config);
                backup.save(path)?;
                if cli.json {
                    println!("{}", serde_json::to_string_pretty(&backup)?);
                } else {
                    println!("exported DeepMate settings to {}", path.display());
                }
                Ok(("cli.config.export".to_string(), 0))
            }
            ConfigAction::Import { path, yes } => {
                let backup = deepmate_core::ConfigBackup::load(path)?;
                confirm_destructive(
                    *yes,
                    cli.json,
                    &format!(
                        "importing {} replaces the current DeepMate settings",
                        path.display()
                    ),
                )?;
                // Keep the settings being replaced so the import is
                // reversible with the same command pointed at the copy.
                let current = deepmate_core::Config::load(&layout.config_path())?;
                let stamp = chrono::Utc::now().format("%Y%m%d-%H%M%S");
                let saved = layout
                    .state_dir()
                    .join(format!("pre-config-import-{stamp}.json"));
                deepmate_core::ConfigBackup::capture(&current)
                    .save(&saved)
                    .with_context(|| format!("failed to save {}", saved.display()))?;
                if !cli.json {
                    println!("saved the previous settings to {}", saved.display());
                }
                backup
                    .config
                    .save(&layout.config_path())
                    .with_context(|| format!("failed to apply {}", path.display()))?;
                if cli.json {
                    println!("{}", serde_json::json!({ "ok": true }));
                } else {
                    println!("imported DeepMate settings from {}", path.display());
                    println!(
                        "note: re-open the desktop app (or re-run commands) for the new settings to take effect"
                    );
                }
                Ok(("cli.config.import".to_string(), 0))
            }
        };
    }

    let action = match &cli.command {
        Command::Config { .. } => unreachable!(),
        Command::Update => unreachable!(),
        Command::Detect => {
            let detection = harness.detect().await?;
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&detection)?);
            } else {
                println!("found: {}", detection.found);
                if let Some(harness) = &detection.harness {
                    println!("harness: {} ({})", harness.id, harness.name);
                    if let Some(version) = &harness.version {
                        println!("version: {version}");
                    }
                }
                if let Some(detail) = &detection.detail {
                    println!("detail: {detail}");
                }
            }
            "cli.detect".to_string()
        }
        Command::Status { scenario } => {
            let profile = scenario.as_deref().unwrap_or("web");
            require_scenario(harness, profile).await?;
            let status = harness.status_scenario(profile).await?;
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&status)?);
            } else {
                println!("scenario: {profile}");
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
        Command::Open { scenario } => {
            let profile = scenario.as_deref().unwrap_or("web");
            require_scenario(harness, profile).await?;
            harness.open_ui_scenario(profile).await?;
            if cli.json {
                println!("{}", serde_json::json!({ "opened": true }));
            } else {
                println!("opened scenario {profile} UI");
            }
            "cli.open".to_string()
        }
        Command::Doctor => {
            let report = harness.doctor().await?;
            // A failing check is a status a script must be able to see; the
            // command still prints the full report either way.
            if report
                .checks
                .iter()
                .any(|check| check.status == deepmate_core::CheckStatus::Fail)
            {
                exit_code = EXIT_DOCTOR_UNHEALTHY;
            }
            if cli.json {
                println!("{}", serde_json::to_string_pretty(&report)?);
            } else {
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
        Command::History { limit } => {
            let (records, skipped) = layout.history().read_lenient()?;
            let total = records.len();
            // Most recent first: the reason to open this is "what did it just
            // do", and the file is append-only.
            let shown: Vec<_> = records.iter().rev().take(*limit).collect();
            if cli.json {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&serde_json::json!({
                        "total": total,
                        "skipped": skipped,
                        "records": shown,
                    }))?
                );
            } else {
                println!(
                    "{total} recorded action(s), showing the last {}",
                    shown.len()
                );
                for record in &shown {
                    match &record.detail {
                        Some(detail) => println!("{}  {}  ({detail})", record.time, record.action),
                        None => println!("{}  {}", record.time, record.action),
                    }
                }
                if skipped > 0 {
                    println!("({skipped} damaged line(s) skipped)");
                }
            }
            "cli.history".to_string()
        }
        Command::Runtime { action } => {
            let name = match action {
                RuntimeAction::Start { scenario, port } => {
                    let profile = scenario.clone().unwrap_or_else(|| "web".to_string());
                    require_scenario(harness, &profile).await?;
                    harness.start_scenario(&profile, *port).await?;
                    "start"
                }
                RuntimeAction::Stop { scenario, all } => {
                    if *all {
                        for instance in harness.instances().await? {
                            if instance.status == RuntimeStatusKind::Running {
                                harness.stop_scenario(&instance.profile).await?;
                            }
                        }
                    } else {
                        let profile = scenario.clone().unwrap_or_else(|| "web".to_string());
                        require_scenario(harness, &profile).await?;
                        harness.stop_scenario(&profile).await?;
                    }
                    "stop"
                }
                RuntimeAction::Restart { scenario } => {
                    let profile = scenario.clone().unwrap_or_else(|| "web".to_string());
                    require_scenario(harness, &profile).await?;
                    harness.restart_scenario(&profile).await?;
                    "restart"
                }
                RuntimeAction::List => {
                    let instances = harness.instances().await?;
                    if cli.json {
                        println!("{}", serde_json::to_string_pretty(&instances)?);
                    } else {
                        for instance in &instances {
                            let surface = match instance.surface {
                                Surface::Web => "web",
                                Surface::Task => "task",
                                Surface::Undetermined => "?",
                            };
                            let state = match instance.status {
                                RuntimeStatusKind::Running => "running",
                                _ => "stopped",
                            };
                            println!(
                                "{:<16} {:<6} {:<8} {}",
                                instance.profile,
                                surface,
                                state,
                                instance.url.as_deref().unwrap_or("-")
                            );
                        }
                    }
                    "list"
                }
                RuntimeAction::Task { scenario, prompt } => {
                    let profile = scenario.clone().ok_or_else(|| {
                        deepmate_core::error::CoreError::InvalidState(
                            "a task scenario must be named with --scenario".to_string(),
                        )
                    })?;
                    let prompt = prompt.join(" ");
                    let (tx, mut rx) = tokio::sync::mpsc::channel(64);
                    let stream = harness.run_task(&profile, &prompt, tx);
                    tokio::pin!(stream);
                    let mut finished: Option<(bool, Option<String>)> = None;
                    loop {
                        tokio::select! {
                            event = rx.recv() => {
                                if let Some(event) = event {
                                    match event {
                                        PluginOpEvent::Line { text } => {
                                            if cli.json {
                                                println!(
                                                    "{}",
                                                    serde_json::json!({"event": "line", "text": text})
                                                );
                                            } else {
                                                println!("{text}");
                                            }
                                        }
                                        PluginOpEvent::Finished { ok, detail } => {
                                            if cli.json {
                                                println!(
                                                    "{}",
                                                    serde_json::json!({
                                                        "event": "finished",
                                                        "ok": ok,
                                                        "detail": detail,
                                                    })
                                                );
                                            } else if !ok {
                                                if let Some(detail) = &detail {
                                                    eprintln!("{detail}");
                                                }
                                            }
                                            finished = Some((ok, detail));
                                        }
                                        _ => {}
                                    }
                                }
                            }
                            result = &mut stream => {
                                // The stream's own result is authoritative: a
                                // task may end without a Finished event when
                                // it fails before the child starts.
                                match result {
                                    Ok(()) => {
                                        if let Some((false, detail)) = finished {
                                            return Err(anyhow!(
                                                "{}",
                                                detail.unwrap_or_else(|| "task failed".to_string())
                                            ));
                                        }
                                    }
                                    Err(err) => return Err(err.into()),
                                }
                                break;
                            }
                        }
                    }
                    "task"
                }
            };
            match name {
                // `list` and `task` print their own structured output.
                "list" | "task" => {}
                _ if cli.json => println!("{}", serde_json::json!({ "ok": true })),
                _ => println!("runtime command completed"),
            }
            format!("cli.runtime.{name}")
        }
        Command::Profile { action } => {
            let action_name = match action {
                ProfileAction::List => {
                    print_list("profiles", harness.profiles().await?, cli.json)?;
                    "list"
                }
                ProfileAction::Create { name, surface } => {
                    let surface = match surface.as_deref() {
                        None | Some("web") => Surface::Web,
                        Some("task") => Surface::Task,
                        Some(other) => {
                            return Err(anyhow!("unknown surface {other:?} (use `web` or `task`)"))
                        }
                    };
                    harness.create_scenario(name, surface).await?;
                    if cli.json {
                        println!("{}", serde_json::json!({ "ok": true }));
                    } else {
                        println!("created scenario {name}");
                    }
                    "create"
                }
                ProfileAction::Remove { name } => {
                    harness.remove_profile(name).await?;
                    if cli.json {
                        println!("{}", serde_json::json!({ "ok": true }));
                    } else {
                        println!("removed profile {name}");
                    }
                    "remove"
                }
            };
            format!("cli.profile.{action_name}")
        }
        Command::Provider { action } => {
            let action_name = match action {
                ProviderAction::List { scenario } => {
                    let profile = scenario.clone().unwrap_or_else(|| "web".to_string());
                    require_scenario(harness, &profile).await?;
                    print_list(
                        "providers",
                        harness.providers_scenario(&profile).await?,
                        cli.json,
                    )?;
                    "list"
                }
                ProviderAction::Set {
                    scenario,
                    id,
                    name,
                    api,
                    base_url,
                    api_key_env,
                    compat,
                } => {
                    let profile = scenario.clone().unwrap_or_else(|| "web".to_string());
                    require_scenario(harness, &profile).await?;
                    let provider = Provider {
                        id: id.clone(),
                        name: name.clone().unwrap_or_else(|| id.clone()),
                        kind: if id == "deepseek-official" {
                            "deepseek".to_string()
                        } else {
                            "pi-ai".to_string()
                        },
                        api: api.clone(),
                        base_url: base_url.clone(),
                        api_key_env: api_key_env.clone(),
                        compat: compat.clone(),
                    };
                    harness.upsert_provider_scenario(&profile, provider).await?;
                    if cli.json {
                        println!("{}", serde_json::json!({ "ok": true }));
                    } else {
                        println!("saved provider {id} in scenario {profile}");
                    }
                    "set"
                }
                ProviderAction::Remove { scenario, id } => {
                    let profile = scenario.clone().unwrap_or_else(|| "web".to_string());
                    require_scenario(harness, &profile).await?;
                    harness.remove_provider_scenario(&profile, id).await?;
                    if cli.json {
                        println!("{}", serde_json::json!({ "ok": true }));
                    } else {
                        println!("removed provider {id} from scenario {profile}");
                    }
                    "remove"
                }
            };
            format!("cli.provider.{action_name}")
        }
        Command::Model { action } => {
            let action_name = match action {
                ModelAction::List { scenario } => {
                    let profile = scenario.clone().unwrap_or_else(|| "web".to_string());
                    require_scenario(harness, &profile).await?;
                    print_list("models", harness.models_scenario(&profile).await?, cli.json)?;
                    "list"
                }
                ModelAction::Set {
                    scenario,
                    provider,
                    id,
                    name,
                    context_window,
                    max_tokens,
                    input,
                    reasoning_efforts,
                    compat,
                } => {
                    let model = Model {
                        id: id.clone(),
                        name: name.clone().unwrap_or_else(|| id.clone()),
                        provider: Some(provider.clone()),
                        context_window: *context_window,
                        max_tokens: *max_tokens,
                        input: input
                            .as_ref()
                            .map(|value| {
                                value
                                    .split(',')
                                    .map(|part| part.trim().to_string())
                                    .collect()
                            })
                            .filter(|parts: &Vec<String>| !parts.is_empty()),
                        reasoning_efforts: reasoning_efforts.clone(),
                        compat: compat.clone(),
                    };
                    let profile = scenario.clone().unwrap_or_else(|| "web".to_string());
                    require_scenario(harness, &profile).await?;
                    harness
                        .upsert_model_scenario(&profile, provider, model)
                        .await?;
                    if cli.json {
                        println!("{}", serde_json::json!({ "ok": true }));
                    } else {
                        println!(
                            "saved model {id} under provider {provider} in scenario {profile}"
                        );
                    }
                    "set"
                }
                ModelAction::Remove {
                    scenario,
                    provider,
                    id,
                } => {
                    let profile = scenario.clone().unwrap_or_else(|| "web".to_string());
                    require_scenario(harness, &profile).await?;
                    harness
                        .remove_model_scenario(&profile, provider, id)
                        .await?;
                    if cli.json {
                        println!("{}", serde_json::json!({ "ok": true }));
                    } else {
                        println!(
                            "removed model {id} from provider {provider} in scenario {profile}"
                        );
                    }
                    "remove"
                }
            };
            format!("cli.model.{action_name}")
        }
        Command::Plugin { action } => match action {
            PluginAction::List { check_updates } => {
                let mut plugins = harness.plugins().await?;
                if *check_updates {
                    plugins = check_for_updates(harness, plugins).await;
                }
                print_list("plugins", plugins, cli.json)?;
                "cli.plugin.list".to_string()
            }
            PluginAction::Install {
                spec,
                profile,
                force,
            } => {
                preflight_compat(harness, spec, *force).await?;
                // Streamed, not silent: a pnpm install can take a minute and
                // the child's progress lines are what tells the user it is
                // working rather than hung.
                stream_plugin_op(harness, profile, PluginOpKind::Install, spec, cli.json).await?;
                "cli.plugin.install".to_string()
            }
            PluginAction::Check { spec } => {
                let report = harness.plugin_compat(spec).await?;
                if cli.json {
                    println!("{}", serde_json::to_string_pretty(&report)?);
                } else {
                    println!("{}: {}", spec, compat_word(report.status));
                    println!("  {}", report.message);
                    if let Some(version) = &report.harness_version {
                        println!("  harness version: {version}");
                    }
                }
                "cli.plugin.check".to_string()
            }
            PluginAction::Remove { id, profile } => {
                stream_plugin_op(harness, profile, PluginOpKind::Remove, id, cli.json).await?;
                "cli.plugin.remove".to_string()
            }
            PluginAction::Disable { id, profile } => {
                // Uses the harness method rather than a bare `plugin remove`:
                // disabling records the package spec so `enable` can restore
                // the same version range.
                harness.disable_plugin(profile, id).await?;
                if cli.json {
                    println!("{}", serde_json::json!({ "ok": true, "disabled": id }));
                } else {
                    println!("disabled {id} in scenario {profile}");
                }
                "cli.plugin.disable".to_string()
            }
            PluginAction::Enable { id, profile } => {
                harness.enable_plugin(profile, id).await?;
                if cli.json {
                    println!("{}", serde_json::json!({ "ok": true, "enabled": id }));
                } else {
                    println!("enabled {id} in scenario {profile}");
                }
                "cli.plugin.enable".to_string()
            }
            PluginAction::Update { id, profile } => {
                // Streamed, like install: `plugin update` can rewrite the
                // whole dependency tree and takes minutes.
                stream_plugin_op(
                    harness,
                    profile,
                    PluginOpKind::Update,
                    id.as_deref().unwrap_or(""),
                    cli.json,
                )
                .await?;
                "cli.plugin.update".to_string()
            }
        },
        Command::Market { action } => match action {
            MarketAction::List => {
                let sources = harness.market_sources().await?;
                print_list("sources", sources, cli.json)?;
                "cli.market.list".to_string()
            }
            MarketAction::Search { query } => {
                let entries = harness.search_plugins(query).await?;
                print_list("market", entries, cli.json)?;
                "cli.market.search".to_string()
            }
        },
        Command::Snapshot { action } => {
            let store = deepmate_core::SnapshotStore::new(layout.snapshots_dir());
            match action {
                SnapshotAction::Export { name } => {
                    let snapshot = harness.capture_snapshot().await?;
                    store.save(name, &snapshot)?;
                    if cli.json {
                        println!("{}", serde_json::to_string_pretty(&snapshot)?);
                    } else {
                        println!(
                            "exported snapshot {name} ({} profiles, {} providers, {} models, {} plugins)",
                            snapshot.profiles.len(),
                            snapshot.providers.len(),
                            snapshot.models.len(),
                            snapshot.plugins.len()
                        );
                    }
                    "cli.snapshot.export".to_string()
                }
                SnapshotAction::Import { name, yes, dry_run } => {
                    let snapshot = store.load(name)?;
                    if *dry_run {
                        println!("{}", serde_json::to_string_pretty(&snapshot)?);
                        if !cli.json {
                            println!(
                                "dry run: would apply {} profiles, {} providers, {} models, {} plugins (nothing changed)",
                                snapshot.profiles.len(),
                                snapshot.providers.len(),
                                snapshot.models.len(),
                                snapshot.plugins.len()
                            );
                        }
                        return Ok(("cli.snapshot.import".to_string(), 0));
                    }
                    confirm_destructive(
                        *yes,
                        cli.json,
                        &format!(
                            "importing snapshot {name} overwrites the current scenarios, providers and models"
                        ),
                    )?;
                    // A restore point is written first: importing is a
                    // replacement, and there was previously no way back.
                    match harness.write_restore_point("snapshot-import") {
                        Ok(Some(path)) => {
                            if !cli.json {
                                println!("saved a restore point to {}", path.display());
                            }
                        }
                        Ok(None) => {}
                        Err(err) => return Err(err).context(
                            "refusing to import: the pre-import restore point could not be written",
                        ),
                    }
                    let report = harness.apply_snapshot(&snapshot).await?;
                    if cli.json {
                        println!("{}", serde_json::to_string_pretty(&report)?);
                    } else {
                        println!(
                            "imported snapshot {name}: {} profiles, {} providers, {} models, {} plugins",
                            report.profiles, report.providers, report.models, report.plugins
                        );
                    }
                    "cli.snapshot.import".to_string()
                }
                SnapshotAction::List => {
                    let names = store.list()?;
                    if cli.json {
                        println!("{}", serde_json::to_string_pretty(&names)?);
                    } else {
                        for name in names {
                            println!("{name}");
                        }
                    }
                    "cli.snapshot.list".to_string()
                }
            }
        }
    };
    Ok((action, exit_code))
}

// Run a plugin operation with its output forwarded to the terminal. The
// failure detail comes back through the event stream (the harness service
// reports the same text the desktop shows).
async fn stream_plugin_op(
    harness: &DeepSeekHarness,
    profile: &str,
    kind: PluginOpKind,
    target: &str,
    json: bool,
) -> anyhow::Result<()> {
    let (tx, mut rx) = tokio::sync::mpsc::channel(64);
    let stream = harness.stream_plugin_op(profile, kind, Some(target), tx);
    tokio::pin!(stream);
    let mut failure: Option<String> = None;
    loop {
        tokio::select! {
            event = rx.recv() => {
                match event {
                    Some(PluginOpEvent::Line { text }) => {
                        if json {
                            println!("{}", serde_json::json!({"event": "line", "text": text}));
                        } else {
                            println!("{text}");
                        }
                    }
                    Some(PluginOpEvent::Finished { ok: false, detail }) => {
                        failure = detail;
                    }
                    Some(_) => {}
                    None => {}
                }
            }
            result = &mut stream => {
                match result {
                    Ok(()) => {
                        if json {
                            println!("{}", serde_json::json!({"ok": true}));
                        } else {
                            println!("done: {} {}", kind_word(kind), target);
                        }
                        return Ok(());
                    }
                    Err(err) => {
                        let message = failure.unwrap_or_else(|| err.to_string());
                        return Err(anyhow!("{message}"));
                    }
                }
            }
        }
    }
}

fn kind_word(kind: PluginOpKind) -> &'static str {
    match kind {
        PluginOpKind::Install => "installed",
        PluginOpKind::Remove => "removed",
        PluginOpKind::Update => "updated",
        PluginOpKind::Task => "ran",
    }
}

// Reject a scenario name that does not exist.
//
// Status/open/provider commands used to answer "not running" (or create a
// fresh profile directory) for a typo, so a mistyped name looked like a
// working-but-idle scenario instead of a mistake.
async fn require_scenario(harness: &DeepSeekHarness, profile: &str) -> anyhow::Result<()> {
    let known = harness.profiles().await?;
    if known.iter().any(|known| known.id == profile) {
        return Ok(());
    }
    let mut available: Vec<&str> = known.iter().map(|known| known.id.as_str()).collect();
    available.sort_unstable();
    // The default scenario is always addressable even before it is
    // scaffolded: `dsh web` is a built-in alias for it.
    if profile == "web" || available.is_empty() {
        return Ok(());
    }
    Err(anyhow!(
        "no such scenario: {profile} (available: {})",
        available.join(", ")
    ))
}

// Ask before an irreversible replacement. A non-interactive stdin must pass
// `--yes` explicitly: silently assuming consent is how a script wipes a
// user's setup, and a prompt nobody can answer would hang instead.
fn confirm_destructive(yes: bool, json: bool, what: &str) -> anyhow::Result<()> {
    if yes {
        return Ok(());
    }
    if !std::io::stdin().is_terminal() {
        return Err(anyhow!(
            "refusing to continue: {what}. Re-run with --yes to confirm"
        ));
    }
    eprint!("{what}\ncontinue? [y/N] ");
    let mut answer = String::new();
    std::io::stdin()
        .read_line(&mut answer)
        .context("failed to read the confirmation")?;
    let accepted = matches!(answer.trim().to_ascii_lowercase().as_str(), "y" | "yes");
    if accepted {
        Ok(())
    } else {
        let _ = json;
        Err(anyhow!("cancelled"))
    }
}

// Run the compatibility check before a plugin installation.
//
// A definite `Incompatible` verdict refuses the install unless `--force` was
// given; an `Unknown` verdict (nothing declared, or the check could not be
// evaluated) and a failed check (offline registry) only note the fact on
// stderr and let the install proceed — a missing signal must never block the
// workflow.
async fn preflight_compat(
    harness: &DeepSeekHarness,
    spec: &str,
    force: bool,
) -> anyhow::Result<()> {
    let report = match harness.plugin_compat(spec).await {
        Ok(report) => report,
        Err(err) => {
            eprintln!("note: compatibility check unavailable, continuing: {err}");
            return Ok(());
        }
    };
    match report.status {
        CompatStatus::Compatible => eprintln!("compatibility: {}", report.message),
        CompatStatus::Unknown => eprintln!("note: compatibility unknown: {}", report.message),
        CompatStatus::Incompatible if force => eprintln!(
            "warning: installing despite incompatibility: {}",
            report.message
        ),
        CompatStatus::Incompatible => {
            return Err(anyhow!(
                "{spec} is incompatible: {}; re-run with --force to install anyway",
                report.message
            ));
        }
    }
    Ok(())
}

// The short human word for a compatibility verdict.
fn compat_word(status: CompatStatus) -> &'static str {
    match status {
        CompatStatus::Compatible => "compatible",
        CompatStatus::Incompatible => "incompatible",
        CompatStatus::Unknown => "unknown",
    }
}

// Ask the market for the latest version of every listed plugin and mark the
// outdated ones. Failures are per-plugin: a network error for one package
// must not fail the whole listing.
//
// The market lookups run concurrently so checking N plugins costs one round
// trip rather than N sequential ones.
async fn check_for_updates(harness: &DeepSeekHarness, plugins: Vec<Plugin>) -> Vec<Plugin> {
    let lookups = plugins.iter().map(|plugin| {
        let id = plugin.id.clone();
        let installed = plugin.version.clone();
        async move {
            match harness.search_plugins(&id).await {
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

// The latest GitHub release, used by the self-update. `DEEPMATE_UPDATE_API_URL`
// overrides the endpoint so the flow can be exercised against a test release.
const UPDATE_API_URL: &str =
    "https://api.github.com/repos/realchendahuang/DeepMate/releases/latest";

#[derive(serde::Deserialize)]
struct ReleaseResponse {
    tag_name: String,
    assets: Vec<ReleaseAssetResponse>,
}

#[derive(serde::Deserialize)]
struct ReleaseAssetResponse {
    name: String,
    browser_download_url: String,
}

// Self-update: download the CLI archive for this platform, verify it against
// the published sha256, extract the `deepmate` binary and replace the
// running executable (rename-dance, safe on unix for a running binary).
//
// A no-op when the current version is already the latest.
async fn update_self(json: bool) -> anyhow::Result<()> {
    let current = env!("CARGO_PKG_VERSION");
    let client = reqwest::Client::builder()
        .user_agent(format!("deepmate/{current}"))
        .timeout(std::time::Duration::from_secs(600))
        .build()
        .context("failed to build HTTP client")?;
    let api =
        std::env::var("DEEPMATE_UPDATE_API_URL").unwrap_or_else(|_| UPDATE_API_URL.to_string());

    let release: ReleaseResponse = client
        .get(&api)
        .send()
        .await
        .context("update check failed")?
        .error_for_status()
        .context("update check failed")?
        .json()
        .await
        .context("invalid release response")?;

    if !deepmate_core::is_newer_version(&release.tag_name, current) {
        if json {
            println!(
                "{}",
                serde_json::json!({ "updated": false, "current": current })
            );
        } else {
            println!("deepmate {current} is up to date");
        }
        return Ok(());
    }

    let assets: Vec<deepmate_core::ReleaseAsset> = release
        .assets
        .iter()
        .map(|asset| deepmate_core::ReleaseAsset {
            name: asset.name.clone(),
            url: asset.browser_download_url.clone(),
        })
        .collect();
    let triple = deepmate_core::target_triple();
    let (tarball, checksum) =
        deepmate_core::pick_release_asset(&assets, &triple, deepmate_core::AssetKind::Tarball)
            .ok_or_else(|| {
                anyhow!(
                    "release {} has no CLI archive for {triple}",
                    release.tag_name
                )
            })?;

    let work = std::env::temp_dir().join(format!("deepmate-update-{}", std::process::id()));
    std::fs::create_dir_all(&work).context("failed to create the update work directory")?;

    let archive_path = work.join(&tarball.name);
    let bytes = client
        .get(&tarball.url)
        .send()
        .await
        .context("failed to download the release archive")?
        .error_for_status()
        .context("failed to download the release archive")?
        .bytes()
        .await
        .context("failed to download the release archive")?;
    std::fs::write(&archive_path, &bytes).context("failed to write the release archive")?;

    let sum_text = client
        .get(&checksum.url)
        .send()
        .await
        .context("failed to download the checksum")?
        .error_for_status()
        .context("failed to download the checksum")?
        .text()
        .await
        .context("failed to download the checksum")?;
    let expected = deepmate_core::parse_checksum_file(&sum_text)
        .ok_or_else(|| anyhow!("the published checksum file is invalid"))?;
    deepmate_core::verify_sha256(&archive_path, &expected)
        .context("the downloaded archive failed verification")?;

    let staged = work.join("deepmate.new");
    extract_binary(&archive_path, "deepmate", &staged)
        .context("failed to extract the binary from the archive")?;
    let exe = replace_self(&staged)?;
    let _ = std::fs::remove_dir_all(&work);

    if json {
        println!(
            "{}",
            serde_json::json!({
                "updated": true,
                "from": current,
                "to": release.tag_name.trim_start_matches('v'),
                "path": exe.display().to_string(),
            })
        );
    } else {
        println!(
            "updated deepmate {current} -> {}",
            release.tag_name.trim_start_matches('v')
        );
        println!("installed at {}; re-run deepmate to use it", exe.display());
    }
    Ok(())
}

// Extract one file from a `.tar.gz` by its base name.
fn extract_binary(
    archive: &std::path::Path,
    name: &str,
    dest: &std::path::Path,
) -> anyhow::Result<()> {
    let file = std::fs::File::open(archive)?;
    let gz = flate2::read::GzDecoder::new(file);
    let mut tar = tar::Archive::new(gz);
    for entry in tar.entries()? {
        let mut entry = entry?;
        let matches = entry
            .path()?
            .file_name()
            .and_then(|part| part.to_str())
            .is_some_and(|part| part == name);
        if matches {
            let mut out = std::fs::File::create(dest)?;
            std::io::copy(&mut entry, &mut out)?;
            return Ok(());
        }
    }
    Err(anyhow!("the archive does not contain {name}"))
}

// Replace the running executable with `new_binary`, restoring the old file
// if the copy fails.
fn replace_self(new_binary: &std::path::Path) -> anyhow::Result<std::path::PathBuf> {
    let exe = std::env::current_exe().context("cannot locate the running binary")?;
    let backup = exe.with_extension("bak");
    std::fs::rename(&exe, &backup).with_context(|| format!("cannot stage {}", exe.display()))?;
    if let Err(err) = std::fs::copy(new_binary, &exe) {
        let _ = std::fs::rename(&backup, &exe);
        return Err(anyhow!("cannot install the new binary: {err}"));
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = std::fs::set_permissions(&exe, std::fs::Permissions::from_mode(0o755));
    }
    let _ = std::fs::remove_file(&backup);
    Ok(exe)
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
        if let (Some(popularity), Some(quality)) = (self.popularity, self.quality) {
            line.push_str(&format!(
                " [popularity {popularity:.1}, quality {quality:.1}]"
            ));
        }
        line
    }
}

impl HumanLine for MarketSourceInfo {
    fn line(&self) -> String {
        format!("{} — {}", self.id, self.description)
    }
}
