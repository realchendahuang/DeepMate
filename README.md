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
        │    Desktop UI    │
        └────────┬─────────┘
                 │
        ┌────────▼─────────┐
        │   Control Core   │
        │                  │
        │ Runtime          │
        │ Profiles         │
        │ Providers        │
        │ Models           │
        │ Plugins          │
        │ Marketplace      │
        │ Doctor           │
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

The UI should not need to know whether a harness stores configuration in YAML, JSON, a database or exposes an API. Those implementation details belong inside the adapter layer.

This keeps the product resilient as harnesses evolve and makes future multi-harness support possible.

## Design principles

### 1. Lightweight by default

No bundled Chromium. No embedded browser shell for the main experience. No duplicate editor stack.

### 2. Respect the harness as the source of truth

DeepMate should manage the harness through its official interfaces and formats whenever possible instead of maintaining a second copy of harness configuration.

### 3. Extend, do not fork

DeepMate should use the existing DeepSeek Harness profile, bundle, plugin, settings and provider mechanisms rather than inventing incompatible replacements.

### 4. Everything behind an adapter

Harness-specific behavior belongs behind a stable adapter boundary so the control core can stay generic.

### 5. Management surface, not work surface

DeepMate manages the environment around the agent. The actual conversation and execution experience remains owned by the harness.

## Planned technology

The current direction is:

- **Rust** for the control core
- a lightweight native cross-platform UI layer
- system-native process and service management where appropriate
- no embedded WebView as the primary Harness interface

The exact UI implementation is still being evaluated and may evolve as the project matures.

## Roadmap

### Phase 1 — DeepSeek Harness foundation

- Runtime discovery and lifecycle management
- Open official Harness Web UI
- Environment diagnostics
- Provider and model management
- Profile discovery and management

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

- Stable public adapter interface
- Additional harness / agent runtime adapters

## Project status

DeepMate is currently in **early development**.

The first goal is to build a solid, minimal foundation for DeepSeek Harness rather than rush into a large feature set.

## Related project

- [DeepSeek Harness](https://github.com/deepseek-ai/deepseek-harness)

## License

License information will be added as the project is prepared for its first public release.
