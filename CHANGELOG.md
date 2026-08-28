# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Self-update loop:
  - `deepmate update` — checks the latest release, downloads the CLI
    archive for this platform, verifies it against the published sha256,
    extracts the binary and replaces the running executable (a no-op when
    already current).
  - The desktop update banner and Settings gained "Download & install":
    the release DMG is downloaded, checksum-verified and handed to the OS
    installer; the drag-to-install step stays in the user's hands. A stale
    banner clears itself when the install finds no newer release.
  - Both flows can be exercised against a test release with
    `DEEPMATE_UPDATE_API_URL`.
- Update availability and asset selection (target-triple matching,
  checksum parsing and verification) now live in the core's `update`
  module, shared by the CLI and the desktop app.

## [0.7.0] - 2026-08-28

### Added

- Plugin compatibility checks: `deepmate plugin check <spec>` (and a
  `plugin_compat` adapter hook) matches a market package's declared
  `engines` requirement against the detected harness version. `plugin
  install` runs the check as a preflight — a definite "incompatible" verdict
  refuses the install unless `--force` is given, while unknown or unavailable
  checks only note the fact. The desktop Market tab runs the same preflight
  when installing from a search result.
- Marketplace trust signals: npm's review scores (popularity, quality) are
  normalized into `MarketEntry` and surfaced in `deepmate market search`
  output and as meters in the desktop Market tab, next to the release date.
  Market search results can now be installed directly from the desktop app.
- Own-settings backup: `deepmate config export <path>` / `config import
  <path>` write and apply DeepMate's own configuration (language, theme, tray
  behavior, update preferences, market defaults) as a portable JSON document
  — the counterpart to harness snapshots. The desktop Settings page offers
  the same flow through native save/open dialogs and re-applies the imported
  preferences live.
- System notifications: a newer release found by the automatic startup check
  raises a desktop notification (opt-out preference alongside the update
  check), and the tray's explicit "Check Updates" action always reports its
  outcome.
- Tray menu additions: "Open Harness" launches the harness web UI from the
  tray, and "Check Updates" runs the release check on demand, with
  notifications localized like the rest of the tray menu.

### Changed

- `deepmate plugin install` gained a `--force` flag to override an
  incompatible compatibility verdict.

## [0.6.0] - 2026-08-26

### Added

- Portable snapshots: `deepmate snapshot export|import|list` and a Snapshots
  section in the desktop Settings page capture a normalized inventory
  (profiles, providers, models, plugins — never secrets) as JSON and apply it
  merge-style to the same adapter, so a setup can move between machines.
- Configuration editing in the desktop app: providers, models and profiles
  can be created, edited and removed through the adapter boundary (upsert
  semantics), with the DeepSeek Harness adapter writing the harness-owned
  files (`settings.yaml` and the profile directories).
- Pi Agent adapter (`--adapter pi-agent`): read-only inventory (providers,
  models, plugins) from the documented `~/.pi/agent` files — the second real
  adapter and a live demonstration of the capability gate.
- System tray: closing the window hides DeepMate to the tray, with a
  localized show/quit menu and macOS dock-click restore; close-to-tray is
  now a Settings preference.
- Start at login: an opt-in preference backed by OS login items, carrying
  the active `--adapter` flag.
- Update checking: the desktop app queries the GitHub releases API (on by
  default, fails quiet when offline) and surfaces a release banner on
  Overview plus a check status in Settings Preferences.

### Changed

- `general.auto_start` now defaults to `false`: registering the app with OS
  login items is opt-in and only happens through the Settings toggle.
- The light-theme class was renamed from the Slint-era inverted `.dark` to
  `.light` (dark remains the default), so Tailwind `dark:` variants can no
  longer be silently misapplied.
- The workspace `.gitignore` now ignores `target/` everywhere, including the
  Tauri dev build directory under `apps/`.

### Fixed

- Close-to-tray was broken: the close handler prevented closing without ever
  hiding the window, and no tray icon existed to restore it, so the close
  button did nothing. Closing now hides to the tray (with `close_to_tray`
  enabled) or quits, and the tray menu restores the window.
- Docs drift: `docs/ARCHITECTURE.md` and `docs/DESIGN_SYSTEM.md` still
  described the retired Slint shell; both now describe the Tauri 2 + React
  implementation, and the README documents the snapshot commands and the
  Pi Agent adapter.

## [0.5.0] - 2026-08-25

### Changed

- Desktop rewritten from the Slint shell to Tauri 2 + React + Tailwind: the
  shared core is reached through a typed command bridge, and the UI ships as a
  Vite-built React app with i18next translations (English / 简体中文).
- The desktop shell now uses a left navigation rail (200px) with Overview,
  Plugins and a pinned Settings entry, replacing the top-bar navigation; below
  768px the rail collapses into a compact top bar with a drawer.
- Plugins was promoted from a Settings tab to its own top-level page
  (Installed / Market), so install, update, remove and market search have a
  dedicated workspace; Settings now holds only Configuration (Profiles,
  Providers, Models) and Preferences.
- DataList detail columns now appear at 1024px (`lg`) instead of 768px, so
  inventory tables stay comfortable once the sidebar takes its width.
- Release workflow builds the desktop app through its own Tauri manifest
  (frontend + `deepmate-desktop`) instead of the workspace binary.

## [0.4.0] - 2026-08-23

### Added

- Desktop management pages: Profiles, Providers, Models, Plugins and Market,
  exposing the same inventory and plugin-lifecycle capabilities as the CLI
  through the shared bridge.
- Plugin lifecycle in the desktop app: install, remove and update a plugin
  from the Plugins page, with install/remove/update history recording mirroring
  CLI semantics.
- Market page in the desktop app: browse curated vs community sources and
  search the market, with provenance (publisher, repository, version) shown
  per result.
- macOS DMG packaging: the desktop app is now bundled as a `DeepMate.app`
  with `Info.plist` (LSUIElement menu-bar app), a generated `.icns` icon and
  an ad-hoc signature, wrapped in a drag-to-install DMG alongside a CLI
  tarball.

### Changed

- The desktop bridge now carries inventory, plugin-lifecycle and market
  commands/events; capability-gated commands report "unsupported" instead of
  an empty list.
- Release workflow now packages macOS separately (`scripts/package-macos.sh`)
  from Linux, so macOS produces a DMG instead of a bare-binary tarball.

## [0.3.0] - 2026-08-23

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

[0.4.0]: https://github.com/realchendahuang/DeepMate/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/realchendahuang/DeepMate/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/realchendahuang/DeepMate/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/realchendahuang/DeepMate/releases/tag/v0.1.0
