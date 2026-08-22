# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Plugin lifecycle in the CLI and DeepSeek Harness adapter: `plugin install`,
  `plugin remove` and `plugin update` forward to the harness's own
  `dsh plugin` workflow so profile, package and bundle reconciliation stay
  owned by the harness.
- Marketplace discovery: `market list` (curated vs community sources) and
  `market search <query>` backed by the npm registry, with results normalized
  into `MarketEntry` records carrying provenance metadata (publisher,
  repository, last-updated).
- Outdated-plugin detection: `plugin list --check-updates` consults the market
  in parallel and marks plugins with a newer available version.
- On-disk market query cache (`cache/marketplace.json`, single most-recent
  query, one-hour TTL) so repeated searches and offline reruns stay cheap.
- `MarketSourceInfo` model and `HarnessAdapter::market_sources()` so market
  sources are adapter-reported instead of hardcoded in the CLI.

### Changed

- `Plugin` now carries `profile`, `latest` and `outdated` fields instead of
  profile-qualified ids.
- Market HTTP requests reuse a single `reqwest::Client` per adapter instead of
  building one per search.

## [0.2.0] - 2026-08-20

Stage 2 desktop shell.

### Added

- Slint desktop shell (`apps/desktop`, binary `deepmate-desktop`) with a
  system tray and close-to-tray behavior honoring `ui.close_to_tray`.
- Overview, Runtime and Doctor pages built on centralized design-token
  theme and shared Sidebar/Card/Badge components.
- Tokio-backed `UiCommand`/`UiEvent` bridge between the desktop UI and the
  core, unit-tested with the fake adapter.
- `deepmate-app` service crate hosting the registry, configuration, logging
  and history helpers shared by the CLI and the desktop app.
- `AdapterRegistry::into_adapter` for consuming an adapter out of the
  registry by id.
- Tag-triggered release workflow publishing `deepmate` and
  `deepmate-desktop` binaries for linux-x86_64, macos-aarch64,
  macos-x86_64 and windows-x86_64.

### Changed

- Workspace version bumped to 0.2.0.
- CI installs the tray-icon Linux dependencies (`libgtk-3-dev`,
  `libayatana-appindicator3-dev`, `libxdo-dev`).

## [0.1.0] - 2026-08-19

Initial Stage 1 foundation.

### Added

- Rust workspace foundation with `deepmate-core`, `deepmate-platform`, the
  `deepseek-harness` adapter and the `deepmate` CLI.
- Core domain model, harness adapter contract, adapter registry and a public
  testkit with a deterministic fake adapter.
- Platform service abstraction covering data-dir resolution, open URL/path
  and process termination.
- DeepSeek Harness adapter with real `dsh` integration: CLI detection
  (including npx-installed launchers), version parsing, web UI reachability,
  runtime start/stop/restart with pid tracking, profile discovery, plugin
  inventory and provider/model catalogs via the documented `$DSH_HOME` file
  contracts.
- `deepmate` CLI command surface: `adapters`, `detect`, `status`, `open`,
  `doctor`, `runtime start|stop|restart`, `profile list`, `provider list`,
  `model list` and `plugin list`, with JSON output, `--data-dir` override and
  capability gating so unsupported commands are rejected with a clear error.
- File-based data layer: OS-convention data directory, TOML configuration,
  append-only JSONL action history and file logging.
- Cross-platform CI (formatting, clippy, tests) with a core purity gate that
  keeps harness-specific names out of `deepmate-core`.

[Unreleased]: https://github.com/realchendahuang/DeepMate/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/realchendahuang/DeepMate/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/realchendahuang/DeepMate/releases/tag/v0.1.0
