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

// List the plugins of one profile: its declared dependencies, with `enabled`
// reflecting whether the package is actually installed in the profile.
pub fn list_plugins(profile_id: &str) -> CoreResult<Vec<Plugin>> {
    let Some(home) = dsh_home() else {
        return Ok(Vec::new());
    };
    let dir = home.join("profiles").join(profile_id);
    let manifest = read_manifest(&dir)?;
    let mut plugins = Vec::new();
    for (name, version) in manifest.dependencies.unwrap_or_default() {
        let installed = dir.join("node_modules").join(&name).is_dir();
        plugins.push(Plugin {
            id: name.clone(),
            name,
            version: Some(version),
            enabled: installed,
        });
    }
    plugins.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(plugins)
}

// List plugins across all profiles, with profile-qualified ids.
pub fn list_all_plugins() -> CoreResult<Vec<Plugin>> {
    let mut plugins = Vec::new();
    for profile in discover_profiles()? {
        for plugin in list_plugins(&profile.id)? {
            plugins.push(Plugin {
                id: format!("{}/{}", profile.id, plugin.id),
                ..plugin
            });
        }
    }
    plugins.sort_by(|a, b| a.id.cmp(&b.id));
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
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct PiAiProviderProfile {
    display_name: Option<String>,
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

// List providers: the always-composed deepseek route plus every pi-ai
// provider profile supplied by the settings document.
pub fn list_providers() -> CoreResult<Vec<Provider>> {
    let settings = read_settings()?;
    let mut providers = vec![Provider {
        id: DEEPSEEK_PROVIDER.to_string(),
        name: "DeepSeek".to_string(),
        kind: "deepseek".to_string(),
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
                models.push(Model {
                    id: model.id.clone(),
                    name: model.name.clone().unwrap_or_else(|| model.id.clone()),
                    provider: Some(DEEPSEEK_PROVIDER.to_string()),
                });
            }
        }
        None => {
            for (id, name) in DEFAULT_DEEPSEEK_MODELS {
                models.push(Model {
                    id: id.to_string(),
                    name: name.to_string(),
                    provider: Some(DEEPSEEK_PROVIDER.to_string()),
                });
            }
        }
    }
    if let Some(pi_ai) = &settings.llm_pi_ai {
        if let Some(routes) = &pi_ai.providers {
            for (route, profile) in routes {
                if let Some(catalog) = &profile.models {
                    for model in catalog {
                        models.push(Model {
                            id: model.id.clone(),
                            name: model.name.clone().unwrap_or_else(|| model.id.clone()),
                            provider: Some(route.clone()),
                        });
                    }
                }
            }
        }
    }
    models.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(models)
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
    fn list_plugins_reads_dependencies_and_install_state() {
        let _guard = ENV_LOCK.lock().unwrap();
        let previous = std::env::var_os(DSH_HOME_ENV);
        let home = fixture_home();
        write_profile(
            &home,
            "web",
            &[("turtle-ui", "^1.0.0"), ("plain-lib", "2.0.0")],
            &["@deepseek-ai/dsh-base"],
        );
        // turtle-ui is installed; plain-lib is declared but not installed.
        std::fs::create_dir_all(home.join("profiles/web/node_modules/turtle-ui")).unwrap();
        std::env::set_var(DSH_HOME_ENV, &home);
        let plugins = list_plugins("web").unwrap();
        assert_eq!(plugins.len(), 2);
        assert_eq!(plugins[0].id, "plain-lib");
        assert!(!plugins[0].enabled);
        assert_eq!(plugins[1].id, "turtle-ui");
        assert!(plugins[1].enabled);
        assert_eq!(plugins[1].version.as_deref(), Some("^1.0.0"));
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
}
