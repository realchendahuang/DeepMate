// DeepMate-owned persistent data: layout, configuration and history.
//
// DeepMate-owned state is transparent and portable: TOML for human-owned
// configuration, JSONL for append-oriented history. Harness-owned state is
// never stored here; it stays with the harness and is reached through the
// active adapter.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::error::{CoreError, CoreResult};

// The file-based layout of DeepMate-owned data.
//
// The root follows the operating system's application-data convention and is
// resolved by the platform layer; the core only works with the root it is
// given, which keeps this module platform-free.
#[derive(Debug, Clone)]
pub struct DataLayout {
    root: PathBuf,
}

impl DataLayout {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    pub fn config_path(&self) -> PathBuf {
        self.root.join("config.toml")
    }

    pub fn adapters_dir(&self) -> PathBuf {
        self.root.join("adapters")
    }

    pub fn cache_dir(&self) -> PathBuf {
        self.root.join("cache")
    }

    pub fn history_dir(&self) -> PathBuf {
        self.root.join("history")
    }

    pub fn snapshots_dir(&self) -> PathBuf {
        self.root.join("snapshots")
    }

    pub fn state_dir(&self) -> PathBuf {
        self.root.join("state")
    }

    pub fn logs_dir(&self) -> PathBuf {
        self.root.join("logs")
    }

    // Create the data directory tree if it does not exist yet.
    pub fn ensure(&self) -> CoreResult<()> {
        for dir in [
            self.adapters_dir(),
            self.cache_dir(),
            self.history_dir(),
            self.snapshots_dir(),
            self.state_dir(),
            self.logs_dir(),
        ] {
            fs::create_dir_all(&dir)?;
        }
        Ok(())
    }

    // The append-oriented action history for this layout.
    pub fn history(&self) -> History {
        History::new(self.history_dir().join("actions.jsonl"))
    }
}

// DeepMate configuration. TOML, human-owned.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    pub general: GeneralConfig,
    pub ui: UiConfig,
    pub market: MarketConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct GeneralConfig {
    pub language: String,
    pub auto_start: bool,
    pub check_updates: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct UiConfig {
    pub theme: String,
    pub close_to_tray: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct MarketConfig {
    pub default_source: String,
    pub refresh_interval_seconds: u64,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            language: "en".to_string(),
            auto_start: true,
            check_updates: true,
        }
    }
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            theme: "system".to_string(),
            close_to_tray: true,
        }
    }
}

impl Default for MarketConfig {
    fn default() -> Self {
        Self {
            default_source: "community".to_string(),
            refresh_interval_seconds: 3600,
        }
    }
}

impl Config {
    // Load configuration from a TOML file. A missing file yields defaults.
    pub fn load(path: &Path) -> CoreResult<Config> {
        match fs::read_to_string(path) {
            Ok(text) => toml::from_str(&text).map_err(|err| {
                CoreError::InvalidState(format!("invalid config {}: {err}", path.display()))
            }),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Config::default()),
            Err(err) => Err(err.into()),
        }
    }

    // Write configuration as TOML, creating parent directories as needed.
    pub fn save(&self, path: &Path) -> CoreResult<()> {
        let text = toml::to_string_pretty(self)
            .map_err(|err| CoreError::InvalidState(format!("failed to serialize config: {err}")))?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        fs::write(path, text)?;
        Ok(())
    }
}

// One append-only history record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionRecord {
    pub time: String,
    pub action: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub adapter: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl ActionRecord {
    pub fn new(action: impl Into<String>) -> Self {
        Self {
            time: chrono::Utc::now().to_rfc3339(),
            action: action.into(),
            adapter: None,
            detail: None,
        }
    }

    pub fn with_adapter(mut self, adapter: impl Into<String>) -> Self {
        self.adapter = Some(adapter.into());
        self
    }

    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

// Append-oriented JSONL history of DeepMate actions.
#[derive(Debug, Clone)]
pub struct History {
    path: PathBuf,
}

impl History {
    pub fn new(path: impl Into<PathBuf>) -> Self {
        Self { path: path.into() }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    // Append one record, creating the file and parent directory as needed.
    pub fn record(&self, record: &ActionRecord) -> CoreResult<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut line = serde_json::to_string(record).map_err(|err| {
            CoreError::InvalidState(format!("failed to serialize history record: {err}"))
        })?;
        line.push('\n');
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        file.write_all(line.as_bytes())?;
        Ok(())
    }

    // Read all records, for inspection, tests and diagnostics.
    pub fn read(&self) -> CoreResult<Vec<ActionRecord>> {
        let text = match fs::read_to_string(&self.path) {
            Ok(text) => text,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(err) => return Err(err.into()),
        };
        text.lines()
            .filter(|line| !line.trim().is_empty())
            .map(|line| {
                serde_json::from_str(line).map_err(|err| {
                    CoreError::InvalidState(format!("invalid history record: {err}"))
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_DIR_SEQ: AtomicU64 = AtomicU64::new(0);

    fn temp_dir() -> PathBuf {
        let seq = TEST_DIR_SEQ.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("deepmate-data-test-{}-{seq}", std::process::id()))
    }

    #[test]
    fn config_defaults_when_file_missing() {
        let dir = temp_dir();
        let config = Config::load(&dir.join("config.toml")).unwrap();
        assert_eq!(config, Config::default());
        assert_eq!(config.general.language, "en");
        assert_eq!(config.market.refresh_interval_seconds, 3600);
    }

    #[test]
    fn config_save_and_load_roundtrip() {
        let dir = temp_dir();
        let path = dir.join("config.toml");
        let mut config = Config::default();
        config.general.language = "zh".to_string();
        config.market.refresh_interval_seconds = 60;
        config.save(&path).unwrap();
        let loaded = Config::load(&path).unwrap();
        assert_eq!(loaded, config);
    }

    #[test]
    fn config_rejects_invalid_toml() {
        let dir = temp_dir();
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");
        fs::write(&path, "not [valid toml").unwrap();
        assert!(Config::load(&path).is_err());
    }

    #[test]
    fn layout_ensure_creates_directory_tree() {
        let dir = temp_dir();
        let layout = DataLayout::new(&dir);
        layout.ensure().unwrap();
        for sub in ["adapters", "cache", "history", "snapshots", "state", "logs"] {
            assert!(dir.join(sub).is_dir(), "missing {sub}");
        }
        assert_eq!(layout.config_path(), dir.join("config.toml"));
    }

    #[test]
    fn history_appends_and_reads_records() {
        let dir = temp_dir();
        let layout = DataLayout::new(&dir);
        layout.ensure().unwrap();
        let history = layout.history();
        history
            .record(&ActionRecord::new("test.one").with_adapter("test"))
            .unwrap();
        history.record(&ActionRecord::new("test.two")).unwrap();
        let records = history.read().unwrap();
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].action, "test.one");
        assert_eq!(records[0].adapter.as_deref(), Some("test"));
        assert_eq!(records[1].action, "test.two");
    }

    #[test]
    fn history_read_returns_empty_when_missing() {
        let dir = temp_dir();
        let history = History::new(dir.join("history").join("actions.jsonl"));
        assert!(history.read().unwrap().is_empty());
    }
}
