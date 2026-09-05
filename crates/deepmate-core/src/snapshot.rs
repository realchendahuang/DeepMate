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
        std::fs::create_dir_all(&self.dir)?;
        let text = serde_json::to_string_pretty(snapshot).map_err(|err| {
            CoreError::InvalidState(format!("failed to serialize snapshot: {err}"))
        })?;
        std::fs::write(self.path_for(name), text)?;
        Ok(())
    }

    // Load a snapshot by name.
    pub fn load(&self, name: &str) -> CoreResult<Snapshot> {
        let path = self.path_for(name);
        let text = std::fs::read_to_string(&path).map_err(|err| {
            if err.kind() == std::io::ErrorKind::NotFound {
                CoreError::InvalidState(format!("snapshot not found: {name}"))
            } else {
                err.into()
            }
        })?;
        serde_json::from_str(&text).map_err(|err| {
            CoreError::InvalidState(format!("invalid snapshot {}: {err}", path.display()))
        })
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
        let path = self.path_for(name);
        match std::fs::remove_file(&path) {
            Ok(()) => Ok(()),
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => Err(CoreError::InvalidState(
                format!("snapshot not found: {name}"),
            )),
            Err(err) => Err(err.into()),
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
}
