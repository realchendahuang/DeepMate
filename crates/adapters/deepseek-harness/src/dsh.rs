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
use deepmate_core::model::{Model, Plugin, Profile, Provider};
use serde_yml::{Mapping, Sequence, Value};

const DSH_HOME_ENV: &str = "DSH_HOME";
const DSH_HOME_DIR_NAME: &str = ".dsh";

// The `dsh.profile` section of a profile's package.json.
#[derive(Debug, Clone, Default, serde::Deserialize)]
struct ProfileManifest {
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
    let text = std::fs::read_to_string(&path)?;
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
        if name == "node_modules" {
            continue;
        }
        let dir = entry.path();
        if !dir.join("package.json").is_file() {
            continue;
        }
        // A corrupt manifest must not hide the profile; the description is
        // best-effort while plugin listing below fails loudly.
        let description = read_manifest(&dir).ok().and_then(|manifest| {
            manifest
                .dsh
                .and_then(|dsh| dsh.profile)
                .and_then(|profile| profile.bundles)
                .map(|bundles| format!("bundles: {}", bundles.join(", ")))
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
    if let Some(bundles) = manifest
        .dsh
        .and_then(|dsh| dsh.profile)
        .and_then(|profile| profile.bundles)
    {
        for bundle in bundles {
            plugins.push(plugin_for(&dir, &shared, profile_id, &bundle, None));
        }
    }
    for (name, version) in manifest.dependencies.unwrap_or_default() {
        plugins.push(plugin_for(&dir, &shared, profile_id, &name, Some(version)));
    }
    plugins.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(plugins)
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

// The single provider route the llm-deepseek plugin owns; the base bundle
// composes it into every profile.
const DEEPSEEK_PROVIDER: &str = "deepseek-official";

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
    reasoning_efforts: Option<serde_yml::Value>,
    #[serde(default)]
    compat: Option<serde_yml::Value>,
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
    compat: Option<serde_yml::Value>,
    models: Option<Vec<CatalogModel>>,
}

fn settings_path() -> Option<PathBuf> {
    dsh_home().map(|home| home.join("settings.yaml"))
}

fn read_settings() -> CoreResult<SettingsDocument> {
    let Some(path) = settings_path() else {
        return Ok(SettingsDocument::default());
    };
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Ok(SettingsDocument::default())
        }
        Err(err) => return Err(err.into()),
    };
    serde_yml::from_str(&text).map_err(|err| {
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

// Convert a `serde_yml::Value` to its JSON string form. `serde_yml::Value`
// implements `Serialize`, so this round-trips opaque blocks losslessly.
fn yaml_to_json(value: &Value) -> Option<String> {
    match value {
        Value::Null => None,
        _ => serde_json::to_string(value).ok(),
    }
}

// List providers: the always-composed deepseek route plus every pi-ai
// provider profile supplied by the settings document.
pub fn list_providers() -> CoreResult<Vec<Provider>> {
    let settings = read_settings()?;
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

// List models: the deepseek catalog (settings override or shipped defaults)
// plus every model of every pi-ai provider profile.
pub fn list_models() -> CoreResult<Vec<Model>> {
    let settings = read_settings()?;
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
// The document is loaded as an order-preserving `serde_yml::Value` tree
// (backed by an insertion-ordered map), only the `llm-pi-ai.providers` and
// `llm-deepseek.models` subtrees are mutated, and the whole document is
// written back. Unmanaged sections (`pet`, `ui-theme`, `llm-deepseek` scalar
// settings, other providers' opaque blocks) survive untouched.
pub struct SettingsEditor;

impl SettingsEditor {
    // Load the settings document as a mutable value tree. A missing file
    // yields an empty document that will be created on first save.
    fn load() -> CoreResult<Value> {
        let Some(path) = settings_path() else {
            return Ok(Value::Mapping(Mapping::new()));
        };
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                return Ok(Value::Mapping(Mapping::new()))
            }
            Err(err) => return Err(err.into()),
        };
        serde_yml::from_str::<Value>(&text).map_err(|err| {
            CoreError::InvalidState(format!("invalid settings {}: {err}", path.display()))
        })
    }

    // Write the document back, backing up the previous file first. Creating
    // parent directories and the backup are both best-effort; a read-only
    // home must not silently claim success, so the write itself is checked.
    fn save(doc: &Value) -> CoreResult<()> {
        let Some(path) = settings_path() else {
            return Ok(());
        };
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        if path.is_file() {
            let backup = PathBuf::from(format!("{}{}", path.display(), SETTINGS_BACKUP_SUFFIX));
            let _ = std::fs::copy(&path, &backup);
        }
        let text = serde_yml::to_string(doc).map_err(|err| {
            CoreError::InvalidState(format!("failed to serialize settings: {err}"))
        })?;
        std::fs::write(&path, text)?;
        Ok(())
    }

    // Run a mutation against the settings document and persist the result.
    fn mutate(mutate: impl FnOnce(&mut Value) -> CoreResult<()>) -> CoreResult<()> {
        let mut doc = Self::load()?;
        mutate(&mut doc)?;
        Self::save(&doc)
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
    pub fn upsert_provider(provider: &Provider) -> CoreResult<()> {
        let provider = provider.clone();
        Self::mutate(move |doc| {
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
    pub fn remove_provider(id: &str) -> CoreResult<()> {
        if id == DEEPSEEK_PROVIDER {
            return Err(CoreError::Unsupported(
                "the deepseek-official route is composed into every profile and cannot be removed"
                    .to_string(),
            ));
        }
        Self::mutate(|doc| {
            Self::providers_map(doc)?.remove(id);
            Ok(())
        })
    }

    // Create or update a model within a provider route's catalog. The route is
    // looked up the same way providers are, so `deepseek-official` targets the
    // `llm-deepseek.models` list and other routes target `llm-pi-ai.providers`.
    pub fn upsert_model(provider_id: &str, model: &Model) -> CoreResult<()> {
        let model = model.clone();
        Self::mutate(move |doc| {
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
    pub fn remove_model(provider_id: &str, id: &str) -> CoreResult<()> {
        let provider_id = provider_id.to_string();
        let id = id.to_string();
        Self::mutate(move |doc| {
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

// Convert a raw JSON string back to a `serde_yml::Value`, validating it.
fn json_to_yaml(raw: &Option<String>) -> CoreResult<Option<Value>> {
    let Some(text) = raw else {
        return Ok(None);
    };
    if text.trim().is_empty() {
        return Ok(None);
    }
    let value: serde_json::Value = serde_json::from_str(text)
        .map_err(|err| CoreError::InvalidState(format!("invalid JSON capability block: {err}")))?;
    let yaml: Value = serde_yml::to_value(value)
        .map_err(|err| CoreError::InvalidState(format!("invalid capability block: {err}")))?;
    Ok(Some(yaml))
}

// Build the `serde_yml::Value` for one model entry.
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
    if !dir.join("package.json").is_file() {
        return Err(CoreError::InvalidState(format!(
            "profile not found: {name}"
        )));
    }
    std::fs::remove_dir_all(&dir)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    // Env-var tests must not race each other.
    static ENV_LOCK: Mutex<()> = Mutex::new(());
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
        let providers = list_providers().unwrap();
        let ids: Vec<&str> = providers.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, ["anthropic", "deepseek-official", "openai"]);
        assert_eq!(providers[2].name, "OpenAI");
        let models = list_models().unwrap();
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
        let providers = list_providers().unwrap();
        assert_eq!(providers.len(), 1);
        assert_eq!(providers[0].id, "deepseek-official");
        let models = list_models().unwrap();
        let ids: Vec<&str> = models.iter().map(|m| m.id.as_str()).collect();
        assert_eq!(ids, ["deepseek-v4-flash", "deepseek-v4-pro"]);
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
        assert!(list_providers().is_err());
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
        SettingsEditor::upsert_provider(&Provider {
            id: "ollama".to_string(),
            name: "Ollama".to_string(),
            kind: "pi-ai".to_string(),
            api: Some("openai-completions".to_string()),
            base_url: Some("http://localhost:11434/v1".to_string()),
            api_key_env: Some("OLLAMA_KEY".to_string()),
            compat: None,
        })
        .unwrap();
        SettingsEditor::upsert_model(
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

        let providers = list_providers().unwrap();
        let ollama = providers.iter().find(|p| p.id == "ollama").unwrap();
        assert_eq!(ollama.name, "Ollama");
        assert_eq!(ollama.api.as_deref(), Some("openai-completions"));
        assert_eq!(
            ollama.base_url.as_deref(),
            Some("http://localhost:11434/v1")
        );
        assert_eq!(ollama.api_key_env.as_deref(), Some("OLLAMA_KEY"));

        let models = list_models().unwrap();
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
        SettingsEditor::upsert_provider(&Provider {
            id: DEEPSEEK_PROVIDER.to_string(),
            name: "DeepSeek".to_string(),
            kind: "deepseek".to_string(),
            api: None,
            base_url: Some("https://api.deepseek.com/custom".to_string()),
            api_key_env: Some("NEW_KEY".to_string()),
            compat: None,
        })
        .unwrap();
        let providers = list_providers().unwrap();
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
        assert!(SettingsEditor::remove_provider(DEEPSEEK_PROVIDER).is_err());

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

        SettingsEditor::remove_provider("cdx").unwrap();
        assert!(list_providers().unwrap().iter().all(|p| p.id != "cdx"));

        // Add a model under a fresh provider, then remove it.
        SettingsEditor::upsert_provider(&Provider {
            id: "ollama".to_string(),
            name: "Ollama".to_string(),
            kind: "pi-ai".to_string(),
            api: None,
            base_url: None,
            api_key_env: None,
            compat: None,
        })
        .unwrap();
        SettingsEditor::upsert_model(
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
        SettingsEditor::remove_model("ollama", "llama-3").unwrap();
        assert!(list_models().unwrap().iter().all(|m| m.id != "llama-3"));

        restore_env(DSH_HOME_ENV, previous);
    }

    #[test]
    fn editor_creates_settings_when_missing() {
        let _guard = ENV_LOCK.lock().unwrap();
        let previous = std::env::var_os(DSH_HOME_ENV);
        let home = fixture_home();
        std::fs::create_dir_all(&home).unwrap();
        std::env::set_var(DSH_HOME_ENV, &home);

        SettingsEditor::upsert_provider(&Provider {
            id: "fresh".to_string(),
            name: "Fresh".to_string(),
            kind: "pi-ai".to_string(),
            api: None,
            base_url: None,
            api_key_env: None,
            compat: None,
        })
        .unwrap();
        let providers = list_providers().unwrap();
        assert!(providers.iter().any(|p| p.id == "fresh"));

        restore_env(DSH_HOME_ENV, previous);
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

        restore_env(DSH_HOME_ENV, previous);
    }
}
