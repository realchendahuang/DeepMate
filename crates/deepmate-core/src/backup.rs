// Portable backups of DeepMate's own configuration.
//
// Snapshots (see `snapshot.rs`) carry the harness inventory; a config backup
// carries DeepMate's own settings — language, theme, tray behavior, update
// preferences and market defaults — so a personal setup can move between
// machines as one file.
//
// The document never contains secrets: DeepMate's configuration holds only
// preference values (an api_key_env is an environment-variable *name* owned
// by the harness config, which never appears here). Export always writes the
// complete document, and import replaces the active configuration with it —
// there is no partial merge, so what you import is exactly what you exported.

use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::data::Config;
use crate::error::{CoreError, CoreResult};

// The backup format version. Bumped when the document shape changes.
const CONFIG_FORMAT: &str = "deepmate-config/1";

// The prefix shared by every backup format generation.
const CONFIG_FORMAT_PREFIX: &str = "deepmate-config";

// A portable, self-describing copy of DeepMate's own configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConfigBackup {
    pub format: String,
    pub created: String,
    pub app_version: String,
    pub config: Config,
}

impl ConfigBackup {
    // Capture the given configuration into a backup document.
    pub fn capture(config: &Config) -> Self {
        Self {
            format: CONFIG_FORMAT.to_string(),
            created: chrono::Utc::now().to_rfc3339(),
            app_version: env!("CARGO_PKG_VERSION").to_string(),
            config: config.clone(),
        }
    }

    // Write the backup document as pretty JSON to `path`.
    pub fn save(&self, path: &Path) -> CoreResult<()> {
        let text = serde_json::to_string_pretty(self)
            .map_err(|err| CoreError::InvalidState(format!("failed to serialize backup: {err}")))?;
        crate::fsutil::write_atomic_string(path, &text)
    }

    // Load a backup document from `path`.
    //
    // The document's `format` must be one this build understands; a
    // further-generation file is refused instead of being applied with
    // fields quietly dropped.
    pub fn load(path: &Path) -> CoreResult<Self> {
        let text = std::fs::read_to_string(path).map_err(|err| {
            if err.kind() == std::io::ErrorKind::NotFound {
                CoreError::InvalidState(format!("backup not found: {}", path.display()))
            } else {
                CoreError::io_at(path, err)
            }
        })?;
        let backup: Self = serde_json::from_str(&text).map_err(|err| {
            CoreError::InvalidState(format!("invalid config backup {}: {err}", path.display()))
        })?;
        backup.check_format()?;
        Ok(backup)
    }

    fn check_format(&self) -> CoreResult<()> {
        let Some((prefix, major)) = self.format.split_once('/') else {
            return Err(CoreError::InvalidState(format!(
                "unrecognized backup format: {}",
                self.format
            )));
        };
        let expected_major = CONFIG_FORMAT.split_once('/').map(|(_, m)| m).unwrap_or("1");
        if prefix != CONFIG_FORMAT_PREFIX || major != expected_major {
            return Err(CoreError::InvalidState(format!(
                "unsupported backup format {} (this build reads {CONFIG_FORMAT})",
                self.format
            )));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_DIR_SEQ: AtomicU64 = AtomicU64::new(0);

    fn temp_dir() -> std::path::PathBuf {
        let seq = TEST_DIR_SEQ.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("deepmate-backup-test-{}-{seq}", std::process::id()))
    }

    #[test]
    fn save_and_load_roundtrip() {
        let dir = temp_dir();
        let mut config = Config::default();
        config.general.language = "zh".to_string();
        config.ui.theme = "light".to_string();
        let backup = ConfigBackup::capture(&config);

        let path = dir.join("sub").join("deepmate-config.json");
        backup.save(&path).unwrap();
        let loaded = ConfigBackup::load(&path).unwrap();
        assert_eq!(loaded, backup);
        assert_eq!(loaded.format, CONFIG_FORMAT);
        assert_eq!(loaded.config.general.language, "zh");
    }

    #[test]
    fn load_missing_file_is_an_error() {
        let dir = temp_dir();
        let err = ConfigBackup::load(&dir.join("missing.json")).unwrap_err();
        assert!(err.to_string().contains("not found"));
    }

    #[test]
    fn load_rejects_unrelated_json() {
        let dir = temp_dir();
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("bad.json");
        std::fs::write(&path, "{\"nope\": true}").unwrap();
        assert!(ConfigBackup::load(&path).is_err());
    }

    #[test]
    fn captured_config_is_json_serializable_and_pure() {
        // The whole point of the format: a machine-carried file that any
        // JSON reader can inspect.
        let backup = ConfigBackup::capture(&Config::default());
        let text = serde_json::to_string_pretty(&backup).unwrap();
        assert!(text.contains("deepmate-config/1"));
    }

    #[test]
    fn load_refuses_a_future_format() {
        let dir = temp_dir();
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("future.json");
        let mut backup = ConfigBackup::capture(&Config::default());
        backup.format = "deepmate-config/7".to_string();
        std::fs::write(&path, serde_json::to_string_pretty(&backup).unwrap()).unwrap();
        let err = ConfigBackup::load(&path).unwrap_err();
        assert!(err.to_string().contains("unsupported backup format"));
    }
}
