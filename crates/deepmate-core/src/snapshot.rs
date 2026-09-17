// Portable snapshots of a harness setup.
//
// A snapshot is a normalized JSON document capturing a harness inventory:
// profiles, providers, models and plugins. It is the portability primitive
// behind `snapshot export` / `snapshot import`: capture a setup on one
// machine, apply it on another.
//
// Snapshots only carry normalized models, so they never embed secrets — a
// provider's `api_key_env` is just an environment-variable name, and the
// harness integration is responsible for never surfacing plaintext
// credentials. Capturing and applying a snapshot (the orchestration that
// reads and writes a real harness) lives in the harness service crate; this
// module holds only the data model and the on-disk store.

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::error::{CoreError, CoreResult};
use crate::model::{Model, Plugin, Profile, Provider};

// The snapshot format version. Bumped when the document shape changes.
pub const SNAPSHOT_FORMAT: &str = "deepmate-snapshot/2";

// The major part of `SNAPSHOT_FORMAT`, used to reject incompatible documents.
const SNAPSHOT_FORMAT_PREFIX: &str = "deepmate-snapshot";

// Validate a snapshot name.
//
// Names become file names under the snapshots directory, so they must not be
// able to escape it: separators, traversal and control characters are
// rejected, as are empty, over-long and hidden names.
pub fn validate_snapshot_name(name: &str) -> CoreResult<()> {
    let trimmed = name.trim();
    if trimmed.is_empty() {
        return Err(CoreError::InvalidState(
            "snapshot name is empty".to_string(),
        ));
    }
    if trimmed != name {
        return Err(CoreError::InvalidState(
            "snapshot name must not start or end with whitespace".to_string(),
        ));
    }
    if name.chars().count() > 64 {
        return Err(CoreError::InvalidState(
            "snapshot name is too long (max 64 characters)".to_string(),
        ));
    }
    if name.starts_with('.') {
        return Err(CoreError::InvalidState(
            "snapshot name must not start with a dot".to_string(),
        ));
    }
    let valid = name
        .chars()
        .all(|c| c.is_alphanumeric() || matches!(c, '-' | '_' | ' '));
    if !valid {
        return Err(CoreError::InvalidState(format!(
            "snapshot name contains unsupported characters: {name}"
        )));
    }
    Ok(())
}

// Reject a document whose format is not one this build understands.
fn check_snapshot_format(format: &str) -> CoreResult<()> {
    let Some((prefix, major)) = format.split_once('/') else {
        return Err(CoreError::InvalidState(format!(
            "unrecognized snapshot format: {format}"
        )));
    };
    let expected_major = SNAPSHOT_FORMAT
        .split_once('/')
        .map(|(_, m)| m)
        .unwrap_or("1");
    if prefix != SNAPSHOT_FORMAT_PREFIX || major != expected_major {
        return Err(CoreError::InvalidState(format!(
            "unsupported snapshot format {format} (this build reads {SNAPSHOT_FORMAT})"
        )));
    }
    Ok(())
}

// A portable, normalized view of a harness setup.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
    pub format: String,
    pub created: String,
    pub profiles: Vec<Profile>,
    pub providers: Vec<Provider>,
    pub models: Vec<Model>,
    pub plugins: Vec<Plugin>,
}

// The result of applying a snapshot to a harness: how many items of each kind
// were created or overwritten. Applying is merge-style (upsert), so nothing on
// the target is deleted.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotReport {
    pub profiles: usize,
    pub providers: usize,
    pub models: usize,
    pub plugins: usize,
}

// Reads and writes snapshots under a directory (the data layout's
// `snapshots/` dir). Snapshots are named `<name>.json`.
#[derive(Debug, Clone)]
pub struct SnapshotStore {
    dir: PathBuf,
}

impl SnapshotStore {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    fn path_for(&self, name: &str) -> PathBuf {
        self.dir.join(format!("{name}.json"))
    }

    // Write a snapshot to `<name>.json`, creating the directory as needed.
    pub fn save(&self, name: &str, snapshot: &Snapshot) -> CoreResult<()> {
        validate_snapshot_name(name)?;
        std::fs::create_dir_all(&self.dir).map_err(|err| CoreError::io_at(&self.dir, err))?;
        let text = serde_json::to_string_pretty(snapshot).map_err(|err| {
            CoreError::InvalidState(format!("failed to serialize snapshot: {err}"))
        })?;
        crate::fsutil::write_atomic_string(&self.path_for(name), &text)
    }

    // Load a snapshot by name.
    pub fn load(&self, name: &str) -> CoreResult<Snapshot> {
        validate_snapshot_name(name)?;
        let path = self.path_for(name);
        let text = std::fs::read_to_string(&path).map_err(|err| {
            if err.kind() == std::io::ErrorKind::NotFound {
                CoreError::InvalidState(format!("snapshot not found: {name}"))
            } else {
                CoreError::io_at(&path, err)
            }
        })?;
        let snapshot: Snapshot = serde_json::from_str(&text).map_err(|err| {
            CoreError::InvalidState(format!("invalid snapshot {}: {err}", path.display()))
        })?;
        check_snapshot_format(&snapshot.format)?;
        Ok(snapshot)
    }

    // List the names of stored snapshots, sorted.
    pub fn list(&self) -> CoreResult<Vec<String>> {
        let entries = match std::fs::read_dir(&self.dir) {
            Ok(entries) => entries,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(err) => return Err(err.into()),
        };
        let mut names = Vec::new();
        for entry in entries.flatten() {
            let path = entry.path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("json") {
                continue;
            }
            if let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) {
                names.push(stem.to_string());
            }
        }
        names.sort();
        Ok(names)
    }

    // Delete a snapshot by name; a missing snapshot is an error.
    pub fn delete(&self, name: &str) -> CoreResult<()> {
        validate_snapshot_name(name)?;
        let path = self.path_for(name);
        match std::fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Err(CoreError::InvalidState(
                format!("snapshot not found: {name}"),
            )),
            Err(err) => Err(CoreError::io_at(&path, err)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn store_roundtrip_and_list() {
        let dir =
            std::env::temp_dir().join(format!("deepmate-snapshot-test-{}", std::process::id()));
        let store = SnapshotStore::new(&dir);
        assert!(store.list().unwrap().is_empty());

        let snapshot = Snapshot {
            format: SNAPSHOT_FORMAT.to_string(),
            created: "2026-08-26T00:00:00Z".to_string(),
            profiles: vec![],
            providers: vec![],
            models: vec![],
            plugins: vec![],
        };
        store.save("coding", &snapshot).unwrap();
        store.save("research", &snapshot).unwrap();

        let names = store.list().unwrap();
        assert_eq!(names, ["coding", "research"]);

        let loaded = store.load("coding").unwrap();
        assert_eq!(loaded, snapshot);

        assert!(store.load("missing").is_err());
    }

    #[test]
    fn store_rejects_malformed_snapshot() {
        let dir =
            std::env::temp_dir().join(format!("deepmate-snapshot-bad-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("bad.json"), "{ not json").unwrap();
        let store = SnapshotStore::new(&dir);
        let err = store.load("bad").unwrap_err();
        assert!(err.to_string().contains("invalid snapshot"));
    }

    #[test]
    fn names_that_could_escape_the_directory_are_rejected() {
        for name in [
            "../escape",
            "a/b",
            "a\\b",
            "",
            "  ",
            ".hidden",
            "trailing ",
            &"x".repeat(65),
        ] {
            assert!(
                validate_snapshot_name(name).is_err(),
                "name should be rejected: {name:?}"
            );
        }
        for name in ["coding", "my setup", "work-2026_09", "中文名"] {
            assert!(
                validate_snapshot_name(name).is_ok(),
                "name should be accepted: {name:?}"
            );
        }
    }

    #[test]
    fn the_store_refuses_path_traversal_names() {
        let dir =
            std::env::temp_dir().join(format!("deepmate-snapshot-escape-{}", std::process::id()));
        let store = SnapshotStore::new(&dir);
        let snapshot = Snapshot {
            format: SNAPSHOT_FORMAT.to_string(),
            created: "2026-08-26T00:00:00Z".to_string(),
            profiles: vec![],
            providers: vec![],
            models: vec![],
            plugins: vec![],
        };
        assert!(store.save("../escape", &snapshot).is_err());
        assert!(store.load("../escape").is_err());
        assert!(store.delete("../escape").is_err());
    }

    #[test]
    fn foreign_formats_are_rejected_on_load() {
        let dir =
            std::env::temp_dir().join(format!("deepmate-snapshot-format-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let store = SnapshotStore::new(&dir);
        let mut snapshot = Snapshot {
            format: "deepmate-snapshot/9".to_string(),
            created: "2026-08-26T00:00:00Z".to_string(),
            profiles: vec![],
            providers: vec![],
            models: vec![],
            plugins: vec![],
        };
        // `save` is format-agnostic (capture wrote it), but `load` must refuse
        // a document this build cannot interpret.
        store.save("future", &snapshot).unwrap();
        let err = store.load("future").unwrap_err();
        assert!(err.to_string().contains("unsupported snapshot format"));

        snapshot.format = "deepmate-snapshot/2".to_string();
        store.save("current", &snapshot).unwrap();
        assert!(store.load("current").is_ok());
    }
}
