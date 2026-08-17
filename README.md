# DeepMate

**A lightweight companion and control plane for AI harnesses.**  
**Starting with DeepSeek Harness.**

DeepMate is a lightweight, cross-platform companion for managing an AI harness without replacing the harness itself.

It is designed to handle the things around the agent runtime — installation, lifecycle, models, providers, profiles, plugins, marketplaces, updates and diagnostics — while leaving the actual working interface to the harness.

For DeepSeek Harness, that means DeepMate can become the place where you manage the environment, then open the official Harness Web UI in your system browser when you are ready to work.

> DeepMate manages the harness. The harness does the work.

## Why DeepMate?

DeepSeek Harness is extremely extensible: models, tools, agent presets, profile bundles and many runtime capabilities can all evolve independently.

That flexibility is powerful, but it also creates a growing management surface:

- Which runtime is installed and running?
- Which provider and model are active?
- Which profiles exist?
- Which plugins are installed, enabled or outdated?
- Which plugin sources can be trusted?
- Is the local environment healthy?
- How do I move the same setup to another machine?

DeepMate aims to make those questions easy to answer without turning into another heavyweight IDE or browser wrapper.

## What DeepMate is

DeepMate is planned as a **control plane** for AI harnesses.

The first adapter targets **DeepSeek Harness**, with a core architecture designed so other harnesses and agent runtimes can be supported later without rewriting the product.

### Core areas

- **Runtime** — install, detect, start, stop, restart, update and inspect the harness runtime
- **Providers** — configure DeepSeek, OpenAI, Anthropic and compatible/custom endpoints
- **Models** — browse and manage model capabilities and defaults
- **Profiles** — manage harness profiles, bundles and configuration layers
- **Plugins** — install, update, remove and inspect plugins
- **Marketplace** — discover plugins from curated and community sources
- **Doctor** — diagnose runtime, dependency, port, configuration and compatibility problems
- **Open Harness** — launch the official working interface in the system browser

## What DeepMate is not

DeepMate deliberately avoids becoming another AI IDE.

It does **not** aim to provide its own:

- chat interface
- terminal
- file explorer
- Git diff viewer
- browser workspace
- editor
- embedded Harness Web UI

There is no reason to duplicate the surface that the harness already owns.

The goal is to stay small, fast and focused.

## Architecture

DeepMate is designed around adapters rather than direct coupling to one harness implementation.

```text
               DeepMate

        ┌──────────────────┐
        │ Desktop UI + CLI │
        └────────┬─────────┘
                 │
        ┌────────▼─────────┐
        │   Control Core   │
        │       Rust       │
        └────────┬─────────┘
                 │
        ┌────────▼─────────┐
        │ Harness Adapter  │
        └────────┬─────────┘
                 │
        ┌────────▼─────────┐
        │ DeepSeek Harness │
        └──────────────────┘
```

The UI does not need to know how a harness stores configuration or exposes its runtime. Those implementation details belong inside the adapter layer.

This keeps the product resilient as harnesses evolve and makes future multi-harness support possible.

For the full technical design, see **[docs/ARCHITECTURE.md](docs/ARCHITECTURE.md)**.

## Design principles

### 1. Lightweight by default

Keep the resident control surface small, fast and focused. The actual Harness work interface opens in the system browser.

### 2. Respect the harness as the source of truth

DeepMate manages the harness through its official interfaces and formats whenever possible instead of maintaining a second copy of harness-owned configuration.

### 3. Extend, do not fork

DeepMate uses the existing DeepSeek Harness profile, bundle, plugin, settings and provider mechanisms rather than inventing incompatible replacements.

### 4. Everything behind an adapter

Harness-specific behavior belongs behind a stable adapter boundary so the control core can stay generic.

### 5. Management surface, not work surface

DeepMate manages the environment around the agent. The actual conversation and execution experience remains owned by the harness.

### 6. Transparent local data

DeepMate-owned configuration and state use portable file formats that are easy to inspect, back up and move between machines.

## Technology stack

The current planned stack is:

- **Rust** — control core, adapters, runtime management and shared domain logic
- **Slint** — lightweight cross-platform desktop UI
- **Tokio** — asynchronous runtime and background work
- **reqwest + rustls** — network access
- **Serde** — serialization foundation
- **TOML** — human-owned DeepMate configuration
- **JSON** — structured state, cache and snapshots
- **JSONL** — append-oriented history and structured records
- **clap** — command-line interface
- **tracing** — structured logging and diagnostics
- **thiserror + anyhow** — domain and application error handling
- **OS secure credential store** — DeepMate-owned secrets
- **JSON-RPC over stdio** — planned public protocol for third-party harness adapters

The desktop app and CLI are both consumers of the same Rust control core.

## Getting started

Requirements: a recent stable Rust toolchain.

```bash
# Build everything
cargo build --workspace

# Run the CLI against the built-in deterministic test adapter
cargo run -- --adapter test status
cargo run -- --adapter test doctor

# Full workspace gate (formatting, clippy, tests)
make ci
```

CLI command surface:

```text
deepmate adapters              List registered adapters
deepmate detect                Detect the active harness
deepmate status                Show the active harness runtime status
deepmate open                  Open the harness UI in the system browser
deepmate doctor                Run environment diagnostics
deepmate runtime start|stop|restart
                                Control the harness runtime
deepmate profile list          List harness profiles
deepmate provider list         List configured providers
deepmate model list            List available models
deepmate plugin list           List installed plugins
```

Append `--json` to any command for machine-readable output. Logs go to
stderr, so JSON on stdout is never polluted.

## Data layout

DeepMate-owned data follows a simple file-based structure:

```text
<DeepMate Data>/
│
├── config.toml
├── adapters/
│   └── deepseek-harness.toml
├── cache/
│   ├── marketplace.json
│   └── plugin-metadata.json
├── history/
│   ├── actions.jsonl
│   └── doctor.jsonl
├── snapshots/
│   └── *.json
└── logs/
    └── deepmate.log
```

The actual root directory follows the operating system's standard application-data convention.

Harness-owned state remains owned by the active harness and is accessed through its adapter.

## Roadmap

### Phase 1 — DeepSeek Harness foundation

- Runtime discovery and lifecycle management
- Open official Harness Web UI
- Environment diagnostics
- Provider and model management
- Profile discovery and management
- Initial CLI

### Phase 2 — Ecosystem management

- Plugin inventory
- Install / update / remove flows
- Marketplace sources
- Compatibility checks
- Plugin diagnostics and trust signals

### Phase 3 — Portable setups

- Export / import configuration
- Environment snapshots
- Profile portability
- Private registries

### Phase 4 — More harnesses

- Stable public adapter protocol
- Additional harness / agent runtime adapters

## Project status

DeepMate is currently in **early development**, with a working Stage 1
foundation:

- Rust workspace with `deepmate-core`, `deepmate-platform` and the
  `deepseek-harness` adapter
- `deepmate` CLI with `adapters`, `detect`, `status`, `open`, `doctor`,
  `runtime`, `profile`, `provider`, `model` and `plugin` commands
- Deterministic `test` adapter for development and CI
- Cross-platform CI (fmt, clippy, tests) with a core purity gate

The first goal is to build a solid, minimal foundation for DeepSeek Harness
rather than rush into a large feature set.

## Related project

- [DeepSeek Harness](https://github.com/deepseek-ai/deepseek-harness)

## License

License information will be added as the project is prepared for its first public release.
