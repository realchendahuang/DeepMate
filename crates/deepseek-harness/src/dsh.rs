// The DeepSeek Harness filesystem contract (`dsh`).
//
// Mirrors the documented `dsh` layout:
// - harness home: `$DSH_HOME` (non-empty) or `~/.dsh`
// - profiles: `<home>/profiles/<name>/package.json`
// - the profile manifest is the `dsh.profile` section of that package.json
// - plugins are the profile's declared `dependencies`
//
// Only stable, documented file contracts are read here; everything else goes
// through the `dsh` CLI.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use deepmate_core::error::{CoreError, CoreResult};
use deepmate_core::model::{Model, Plugin, Profile, Provider, Surface};
// YAML surface: noyalib's `serde_yaml`-compatible shim (string-keyed
// mappings with `Value::insert`), replacing the archived `serde_yml`.
use noyalib::compat::serde_yaml as yaml;
use noyalib::compat::serde_yaml::{Mapping, Sequence, Value};

const DSH_HOME_ENV: &str = "DSH_HOME";
const DSH_HOME_DIR_NAME: &str = ".dsh";

// The official in-box bundles a scenario's surface is mounted from. The
// engine resolves these from the same installation as the running `dsh`, so
// a surface is a declaration of one of these plus the base bundle, not a
// separate concept of its own.
pub const BASE_BUNDLE: &str = "@deepseek-ai/dsh-base";
pub const WEB_APP_BUNDLE: &str = "@deepseek-ai/dsh-web-app";
pub const HEADLESS_BUNDLE: &str = "@deepseek-ai/dsh-headless";

// The `dsh.profile` section of a profile's package.json.
#[derive(Debug, Clone, Default, serde::Deserialize)]
struct ProfileManifest {
    // An optional user-facing description. A profile created by the launcher
    // has none; DeepMate writes one when the user edits a scenario.
    description: Option<String>,
    dependencies: Option<BTreeMap<String, String>>,
    dsh: Option<DshSection>,
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
struct DshSection {
    profile: Option<DshProfileSection>,
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
struct DshProfileSection {
    bundles: Option<Vec<String>>,
}

#[cfg(target_os = "windows")]
fn os_home() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE").map(PathBuf::from)
}

#[cfg(not(target_os = "windows"))]
fn os_home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

// Resolve the harness home: `$DSH_HOME` (non-empty) or `~/.dsh`.
pub fn dsh_home() -> Option<PathBuf> {
    if let Some(home) = std::env::var_os(DSH_HOME_ENV) {
        let home = PathBuf::from(home);
        if !home.as_os_str().is_empty() {
            return Some(home);
        }
    }
    os_home().map(|home| home.join(DSH_HOME_DIR_NAME))
}

fn read_manifest(dir: &Path) -> CoreResult<ProfileManifest> {
    let path = dir.join("package.json");
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        // A profile that has never been created (a fresh harness home, or a
        // scenario that was declared but never started) has no manifest yet.
        // Every caller of this function is a read-only query — status,
        // plugin listing, bundle declarations — so the empty manifest is the
        // honest answer; failing here surfaced a raw I/O error to the user.
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Ok(ProfileManifest::default())
        }
        Err(err) => return Err(err.into()),
    };
    serde_json::from_str(&text).map_err(|err| {
        CoreError::InvalidState(format!(
            "invalid profile manifest {}: {err}",
            path.display()
        ))
    })
}

// Discover profiles under the harness home.
//
// A profile is a directory under `<home>/profiles` that carries a
// package.json with a `dsh.profile` manifest section. The launcher-maintained
// `node_modules` fallback directory is not a profile.
pub fn discover_profiles() -> CoreResult<Vec<Profile>> {
    let Some(home) = dsh_home() else {
        return Ok(Vec::new());
    };
    let profiles_dir = home.join("profiles");
    let entries = match std::fs::read_dir(&profiles_dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(err) => return Err(err.into()),
    };
    let mut profiles = Vec::new();
    for entry in entries.flatten() {
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if !file_type.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        // `node_modules` is the launcher's shared fallback, and any dot
        // directory is DeepMate's own bookkeeping (`.deepmate-trash` holds
        // removed profiles); neither is a scenario.
        if name == "node_modules" || name.starts_with('.') {
            continue;
        }
        let dir = entry.path();
        if !dir.join("package.json").is_file() {
            continue;
        }
        // A corrupt manifest must not hide the profile; the description is
        // best-effort while plugin listing below fails loudly. A manifest
        // `description` field wins; otherwise fall back to the bundle list.
        let description = read_manifest(&dir).ok().and_then(|manifest| {
            manifest.description.or_else(|| {
                manifest
                    .dsh
                    .and_then(|dsh| dsh.profile)
                    .and_then(|profile| profile.bundles)
                    .map(|bundles| format!("bundles: {}", bundles.join(", ")))
            })
        });
        profiles.push(Profile {
            id: name.clone(),
            name,
            description,
        });
    }
    profiles.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(profiles)
}

// The two node_modules roots a package can be installed into: the
// profile-local directory first, then the launcher-maintained shared
// fallback used by the real dsh web profile.
fn module_dir(profile_dir: &Path, shared: &Path, name: &str) -> Option<PathBuf> {
    let local = profile_dir.join("node_modules").join(name);
    if local.is_dir() {
        return Some(local);
    }
    let fallback = shared.join(name);
    if fallback.is_dir() {
        return Some(fallback);
    }
    None
}

// Best-effort installed version read from a package's own package.json.
fn installed_version(dir: &Path) -> Option<String> {
    let text = std::fs::read_to_string(dir.join("package.json")).ok()?;
    let value: serde_json::Value = serde_json::from_str(&text).ok()?;
    value
        .get("version")
        .and_then(|v| v.as_str())
        .map(|v| v.to_string())
}

// Build one plugin record for a bundle or dependency.
fn plugin_for(
    profile_dir: &Path,
    shared: &Path,
    profile_id: &str,
    id: &str,
    declared: Option<String>,
) -> Plugin {
    let installed = module_dir(profile_dir, shared, id);
    Plugin {
        id: id.to_string(),
        name: id.to_string(),
        // Prefer the actually installed version; fall back to the declared
        // range only when the package is not installed.
        version: installed
            .as_deref()
            .and_then(installed_version)
            .or(declared),
        enabled: installed.is_some(),
        profile: profile_id.to_string(),
        latest: None,
        outdated: false,
        // The trust tier is enriched from the curated market by the service
        // layer; the raw file scan has no market knowledge.
        trust: None,
    }
}

// List the plugins of one profile: its bundle layers (the profile's own
// `dsh.profile.bundles`) plus its declared dependencies, with `enabled`
// reflecting whether each package is actually installed in the profile-local
// node_modules or the launcher-maintained shared fallback.
pub fn list_plugins(profile_id: &str) -> CoreResult<Vec<Plugin>> {
    let Some(home) = dsh_home() else {
        return Ok(Vec::new());
    };
    let dir = home.join("profiles").join(profile_id);
    let shared = home.join("profiles").join("node_modules");
    let manifest = read_manifest(&dir)?;
    let mut plugins = Vec::new();
    let dependencies = manifest.dependencies.unwrap_or_default();
    if let Some(bundles) = manifest
        .dsh
        .and_then(|dsh| dsh.profile)
        .and_then(|profile| profile.bundles)
    {
        for bundle in bundles {
            // A package can be both a bundle layer and an npm dependency;
            // list it once, keeping the dependency's declared range as the
            // version fallback.
            let declared = dependencies.get(&bundle).cloned();
            plugins.push(plugin_for(&dir, &shared, profile_id, &bundle, declared));
        }
    }
    for (name, version) in &dependencies {
        if plugins.iter().any(|plugin| &plugin.id == name) {
            continue;
        }
        plugins.push(plugin_for(
            &dir,
            &shared,
            profile_id,
            name,
            Some(version.clone()),
        ));
    }
    plugins.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(plugins)
}

// Derive a profile's run surface from its installed bundle set. The engine
// has no surface field of its own — the web console comes from
// `dsh-web-app`, one-shot tasks from `dsh-headless` — so the surface is
// read off the installed, enabled bundles.
pub fn profile_surface(profile_id: &str) -> CoreResult<Surface> {
    let plugins = list_plugins(profile_id)?;
    let any = |id: &str| {
        plugins
            .iter()
            .any(|plugin| plugin.id == id && plugin.enabled)
    };
    if any(WEB_APP_BUNDLE) {
        return Ok(Surface::Web);
    }
    if any(HEADLESS_BUNDLE) {
        return Ok(Surface::Task);
    }
    Ok(Surface::Undetermined)
}

// List plugins across all profiles.
pub fn list_all_plugins() -> CoreResult<Vec<Plugin>> {
    let mut plugins = Vec::new();
    for profile in discover_profiles()? {
        plugins.extend(list_plugins(&profile.id)?);
    }
    plugins.sort_by(|a, b| {
        (a.profile.as_str(), a.id.as_str()).cmp(&(b.profile.as_str(), b.id.as_str()))
    });
    Ok(plugins)
}

// The declared version range of a plugin in a profile's manifest
// dependencies, when the manifest declares it. Used to capture a package
// spec before disabling it, so enabling can restore the same range.
pub fn declared_spec(profile_id: &str, id: &str) -> CoreResult<Option<String>> {
    let Some(home) = dsh_home() else {
        return Ok(None);
    };
    let dir = home.join("profiles").join(profile_id);
    let manifest = read_manifest(&dir)?;
    Ok(manifest
        .dependencies
        .and_then(|dependencies| dependencies.get(id).cloned()))
}

// True when a plugin is declared in a profile's `dsh.profile.bundles` list.
// Bundle declarations are what the harness web UI actually loads at runtime,
// independently of the declared dependencies.
pub fn bundle_declared(profile_id: &str, id: &str) -> CoreResult<bool> {
    let Some(home) = dsh_home() else {
        return Ok(false);
    };
    let dir = home.join("profiles").join(profile_id);
    let manifest = read_manifest(&dir)?;
    Ok(manifest
        .dsh
        .and_then(|dsh| dsh.profile)
        .and_then(|profile| profile.bundles)
        .is_some_and(|bundles| bundles.iter().any(|bundle| bundle == id)))
}

// True when an installed package declares a web client bundle in its own
// package.json (`dsh.client`). Server-side patch bundles (skills and tools
// only, `dsh.bundle` without `dsh.client`) never get a
// `/plugins/<id>/client.js` route, so the runtime load probe must not
// expect one.
pub fn declares_web_client(profile_id: &str, id: &str) -> CoreResult<bool> {
    let Some(home) = dsh_home() else {
        return Ok(false);
    };
    let path = home
        .join("profiles")
        .join(profile_id)
        .join("node_modules")
        .join(id)
        .join("package.json");
    let Ok(text) = std::fs::read_to_string(path) else {
        return Ok(false);
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
        return Ok(false);
    };
    let client = value.get("dsh").and_then(|dsh| dsh.get("client"));
    Ok(client.is_some_and(|client| {
        client
            .get("platform")
            .and_then(|platform| platform.as_str())
            .is_none_or(|platform| platform == "web")
    }))
}

// True when a package is physically installed in the profile's own
// node_modules, as opposed to the launcher-maintained shared fallback.
// Only profile-local packages get a `/plugins/<id>/client.js` loader entry
// served by the harness web UI (verified against the real `dsh web`
// server); shared-fallback bundles load through a different mechanism and
// plain libraries have no client entry at all.
pub fn installed_in_profile(profile_id: &str, id: &str) -> CoreResult<bool> {
    let Some(home) = dsh_home() else {
        return Ok(false);
    };
    let dir = home.join("profiles").join(profile_id);
    Ok(dir.join("node_modules").join(id).is_dir())
}

// The backup file written before the profile manifest is edited, so a
// destructive change can be rolled back. Mirrors the harness's own
// `package.json.bak-*` habit and SettingsEditor's `.deepmate.bak`.
const MANIFEST_BACKUP_SUFFIX: &str = ".deepmate.bak";

// Copy `path` to `<path><suffix>` before it is rewritten. The backup is the
// only way back from a destructive edit, so a failed copy is refused: the
// caller must not overwrite a file it could not preserve. A missing source
// file is not a failure (there is nothing to lose yet).
fn write_backup(path: &Path, suffix: &str) -> CoreResult<()> {
    if !path.is_file() {
        return Ok(());
    }
    let backup = PathBuf::from(format!("{}{suffix}", path.display()));
    std::fs::copy(path, &backup).map_err(|err| {
        CoreError::InvalidState(format!(
            "refusing to rewrite {} because its backup could not be written to {}: {err}",
            path.display(),
            backup.display()
        ))
    })?;
    Ok(())
}

// Strip a plugin from a profile's `dsh.profile.bundles` list, rewriting the
// manifest in place while preserving every other field and key order.
//
// The harness's own `dsh plugin remove` drops the dependency but leaves the
// bundle declaration behind; a leftover declaration makes the web UI fail
// to load the plugin's client bundle on every boot. This is the compensating
// cleanup, and it also serves as the removal path for bundle-only entries
// that were never installed. Returns true when the entry existed and was
// removed; false (no write) when the plugin was not declared.
pub fn remove_bundle(profile_id: &str, id: &str) -> CoreResult<bool> {
    let Some(home) = dsh_home() else {
        return Ok(false);
    };
    let dir = home.join("profiles").join(profile_id);
    let path = dir.join("package.json");
    let text = std::fs::read_to_string(&path)?;
    let mut doc: serde_json::Value = serde_json::from_str(&text).map_err(|err| {
        CoreError::InvalidState(format!(
            "invalid profile manifest {}: {err}",
            path.display()
        ))
    })?;
    let Some(profile) = doc.get_mut("dsh").and_then(|dsh| dsh.get_mut("profile")) else {
        return Ok(false);
    };
    let Some(value) = profile.get_mut("bundles") else {
        return Ok(false);
    };
    let Some(bundles) = value.as_array_mut() else {
        return Err(CoreError::InvalidState(format!(
            "`dsh.profile.bundles` is not a list in {}",
            path.display()
        )));
    };
    let before = bundles.len();
    bundles.retain(|bundle| bundle.as_str() != Some(id));
    if bundles.len() == before {
        return Ok(false);
    }
    if path.is_file() {
        write_backup(&path, MANIFEST_BACKUP_SUFFIX)?;
    }
    let text = serde_json::to_string_pretty(&doc).map_err(|err| {
        CoreError::InvalidState(format!("failed to serialize {}: {err}", path.display()))
    })?;
    std::fs::write(&path, text)?;
    Ok(true)
}

// The single provider route the llm-deepseek plugin owns; the base bundle
// composes it into every profile.
const DEEPSEEK_PROVIDER: &str = "deepseek-official";

// True when an installed package declares a bundle patch
// (`"dsh": { "bundle": { "patch": ... } }`) in its own manifest — the exact
// standard the engine's own bundle reconciliation uses when deciding whether
// a dependency joins the profile's bundle layer stack. Client-only packages
// (`dsh.client` without a patch) are not bundles by that standard, so they
// are intentionally not declared here. Both install locations are
// considered, mirroring `module_dir`.
pub fn declares_dsh_capability(profile_id: &str, id: &str) -> CoreResult<bool> {
    let Some(home) = dsh_home() else {
        return Ok(false);
    };
    let dir = home.join("profiles").join(profile_id);
    let shared = home.join("profiles").join("node_modules");
    let Some(pkg) = module_dir(&dir, &shared, id) else {
        return Ok(false);
    };
    let Ok(text) = std::fs::read_to_string(pkg.join("package.json")) else {
        return Ok(false);
    };
    let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
        return Ok(false);
    };
    Ok(value
        .get("dsh")
        .and_then(|dsh| dsh.get("bundle"))
        .and_then(|bundle| bundle.get("patch"))
        .and_then(|patch| patch.as_str())
        .is_some_and(|patch| !patch.is_empty()))
}

// Add a plugin to a profile's `dsh.profile.bundles` list, rewriting the
// manifest in place while preserving every other field and key order.
//
// Current engines reconcile `dsh.profile.bundles` themselves after every
// successful `dsh plugin` run, so this is normally a no-op there; on older
// engines (or for leftovers from manual pnpm runs) the declaration would
// stay missing and the harness web UI would never load the plugin at
// runtime. This is the compensating write, the mirror image of
// `remove_bundle`. Only real bundles (declaring a bundle patch) are declared
// here; plain libraries and client-only packages are left alone. Idempotent:
// returns true when the entry was added, false when it was already declared
// (no write).
pub fn add_bundle(profile_id: &str, id: &str) -> CoreResult<bool> {
    let Some(home) = dsh_home() else {
        return Ok(false);
    };
    let dir = home.join("profiles").join(profile_id);
    let path = dir.join("package.json");
    let text = std::fs::read_to_string(&path)?;
    let mut doc: serde_json::Value = serde_json::from_str(&text).map_err(|err| {
        CoreError::InvalidState(format!(
            "invalid profile manifest {}: {err}",
            path.display()
        ))
    })?;
    // `dsh.profile.bundles`, created on demand so a freshly scaffolded
    // profile (which has `"dsh": { "profile": {} }`) gains the section.
    let root = doc.as_object_mut().ok_or_else(|| {
        CoreError::InvalidState(format!(
            "manifest root is not a mapping in {}",
            path.display()
        ))
    })?;
    let dsh = root.entry("dsh").or_insert_with(|| serde_json::json!({}));
    let dsh = dsh.as_object_mut().ok_or_else(|| {
        CoreError::InvalidState(format!("`dsh` is not a mapping in {}", path.display()))
    })?;
    let profile = dsh
        .entry("profile")
        .or_insert_with(|| serde_json::json!({}));
    let profile = profile.as_object_mut().ok_or_else(|| {
        CoreError::InvalidState(format!(
            "`dsh.profile` is not a mapping in {}",
            path.display()
        ))
    })?;
    let bundles = profile
        .entry("bundles")
        .or_insert_with(|| serde_json::json!([]));
    let bundles = bundles.as_array_mut().ok_or_else(|| {
        CoreError::InvalidState(format!(
            "`dsh.profile.bundles` is not a list in {}",
            path.display()
        ))
    })?;
    if bundles.iter().any(|bundle| bundle.as_str() == Some(id)) {
        return Ok(false);
    }
    bundles.push(serde_json::Value::from(id));
    if path.is_file() {
        write_backup(&path, MANIFEST_BACKUP_SUFFIX)?;
    }
    let text = serde_json::to_string_pretty(&doc).map_err(|err| {
        CoreError::InvalidState(format!("failed to serialize {}: {err}", path.display()))
    })?;
    std::fs::write(&path, text)?;
    Ok(true)
}

// Rename a profile: move its directory and update the manifest `name` field.
//
// The harness has no rename contract, so this is a DeepMate-side move of the
// whole directory; dependencies, bundles and node_modules move with it.
// DeepMate-owned references to the old name (e.g. the disabled-plugin
// registry) are the caller's responsibility.
pub fn rename_profile(old: &str, new: &str) -> CoreResult<()> {
    if old.is_empty() || new.is_empty() {
        return Err(CoreError::InvalidState(
            "profile name must not be empty".to_string(),
        ));
    }
    for name in [old, new] {
        if name == "node_modules" || name.contains('/') || name.contains('\\') {
            return Err(CoreError::InvalidState(format!(
                "invalid profile name: {name:?}"
            )));
        }
    }
    if old == new {
        return Err(CoreError::InvalidState(
            "the new profile name must differ from the old one".to_string(),
        ));
    }
    let Some(home) = dsh_home() else {
        return Err(CoreError::InvalidState(
            "could not resolve the harness home directory".to_string(),
        ));
    };
    let old_dir = home.join("profiles").join(old);
    let new_dir = home.join("profiles").join(new);
    if !old_dir.join("package.json").is_file() {
        return Err(CoreError::InvalidState(format!("profile not found: {old}")));
    }
    if new_dir.join("package.json").is_file() {
        return Err(CoreError::InvalidState(format!(
            "profile already exists: {new}"
        )));
    }
    // Rewrite the manifest `name` while the file still lives in the old
    // directory, with the usual backup habit.
    let manifest = old_dir.join("package.json");
    let text = std::fs::read_to_string(&manifest)?;
    let mut doc: serde_json::Value = serde_json::from_str(&text).map_err(|err| {
        CoreError::InvalidState(format!(
            "invalid profile manifest {}: {err}",
            manifest.display()
        ))
    })?;
    if let Some(name) = doc.get_mut("name") {
        *name = serde_json::Value::from(format!("dsh-profile-{new}"));
    }
    write_backup(&manifest, MANIFEST_BACKUP_SUFFIX)?;
    let text = serde_json::to_string_pretty(&doc).map_err(|err| {
        CoreError::InvalidState(format!("failed to serialize {}: {err}", manifest.display()))
    })?;
    std::fs::write(&manifest, text)?;
    std::fs::rename(&old_dir, &new_dir).map_err(|err| {
        CoreError::InvalidState(format!("failed to rename profile {old} to {new}: {err}"))
    })
}

// The advisory catalog the llm-deepseek plugin ships when the settings
// section does not override `models`.
const DEFAULT_DEEPSEEK_MODELS: [(&str, &str); 2] = [
    ("deepseek-v4-flash", "DeepSeek-V4-Flash"),
    ("deepseek-v4-pro", "DeepSeek-V4-Pro"),
];

// The user-settings document (`$DSH_HOME/settings.yaml`, hot-reloaded). The
// web Models page writes it; only the llm namespaces are read here.
#[derive(Debug, Clone, Default, serde::Deserialize)]
struct SettingsDocument {
    #[serde(rename = "llm-deepseek")]
    llm_deepseek: Option<LlmDeepseekSection>,
    #[serde(rename = "llm-pi-ai")]
    llm_pi_ai: Option<LlmPiAiSection>,
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
struct LlmDeepseekSection {
    #[serde(rename = "apiKeyEnv")]
    api_key_env: Option<String>,
    #[serde(rename = "baseURL")]
    base_url: Option<String>,
    models: Option<Vec<CatalogModel>>,
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
struct LlmPiAiSection {
    providers: Option<BTreeMap<String, PiAiProviderProfile>>,
}

#[derive(Debug, Clone, serde::Deserialize)]
struct CatalogModel {
    id: String,
    name: Option<String>,
    #[serde(rename = "contextWindow", default)]
    context_window: Option<u64>,
    #[serde(rename = "maxTokens", default)]
    max_tokens: Option<u64>,
    #[serde(default)]
    input: Option<Vec<String>>,
    #[serde(rename = "reasoningEfforts", default)]
    reasoning_efforts: Option<yaml::Value>,
    #[serde(default)]
    compat: Option<yaml::Value>,
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct PiAiProviderProfile {
    display_name: Option<String>,
    api: Option<String>,
    #[serde(rename = "baseURL")]
    base_url: Option<String>,
    #[serde(rename = "apiKeyEnv")]
    api_key_env: Option<String>,
    compat: Option<yaml::Value>,
    models: Option<Vec<CatalogModel>>,
}

// The redirection block appended to a profile's cordis.patch.yml: the
// `settings` row of the settings-file plugin points at a per-scenario
// document the harness resolves at boot (`dshHomePath` is the expression the
// base bundle already uses). Home-level cordis.patch.yml must never touch
// the `settings` row — a later layer replaces the whole row config.
const SCENE_SETTINGS_FILE: &str = "settings.yaml";
const SCENE_SETTINGS_PATCH: &str = "- id: settings
  name: '@deepseek-ai/dsh-settings-file'
  config:
    path: !!js dshHomePath('profiles/__SCENE__/settings.yaml')
";

// The per-scenario settings document, when the profile's cordis.patch.yml
// redirects the `settings` row to one. The harness resolves
// `!!js dshHomePath('profiles/<scene>/settings.yaml')` at boot; the custom
// tag defeats the YAML parser, so the expression is extracted textually. A plain
// literal `path:` is parsed structurally as a fallback.
fn scene_settings_path(profile_id: &str) -> CoreResult<Option<PathBuf>> {
    let Some(home) = dsh_home() else {
        return Ok(None);
    };
    let dir = home.join("profiles").join(profile_id);
    let Ok(text) = std::fs::read_to_string(dir.join("cordis.patch.yml")) else {
        return Ok(None);
    };
    const MARKER: &str = "dshHomePath('";
    if let Some(start) = text.find(MARKER) {
        let rest = &text[start + MARKER.len()..];
        if let Some(end) = rest.find("')") {
            let rel = &rest[..end];
            if !rel.is_empty() {
                // The path is read back out of a document the user can edit:
                // `dshHomePath('../../etc/x')` must not be honoured, or the
                // settings editor would read and write arbitrary files.
                return Ok(Some(resolve_inside_home(&home, rel)?));
            }
        }
    }
    if let Ok(patch) = yaml::from_str::<Value>(&text) {
        if let Some(rows) = patch.as_sequence() {
            for row in rows {
                if row.get("id").and_then(|v| v.as_str()) == Some("settings") {
                    if let Some(path) = row
                        .get("config")
                        .and_then(|config| config.get("path"))
                        .and_then(|path| path.as_str())
                    {
                        return Ok(Some(resolve_inside_home(&home, path)?));
                    }
                }
            }
        }
    }
    Ok(None)
}

// Resolve a `dshHomePath(...)` argument, refusing anything that leaves the
// harness home.
//
// The check is a whitelist rather than a list of known-bad shapes, because
// `Path::join` has a case that a blacklist misses: on Windows a path with a
// drive prefix but no root (`C:settings.yaml`) is neither absolute nor
// free of `..`, yet joining it replaces the base path entirely. Accepting
// only plain name components rules that out by construction — the paths
// DeepMate itself writes (`profiles/<scene>/settings.yaml`) are exactly
// that shape.
fn resolve_inside_home(home: &Path, raw: &str) -> CoreResult<PathBuf> {
    let candidate = Path::new(raw);
    let plain = !raw.is_empty()
        && candidate.components().all(|component| {
            matches!(
                component,
                std::path::Component::Normal(_) | std::path::Component::CurDir
            )
        });
    if !plain {
        return Err(CoreError::InvalidState(format!(
            "settings path must stay inside the harness home: {raw}"
        )));
    }
    Ok(home.join(candidate))
}

// Bootstrap a scenario's isolated settings document: append the settings-row
// redirection to the profile's cordis.patch.yml (when not already present)
// and make the per-scenario file exist. Any LLM configuration from the
// legacy global document is carried over once, so an upgrade never loses
// existing routes or models. The running scenario must be restarted for the
// redirection to take effect (its settings-file provider pins the path at
// boot).
pub fn ensure_scene_settings(profile_id: &str) -> CoreResult<()> {
    let Some(home) = dsh_home() else {
        return Ok(());
    };
    let dir = home.join("profiles").join(profile_id);
    let patch = dir.join("cordis.patch.yml");
    let scene = dir.join(SCENE_SETTINGS_FILE);
    if scene_settings_path(profile_id)?.is_none() {
        let text: String = std::fs::read_to_string(&patch).unwrap_or_default();
        if !text.contains("id: settings") {
            let mut out = text;
            if !out.is_empty() && !out.ends_with('\n') {
                out.push('\n');
            }
            out.push_str(&SCENE_SETTINGS_PATCH.replace("__SCENE__", profile_id));
            if patch.is_file() {
                write_backup(&patch, MANIFEST_BACKUP_SUFFIX)?;
            }
            if let Some(parent) = patch.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&patch, out)?;
        }
    }
    if !scene.is_file() {
        // Carry the legacy global document's LLM sections over once.
        let global = home.join("settings.yaml");
        if let Ok(text) = std::fs::read_to_string(&global) {
            if let Ok(doc) = yaml::from_str::<Value>(&text) {
                if let Some(map) = doc.as_mapping() {
                    let mut carry = Mapping::new();
                    for key in ["llm-deepseek", "llm-pi-ai", "agent-default-model"] {
                        if let Some(value) = map.get(key) {
                            carry.insert(key.to_string(), value.clone());
                        }
                    }
                    if !carry.is_empty() {
                        let out = yaml::to_string(&Value::Mapping(carry)).map_err(|err| {
                            CoreError::InvalidState(format!(
                                "failed to serialize migrated settings: {err}"
                            ))
                        })?;
                        if let Some(parent) = scene.parent() {
                            std::fs::create_dir_all(parent)?;
                        }
                        std::fs::write(&scene, out)?;
                    }
                }
            }
        }
    }
    Ok(())
}

// The settings document a profile reads and writes: the per-scenario
// document when the profile redirects it, the legacy global `settings.yaml`
// otherwise.
fn settings_path(profile_id: &str) -> CoreResult<Option<PathBuf>> {
    Ok(match scene_settings_path(profile_id)? {
        Some(path) => Some(path),
        None => dsh_home().map(|home| home.join("settings.yaml")),
    })
}

fn read_settings(profile_id: &str) -> CoreResult<SettingsDocument> {
    let Some(path) = settings_path(profile_id)? else {
        return Ok(SettingsDocument::default());
    };
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Ok(SettingsDocument::default())
        }
        Err(err) => return Err(err.into()),
    };
    if text.trim().is_empty() {
        return Ok(SettingsDocument::default());
    }
    yaml::from_str(&text).map_err(|err| {
        CoreError::InvalidState(format!("invalid settings {}: {err}", path.display()))
    })
}

// Normalize one catalog entry into a `Model`, attaching its provider route.
fn model_from_catalog(provider: &str, model: &CatalogModel) -> Model {
    Model {
        id: model.id.clone(),
        name: model.name.clone().unwrap_or_else(|| model.id.clone()),
        provider: Some(provider.to_string()),
        context_window: model.context_window,
        max_tokens: model.max_tokens,
        input: model.input.clone(),
        reasoning_efforts: model.reasoning_efforts.as_ref().and_then(yaml_to_json),
        compat: model.compat.as_ref().and_then(yaml_to_json),
    }
}

// Convert a `yaml::Value` to its JSON string form. `yaml::Value`
// implements `Serialize`, so this round-trips opaque blocks losslessly.
fn yaml_to_json(value: &Value) -> Option<String> {
    match value {
        Value::Null => None,
        _ => serde_json::to_string(value).ok(),
    }
}

// List the providers of one scenario: the always-composed deepseek route
// plus every pi-ai provider profile supplied by the scenario's settings
// document.
pub fn list_providers(profile_id: &str) -> CoreResult<Vec<Provider>> {
    let settings = read_settings(profile_id)?;
    let mut providers = vec![Provider {
        id: DEEPSEEK_PROVIDER.to_string(),
        name: "DeepSeek".to_string(),
        kind: "deepseek".to_string(),
        api: None,
        base_url: settings
            .llm_deepseek
            .as_ref()
            .and_then(|s| s.base_url.clone()),
        api_key_env: settings
            .llm_deepseek
            .as_ref()
            .and_then(|s| s.api_key_env.clone()),
        compat: None,
    }];
    if let Some(pi_ai) = &settings.llm_pi_ai {
        if let Some(routes) = &pi_ai.providers {
            for (route, profile) in routes {
                providers.push(Provider {
                    id: route.clone(),
                    name: profile
                        .display_name
                        .clone()
                        .unwrap_or_else(|| route.clone()),
                    kind: "pi-ai".to_string(),
                    api: profile.api.clone(),
                    base_url: profile.base_url.clone(),
                    api_key_env: profile.api_key_env.clone(),
                    compat: profile.compat.as_ref().and_then(yaml_to_json),
                });
            }
        }
    }
    providers.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(providers)
}

// List the models of one scenario: the deepseek catalog (settings override
// or shipped defaults) plus every model of every pi-ai provider profile in
// the scenario's settings document.
pub fn list_models(profile_id: &str) -> CoreResult<Vec<Model>> {
    let settings = read_settings(profile_id)?;
    let mut models = Vec::new();
    match settings
        .llm_deepseek
        .as_ref()
        .and_then(|section| section.models.as_ref())
    {
        Some(catalog) => {
            for model in catalog {
                models.push(model_from_catalog(DEEPSEEK_PROVIDER, model));
            }
        }
        None => {
            for (id, name) in DEFAULT_DEEPSEEK_MODELS {
                models.push(Model {
                    id: id.to_string(),
                    name: name.to_string(),
                    provider: Some(DEEPSEEK_PROVIDER.to_string()),
                    context_window: None,
                    max_tokens: None,
                    input: None,
                    reasoning_efforts: None,
                    compat: None,
                });
            }
        }
    }
    if let Some(pi_ai) = &settings.llm_pi_ai {
        if let Some(routes) = &pi_ai.providers {
            for (route, profile) in routes {
                if let Some(catalog) = &profile.models {
                    for model in catalog {
                        models.push(model_from_catalog(route, model));
                    }
                }
            }
        }
    }
    models.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(models)
}

// The backup file written before the settings document is edited, so a
// destructive save can be rolled back. Mirrors the harness's own
// `settings.yaml.bak-*` habit.
const SETTINGS_BACKUP_SUFFIX: &str = ".deepmate.bak";

// Reads and rewrites the harness settings document (`$DSH_HOME/settings.yaml`)
// while preserving every section DeepMate does not manage.
//
// The document is loaded as an order-preserving `yaml::Value` tree
// (backed by an insertion-ordered map), only the `llm-pi-ai.providers` and
// `llm-deepseek.models` subtrees are mutated, and the whole document is
// written back. Unmanaged sections (`pet`, `ui-theme`, `llm-deepseek` scalar
// settings, other providers' opaque blocks) survive untouched.
pub struct SettingsEditor;

impl SettingsEditor {
    // Load a scenario's settings document as a mutable value tree. A missing
    // file yields an empty document that will be created on first save.
    fn load(profile_id: &str) -> CoreResult<Value> {
        let Some(path) = settings_path(profile_id)? else {
            return Ok(Value::Mapping(Mapping::new()));
        };
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Value::Mapping(Mapping::new()))
            }
            Err(err) => return Err(err.into()),
        };
        if text.trim().is_empty() {
            return Ok(Value::Mapping(Mapping::new()));
        }
        yaml::from_str::<Value>(&text).map_err(|err| {
            CoreError::InvalidState(format!("invalid settings {}: {err}", path.display()))
        })
    }

    // Write a scenario's settings document back, backing up the previous
    // file first. Creating parent directories and the backup are both
    // best-effort; a read-only home must not silently claim success, so the
    // write itself is checked.
    fn save(profile_id: &str, doc: &Value) -> CoreResult<()> {
        let Some(path) = settings_path(profile_id)? else {
            return Ok(());
        };
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        if path.is_file() {
            write_backup(&path, SETTINGS_BACKUP_SUFFIX)?;
        }
        let text = yaml::to_string(doc).map_err(|err| {
            CoreError::InvalidState(format!("failed to serialize settings: {err}"))
        })?;
        std::fs::write(&path, text)?;
        Ok(())
    }

    // Run a mutation against a scenario's settings document and persist it.
    fn mutate(
        profile_id: &str,
        mutate: impl FnOnce(&mut Value) -> CoreResult<()>,
    ) -> CoreResult<()> {
        let mut doc = Self::load(profile_id)?;
        mutate(&mut doc)?;
        Self::save(profile_id, &doc)
    }

    // The `llm-pi-ai.providers` mapping, created on demand.
    fn providers_map(doc: &mut Value) -> CoreResult<&mut Mapping> {
        if doc.get("llm-pi-ai").is_none() {
            doc.insert("llm-pi-ai", Value::Mapping(Mapping::new()));
        }
        let pi_ai = doc
            .get_mut("llm-pi-ai")
            .and_then(|v| v.as_mapping_mut())
            .ok_or_else(|| CoreError::InvalidState("`llm-pi-ai` is not a mapping".to_string()))?;
        if pi_ai.get("providers").is_none() {
            pi_ai.insert("providers", Value::Mapping(Mapping::new()));
        }
        pi_ai
            .get_mut("providers")
            .and_then(|v| v.as_mapping_mut())
            .ok_or_else(|| {
                CoreError::InvalidState("`llm-pi-ai.providers` is not a mapping".to_string())
            })
    }

    // The `llm-deepseek` mapping, created on demand.
    fn deepseek_map(doc: &mut Value) -> CoreResult<&mut Mapping> {
        if doc.get("llm-deepseek").is_none() {
            doc.insert("llm-deepseek", Value::Mapping(Mapping::new()));
        }
        doc.get_mut("llm-deepseek")
            .and_then(|v| v.as_mapping_mut())
            .ok_or_else(|| CoreError::InvalidState("`llm-deepseek` is not a mapping".to_string()))
    }

    // Create or update a provider route.
    //
    // The `deepseek-official` route is a special case: it maps to the
    // `llm-deepseek` section (base URL, key env and models), while every other
    // route lives under `llm-pi-ai.providers`. `kind` is not persisted — it
    // only distinguishes the deepseek route from pi-ai routes for display.
    pub fn upsert_provider(profile_id: &str, provider: &Provider) -> CoreResult<()> {
        let provider = provider.clone();
        Self::mutate(profile_id, move |doc| {
            if provider.id == DEEPSEEK_PROVIDER {
                let section = Self::deepseek_map(doc)?;
                if let Some(base_url) = &provider.base_url {
                    section.insert("baseURL", Value::from(base_url.as_str()));
                }
                if let Some(api_key_env) = &provider.api_key_env {
                    section.insert("apiKeyEnv", Value::from(api_key_env.as_str()));
                }
            } else {
                let providers = Self::providers_map(doc)?;
                let entry = providers
                    .get_mut(&provider.id)
                    .cloned()
                    .unwrap_or_else(|| Value::Mapping(Mapping::new()));
                let mut entry = entry.as_mapping().cloned().ok_or_else(|| {
                    CoreError::InvalidState("provider entry is not a mapping".to_string())
                })?;
                entry.insert("displayName", Value::from(provider.name.as_str()));
                if let Some(api) = &provider.api {
                    entry.insert("api", Value::from(api.as_str()));
                }
                if let Some(base_url) = &provider.base_url {
                    entry.insert("baseURL", Value::from(base_url.as_str()));
                }
                if let Some(api_key_env) = &provider.api_key_env {
                    entry.insert("apiKeyEnv", Value::from(api_key_env.as_str()));
                }
                if let Some(compat) = json_to_yaml(&provider.compat)? {
                    entry.insert("compat", compat);
                }
                providers.insert(provider.id.clone(), Value::Mapping(entry));
            }
            Ok(())
        })
    }

    // Remove a provider route. The `deepseek-official` route cannot be removed
    // (it is composed into every profile by the base bundle); removing it is a
    // no-op that reports the attempt is unsupported.
    pub fn remove_provider(profile_id: &str, id: &str) -> CoreResult<()> {
        if id == DEEPSEEK_PROVIDER {
            return Err(CoreError::Unsupported(
                "the deepseek-official route is composed into every profile and cannot be removed"
                    .to_string(),
            ));
        }
        Self::mutate(profile_id, |doc| {
            Self::providers_map(doc)?.remove(id);
            Ok(())
        })
    }

    // Create or update a model within a provider route's catalog. The route is
    // looked up the same way providers are, so `deepseek-official` targets the
    // `llm-deepseek.models` list and other routes target `llm-pi-ai.providers`.
    pub fn upsert_model(profile_id: &str, provider_id: &str, model: &Model) -> CoreResult<()> {
        let model = model.clone();
        Self::mutate(profile_id, move |doc| {
            let catalog = if provider_id == DEEPSEEK_PROVIDER {
                Self::deepseek_map(doc)?.get_mut("models")
            } else {
                let providers = Self::providers_map(doc)?;
                providers
                    .get_mut(provider_id)
                    .and_then(|entry| entry.get_mut("models"))
            };
            let entry = model_to_yaml(&model)?;
            match catalog {
                Some(Value::Sequence(seq)) => {
                    upsert_in_sequence(seq, model.id.as_str(), entry);
                }
                // A missing catalog is created with a single entry.
                _ => {
                    let catalog_owner = if provider_id == DEEPSEEK_PROVIDER {
                        Self::deepseek_map(doc)?
                    } else {
                        let providers = Self::providers_map(doc)?;
                        let entry = providers
                            .get_mut(provider_id)
                            .ok_or_else(|| {
                                CoreError::InvalidState(format!(
                                    "provider not found: {provider_id}"
                                ))
                            })?
                            .as_mapping_mut()
                            .ok_or_else(|| {
                                CoreError::InvalidState(
                                    "provider entry is not a mapping".to_string(),
                                )
                            })?;
                        entry
                    };
                    let seq = vec![entry.clone()];
                    catalog_owner.insert("models", Value::Sequence(seq));
                }
            }
            Ok(())
        })
    }

    // Remove a model from a provider route's catalog by id.
    pub fn remove_model(profile_id: &str, provider_id: &str, id: &str) -> CoreResult<()> {
        let provider_id = provider_id.to_string();
        let id = id.to_string();
        Self::mutate(profile_id, move |doc| {
            let catalog = if provider_id == DEEPSEEK_PROVIDER {
                Self::deepseek_map(doc)?.get_mut("models")
            } else {
                let providers = Self::providers_map(doc)?;
                providers
                    .get_mut(&provider_id)
                    .and_then(|entry| entry.get_mut("models"))
            };
            if let Some(Value::Sequence(seq)) = catalog {
                seq.retain(|value| {
                    value
                        .get("id")
                        .and_then(|v| v.as_str())
                        .map(|existing| existing != id)
                        .unwrap_or(true)
                });
            }
            Ok(())
        })
    }
}

// The files the desktop "advanced" raw editor can read and write.
//
// Only known harness-owned documents are reachable: the global settings
// document, and a scenario's isolated settings or cordis patch. Each scope
// resolves to a fixed path under the harness home, so no user-supplied path
// can escape the allowed set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdvancedFileScope {
    // The legacy global `$DSH_HOME/settings.yaml`.
    GlobalSettings,
    // A scenario's isolated `profiles/<name>/settings.yaml`.
    SceneSettings,
    // A scenario's `profiles/<name>/cordis.patch.yml` (row overrides incl.
    // the settings-row redirection).
    SceneCordis,
}

// Reject profile names that could escape the profiles directory or corrupt
// the YAML a name is interpolated into.
//
// The name becomes a directory under `profiles/` and is spliced into the
// `cordis.patch.yml` redirection block, so separators, traversal, quotes,
// newlines and control characters are all refused. Only letters, digits,
// ASCII `-`, `_` and the non-ASCII word characters that appear in real
// scenario names are accepted.
pub fn validate_profile_name(name: &str) -> CoreResult<()> {
    if name.is_empty() {
        return Err(CoreError::InvalidState(
            "profile name must not be empty".to_string(),
        ));
    }
    if name.chars().count() > 64 {
        return Err(CoreError::InvalidState(
            "profile name is too long (max 64 characters)".to_string(),
        ));
    }
    if name == "node_modules" || name == "." || name == ".." {
        return Err(CoreError::InvalidState(format!(
            "invalid profile name: {name:?}"
        )));
    }
    let valid = name
        .chars()
        .all(|c| c.is_alphanumeric() || matches!(c, '-' | '_' | ' '));
    if !valid {
        return Err(CoreError::InvalidState(format!(
            "invalid profile name: {name:?}"
        )));
    }
    Ok(())
}

impl AdvancedFileScope {
    fn resolve(self, name: Option<&str>) -> CoreResult<PathBuf> {
        let Some(home) = dsh_home() else {
            return Err(CoreError::InvalidState(
                "could not resolve the harness home directory".to_string(),
            ));
        };
        match self {
            Self::GlobalSettings => Ok(home.join("settings.yaml")),
            Self::SceneSettings | Self::SceneCordis => {
                let name = name.unwrap_or_default();
                validate_profile_name(name)?;
                let file = if self == Self::SceneCordis {
                    "cordis.patch.yml"
                } else {
                    "settings.yaml"
                };
                Ok(home.join("profiles").join(name).join(file))
            }
        }
    }
}

// Read one advanced document as raw text. `Ok(None)` when the file does not
// exist yet (the editor then starts from an empty draft).
pub fn read_advanced_file(
    scope: AdvancedFileScope,
    name: Option<&str>,
) -> CoreResult<Option<String>> {
    let path = scope.resolve(name)?;
    match std::fs::read_to_string(&path) {
        Ok(text) => Ok(Some(text)),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(err) => Err(err.into()),
    }
}

// Write one advanced document back, backing up the previous file first
// (mirroring the settings editor's `.deepmate.bak` habit).
//
// Both documents are validated as YAML before the write, so a syntax error
// refuses the save with a readable message instead of letting the harness
// fail at its next boot. `cordis.patch.yml` needs one accommodation: it
// carries `!!js dshHomePath(...)` expressions, so every `!!js` payload is
// rewritten to a plain scalar for the duration of the check. That keeps the
// structure (rows, ids, nesting) validated while tolerating the custom tag.
pub fn save_advanced_file(
    scope: AdvancedFileScope,
    name: Option<&str>,
    content: &str,
) -> CoreResult<()> {
    let path = scope.resolve(name)?;
    let to_validate = if scope == AdvancedFileScope::SceneCordis {
        neutralize_js_tags(content)
    } else {
        content.to_string()
    };
    yaml::from_str::<Value>(&to_validate).map_err(|err| {
        CoreError::InvalidState(format!("invalid settings {}: {err}", path.display()))
    })?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|err| CoreError::io_at(parent, err))?;
    }
    if path.is_file() {
        write_backup(&path, SETTINGS_BACKUP_SUFFIX)?;
    }
    deepmate_core::write_atomic_string(&path, content)?;
    Ok(())
}

// Replace every `!!js <expression>` payload with a quoted placeholder so the
// rest of the document can be parsed by a YAML reader that has no handler for
// the custom tag. The expression runs to the end of its line, which is how
// the harness writes it.
fn neutralize_js_tags(content: &str) -> String {
    content
        .lines()
        .map(|line| match line.find("!!js") {
            Some(index) => {
                let mut replaced = line[..index].to_string();
                replaced.push_str("deepmate-js-expression");
                replaced
            }
            None => line.to_string(),
        })
        .collect::<Vec<_>>()
        .join("\n")
}

// Convert a raw JSON string back to a `yaml::Value`, validating it.
fn json_to_yaml(raw: &Option<String>) -> CoreResult<Option<Value>> {
    let Some(text) = raw else {
        return Ok(None);
    };
    if text.trim().is_empty() {
        return Ok(None);
    }
    let value: serde_json::Value = serde_json::from_str(text)
        .map_err(|err| CoreError::InvalidState(format!("invalid JSON capability block: {err}")))?;
    let yaml: Value = yaml::to_value(value)
        .map_err(|err| CoreError::InvalidState(format!("invalid capability block: {err}")))?;
    Ok(Some(yaml))
}

// Build the `yaml::Value` for one model entry.
fn model_to_yaml(model: &Model) -> CoreResult<Value> {
    let mut entry = Mapping::new();
    entry.insert("id", Value::from(model.id.as_str()));
    entry.insert("name", Value::from(model.name.as_str()));
    if let Some(window) = model.context_window {
        entry.insert("contextWindow", Value::from(window));
    }
    if let Some(tokens) = model.max_tokens {
        entry.insert("maxTokens", Value::from(tokens));
    }
    if let Some(input) = &model.input {
        let seq: Sequence = input.iter().map(|m| Value::from(m.as_str())).collect();
        entry.insert("input", Value::Sequence(seq));
    }
    if let Some(reasoning) = json_to_yaml(&model.reasoning_efforts)? {
        entry.insert("reasoningEfforts", reasoning);
    }
    if let Some(compat) = json_to_yaml(&model.compat)? {
        entry.insert("compat", compat);
    }
    Ok(Value::Mapping(entry))
}

// Upsert one model entry into a `models` sequence by id, preserving the
// position of an existing entry and appending new ones.
fn upsert_in_sequence(seq: &mut Sequence, id: &str, entry: Value) {
    for value in seq.iter_mut() {
        if value
            .get("id")
            .and_then(|v| v.as_str())
            .map(|existing| existing == id)
            .unwrap_or(false)
        {
            *value = entry;
            return;
        }
    }
    seq.push(entry);
}

// Create a new profile directory with a minimal manifest, so a later
// `dsh plugin --profile <name>` has a `package.json` to reconcile against.
pub fn create_profile(name: &str) -> CoreResult<()> {
    if name.is_empty() || name == "node_modules" || name.contains('/') || name.contains('\\') {
        return Err(CoreError::InvalidState(format!(
            "invalid profile name: {name:?}"
        )));
    }
    let Some(home) = dsh_home() else {
        return Err(CoreError::InvalidState(
            "could not resolve the harness home directory".to_string(),
        ));
    };
    let dir = home.join("profiles").join(name);
    if dir.join("package.json").is_file() {
        return Err(CoreError::InvalidState(format!(
            "profile already exists: {name}"
        )));
    }
    std::fs::create_dir_all(&dir)?;
    let manifest = serde_json::json!({
        "name": format!("dsh-profile-{name}"),
        "private": true,
        "dependencies": {},
        "dsh": { "profile": {} },
    });
    std::fs::write(
        dir.join("package.json"),
        serde_json::to_string_pretty(&manifest).map_err(|err| {
            CoreError::InvalidState(format!("failed to serialize manifest: {err}"))
        })?,
    )?;
    Ok(())
}

// Remove a profile directory. Refuses to remove the launcher-maintained
// `node_modules` fallback or any path that does not look like a profile.
pub fn remove_profile(name: &str) -> CoreResult<()> {
    validate_profile_name(name)?;
    let Some(home) = dsh_home() else {
        return Err(CoreError::InvalidState(
            "could not resolve the harness home directory".to_string(),
        ));
    };
    let dir = home.join("profiles").join(name);
    if !dir.join("package.json").is_file() {
        return Err(CoreError::InvalidState(format!(
            "profile not found: {name}"
        )));
    }
    // A profile holds settings, providers, models and its installed packages;
    // deleting it is the most destructive thing DeepMate does to harness
    // state. Move it into a timestamped trash directory under the harness
    // home instead of unlinking it, so a mistaken delete is recoverable by
    // hand and nothing is destroyed irreversibly.
    let trash = home.join("profiles").join(".deepmate-trash");
    std::fs::create_dir_all(&trash).map_err(|err| CoreError::io_at(&trash, err))?;
    let stamp = chrono::Utc::now().format("%Y%m%d-%H%M%S");
    let target = trash.join(format!("{name}-{stamp}"));
    std::fs::rename(&dir, &target).map_err(|err| {
        CoreError::InvalidState(format!(
            "failed to remove profile {name}: could not move {} to {}: {err}",
            dir.display(),
            target.display()
        ))
    })?;
    let _ = stamp;
    Ok(())
}

// Update a profile's `description` field in its manifest.
//
// `None` (or an empty string) removes the field, so the engine's bundles
// fallback description takes over again. Follows `rename_profile`'s
// read-edit-write habit with a `.deepmate.bak` backup.
pub fn set_profile_description(name: &str, description: Option<String>) -> CoreResult<()> {
    validate_profile_name(name)?;
    let Some(home) = dsh_home() else {
        return Err(CoreError::InvalidState(
            "could not resolve the harness home directory".to_string(),
        ));
    };
    let manifest = home.join("profiles").join(name).join("package.json");
    if !manifest.is_file() {
        return Err(CoreError::InvalidState(format!(
            "profile not found: {name}"
        )));
    }
    let text = std::fs::read_to_string(&manifest)?;
    let mut doc: serde_json::Value = serde_json::from_str(&text).map_err(|err| {
        CoreError::InvalidState(format!(
            "invalid profile manifest {}: {err}",
            manifest.display()
        ))
    })?;
    match description.as_deref() {
        Some("") | None => {
            if let Some(object) = doc.as_object_mut() {
                object.remove("description");
            }
        }
        Some(text) => {
            doc["description"] = serde_json::Value::from(text);
        }
    }
    write_backup(&manifest, MANIFEST_BACKUP_SUFFIX)?;
    let text = serde_json::to_string_pretty(&doc).map_err(|err| {
        CoreError::InvalidState(format!("failed to serialize {}: {err}", manifest.display()))
    })?;
    std::fs::write(&manifest, text)?;
    Ok(())
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use std::sync::Mutex;

    // Env-var tests must not race each other.
    pub(crate) static ENV_LOCK: Mutex<()> = Mutex::new(());
    static FIXTURE_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

    fn fixture_home() -> PathBuf {
        let n = FIXTURE_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        std::env::temp_dir().join(format!("deepmate-dsh-test-{}-{n}", std::process::id()))
    }

    fn write_profile(home: &Path, name: &str, deps: &[(&str, &str)], bundles: &[&str]) {
        let dir = home.join("profiles").join(name);
        std::fs::create_dir_all(&dir).unwrap();
        let mut deps_map = serde_json::Map::new();
        for (dep, version) in deps {
            deps_map.insert(
                dep.to_string(),
                serde_json::Value::String(version.to_string()),
            );
        }
        let manifest = serde_json::json!({
            "name": format!("dsh-profile-{name}"),
            "private": true,
            "dependencies": deps_map,
            "dsh": { "profile": { "bundles": bundles } },
        });
        std::fs::write(
            dir.join("package.json"),
            serde_json::to_string_pretty(&manifest).unwrap(),
        )
        .unwrap();
    }

    fn restore_env(key: &str, previous: Option<std::ffi::OsString>) {
        match previous {
            Some(value) => std::env::set_var(key, value),
            None => std::env::remove_var(key),
        }
    }

    #[test]
    fn dsh_home_honors_env_override() {
        let _guard = ENV_LOCK.lock().unwrap();
        let previous = std::env::var_os(DSH_HOME_ENV);
        let home = fixture_home();
        std::env::set_var(DSH_HOME_ENV, &home);
        assert_eq!(dsh_home(), Some(home));
        restore_env(DSH_HOME_ENV, previous);
    }

    #[test]
    fn discover_profiles_lists_manifest_directories() {
        let _guard = ENV_LOCK.lock().unwrap();
        let previous = std::env::var_os(DSH_HOME_ENV);
        let home = fixture_home();
        write_profile(
            &home,
            "web",
            &[],
            &["@deepseek-ai/dsh-base", "@deepseek-ai/dsh-web-app"],
        );
        write_profile(&home, "headless", &[], &["@deepseek-ai/dsh-base"]);
        // A directory without a manifest is not a profile.
        std::fs::create_dir_all(home.join("profiles").join("scratch")).unwrap();
        // The launcher-maintained fallback is not a profile.
        std::fs::create_dir_all(home.join("profiles").join("node_modules")).unwrap();
        std::env::set_var(DSH_HOME_ENV, &home);
        let profiles = discover_profiles().unwrap();
        let ids: Vec<&str> = profiles.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, ["headless", "web"]);
        assert_eq!(
            profiles[1].description.as_deref(),
            Some("bundles: @deepseek-ai/dsh-base, @deepseek-ai/dsh-web-app")
        );
        restore_env(DSH_HOME_ENV, previous);
    }

    #[test]
    fn discover_profiles_returns_empty_without_home() {
        let _guard = ENV_LOCK.lock().unwrap();
        let previous = std::env::var_os(DSH_HOME_ENV);
        let previous_home = std::env::var_os("HOME");
        std::env::remove_var(DSH_HOME_ENV);
        // Point HOME at an empty fixture so a real ~/.dsh never leaks in.
        std::env::set_var("HOME", fixture_home());
        assert!(discover_profiles().unwrap().is_empty());
        restore_env("HOME", previous_home);
        restore_env(DSH_HOME_ENV, previous);
    }

    #[test]
    fn list_plugins_reads_bundles_dependencies_and_install_state() {
        let _guard = ENV_LOCK.lock().unwrap();
        let previous = std::env::var_os(DSH_HOME_ENV);
        let home = fixture_home();
        write_profile(
            &home,
            "web",
            &[("turtle-ui", "^1.0.0"), ("plain-lib", "2.0.0")],
            &["@deepseek-ai/dsh-base"],
        );
        // turtle-ui is installed; plain-lib and the base bundle are not.
        std::fs::create_dir_all(home.join("profiles/web/node_modules/turtle-ui")).unwrap();
        std::env::set_var(DSH_HOME_ENV, &home);
        let plugins = list_plugins("web").unwrap();
        assert_eq!(plugins.len(), 3);
        assert_eq!(plugins[0].id, "@deepseek-ai/dsh-base");
        assert!(!plugins[0].enabled);
        assert_eq!(plugins[1].id, "plain-lib");
        assert!(!plugins[1].enabled);
        assert_eq!(plugins[1].version.as_deref(), Some("2.0.0"));
        assert_eq!(plugins[2].id, "turtle-ui");
        assert!(plugins[2].enabled);
        assert_eq!(plugins[2].version.as_deref(), Some("^1.0.0"));
        restore_env(DSH_HOME_ENV, previous);
    }

    #[test]
    fn list_plugins_deduplicates_bundle_and_dependency_entries() {
        // The real web profile declares the same package both as a bundle
        // layer and as an npm dependency; the plugin must be listed once,
        // keeping the dependency's range as the version fallback.
        let _guard = ENV_LOCK.lock().unwrap();
        let previous = std::env::var_os(DSH_HOME_ENV);
        let home = fixture_home();
        write_profile(
            &home,
            "web",
            &[("dsh-context", "^0.19.2"), ("@liustack/modlens", "^3.23.1")],
            &["dsh-context", "@liustack/modlens", "@deepseek-ai/dsh-base"],
        );
        std::env::set_var(DSH_HOME_ENV, &home);
        let plugins = list_plugins("web").unwrap();
        let ids: Vec<&str> = plugins.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(
            ids,
            ["@deepseek-ai/dsh-base", "@liustack/modlens", "dsh-context"]
        );
        let context = plugins.iter().find(|p| p.id == "dsh-context").unwrap();
        assert_eq!(context.version.as_deref(), Some("^0.19.2"));
        restore_env(DSH_HOME_ENV, previous);
    }

    #[test]
    fn plugins_resolve_from_launcher_shared_fallback() {
        let _guard = ENV_LOCK.lock().unwrap();
        let previous = std::env::var_os(DSH_HOME_ENV);
        let home = fixture_home();
        write_profile(&home, "web", &[], &["@deepseek-ai/dsh-base"]);
        // The real dsh web profile installs its bundles into the shared
        // launcher fallback, not the profile directory, and carries a real
        // installed version.
        let bundle_dir = home.join("profiles/node_modules/@deepseek-ai/dsh-base");
        std::fs::create_dir_all(&bundle_dir).unwrap();
        std::fs::write(
            bundle_dir.join("package.json"),
            r#"{"name": "@deepseek-ai/dsh-base", "version": "0.3.1"}"#,
        )
        .unwrap();
        std::env::set_var(DSH_HOME_ENV, &home);
        let plugins = list_plugins("web").unwrap();
        assert_eq!(plugins.len(), 1);
        assert_eq!(plugins[0].id, "@deepseek-ai/dsh-base");
        assert!(plugins[0].enabled);
        assert_eq!(plugins[0].version.as_deref(), Some("0.3.1"));
        restore_env(DSH_HOME_ENV, previous);
    }

    #[test]
    fn remove_bundle_strips_entry_and_preserves_manifest() {
        let _guard = ENV_LOCK.lock().unwrap();
        let previous = std::env::var_os(DSH_HOME_ENV);
        let home = fixture_home();
        write_profile(
            &home,
            "web",
            &[("turtle-ui", "^1.0.0")],
            &["@deepseek-ai/dsh-base", "turtle-ui"],
        );
        std::env::set_var(DSH_HOME_ENV, &home);

        assert!(bundle_declared("web", "turtle-ui").unwrap());
        assert!(remove_bundle("web", "turtle-ui").unwrap());
        // A second strip finds nothing and does not rewrite.
        assert!(!remove_bundle("web", "turtle-ui").unwrap());
        assert!(!bundle_declared("web", "turtle-ui").unwrap());
        assert!(bundle_declared("web", "@deepseek-ai/dsh-base").unwrap());

        // The declared dependency survives the rewrite untouched.
        let plugins = list_plugins("web").unwrap();
        assert_eq!(plugins.len(), 2);
        assert!(plugins.iter().any(|p| p.id == "turtle-ui"));
        assert!(!plugins.iter().any(|p| p.id == "turtle-ui" && p.enabled));

        // And the backup was written before the edit.
        assert!(home
            .join("profiles/web/package.json.deepmate.bak")
            .is_file());

        restore_env(DSH_HOME_ENV, previous);
    }

    #[test]
    fn remove_bundle_noops_without_bundle_declaration() {
        let _guard = ENV_LOCK.lock().unwrap();
        let previous = std::env::var_os(DSH_HOME_ENV);
        let home = fixture_home();
        write_profile(&home, "web", &[("plain-lib", "1.0.0")], &[]);
        std::env::set_var(DSH_HOME_ENV, &home);

        assert!(!bundle_declared("web", "plain-lib").unwrap());
        assert!(!remove_bundle("web", "plain-lib").unwrap());
        // A dep-only removal must not destroy the manifest.
        let plugins = list_plugins("web").unwrap();
        assert!(plugins.iter().any(|p| p.id == "plain-lib"));

        restore_env(DSH_HOME_ENV, previous);
    }

    #[test]
    fn remove_bundle_errors_on_missing_profile() {
        let _guard = ENV_LOCK.lock().unwrap();
        let previous = std::env::var_os(DSH_HOME_ENV);
        let home = fixture_home();
        std::env::set_var(DSH_HOME_ENV, &home);
        // A missing profile has no manifest to rewrite; the caller must see
        // the error rather than silently succeed.
        assert!(remove_bundle("missing", "pkg").is_err());
        restore_env(DSH_HOME_ENV, previous);
    }

    #[test]
    fn add_bundle_appends_and_is_idempotent() {
        let _guard = ENV_LOCK.lock().unwrap();
        let previous = std::env::var_os(DSH_HOME_ENV);
        let home = fixture_home();
        // A profile whose bundle list does not yet contain the plugin.
        write_profile(
            &home,
            "web",
            &[("turtle-ui", "^1.0.0")],
            &["@deepseek-ai/dsh-base"],
        );
        std::env::set_var(DSH_HOME_ENV, &home);

        assert!(add_bundle("web", "turtle-ui").unwrap());
        assert!(bundle_declared("web", "turtle-ui").unwrap());
        // A second add finds it already declared and does not rewrite.
        assert!(!add_bundle("web", "turtle-ui").unwrap());
        assert!(bundle_declared("web", "@deepseek-ai/dsh-base").unwrap());
        // The dependency and other bundles survive untouched.
        let plugins = list_plugins("web").unwrap();
        assert!(plugins.iter().any(|p| p.id == "turtle-ui"));
        assert!(home
            .join("profiles/web/package.json.deepmate.bak")
            .is_file());

        restore_env(DSH_HOME_ENV, previous);
    }

    #[test]
    fn add_bundle_creates_the_bundle_section_on_demand() {
        let _guard = ENV_LOCK.lock().unwrap();
        let previous = std::env::var_os(DSH_HOME_ENV);
        let home = fixture_home();
        // A freshly scaffolded profile has `"dsh": { "profile": {} }` with no
        // bundles list yet.
        let dir = home.join("profiles/daily");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("package.json"),
            serde_json::to_string_pretty(&serde_json::json!({
                "name": "dsh-profile-daily",
                "private": true,
                "dependencies": { "turtle-ui": "^1.0.0" },
                "dsh": { "profile": {} },
            }))
            .unwrap(),
        )
        .unwrap();
        std::env::set_var(DSH_HOME_ENV, &home);

        assert!(add_bundle("daily", "turtle-ui").unwrap());
        assert!(bundle_declared("daily", "turtle-ui").unwrap());

        restore_env(DSH_HOME_ENV, previous);
    }

    #[test]
    fn declares_dsh_capability_reads_the_installed_manifest() {
        let _guard = ENV_LOCK.lock().unwrap();
        let previous = std::env::var_os(DSH_HOME_ENV);
        let home = fixture_home();
        write_profile(
            &home,
            "web",
            &[("dsh-hook", "^1.0.0"), ("plain-lib", "2.0.0")],
            &[],
        );
        // dsh-hook is a real bundle (dsh.bundle.patch), plain-lib is not a
        // plugin at all, and a client-only package is not a bundle either.
        let dsh_dir = home.join("profiles/web/node_modules/dsh-hook");
        std::fs::create_dir_all(&dsh_dir).unwrap();
        std::fs::write(
            dsh_dir.join("package.json"),
            r#"{ "name": "dsh-hook", "version": "1.0.0", "dsh": { "bundle": { "patch": "./cordis.patch.yml" } } }"#,
        )
        .unwrap();
        std::fs::create_dir_all(home.join("profiles/web/node_modules/plain-lib")).unwrap();
        std::fs::write(
            home.join("profiles/web/node_modules/plain-lib/package.json"),
            r#"{ "name": "plain-lib", "version": "2.0.0" }"#,
        )
        .unwrap();
        let client_dir = home.join("profiles/web/node_modules/client-only");
        std::fs::create_dir_all(&client_dir).unwrap();
        std::fs::write(
            client_dir.join("package.json"),
            r#"{ "name": "client-only", "version": "1.0.0", "dsh": { "client": { "platform": "web" } } }"#,
        )
        .unwrap();
        std::env::set_var(DSH_HOME_ENV, &home);

        assert!(declares_dsh_capability("web", "dsh-hook").unwrap());
        assert!(!declares_dsh_capability("web", "plain-lib").unwrap());
        assert!(
            !declares_dsh_capability("web", "client-only").unwrap(),
            "client-only packages are not bundles by the engine's standard"
        );
        // Nothing installed: not a plugin by any measure.
        assert!(!declares_dsh_capability("web", "absent").unwrap());

        restore_env(DSH_HOME_ENV, previous);
    }

    #[test]
    fn rename_profile_moves_directory_and_updates_the_manifest_name() {
        let _guard = ENV_LOCK.lock().unwrap();
        let previous = std::env::var_os(DSH_HOME_ENV);
        let home = fixture_home();
        write_profile(
            &home,
            "daily",
            &[("turtle-ui", "^1.0.0")],
            &["@deepseek-ai/dsh-base"],
        );
        std::env::set_var(DSH_HOME_ENV, &home);

        rename_profile("daily", "dev").unwrap();
        assert!(!home.join("profiles/daily/package.json").is_file());
        assert!(home.join("profiles/dev/package.json").is_file());
        // Dependencies and bundles moved with the directory.
        let plugins = list_plugins("dev").unwrap();
        assert_eq!(plugins.len(), 2);
        let manifest: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(home.join("profiles/dev/package.json")).unwrap(),
        )
        .unwrap();
        assert_eq!(manifest["name"], "dsh-profile-dev");
        // The manifest backup was written before the rewrite.
        assert!(home
            .join("profiles/dev/package.json.deepmate.bak")
            .is_file());

        restore_env(DSH_HOME_ENV, previous);
    }

    #[test]
    fn rename_profile_validates_names_and_collisions() {
        let _guard = ENV_LOCK.lock().unwrap();
        let previous = std::env::var_os(DSH_HOME_ENV);
        let home = fixture_home();
        write_profile(&home, "daily", &[], &[]);
        write_profile(&home, "dev", &[], &[]);
        std::env::set_var(DSH_HOME_ENV, &home);

        assert!(rename_profile("", "x").is_err());
        assert!(rename_profile("x", "").is_err());
        assert!(rename_profile("node_modules", "x").is_err());
        assert!(rename_profile("a/b", "x").is_err());
        assert!(rename_profile("daily", "daily").is_err());
        assert!(rename_profile("missing", "x").is_err());
        assert!(rename_profile("daily", "dev").is_err());
        // None of the failed attempts moved anything.
        assert!(home.join("profiles/daily/package.json").is_file());
        assert!(home.join("profiles/dev/package.json").is_file());

        restore_env(DSH_HOME_ENV, previous);
    }

    #[test]
    fn discover_profiles_prefers_the_manifest_description() {
        let _guard = ENV_LOCK.lock().unwrap();
        let previous = std::env::var_os(DSH_HOME_ENV);
        let home = fixture_home();
        let dir = home.join("profiles/daily");
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("package.json"),
            serde_json::to_string_pretty(&serde_json::json!({
                "name": "dsh-profile-daily",
                "private": true,
                "description": "日常打字",
                "dependencies": {},
                "dsh": { "profile": { "bundles": ["@deepseek-ai/dsh-base"] } },
            }))
            .unwrap(),
        )
        .unwrap();
        std::env::set_var(DSH_HOME_ENV, &home);

        let profiles = discover_profiles().unwrap();
        assert_eq!(profiles[0].name, "daily");
        assert_eq!(profiles[0].description.as_deref(), Some("日常打字"));

        restore_env(DSH_HOME_ENV, previous);
    }

    #[test]
    fn ensure_scene_settings_redirects_and_migrates_once() {
        let _guard = ENV_LOCK.lock().unwrap();
        let previous = std::env::var_os(DSH_HOME_ENV);
        let home = fixture_home();
        // A legacy global document carries the existing LLM route.
        std::fs::create_dir_all(home.join("profiles/web")).unwrap();
        std::fs::write(
            home.join("settings.yaml"),
            "llm-pi-ai:\n  providers:\n    my-provider:\n      baseURL: https://global/v1\n",
        )
        .unwrap();
        std::env::set_var(DSH_HOME_ENV, &home);

        ensure_scene_settings("web").unwrap();
        // The redirection points at the scenario's own document.
        let scene = scene_settings_path("web").unwrap().unwrap();
        assert_eq!(scene, home.join("profiles/web/settings.yaml"));
        // The legacy LLM section was carried over exactly once.
        assert!(scene.is_file());
        let migrated = std::fs::read_to_string(&scene).unwrap();
        assert!(migrated.contains("my-provider"), "{migrated}");
        // The global document is untouched.
        let global = std::fs::read_to_string(home.join("settings.yaml")).unwrap();
        assert!(global.contains("llm-pi-ai"));

        // A second bootstrap does not duplicate the redirection row.
        ensure_scene_settings("web").unwrap();
        let patch = std::fs::read_to_string(home.join("profiles/web/cordis.patch.yml")).unwrap();
        assert_eq!(patch.matches("id: settings").count(), 1, "{patch}");

        restore_env(DSH_HOME_ENV, previous);
    }

    #[test]
    fn scene_settings_isolate_providers_and_models() {
        let _guard = ENV_LOCK.lock().unwrap();
        let previous = std::env::var_os(DSH_HOME_ENV);
        let home = fixture_home();
        std::fs::create_dir_all(home.join("profiles/web")).unwrap();
        std::fs::create_dir_all(home.join("profiles/coding")).unwrap();
        std::env::set_var(DSH_HOME_ENV, &home);

        ensure_scene_settings("web").unwrap();
        ensure_scene_settings("coding").unwrap();
        // The same route name configured differently per scenario.
        let provider = |base_url: &str| Provider {
            id: "gateway".to_string(),
            name: "Gateway".to_string(),
            kind: "pi-ai".to_string(),
            api: Some("openai-completions".to_string()),
            base_url: Some(base_url.to_string()),
            api_key_env: Some("GW_KEY".to_string()),
            compat: None,
        };
        SettingsEditor::upsert_provider("web", &provider("https://web-gw/v1")).unwrap();
        SettingsEditor::upsert_provider("coding", &provider("https://coding-gw/v1")).unwrap();

        let web = list_providers("web").unwrap();
        let coding = list_providers("coding").unwrap();
        let web_gw = web.iter().find(|p| p.id == "gateway").unwrap();
        let coding_gw = coding.iter().find(|p| p.id == "gateway").unwrap();
        assert_eq!(web_gw.base_url.as_deref(), Some("https://web-gw/v1"));
        assert_eq!(coding_gw.base_url.as_deref(), Some("https://coding-gw/v1"));

        // Models follow their provider per scenario.
        let model = |id: &str| Model {
            id: id.to_string(),
            name: id.to_string(),
            provider: Some("gateway".to_string()),
            context_window: None,
            max_tokens: None,
            input: None,
            reasoning_efforts: None,
            compat: None,
        };
        SettingsEditor::upsert_model("web", "gateway", &model("model-x")).unwrap();
        SettingsEditor::upsert_model("coding", "gateway", &model("model-y")).unwrap();
        let web_models = list_models("web").unwrap();
        let coding_models = list_models("coding").unwrap();
        assert!(web_models.iter().any(|m| m.id == "model-x"));
        assert!(!web_models.iter().any(|m| m.id == "model-y"));
        assert!(!coding_models.iter().any(|m| m.id == "model-x"));
        assert!(coding_models.iter().any(|m| m.id == "model-y"));

        restore_env(DSH_HOME_ENV, previous);
    }

    #[test]
    fn scene_settings_fall_back_to_the_global_document() {
        let _guard = ENV_LOCK.lock().unwrap();
        let previous = std::env::var_os(DSH_HOME_ENV);
        let home = fixture_home();
        std::fs::create_dir_all(home.join("profiles/legacy")).unwrap();
        std::fs::write(
            home.join("settings.yaml"),
            "llm-pi-ai:\n  providers:\n    old-route:\n      baseURL: https://global/v1\n",
        )
        .unwrap();
        std::env::set_var(DSH_HOME_ENV, &home);

        // A profile without a redirection reads the global document.
        let providers = list_providers("legacy").unwrap();
        assert!(providers.iter().any(|p| p.id == "old-route"));

        restore_env(DSH_HOME_ENV, previous);
    }

    #[test]
    fn providers_and_models_read_settings() {
        let _guard = ENV_LOCK.lock().unwrap();
        let previous = std::env::var_os(DSH_HOME_ENV);
        let home = fixture_home();
        std::fs::create_dir_all(&home).unwrap();
        std::fs::write(
            home.join("settings.yaml"),
            r#"
llm-deepseek:
  models:
    - id: deepseek-v4-flash
      name: DeepSeek-V4-Flash
    - id: custom-model
      name: Custom Model
llm-pi-ai:
  providers:
    openai:
      displayName: OpenAI
      models:
        - id: gpt-4o
          name: GPT-4o
    anthropic:
      models:
        - id: claude-sonnet-4
"#,
        )
        .unwrap();
        std::env::set_var(DSH_HOME_ENV, &home);
        let providers = list_providers("web").unwrap();
        let ids: Vec<&str> = providers.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, ["anthropic", "deepseek-official", "openai"]);
        assert_eq!(providers[2].name, "OpenAI");
        let models = list_models("web").unwrap();
        let ids: Vec<&str> = models.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(
            ids,
            [
                "claude-sonnet-4",
                "custom-model",
                "deepseek-v4-flash",
                "gpt-4o"
            ]
        );
        assert_eq!(models[3].provider.as_deref(), Some("openai"));
        restore_env(DSH_HOME_ENV, previous);
    }

    #[test]
    fn providers_and_models_default_without_settings() {
        let _guard = ENV_LOCK.lock().unwrap();
        let previous = std::env::var_os(DSH_HOME_ENV);
        let home = fixture_home();
        std::fs::create_dir_all(&home).unwrap();
        std::env::set_var(DSH_HOME_ENV, &home);
        let providers = list_providers("web").unwrap();
        assert_eq!(providers.len(), 1);
        assert_eq!(providers[0].id, "deepseek-official");
        let models = list_models("web").unwrap();
        let ids: Vec<&str> = models.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(ids, ["deepseek-v4-flash", "deepseek-v4-pro"]);
        restore_env(DSH_HOME_ENV, previous);
    }

    #[test]
    fn missing_profile_manifest_reads_as_empty() {
        let _guard = ENV_LOCK.lock().unwrap();
        let previous = std::env::var_os(DSH_HOME_ENV);
        let home = fixture_home();
        std::fs::create_dir_all(&home).unwrap();
        std::env::set_var(DSH_HOME_ENV, &home);
        // A fresh harness home has no profiles: every read-only query must
        // answer rather than fail (this is what `deepmate status` hits).
        assert_eq!(profile_surface("web").unwrap(), Surface::Undetermined);
        assert!(list_plugins("web").unwrap().is_empty());
        assert_eq!(declared_spec("web", "dsh-mnemon").unwrap(), None);
        assert!(!bundle_declared("web", "dsh-mnemon").unwrap());
        restore_env(DSH_HOME_ENV, previous);
    }

    #[test]
    fn settings_rejects_invalid_yaml() {
        let _guard = ENV_LOCK.lock().unwrap();
        let previous = std::env::var_os(DSH_HOME_ENV);
        let home = fixture_home();
        std::fs::create_dir_all(&home).unwrap();
        std::fs::write(home.join("settings.yaml"), "llm-deepseek: [unclosed").unwrap();
        std::env::set_var(DSH_HOME_ENV, &home);
        assert!(list_providers("web").is_err());
        restore_env(DSH_HOME_ENV, previous);
    }

    // A minimal settings document with both a deepseek route and one pi-ai
    // provider carrying an opaque compat block.
    fn write_settings(home: &Path) {
        std::fs::create_dir_all(home).unwrap();
        std::fs::write(
            home.join("settings.yaml"),
            r#"
ui-theme:
  preference: system
llm-deepseek:
  apiKeyEnv: DS_API_KEY
  baseURL: https://api.deepseek.com
  models:
    - id: deepseek-v4-flash
      name: DeepSeek-V4-Flash
llm-pi-ai:
  providers:
    cdx:
      displayName: Codex
      api: openai-responses
      baseURL: https://example.com/v1
      apiKeyEnv: CDX_API_KEY
      compat:
        supportsStore: false
      models:
        - id: gpt-5.6-sol
          name: GPT 5.6 Sol
          contextWindow: 262144
          maxTokens: 32768
pet:
  visible: false
"#,
        )
        .unwrap();
    }

    #[test]
    fn editor_upserts_and_reads_back_provider_and_model() {
        let _guard = ENV_LOCK.lock().unwrap();
        let previous = std::env::var_os(DSH_HOME_ENV);
        let home = fixture_home();
        write_settings(&home);
        std::env::set_var(DSH_HOME_ENV, &home);

        // Add a new pi-ai provider and a model under it.
        SettingsEditor::upsert_provider(
            "web",
            &Provider {
                id: "ollama".to_string(),
                name: "Ollama".to_string(),
                kind: "pi-ai".to_string(),
                api: Some("openai-completions".to_string()),
                base_url: Some("http://localhost:11434/v1".to_string()),
                api_key_env: Some("OLLAMA_KEY".to_string()),
                compat: None,
            },
        )
        .unwrap();
        SettingsEditor::upsert_model(
            "web",
            "ollama",
            &Model {
                id: "llama-3".to_string(),
                name: "Llama 3".to_string(),
                provider: Some("ollama".to_string()),
                context_window: Some(8192),
                max_tokens: Some(4096),
                input: Some(vec!["text".to_string()]),
                reasoning_efforts: Some(r#"{"max":"max"}"#.to_string()),
                compat: None,
            },
        )
        .unwrap();

        let providers = list_providers("web").unwrap();
        let ollama = providers.iter().find(|p| p.id == "ollama").unwrap();
        assert_eq!(ollama.name, "Ollama");
        assert_eq!(ollama.api.as_deref(), Some("openai-completions"));
        assert_eq!(
            ollama.base_url.as_deref(),
            Some("http://localhost:11434/v1")
        );
        assert_eq!(ollama.api_key_env.as_deref(), Some("OLLAMA_KEY"));

        let models = list_models("web").unwrap();
        let llama = models.iter().find(|m| m.id == "llama-3").unwrap();
        assert_eq!(llama.name, "Llama 3");
        assert_eq!(llama.provider.as_deref(), Some("ollama"));
        assert_eq!(llama.context_window, Some(8192));
        assert_eq!(llama.max_tokens, Some(4096));
        assert_eq!(llama.input.as_deref(), Some(&["text".to_string()][..]));
        assert_eq!(llama.reasoning_efforts.as_deref(), Some(r#"{"max":"max"}"#));

        restore_env(DSH_HOME_ENV, previous);
    }

    #[test]
    fn editor_updates_deepseek_official_route() {
        let _guard = ENV_LOCK.lock().unwrap();
        let previous = std::env::var_os(DSH_HOME_ENV);
        let home = fixture_home();
        write_settings(&home);
        std::env::set_var(DSH_HOME_ENV, &home);

        // The deepseek-official route writes into the llm-deepseek section.
        SettingsEditor::upsert_provider(
            "web",
            &Provider {
                id: DEEPSEEK_PROVIDER.to_string(),
                name: "DeepSeek".to_string(),
                kind: "deepseek".to_string(),
                api: None,
                base_url: Some("https://api.deepseek.com/custom".to_string()),
                api_key_env: Some("NEW_KEY".to_string()),
                compat: None,
            },
        )
        .unwrap();
        let providers = list_providers("web").unwrap();
        let ds = providers
            .iter()
            .find(|p| p.id == DEEPSEEK_PROVIDER)
            .unwrap();
        assert_eq!(
            ds.base_url.as_deref(),
            Some("https://api.deepseek.com/custom")
        );
        assert_eq!(ds.api_key_env.as_deref(), Some("NEW_KEY"));

        // And removing it is unsupported.
        assert!(SettingsEditor::remove_provider("web", DEEPSEEK_PROVIDER).is_err());

        restore_env(DSH_HOME_ENV, previous);
    }

    #[test]
    fn editor_preserves_unmanaged_sections_and_compat() {
        let _guard = ENV_LOCK.lock().unwrap();
        let previous = std::env::var_os(DSH_HOME_ENV);
        let home = fixture_home();
        write_settings(&home);
        std::env::set_var(DSH_HOME_ENV, &home);

        // Editing one model must leave the pet section and the cdx compat
        // block intact.
        SettingsEditor::upsert_model(
            "web",
            "cdx",
            &Model {
                id: "gpt-5.6-sol".to_string(),
                name: "GPT 5.6 Sol (renamed)".to_string(),
                provider: Some("cdx".to_string()),
                context_window: None,
                max_tokens: None,
                input: None,
                reasoning_efforts: None,
                compat: None,
            },
        )
        .unwrap();

        let text = std::fs::read_to_string(home.join("settings.yaml")).unwrap();
        assert!(text.contains("pet:"));
        assert!(text.contains("visible: false"));
        assert!(text.contains("supportsStore: false"));
        assert!(text.contains("GPT 5.6 Sol (renamed)"));
        assert!(text.contains("ui-theme:"));

        restore_env(DSH_HOME_ENV, previous);
    }

    #[test]
    fn editor_removes_provider_and_model() {
        let _guard = ENV_LOCK.lock().unwrap();
        let previous = std::env::var_os(DSH_HOME_ENV);
        let home = fixture_home();
        write_settings(&home);
        std::env::set_var(DSH_HOME_ENV, &home);

        SettingsEditor::remove_provider("web", "cdx").unwrap();
        assert!(list_providers("web").unwrap().iter().all(|p| p.id != "cdx"));

        // Add a model under a fresh provider, then remove it.
        SettingsEditor::upsert_provider(
            "web",
            &Provider {
                id: "ollama".to_string(),
                name: "Ollama".to_string(),
                kind: "pi-ai".to_string(),
                api: None,
                base_url: None,
                api_key_env: None,
                compat: None,
            },
        )
        .unwrap();
        SettingsEditor::upsert_model(
            "web",
            "ollama",
            &Model {
                id: "llama-3".to_string(),
                name: "Llama 3".to_string(),
                provider: Some("ollama".to_string()),
                context_window: None,
                max_tokens: None,
                input: None,
                reasoning_efforts: None,
                compat: None,
            },
        )
        .unwrap();
        SettingsEditor::remove_model("web", "ollama", "llama-3").unwrap();
        assert!(list_models("web")
            .unwrap()
            .iter()
            .all(|m| m.id != "llama-3"));

        restore_env(DSH_HOME_ENV, previous);
    }

    #[test]
    fn editor_creates_settings_when_missing() {
        let _guard = ENV_LOCK.lock().unwrap();
        let previous = std::env::var_os(DSH_HOME_ENV);
        let home = fixture_home();
        std::fs::create_dir_all(&home).unwrap();
        std::env::set_var(DSH_HOME_ENV, &home);

        SettingsEditor::upsert_provider(
            "web",
            &Provider {
                id: "fresh".to_string(),
                name: "Fresh".to_string(),
                kind: "pi-ai".to_string(),
                api: None,
                base_url: None,
                api_key_env: None,
                compat: None,
            },
        )
        .unwrap();
        let providers = list_providers("web").unwrap();
        assert!(providers.iter().any(|p| p.id == "fresh"));

        restore_env(DSH_HOME_ENV, previous);
    }

    // The settings path is read back out of a document the user can edit;
    // `..` and absolute paths must not be honoured, or the advanced editor
    // would read and write files outside the harness home.
    #[test]
    fn a_settings_path_cannot_escape_the_harness_home() {
        let home = Path::new("/tmp/deepmate-home-test");
        // Inside the home is fine.
        assert_eq!(
            resolve_inside_home(home, "profiles/web/settings.yaml").unwrap(),
            home.join("profiles/web/settings.yaml")
        );
        for escape in [
            "../../../etc/passwd",
            "profiles/../../secrets.yaml",
            "./../outside.yaml",
        ] {
            assert!(
                resolve_inside_home(home, escape).is_err(),
                "path should be refused: {escape}"
            );
        }
        // Absolute paths have no platform-neutral spelling, so those cases
        // are built from the OS the test runs on rather than hard-coded. The
        // drive-relative form matters most: it is neither absolute nor a
        // `..` traversal, yet `Path::join` would replace the base with it.
        let (absolute, drive_relative) = if cfg!(windows) {
            ("C:\\Windows\\win.ini", Some("C:settings.yaml"))
        } else {
            ("/etc/passwd", None)
        };
        assert!(
            resolve_inside_home(home, absolute).is_err(),
            "an absolute path must be refused: {absolute}"
        );
        if let Some(drive_relative) = drive_relative {
            assert!(
                resolve_inside_home(home, drive_relative).is_err(),
                "a drive-relative path must be refused: {drive_relative}"
            );
        }
    }

    // cordis.patch.yml carries `!!js` expressions the YAML reader cannot
    // handle; they are neutralized for validation only, so a real syntax
    // error is still caught while the file keeps its expressions on disk.
    #[test]
    fn cordis_patch_validation_tolerates_js_tags_but_catches_errors() {
        let good = "- id: settings\n  name: '@deepseek-ai/dsh-settings-file'\n  config:\n    path: !!js dshHomePath('profiles/web/settings.yaml')\n";
        let neutralized = neutralize_js_tags(good);
        assert!(!neutralized.contains("!!js"), "the tag must be neutralized");
        assert!(yaml::from_str::<Value>(&neutralized).is_ok());

        // A genuine structural error is still rejected.
        let broken = "- id: settings\n  config:\n   path: [unclosed\n";
        let broken_neutralized = neutralize_js_tags(broken);
        assert!(yaml::from_str::<Value>(&broken_neutralized).is_err());
    }

    #[test]
    fn profile_create_and_remove_roundtrip() {
        let _guard = ENV_LOCK.lock().unwrap();
        let previous = std::env::var_os(DSH_HOME_ENV);
        let home = fixture_home();
        std::fs::create_dir_all(home.join("profiles")).unwrap();
        std::env::set_var(DSH_HOME_ENV, &home);

        create_profile("tui").unwrap();
        assert!(discover_profiles().unwrap().iter().any(|p| p.id == "tui"));
        // Duplicate create is rejected.
        assert!(create_profile("tui").is_err());
        // Invalid names are rejected.
        assert!(create_profile("a/b").is_err());
        assert!(create_profile("node_modules").is_err());

        remove_profile("tui").unwrap();
        assert!(discover_profiles().unwrap().iter().all(|p| p.id != "tui"));
        // Removing a missing profile is rejected.
        assert!(remove_profile("tui").is_err());

        // Removal is recoverable: the directory is moved into the trash, not
        // deleted, and the trash itself is never listed as a scenario.
        let trash = home.join("profiles").join(".deepmate-trash");
        let recovered: Vec<_> = std::fs::read_dir(&trash)
            .unwrap()
            .flatten()
            .filter(|entry| entry.file_name().to_string_lossy().starts_with("tui-"))
            .collect();
        assert_eq!(
            recovered.len(),
            1,
            "expected the removed profile in the trash"
        );
        assert!(discover_profiles()
            .unwrap()
            .iter()
            .all(|p| p.id != ".deepmate-trash"));

        restore_env(DSH_HOME_ENV, previous);
    }

    #[test]
    fn set_profile_description_writes_and_clears_the_manifest_field() {
        let _guard = ENV_LOCK.lock().unwrap();
        let previous = std::env::var_os(DSH_HOME_ENV);
        let home = fixture_home();
        std::fs::create_dir_all(home.join("profiles")).unwrap();
        write_profile(&home, "work", &[], &["@deepseek-ai/dsh-base"]);
        std::env::set_var(DSH_HOME_ENV, &home);

        set_profile_description("work", Some("日常打字".to_string())).unwrap();
        let profiles = discover_profiles().unwrap();
        let work = profiles.iter().find(|p| p.id == "work").unwrap();
        assert_eq!(work.description.as_deref(), Some("日常打字"));

        // Clearing the field restores the engine's bundles fallback.
        set_profile_description("work", Some(String::new())).unwrap();
        let profiles = discover_profiles().unwrap();
        let work = profiles.iter().find(|p| p.id == "work").unwrap();
        assert_eq!(
            work.description.as_deref(),
            Some("bundles: @deepseek-ai/dsh-base")
        );

        // A missing profile is rejected.
        assert!(set_profile_description("ghost", Some("x".to_string())).is_err());
        assert!(set_profile_description("a/b", Some("x".to_string())).is_err());

        restore_env(DSH_HOME_ENV, previous);
    }

    #[test]
    fn advanced_files_roundtrip_and_validate() {
        let _guard = ENV_LOCK.lock().unwrap();
        let previous = std::env::var_os(DSH_HOME_ENV);
        let home = fixture_home();
        std::fs::create_dir_all(home.join("profiles")).unwrap();
        std::env::set_var(DSH_HOME_ENV, &home);

        // Scene settings: save, read back, refuse invalid YAML.
        let scene = AdvancedFileScope::SceneSettings;
        assert_eq!(read_advanced_file(scene, Some("work")).unwrap(), None);
        save_advanced_file(
            scene,
            Some("work"),
            "llm-deepseek:\n  baseURL: https://api.example.com\n",
        )
        .unwrap();
        assert_eq!(
            read_advanced_file(scene, Some("work")).unwrap().as_deref(),
            Some("llm-deepseek:\n  baseURL: https://api.example.com\n")
        );
        assert!(save_advanced_file(scene, Some("work"), "x: { a: 1").is_err());

        // The global settings document is reachable without a scenario name.
        let global = AdvancedFileScope::GlobalSettings;
        save_advanced_file(global, None, "agent-default-model: deepseek-v4-flash\n").unwrap();
        assert_eq!(
            read_advanced_file(global, None).unwrap().as_deref(),
            Some("agent-default-model: deepseek-v4-flash\n")
        );

        // cordis.patch.yml is exempt from YAML validation: its custom
        // `dshHomePath` tag defeats the YAML parser.
        let cordis = AdvancedFileScope::SceneCordis;
        save_advanced_file(
            cordis,
            Some("work"),
            "- id: settings\n  name: '@deepseek-ai/dsh-settings-file'\n  config:\n    path: !!js dshHomePath('profiles/work/settings.yaml')\n",
        )
        .unwrap();
        assert!(read_advanced_file(cordis, Some("work")).unwrap().is_some());

        // Names that could escape the profiles directory are refused.
        for bad in ["../evil", "a/b", "node_modules", ""] {
            assert!(
                save_advanced_file(scene, Some(bad), "x: 1").is_err(),
                "{bad:?}"
            );
            assert!(read_advanced_file(scene, Some(bad)).is_err(), "{bad:?}");
        }

        restore_env(DSH_HOME_ENV, previous);
    }
}
