// The Pi Agent filesystem contract (`pi`).
//
// Pi Agent is an AI coding assistant CLI. Its data lives under `~/.pi/agent`:
// - `models.json` — providers (`name`, `baseUrl`, `api`, `apiKey`, `models[]`)
//   and their model catalogs (`contextWindow`, `maxTokens`, `thinkingLevelMap`,
//   `compat`, `input`).
// - `settings.json` — `packages[]` (installed extensions), `defaultProvider`,
//   `defaultModel`, `enabledModels`.
//
// Only the stable, documented file contracts are read here. The `apiKey` field
// is deliberately never read: DeepMate must not carry plaintext secrets into
// its own models, history, snapshots or logs (architectural rule 7).

use std::collections::BTreeMap;
use std::path::PathBuf;

use deepmate_core::error::{CoreError, CoreResult};
use deepmate_core::model::{Model, Plugin, Provider};

const PI_HOME_ENV: &str = "PI_HOME";
const PI_HOME_DIR_NAME: &str = ".pi";

#[cfg(target_os = "windows")]
fn os_home() -> Option<PathBuf> {
    std::env::var_os("USERPROFILE").map(PathBuf::from)
}

#[cfg(not(target_os = "windows"))]
fn os_home() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

// Resolve the Pi Agent home: `$PI_HOME` (non-empty) or `~/.pi`.
pub fn pi_home() -> Option<PathBuf> {
    if let Some(home) = std::env::var_os(PI_HOME_ENV) {
        let home = PathBuf::from(home);
        if !home.as_os_str().is_empty() {
            return Some(home);
        }
    }
    os_home().map(|home| home.join(PI_HOME_DIR_NAME))
}

fn agent_dir() -> Option<PathBuf> {
    pi_home().map(|home| home.join("agent"))
}

fn models_path() -> Option<PathBuf> {
    agent_dir().map(|dir| dir.join("models.json"))
}

fn settings_path() -> Option<PathBuf> {
    agent_dir().map(|dir| dir.join("settings.json"))
}

// The `models.json` document. `apiKey` is intentionally absent from the
// struct so it can never be deserialized into a DeepMate model.
#[derive(Debug, Clone, Default, serde::Deserialize)]
struct ModelsDocument {
    providers: Option<BTreeMap<String, PiProvider>>,
}

#[derive(Debug, Clone, Default, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct PiProvider {
    name: Option<String>,
    base_url: Option<String>,
    api: Option<String>,
    models: Option<Vec<PiModel>>,
}

#[derive(Debug, Clone, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct PiModel {
    id: String,
    name: Option<String>,
    context_window: Option<u64>,
    max_tokens: Option<u64>,
    input: Option<Vec<String>>,
    thinking_level_map: Option<serde_json::Value>,
    compat: Option<serde_json::Value>,
}

// The `settings.json` document. Only the extension list is read.
#[derive(Debug, Clone, Default, serde::Deserialize)]
struct SettingsDocument {
    packages: Option<Vec<String>>,
}

fn read_models() -> CoreResult<ModelsDocument> {
    let Some(path) = models_path() else {
        return Ok(ModelsDocument::default());
    };
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            return Ok(ModelsDocument::default())
        }
        Err(err) => return Err(err.into()),
    };
    serde_json::from_str(&text)
        .map_err(|err| CoreError::InvalidState(format!("invalid models {}: {err}", path.display())))
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
    serde_json::from_str(&text).map_err(|err| {
        CoreError::InvalidState(format!("invalid settings {}: {err}", path.display()))
    })
}

fn json_to_string(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::Null => None,
        _ => serde_json::to_string(value).ok(),
    }
}

// List providers from `models.json`. The `apiKey` field is never surfaced;
// `api_key_env` stays None because Pi stores the secret inline rather than in
// an environment variable.
pub fn list_providers() -> CoreResult<Vec<Provider>> {
    let doc = read_models()?;
    let mut providers = Vec::new();
    if let Some(routes) = doc.providers {
        for (route, profile) in routes {
            providers.push(Provider {
                id: route.clone(),
                name: profile.name.clone().unwrap_or_else(|| route.clone()),
                kind: "pi".to_string(),
                api: profile.api.clone(),
                base_url: profile.base_url.clone(),
                api_key_env: None,
                compat: None,
            });
        }
    }
    providers.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(providers)
}

// List models from `models.json`, attaching each to its provider route.
pub fn list_models() -> CoreResult<Vec<Model>> {
    let doc = read_models()?;
    let mut models = Vec::new();
    if let Some(routes) = doc.providers {
        for (route, profile) in routes {
            if let Some(catalog) = profile.models {
                for model in catalog {
                    models.push(Model {
                        id: model.id.clone(),
                        name: model.name.clone().unwrap_or_else(|| model.id.clone()),
                        provider: Some(route.clone()),
                        context_window: model.context_window,
                        max_tokens: model.max_tokens,
                        input: model.input.clone(),
                        reasoning_efforts: model
                            .thinking_level_map
                            .as_ref()
                            .and_then(json_to_string),
                        compat: model.compat.as_ref().and_then(json_to_string),
                    });
                }
            }
        }
    }
    models.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(models)
}

// List installed extensions from `settings.json` as plugins. Pi extensions
// have no per-extension version or enable flag exposed in settings.json, so
// those fields stay empty/false.
pub fn list_plugins() -> CoreResult<Vec<Plugin>> {
    let doc = read_settings()?;
    let mut plugins = Vec::new();
    for package in doc.packages.unwrap_or_default() {
        plugins.push(Plugin {
            id: package.clone(),
            name: package.clone(),
            version: None,
            enabled: true,
            profile: String::new(),
            latest: None,
            outdated: false,
        });
    }
    plugins.sort_by(|a, b| a.id.cmp(&b.id));
    Ok(plugins)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());
    static FIXTURE_SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

    fn fixture_home() -> PathBuf {
        let n = FIXTURE_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        std::env::temp_dir().join(format!("deepmate-pi-test-{}-{n}", std::process::id()))
    }

    fn restore_env(key: &str, previous: Option<std::ffi::OsString>) {
        match previous {
            Some(value) => std::env::set_var(key, value),
            None => std::env::remove_var(key),
        }
    }

    fn write_models(home: &Path) {
        let agent = home.join("agent");
        std::fs::create_dir_all(&agent).unwrap();
        std::fs::write(
            agent.join("models.json"),
            r#"{
  "providers": {
    "ds": {
      "name": "DeepSeek Official",
      "baseUrl": "https://api.deepseek.com",
      "api": "openai-responses",
      "apiKey": "sk-secret-should-never-leak",
      "models": [
        { "id": "deepseek-v4-flash", "name": "DeepSeek V4 Flash", "contextWindow": 1000000, "maxTokens": 131072 }
      ]
    },
    "oc": {
      "name": "Ollama Cloud",
      "baseUrl": "https://ollama.com/v1",
      "api": "openai-responses",
      "apiKey": "another-secret",
      "models": [
        {
          "id": "deepseek-v4-flash:cloud",
          "name": "DeepSeek V4 Flash Cloud",
          "contextWindow": 786432,
          "maxTokens": 65536,
          "input": ["text", "image"],
          "thinkingLevelMap": { "max": "max" },
          "compat": { "supportsDeveloperRole": false }
        }
      ]
    }
  }
}"#,
        )
        .unwrap();
    }

    fn write_settings(home: &Path) {
        let agent = home.join("agent");
        std::fs::create_dir_all(&agent).unwrap();
        std::fs::write(
            agent.join("settings.json"),
            r#"{
  "defaultProvider": "ds",
  "defaultModel": "deepseek-v4-flash",
  "packages": ["npm:pi-subagents", "npm:pi-mcp-adapter"]
}"#,
        )
        .unwrap();
    }

    #[test]
    fn pi_home_honors_env_override() {
        let _guard = ENV_LOCK.lock().unwrap();
        let previous = std::env::var_os(PI_HOME_ENV);
        let home = fixture_home();
        std::env::set_var(PI_HOME_ENV, &home);
        assert_eq!(pi_home(), Some(home));
        restore_env(PI_HOME_ENV, previous);
    }

    #[test]
    fn providers_strip_api_key() {
        let _guard = ENV_LOCK.lock().unwrap();
        let previous = std::env::var_os(PI_HOME_ENV);
        let home = fixture_home();
        write_models(&home);
        std::env::set_var(PI_HOME_ENV, &home);

        let providers = list_providers().unwrap();
        let ids: Vec<&str> = providers.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, ["ds", "oc"]);
        // The secret must never surface anywhere in the normalized model.
        for provider in &providers {
            assert_eq!(provider.api_key_env, None);
            assert!(!format!("{provider:?}").contains("secret"));
        }
        assert_eq!(
            providers[0].base_url.as_deref(),
            Some("https://api.deepseek.com")
        );

        restore_env(PI_HOME_ENV, previous);
    }

    #[test]
    fn models_read_capability_fields() {
        let _guard = ENV_LOCK.lock().unwrap();
        let previous = std::env::var_os(PI_HOME_ENV);
        let home = fixture_home();
        write_models(&home);
        std::env::set_var(PI_HOME_ENV, &home);

        let models = list_models().unwrap();
        let cloud = models
            .iter()
            .find(|m| m.id == "deepseek-v4-flash:cloud")
            .unwrap();
        assert_eq!(cloud.provider.as_deref(), Some("oc"));
        assert_eq!(cloud.context_window, Some(786432));
        assert_eq!(
            cloud.input.as_deref(),
            Some(&["text".to_string(), "image".to_string()][..])
        );
        assert_eq!(cloud.reasoning_efforts.as_deref(), Some(r#"{"max":"max"}"#));
        assert_eq!(
            cloud.compat.as_deref(),
            Some(r#"{"supportsDeveloperRole":false}"#)
        );

        restore_env(PI_HOME_ENV, previous);
    }

    #[test]
    fn plugins_read_settings_packages() {
        let _guard = ENV_LOCK.lock().unwrap();
        let previous = std::env::var_os(PI_HOME_ENV);
        let home = fixture_home();
        write_settings(&home);
        std::env::set_var(PI_HOME_ENV, &home);

        let plugins = list_plugins().unwrap();
        let ids: Vec<&str> = plugins.iter().map(|p| p.id.as_str()).collect();
        assert_eq!(ids, ["npm:pi-mcp-adapter", "npm:pi-subagents"]);

        restore_env(PI_HOME_ENV, previous);
    }

    #[test]
    fn empty_without_home() {
        let _guard = ENV_LOCK.lock().unwrap();
        let previous = std::env::var_os(PI_HOME_ENV);
        let previous_home = std::env::var_os("HOME");
        std::env::remove_var(PI_HOME_ENV);
        std::env::set_var("HOME", fixture_home());
        assert!(list_providers().unwrap().is_empty());
        assert!(list_models().unwrap().is_empty());
        assert!(list_plugins().unwrap().is_empty());
        restore_env("HOME", previous_home);
        restore_env(PI_HOME_ENV, previous);
    }
}
