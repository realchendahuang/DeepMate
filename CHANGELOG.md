# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
