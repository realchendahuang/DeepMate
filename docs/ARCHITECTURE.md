# DeepMate Architecture

This document defines the current technical direction for DeepMate.

DeepMate is a lightweight, cross-platform companion and control plane for AI harnesses. The first supported harness is DeepSeek Harness, but the architecture is designed around adapters so additional harnesses can be integrated later without rewriting the core product.

> DeepMate manages the harness. The harness does the work.

## Goals

DeepMate is designed around a few constraints:

- Stay lightweight enough to run comfortably as a tray companion.
- Use a native desktop UI without embedding the Harness Web UI.
- Keep the control core independent from any single harness.
- Treat the harness as the source of truth for harness-owned state.
- Make adapters replaceable as upstream harness APIs evolve.
- Keep DeepMate-owned data transparent, portable and easy to inspect.
- Expose the same core capabilities to both the desktop app and CLI.
- Leave room for third-party harness adapters in the future.

## High-level architecture

```text
                         DeepMate

              ┌────────────────────────┐
              │                        │
              │   Desktop       CLI    │
              │   Slint       deepmate │
              │      \         /       │
              │       \       /        │
              │        Control Core    │
              │           Rust         │
              │                        │
              └───────────┬────────────┘
                          │
                  Harness Adapter API
                          │
             ┌────────────┴────────────┐
             │                         │
     DeepSeek Harness Adapter    Future Adapters
             │                         │
             ▼                         ▼
      DeepSeek Harness          Other Harnesses
```

The desktop application and CLI are consumers of the same Rust core. Harness-specific behavior lives behind the adapter boundary.

## Technology stack

### Core language

**Rust** is the primary implementation language for DeepMate.

The control core, adapter interfaces, runtime management, configuration, marketplace logic, diagnostics and CLI should all live in Rust.

### Desktop UI

**Slint** is the planned desktop UI layer.

The UI should stay thin: it renders state from the core, sends commands to the core and reacts to events. Business rules should not live inside UI callbacks.

The desktop app should support:

- Windows
- macOS
- Linux
- system tray / menu bar operation
- a lightweight control-center window
- system theme integration where practical

### Async runtime

**Tokio** is used for asynchronous work such as:

- process output streaming
- runtime health checks
- plugin installation progress
- marketplace requests
- downloads
- diagnostics
- adapter communication

The Slint event loop and Tokio runtime remain separate. Background work reports results back to the UI through a small application state / event boundary.

### HTTP

**reqwest + rustls** is used for network access.

Typical consumers include:

- marketplace sources
- npm-compatible registries
- GitHub-backed sources
- adapter-specific HTTP APIs
- update metadata

### Serialization and file formats

DeepMate uses **Serde** as the serialization foundation.

Planned formats:

- **TOML** — human-owned DeepMate configuration
- **JSON** — machine-managed structured state, cache and snapshots
- **JSONL** — append-only history and structured event records
- **plain text logs** — normal runtime logging and support diagnostics

### CLI

**clap** is used for the `deepmate` command-line interface.

The CLI is a first-class consumer of `deepmate-core`, not a wrapper around the desktop application.

Example command surface:

```text
deepmate status
deepmate open
deepmate doctor

deepmate runtime start
deepmate runtime stop
deepmate runtime restart

deepmate profile list

deepmate model list

deepmate plugin list
deepmate plugin search <query>
deepmate plugin install <package>
```

The exact command surface may evolve, but the architectural rule stays the same: desktop and CLI call the same core services.

### Logging and errors

Planned libraries:

- **tracing** / **tracing-subscriber** for structured logging
- **tracing-appender** for file output
- **thiserror** for library/domain errors
- **anyhow** at application boundaries

Diagnostics should preserve machine-readable error categories so the Doctor surface can provide actionable explanations instead of only raw strings.

### Secrets

Harness-owned credentials remain owned by the harness and are accessed through the corresponding adapter.

DeepMate-owned secrets, such as private registry credentials, should use the operating system's secure credential store through a cross-platform Rust keyring abstraction.

Secrets should never be written into normal TOML, JSON, JSONL or log files.

## Workspace layout

The planned Rust workspace is:

```text
DeepMate/
│
├── Cargo.toml
├── README.md
├── docs/
│   └── ARCHITECTURE.md
│
├── crates/
│   ├── deepmate-core/
│   ├── deepmate-protocol/
│   ├── deepmate-market/
│   ├── deepmate-platform/
│   └── adapters/
│       └── deepseek-harness/
│
└── apps/
    ├── desktop/
    │   ├── src/
    │   └── ui/
    └── cli/
```

The exact crate boundaries can evolve as implementation starts, but the core separation should remain intact.

## Core responsibilities

`deepmate-core` owns product-level use cases and shared domain types.

Major areas:

```text
RuntimeManager
ProfileManager
ProviderManager
ModelManager
PluginManager
MarketManager
Doctor
SnapshotManager
AdapterRegistry
```

The core does not know how DeepSeek Harness stores its settings or how another harness implements plugins. It works with normalized domain models exposed by adapters.

Example normalized types:

```text
HarnessInfo
RuntimeStatus
Profile
Provider
Model
Plugin
PluginVersion
DoctorReport
Snapshot
Capability
```

## Harness adapter layer

The adapter boundary is the most important architectural contract in DeepMate.

Conceptually, each adapter implements capabilities such as:

```rust
trait HarnessAdapter {
    fn metadata(&self) -> AdapterMetadata;
    fn capabilities(&self) -> AdapterCapabilities;

    async fn detect(&self) -> Result<Detection>;
    async fn status(&self) -> Result<RuntimeStatus>;

    async fn start(&self) -> Result<()>;
    async fn stop(&self) -> Result<()>;
    async fn restart(&self) -> Result<()>;
    async fn open_ui(&self) -> Result<()>;

    async fn profiles(&self) -> Result<Vec<Profile>>;
    async fn providers(&self) -> Result<Vec<Provider>>;
    async fn models(&self) -> Result<Vec<Model>>;
    async fn plugins(&self) -> Result<Vec<Plugin>>;

    async fn doctor(&self) -> Result<DoctorReport>;
}
```

The real Rust trait will be shaped by implementation experience. The important part is capability-based separation rather than exposing harness-specific files or commands directly to the UI.

### Capability-driven UI

Not every harness will support the same concepts.

An adapter therefore declares capabilities, for example:

```text
runtime
profiles
providers
models
plugins
marketplace
skills
mcp
snapshots
```

The desktop UI and CLI should expose only the features supported by the active adapter.

This lets DeepMate remain generic without forcing every harness into the same feature model.

## DeepSeek Harness adapter

The first adapter targets DeepSeek Harness.

The adapter should prefer official DeepSeek Harness interfaces and commands whenever they cover the required operation.

Initial integration order:

1. DeepSeek Harness CLI
2. official runtime / remote / SDK surfaces as they become suitable
3. a minimal DeepMate bridge only for capabilities that need an in-process Harness service
4. direct file access only where it is part of a stable, documented contract

For plugin operations, the adapter should use the official Harness plugin workflow so profile initialization, package management and bundle reconciliation remain owned by DeepSeek Harness.

Harness Web UI remains the working interface. DeepMate opens it with the operating system's default browser.

## Adapter compatibility layer

Upstream harnesses can change quickly. DeepMate should isolate version-specific behavior inside the adapter.

Conceptually:

```text
DeepMate Core
     │
     ▼
Harness Adapter
     │
     ├── compatibility rules
     ├── feature detection
     ├── CLI/API mapping
     └── version-specific behavior
```

The rest of the product should not depend on individual Harness config keys, file paths or internal implementation details.

## Third-party adapter protocol

First-party adapters can be compiled into DeepMate.

A future public third-party adapter system should use a process boundary rather than a language-specific binary ABI.

Planned protocol direction:

**JSON-RPC over stdio**

```text
DeepMate
   │
   │ JSON-RPC / stdio
   ▼
Third-party Adapter Process
   │
   ▼
External Harness
```

This allows adapters to be implemented in Rust, Go, Python, Node.js or other languages while keeping the DeepMate host stable.

A future adapter manifest could describe:

```json
{
  "id": "deepseek-harness",
  "name": "DeepSeek Harness",
  "version": "1.0.0",
  "protocol": 1,
  "executable": "deepmate-adapter-dsh",
  "capabilities": [
    "runtime",
    "profiles",
    "providers",
    "models",
    "plugins",
    "doctor"
  ]
}
```

The protocol should be versioned independently from the DeepMate application version.

## Platform abstraction

Operating-system-specific behavior should live behind a platform service boundary.

```text
PlatformService
│
├── macOS
│   ├── process management
│   ├── launchd integration
│   ├── secure credential store
│   └── open URL / file
│
├── Windows
│   ├── process management
│   ├── user startup / scheduled task integration
│   ├── secure credential store
│   └── shell open
│
└── Linux
    ├── process management
    ├── systemd --user integration
    ├── secure credential store
    └── xdg-open
```

Platform-specific conditionals should stay concentrated in this layer rather than being scattered through core business logic.

## Runtime management

DeepMate should support two runtime strategies where an adapter allows it.

### System runtime

Use an already installed harness and its existing dependencies.

This is the preferred path for users who already have a working local environment.

### Managed runtime

DeepMate may install and maintain an isolated runtime for users who want a guided setup.

Managed runtimes should live outside the application bundle and remain independently replaceable or removable.

The desktop app itself should stay small even when a managed harness runtime is installed.

## Process ownership

DeepMate should avoid introducing a permanent application daemon unless a concrete feature requires one.

The desktop process can remain in the system tray while active. Long-running harness processes should use the adapter's platform-appropriate runtime strategy.

Conceptually:

```text
DeepMate Desktop
      │
      ▼
DeepMate Core
      │
      ▼
Platform / Harness Adapter
      │
      ▼
Harness Runtime
```

The harness runtime does not need to be tied to the lifetime of the main control-center window.

## Marketplace architecture

Marketplace support is source-driven.

```text
MarketSource
│
├── curated registry
├── npm-compatible registry
├── GitHub-backed source
├── private registry
├── JSON registry
└── local source
```

Each source normalizes data into a shared `PluginRecord` model.

Example fields:

```text
id
name
package
version
repository
source
description
category
capabilities
compatibility
dependencies
risk_flags
verified_source
updated_at
```

The marketplace layer is separate from the Harness adapter: discovery can be generic, while installation remains adapter-owned because each harness can have different installation semantics.

## Plugin trust metadata

DeepMate should make plugin provenance and risk signals visible before installation.

Potential signals include:

- source repository
- package-to-repository consistency
- manifest validity
- install/build scripts
- host-side execution
- compatibility range
- dependency conflicts
- maintenance freshness

These signals are metadata and diagnostics. Installation authority remains with the user.

## File-based DeepMate data

DeepMate-owned state should remain transparent and portable.

Conceptual data layout:

```text
<DeepMate Data>/
│
├── config.toml
│
├── adapters/
│   └── deepseek-harness.toml
│
├── cache/
│   ├── marketplace.json
│   └── plugin-metadata.json
│
├── history/
│   ├── actions.jsonl
│   └── doctor.jsonl
│
├── snapshots/
│   ├── coding.json
│   └── research.json
│
├── state/
│   └── harness.pid
│
└── logs/
    └── deepmate.log
```

The actual root directory should follow each operating system's standard application-data convention.

### TOML

TOML is for human-owned configuration that users may reasonably inspect or edit.

Example:

```toml
[general]
language = "en"
auto_start = true
check_updates = true

[ui]
theme = "system"
close_to_tray = true

[market]
default_source = "community"
refresh_interval_seconds = 3600
```

### JSON

JSON is for machine-managed structured state, cached indexes and portable snapshots.

Examples:

```text
cache/marketplace.json
cache/plugin-metadata.json
snapshots/coding.json
```

Caches should be rebuildable from their upstream source.

### JSONL

JSONL is for append-oriented event history where one record per line is useful.

Example:

```json
{"time":"...","action":"plugin.install","plugin":"example"}
{"time":"...","action":"runtime.restart","adapter":"deepseek-harness"}
{"time":"...","action":"profile.switch","profile":"web"}
```

This format is easy to stream, inspect, archive and export for diagnostics.

### Logs

Normal application logs should use rolling plain-text files generated from the structured tracing pipeline.

Support exports should sanitize user paths and secret-like values before sharing.

## Source of truth

DeepMate distinguishes between DeepMate-owned state and harness-owned state.

### DeepMate-owned

Examples:

- UI preferences
- marketplace cache
- favorites
- adapter preferences
- local action history
- snapshots generated by DeepMate

### Harness-owned

Examples:

- harness configuration
- provider configuration
- harness profiles
- harness plugin state
- harness credentials
- harness sessions

Harness-owned state should be accessed through the active adapter and remain authoritative in the harness itself.

## Snapshots

Snapshots are portable JSON documents representing a normalized view of a setup.

A snapshot may include:

```text
adapter
adapter version
runtime metadata
profiles
providers
models
plugins
selected settings
```

Sensitive credential values should never be embedded in normal snapshots.

Snapshots should support:

- export
- import
- validation
- compatibility reporting
- human-readable diffing

## Doctor

Doctor is a first-class core service, not a collection of ad-hoc error popups.

A `DoctorReport` should contain structured checks such as:

```text
runtime installed
runtime version
runtime reachable
required tools available
port conflicts
profile validity
plugin compatibility
provider configuration
adapter compatibility
filesystem permissions
update availability
```

Each check should carry:

```text
id
status
summary
details
suggested_action
```

The desktop app can render the report visually, while the CLI can print the same report in human-readable or JSON form.

## UI architecture

The UI should be componentized from the beginning.

Planned structure:

```text
apps/desktop/ui/
│
├── App.slint
│
├── components/
│   ├── Sidebar.slint
│   ├── Button.slint
│   ├── Card.slint
│   ├── Badge.slint
│   ├── PluginCard.slint
│   └── ModelRow.slint
│
├── pages/
│   ├── Overview.slint
│   ├── Runtime.slint
│   ├── Models.slint
│   ├── Profiles.slint
│   ├── Plugins.slint
│   ├── Market.slint
│   ├── Doctor.slint
│   └── Settings.slint
│
└── theme/
    ├── Colors.slint
    ├── Typography.slint
    └── Metrics.slint
```

Design tokens should be centralized so visual consistency does not depend on page-level hard-coded values.

## Initial implementation order

### Stage 1 — Foundation (implemented in v0.1.0)

- Rust workspace
- `deepmate-core`
- platform abstraction
- DeepSeek Harness adapter
- runtime detection
- runtime start / stop / restart
- `deepmate status`
- `deepmate doctor`
- system browser open

### Stage 2 — Desktop shell (implemented in v0.2.0)

- Slint application shell
- system tray
- Overview page
- Runtime page
- Doctor page
- shared application state / event bridge

### Stage 3 — Harness configuration

- profiles
- providers
- models
- adapter capability detection
- configuration editing through supported Harness interfaces

### Stage 4 — Plugins and marketplace

- plugin inventory
- install / update / remove
- market source abstraction
- plugin search
- trust and compatibility metadata

### Stage 5 — Portability

- snapshots
- import / export
- private registries
- portable setup workflows

### Stage 6 — Adapter ecosystem

- public adapter protocol
- JSON-RPC stdio host
- adapter manifests
- additional harness adapters

## Architectural rules

The following rules should be treated as project invariants:

1. **Desktop and CLI share the same core.**
2. **Harness-specific behavior stays behind adapters.**
3. **Platform-specific behavior stays behind platform services.**
4. **The harness remains authoritative for harness-owned state.**
5. **DeepMate-owned persistent data uses transparent portable files.**
6. **Caches are rebuildable.**
7. **Secrets do not enter normal config, history, snapshots or logs.**
8. **The Harness Web UI opens in the user's system browser.**
9. **The control-center UI contains management surfaces, not a replacement workbench.**
10. **New abstractions are added only when a real capability requires them.**

## Current product boundary

DeepMate owns the management experience around a harness:

```text
Runtime
Profiles
Providers
Models
Plugins
Marketplace
Doctor
Snapshots
Open Harness
```

The harness owns the actual agent work experience.

That boundary is what allows DeepMate to stay small while still becoming a useful universal control layer for multiple AI harnesses over time.
