// DeepMate-owned plugin state: the disabled-plugin registry.
//
// Harness plugins are npm dependencies of a profile, and the harness only
// loads declared, installed dependencies — so "disable" is a real uninstall
// plus a DeepMate-side record of the package spec, and "enable" reinstalls
// that spec. The registry lives under the DeepMate data directory
// (`state/disabled-plugins.json`), never in the harness home, so it survives
// uninstalls and stays out of the harness's own file contract.

use std::path::{Path, PathBuf};

use deepmate_core::error::{CoreError, CoreResult};
use deepmate_core::model::DisabledPlugin;

const STATE_DIR: &str = "state";
const FILENAME: &str = "disabled-plugins.json";
const SCHEMA: u32 = 1;

// The disabled-plugin registry. A missing file is an empty registry; a newer
// schema is refused rather than misread.
#[derive(Debug, Clone, Default)]
pub struct DisabledRegistry {
    path: Option<PathBuf>,
}

impl DisabledRegistry {
    pub fn new(data_root: Option<PathBuf>) -> Self {
        Self {
            path: data_root.map(|root| root.join(STATE_DIR).join(FILENAME)),
        }
    }

    // All disabled-plugin records, sorted by (profile, id).
    pub fn list(&self) -> CoreResult<Vec<DisabledPlugin>> {
        let Some(path) = &self.path else {
            return Ok(Vec::new());
        };
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(err) => return Err(err.into()),
        };
        let file: RegistryFile = serde_json::from_str(&text).map_err(|err| {
            CoreError::InvalidState(format!(
                "invalid disabled-plugin registry {}: {err}",
                path.display()
            ))
        })?;
        if file.schema > SCHEMA {
            return Err(CoreError::InvalidState(format!(
                "disabled-plugin registry schema {} is newer than this build supports ({SCHEMA})",
                file.schema
            )));
        }
        let mut plugins = file.plugins;
        plugins.sort_by(|a, b| (&a.profile, &a.id).cmp(&(&b.profile, &b.id)));
        Ok(plugins)
    }

    // The recorded spec of one plugin, for enabling it again.
    pub fn spec(&self, profile: &str, id: &str) -> CoreResult<Option<String>> {
        Ok(self
            .list()?
            .into_iter()
            .find(|plugin| plugin.profile == profile && plugin.id == id)
            .map(|plugin| plugin.spec))
    }

    // Record a disabled plugin, replacing any existing record for the same
    // profile + id.
    pub fn record(&self, profile: &str, id: &str, spec: &str) -> CoreResult<()> {
        let Some(path) = &self.path else {
            return Err(CoreError::InvalidState(
                "no data directory; cannot record disabled plugins".to_string(),
            ));
        };
        let mut plugins = self.list()?;
        if let Some(existing) = plugins
            .iter_mut()
            .find(|plugin| plugin.profile == profile && plugin.id == id)
        {
            existing.spec = spec.to_string();
        } else {
            plugins.push(DisabledPlugin {
                profile: profile.to_string(),
                id: id.to_string(),
                spec: spec.to_string(),
            });
        }
        self.write(path, &plugins)
    }

    // Drop the record for one plugin (re-enabled or removed for good).
    pub fn remove(&self, profile: &str, id: &str) -> CoreResult<()> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        let plugins = self.list()?;
        let before = plugins.len();
        let kept: Vec<DisabledPlugin> = plugins
            .into_iter()
            .filter(|plugin| plugin.profile != profile || plugin.id != id)
            .collect();
        if kept.len() == before {
            return Ok(());
        }
        self.write(path, &kept)
    }

    // Rewrite every record's profile key (a profile rename), so disabled
    // plugins stay restorable under the new profile name.
    pub fn rename(&self, old: &str, new: &str) -> CoreResult<()> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        if old == new {
            return Ok(());
        }
        let plugins = self.list()?;
        if !plugins.iter().any(|plugin| plugin.profile == old) {
            return Ok(());
        }
        let mut renamed: Vec<DisabledPlugin> = plugins
            .into_iter()
            .map(|mut plugin| {
                if plugin.profile == old {
                    plugin.profile = new.to_string();
                }
                plugin
            })
            .collect();
        renamed.sort_by(|a, b| (&a.profile, &a.id).cmp(&(&b.profile, &b.id)));
        self.write(path, &renamed)
    }

    fn write(&self, path: &Path, plugins: &[DisabledPlugin]) -> CoreResult<()> {
        let file = RegistryFile {
            schema: SCHEMA,
            plugins: plugins.to_vec(),
        };
        let text = serde_json::to_string_pretty(&file).map_err(|err| {
            CoreError::InvalidState(format!(
                "failed to serialize disabled-plugin registry: {err}"
            ))
        })?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, text)?;
        Ok(())
    }
}

#[derive(Debug, serde::Serialize, serde::Deserialize)]
struct RegistryFile {
    schema: u32,
    plugins: Vec<DisabledPlugin>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_registry() -> (DisabledRegistry, PathBuf) {
        let dir = std::env::temp_dir().join(format!(
            "deepmate-registry-test-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let registry = DisabledRegistry::new(Some(dir.clone()));
        (registry, dir)
    }

    #[test]
    fn missing_file_is_an_empty_registry() {
        let (registry, dir) = temp_registry();
        assert!(registry.list().unwrap().is_empty());
        assert_eq!(registry.spec("web", "dsh-mnemon").unwrap(), None);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn no_data_root_is_an_empty_registry() {
        let registry = DisabledRegistry::new(None);
        assert!(registry.list().unwrap().is_empty());
        assert!(registry.record("web", "dsh-mnemon", "^0.2").is_err());
    }

    #[test]
    fn record_list_and_spec_roundtrip() {
        let (registry, dir) = temp_registry();
        registry.record("web", "dsh-mnemon", "^0.2").unwrap();
        registry.record("headless", "dsh-lens", "^0.1").unwrap();
        let plugins = registry.list().unwrap();
        assert_eq!(plugins.len(), 2);
        assert_eq!(plugins[0].profile, "headless");
        assert_eq!(plugins[1].profile, "web");
        assert_eq!(
            registry.spec("web", "dsh-mnemon").unwrap().as_deref(),
            Some("^0.2")
        );
        assert_eq!(registry.spec("web", "absent").unwrap(), None);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn record_replaces_the_spec_for_the_same_plugin() {
        let (registry, dir) = temp_registry();
        registry.record("web", "dsh-mnemon", "^0.2").unwrap();
        registry.record("web", "dsh-mnemon", "^0.3").unwrap();
        let plugins = registry.list().unwrap();
        assert_eq!(plugins.len(), 1);
        assert_eq!(plugins[0].spec, "^0.3");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn remove_clears_only_the_matching_record() {
        let (registry, dir) = temp_registry();
        registry.record("web", "a", "^0.1").unwrap();
        registry.record("web", "b", "^0.1").unwrap();
        registry.remove("web", "a").unwrap();
        let plugins = registry.list().unwrap();
        assert_eq!(plugins.len(), 1);
        assert_eq!(plugins[0].id, "b");
        registry.remove("web", "a").unwrap();
        assert_eq!(registry.list().unwrap().len(), 1);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn rename_rewrites_only_the_matching_profile_key() {
        let (registry, dir) = temp_registry();
        registry.record("web", "a", "^0.1").unwrap();
        registry.record("daily", "b", "^0.2").unwrap();
        registry.rename("daily", "dev").unwrap();
        let plugins = registry.list().unwrap();
        assert_eq!(plugins.len(), 2);
        assert_eq!(plugins[0].profile, "dev");
        assert_eq!(plugins[0].id, "b");
        assert_eq!(plugins[0].spec, "^0.2");
        assert_eq!(plugins[1].profile, "web");
        // Renaming to the same name, or a name with no records, is a no-op.
        registry.rename("web", "web").unwrap();
        registry.rename("absent", "x").unwrap();
        assert_eq!(registry.list().unwrap().len(), 2);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn newer_schema_is_refused() {
        let (registry, dir) = temp_registry();
        let path = dir.join("state").join(FILENAME);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, r#"{ "schema": 2, "plugins": [] }"#).unwrap();
        assert!(registry.list().is_err());
        std::fs::remove_dir_all(&dir).ok();
    }
}
