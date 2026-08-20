# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

This section collects changes that will ship as 0.2.0.

## [0.1.0] - 2025-08-19

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

[Unreleased]: https://github.com/realchendahuang/DeepMate/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/realchendahuang/DeepMate/releases/tag/v0.1.0
