// DeepMate-owned persistent data: layout, configuration and history.
//
// DeepMate-owned state is transparent and portable: TOML for human-owned
// configuration, JSONL for append-oriented history. Harness-owned state is
// never stored here; it stays with the harness and is reached through the
// harness service layer.

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
            self.cache_dir(),
            self.history_dir(),
            self.snapshots_dir(),
            self.state_dir(),
            self.logs_dir(),
        ] {
            fs::create_dir_all(&dir).map_err(|err| CoreError::io_at(&dir, err))?;
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
    // Desktop notification for a newly found release. Explicit checks from
    // the tray always report their result; this only gates the automatic
    // startup check.
    pub notify_updates: bool,
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
            language: "zh".to_string(),
            // Auto-start is opt-in: enabling it registers the app with the
            // operating system's login items, which must never happen without
            // an explicit user action.
            auto_start: false,
            check_updates: true,
            notify_updates: true,
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
            Err(err) => Err(CoreError::io_at(path, err)),
        }
    }

    // Load configuration, recovering from a corrupt file.
    //
    // A config that fails to parse must never leave the app in a permanent
    // error state, and must never be silently destroyed: the bad file is
    // quarantined next to the config (see `fsutil::quarantine`) and the
    // caller learns where it went, while a fresh defaults file is written so
    // subsequent loads and saves succeed.
    pub fn load_recovering(path: &Path) -> CoreResult<(Config, Option<PathBuf>)> {
        match Self::load(path) {
            Ok(config) => Ok((config, None)),
            Err(err) if err.code() == "invalid_state" => {
                let quarantined = crate::fsutil::quarantine(path).ok();
                let config = Config::default();
                config.save(path)?;
                Ok((config, quarantined))
            }
            Err(err) => Err(err),
        }
    }

    // Write configuration as TOML, creating parent directories as needed.
    // The write is atomic: a crash can never leave a half-written file.
    pub fn save(&self, path: &Path) -> CoreResult<()> {
        let text = toml::to_string_pretty(self)
            .map_err(|err| CoreError::InvalidState(format!("failed to serialize config: {err}")))?;
        crate::fsutil::write_atomic_string(path, &text)
    }
}

// One append-only history record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ActionRecord {
    pub time: String,
    pub action: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

impl ActionRecord {
    pub fn new(action: impl Into<String>) -> Self {
        Self {
            time: chrono::Utc::now().to_rfc3339(),
            action: action.into(),
            detail: None,
        }
    }

    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }
}

// Append-oriented JSONL history of DeepMate actions.
//
// The file is capped so it cannot grow without bound: once it exceeds
// `MAX_HISTORY_BYTES` it is rotated to a single `actions.1.jsonl` generation.
#[derive(Debug, Clone)]
pub struct History {
    path: PathBuf,
}

// Rotate the history file once it grows past this size.
const MAX_HISTORY_BYTES: u64 = 1 << 20;

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
            fs::create_dir_all(parent).map_err(|err| CoreError::io_at(parent, err))?;
        }
        self.rotate_if_needed()?;
        let mut line = serde_json::to_string(record).map_err(|err| {
            CoreError::InvalidState(format!("failed to serialize history record: {err}"))
        })?;
        line.push('\n');
        let mut file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
            .map_err(|err| CoreError::io_at(&self.path, err))?;
        file.write_all(line.as_bytes())
            .map_err(|err| CoreError::io_at(&self.path, err))?;
        Ok(())
    }

    // Move the history aside when it outgrows its cap. Only one old
    // generation is kept; the previous one is replaced.
    fn rotate_if_needed(&self) -> CoreResult<()> {
        let size = match fs::metadata(&self.path) {
            Ok(meta) => meta.len(),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(()),
            Err(err) => return Err(CoreError::io_at(&self.path, err)),
        };
        if size < MAX_HISTORY_BYTES {
            return Ok(());
        }
        let rotated = self.path.with_file_name("actions.1.jsonl");
        let _ = fs::remove_file(&rotated);
        fs::rename(&self.path, &rotated).map_err(|err| CoreError::io_at(&self.path, err))
    }

    // Read all records for inspection and diagnostics.
    //
    // A damaged line (a truncated append after a crash, a hand edit) is
    // skipped instead of failing the whole file; the number of skipped lines
    // is reported alongside the records.
    pub fn read(&self) -> CoreResult<Vec<ActionRecord>> {
        Ok(self.read_lenient()?.0)
    }

    pub fn read_lenient(&self) -> CoreResult<(Vec<ActionRecord>, usize)> {
        let text = match fs::read_to_string(&self.path) {
            Ok(text) => text,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok((Vec::new(), 0)),
            Err(err) => return Err(CoreError::io_at(&self.path, err)),
        };
        let mut records = Vec::new();
        let mut skipped = 0usize;
        for line in text.lines().filter(|line| !line.trim().is_empty()) {
            match serde_json::from_str(line) {
                Ok(record) => records.push(record),
                Err(_) => skipped += 1,
            }
        }
        Ok((records, skipped))
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
        assert_eq!(config.general.language, "zh");
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
        for sub in ["cache", "history", "snapshots", "state", "logs"] {
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
        history.record(&ActionRecord::new("test.one")).unwrap();
        history.record(&ActionRecord::new("test.two")).unwrap();
        let records = history.read().unwrap();
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].action, "test.one");
        assert_eq!(records[1].action, "test.two");
    }

    #[test]
    fn history_read_returns_empty_when_missing() {
        let dir = temp_dir();
        let history = History::new(dir.join("history").join("actions.jsonl"));
        assert!(history.read().unwrap().is_empty());
    }

    #[test]
    fn config_recovering_quarantines_a_corrupt_file() {
        let dir = temp_dir();
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");
        fs::write(&path, "not [valid toml").unwrap();

        let (config, quarantined) = Config::load_recovering(&path).unwrap();
        assert_eq!(config, Config::default());

        let quarantined = quarantined.expect("corrupt file is quarantined");
        assert_eq!(fs::read_to_string(&quarantined).unwrap(), "not [valid toml");
        // The config path holds a usable defaults file again.
        assert_eq!(Config::load(&path).unwrap(), Config::default());
    }

    #[test]
    fn history_skips_damaged_lines_instead_of_failing() {
        let dir = temp_dir();
        let layout = DataLayout::new(&dir);
        layout.ensure().unwrap();
        let history = layout.history();
        history.record(&ActionRecord::new("test.one")).unwrap();
        {
            use std::io::Write as _;
            let mut file = fs::OpenOptions::new()
                .append(true)
                .open(history.path())
                .unwrap();
            // A truncated append after a crash.
            file.write_all(b"{\"time\":\"2026-01-01T00:00:00Z\"")
                .unwrap();
            file.write_all(b"\n").unwrap();
        }
        history.record(&ActionRecord::new("test.two")).unwrap();

        let (records, skipped) = history.read_lenient().unwrap();
        assert_eq!(skipped, 1);
        assert_eq!(records.len(), 2);
        assert_eq!(records[1].action, "test.two");
    }

    #[test]
    fn history_rotates_past_the_size_cap() {
        let dir = temp_dir();
        fs::create_dir_all(&dir).unwrap();
        let history = History::new(dir.join("actions.jsonl"));
        // Pre-seed an oversized file and verify the next append rotates it.
        fs::write(history.path(), "x".repeat((1 << 20) as usize)).unwrap();
        history
            .record(&ActionRecord::new("after.rotation"))
            .unwrap();

        assert!(dir.join("actions.1.jsonl").exists());
        let records = history.read().unwrap();
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].action, "after.rotation");
    }
}
