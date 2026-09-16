# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Fixed

- Windows and Linux clippy: the desktop crate's `RunEvent` handler, the
  `RunEvent` import, the test env-lock re-export, three harness tests that
  call the unix-only `probe_server` helper, and the seven private test
  fixtures those tests reach are now gated to the platforms that use them
  (ungated, they compile as dead code off macOS). These only reproduced
  outside macOS, so the release push was the first time CI saw them.
- CI runs tests with `--no-fail-fast`, so a platform failure reports every
  failing binary at once instead of one per debugging cycle. The
  `profile create/remove` CLI test is unix-only (its fake launcher is a shell
  script); on Windows the desktop crate's test binary is excluded because it
  fails to load there before any test runs, while clippy still compiles it.
  Both gaps are noted in the workflow.

## [0.8.0] - 2026-09-17

### Added

- Scenario module: the desktop "profiles" concept is now called "scenarios"
  (设置 → 场景). The Settings page gained a proper new-scenario dialog,
  per-row rename and plugin counts, and a scenario detail view that edits a
  scenario's plugin set visually — an installed list with enable/disable,
  update and remove, plus an in-place market search to add plugins behind
  the compatibility preflight.
- Profile rename (`rename_profile` in the service layer and the desktop
  command surface): moves the profile directory, updates the manifest
  `name` field and carries disabled-plugin records over. The launcher-owned
  `web` profile is protected from rename and remove at the service layer
  (previously only the UI refused).
- Bundle-install reconciliation: `dsh plugin add` installs the dependency
  but leaves `dsh.profile.bundles` untouched, so a freshly installed plugin
  was never loaded by the harness web UI. An install of a package that
  declares harness capabilities now also declares the bundle
  (`add_bundle`, idempotent); plain library dependencies are left alone.
- Profile manifests may carry a `description` field, surfaced by discovery
  with the synthesized bundle list as fallback.
- Curated plugin list:
  - The marketplace's **Curated** source is now driven by a
    DeepMate-maintained list (`plugins/curated.json` in this repository)
    instead of the `@deepseek-ai` npm scope heuristic. Anyone can propose a
    plugin by opening a pull request against the list.
  - The list is fetched from the raw GitHub URL at runtime, cached under the
    data directory (`cache/curated.json`) and merged ahead of npm search
    results, so curated entries always surface first. A stale cache keeps
    the curated source working offline; `DEEPMATE_CURATED_LIST_URL`
    overrides the fetch URL for tests and mirrors.
  - First batch: 68 entries (2 official + 66 community), assembled by
    cross-referencing the community's curated indexes
    (AdamPlatin123/awesome-dsh-plugins, dshworks/awesome-dsh-plugins)
    against the npm registry, with every package verified to exist.
  - The desktop market tab opens on the curated storefront: an empty search
    shows the curated list without hitting the npm registry, so the store
    is browsable before any query is typed.
  - The storefront gained category filters (memory, vision, MCP, chat,
    remote, office, dev, …) with per-category counts, and trust badges
    that distinguish official (DeepSeek Harness vendor), vetted
    (DeepMate-reviewed) and community entries. Curated entries carry a
    `category` field; npm search results are uncategorized.
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
- Live plugin-operation progress:
  - The desktop Plugins page now runs install / update / remove through a
    streamed command (`plugin_op_stream`): the harness CLI's output lines
    arrive over a Tauri channel and render in a live progress dialog, with
    the outcome (success or the failure detail) shown when the operation
    ends.
  - The stream carries a well-formed Started/Line/Finished sequence; the
    harness layer forwards the child process's stdout/stderr lines as they
    arrive.
- Semantic types in the generated bindings: `MarketEntry.updated` is now a
  real `Date` on the frontend (converted from the RFC3339 wire format by the
  generated binding code) instead of a string the UI had to slice; the
  desktop market list renders it with `toLocaleDateString()`.
- The Settings page gained an About section showing the app version, which
  is exported into the bindings as a constant (`appVersion`) from
  `CARGO_PKG_VERSION` instead of being hard-coded in the UI.

### Changed

- Desktop Overview page redesigned: the runtime status is the hero (large
  status word with a tone-colored dot, a meta line with harness name,
  version and PID, and a state-driven primary action — Open Harness while
  running, Start otherwise, Run Doctor when the harness is missing). The
  removed decorative capability badges and the old detection badges are gone;
  the sidebar footer shows the harness name instead of the adapter id.
- `HarnessInfo` no longer carries `adapter_version`, `DoctorReport` no longer
  carries `adapter_id`, and the desktop overview `counts` are plain numbers
  (`InventoryCounts`) instead of nullable capability-gated values.
- CLI `status`/`doctor` output no longer print an `adapter:` line, and
  `detect` no longer prints `adapter version:`. Deterministic fake-adapter
  CLI tests were replaced by isolated `DSH_HOME` fixture tests; snapshot
  roundtrip tests run against a temp `DSH_HOME` instead of `--adapter test`.
- Snapshot files no longer carry `adapter`/`adapter_version` fields and the
  format moved to `deepmate-snapshot/2`; the cross-harness rejection in
  `snapshot import` is gone. The action-history JSONL no longer records a
  harness attribute at all.
- The on-disk data layout no longer creates an `adapters/` subdirectory.
- **Scenarios are the top-level unit.** The sidebar gained a scenario group
  above the workspace tools; switching scenarios switches the whole setup.
  Each scenario's home page owns its runtime controls, its providers &
  models and its plugins; the old global "profiles" and "models" settings
  pages are gone (providers/models live per scenario now).
- Provider/model configuration is isolated per scenario: the engine's
  `settings` row is redirected per profile (`cordis.patch.yml` →
  `profiles/<scenario>/settings.yaml`), so the scenario's engine process —
  web Models page included — reads and writes its own document. Existing
  global LLM configuration is migrated into the `web` scenario once; the
  redirection takes effect on scenario restart.
- `runtime` CLI/desktop commands drive any scenario: `runtime start|stop|
  restart --scenario`, `runtime list`, `runtime task <scenario> "prompt"`;
  `provider`/`model` subcommands take `--scenario` (default `web`).
- The plugins `bundles.load` doctor check now probes only the running
  scenario and its fix mode restarts the scenario (a running console keeps
  the bundle set it started with).
- Build, tooling & cleanup:
  - The CI check job installs the Tauri Linux system dependencies (the
    desktop shell is a workspace member now), concurrent runs for a branch
    cancel each other, clippy/test run with `--locked`, and the desktop job
    runs the frontend `lint` + `typecheck` + `test` gate.
  - `make verify` runs the Rust gate plus the frontend gate; `make
    desktop-check` runs the frontend gate alone. `make cache-clean` prunes
    Cargo's incremental-cache fingerprints, and `profile.dev.package."*"`
    drops debuginfo for dependencies to keep `target/debug` small.
  - The bindings generator writes to `src/shared/api/bindings.ts` (the one
    path the frontend imports); the duplicated `src/bindings.ts` is gone.
  - The app version has a single source of truth: every crate inherits
    `version.workspace = true` and `tauri.conf.json` no longer pins its own
    `version` (it falls back to the Cargo version), so a release bump is
    3 files instead of 5.
- Removed dead code: the unreferenced `plugin_remove`, `plugin_update`,
  `plugin_forget` and `list_market_sources` commands (and their wrappers),
  six unused plugin-store actions, unused Tauri `process`/`store` plugins,
  the unused `next-themes` and Tauri JS plugin packages, and the stale
  `scripts/` helpers.

### Removed

- The harness-adapter abstraction is gone; DeepSeek Harness is now the one
  hard-wired harness:
  - Deleted the `HarnessAdapter` trait, `AdapterCapabilities` and all
    capability gating (CLI `require_capability`, desktop command gates,
    capability-gated overview counts), the `AdapterRegistry`, and the
    `testkit` fake adapter from `deepmate-core`.
  - Deleted the `pi-agent` crate. `deepseek-harness` moved from
    `crates/adapters/deepseek-harness` to `crates/deepseek-harness` and its
    struct was renamed `DeepSeekHarnessAdapter` → `DeepSeekHarness` with the
    trait impl converted into inherent methods (snapshot capture/apply are
    now service methods; the core keeps only the snapshot data model and
    store).
  - Removed the `--adapter` flag from the CLI and the desktop app, and the
    `deepmate adapters` subcommand. `deepmate-core` stays a pure data layer;
    the CI core purity gate is unchanged.

### Fixed

- Desktop: switching providers no longer carries the previous provider's
  form values into the new selection. The detail pane remounts per provider
  (it is keyed by provider id), so an unsaved edit can never be written onto
  the wrong provider.
- Desktop: the 10-second overview poll no longer disables every control in
  the app. Read-only refreshes track themselves without blocking the UI, and
  concurrent actions no longer clear each other's busy flag.
- Desktop: a harness CLI installed after DeepMate launched is now detected
  without restarting the app; only successful CLI lookups are cached.
- Port assignment no longer hands out a `preferred` port that is already
  owned by another scenario or currently listening, which previously let two
  scenarios fight over one port and fail at the boot timeout.
- English plural forms are correct at count = 1 across the overview, doctor
  and scenario strings ("1 provider" instead of "1 providers"); the doctor
  badge labels ("3/7 Passed") are localized instead of hard-coded.
- `deepmate status` no longer fails with a raw I/O error on a harness home
  where the scenario has never been created: a missing profile manifest now
  reads as an empty manifest for every read-only query (status, plugin
  listing, bundle declaration), matching what those callers already assumed.
- Windows CI: the group-kill `pid` is unix-only now, fixing the
  `unused variable` clippy failure that had kept `main` red.

### Security

- Replaced the archived, unsound `serde_yml` YAML parser with `noyalib`
  (its `serde_yaml`-compatible shim), keeping the same API surface. The
  editor surface is unchanged; 104 harness tests cover it.

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
