// Portable snapshots of a harness setup.
//
// A snapshot is a normalized JSON document capturing the inventory a harness
// exposes through its adapter: profiles, providers, models and plugins. It is
// the portability primitive behind `snapshot export` / `snapshot import`:
// capture a setup on one machine, apply it on another.
//
// Snapshots only carry normalized models, so they never embed secrets — a
// provider's `api_key_env` is just an environment-variable name, and the
// adapter is responsible for never surfacing plaintext credentials (see the
// pi-agent adapter, which deliberately skips `apiKey`).

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

use crate::adapter::HarnessAdapter;
use crate::error::{CoreError, CoreResult};
use crate::model::{Model, Plugin, Profile, Provider};

// The snapshot format version. Bumped when the document shape changes.
const SNAPSHOT_FORMAT: &str = "deepmate-snapshot/1";

// A portable, normalized view of a harness setup.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
    pub format: String,
    pub created: String,
    pub adapter: String,
    pub adapter_version: String,
    pub profiles: Vec<Profile>,
    pub providers: Vec<Provider>,
    pub models: Vec<Model>,
    pub plugins: Vec<Plugin>,
}

// The result of applying a snapshot to a target adapter: how many items of
// each kind were created or overwritten. Applying is merge-style (upsert), so
// nothing on the target is deleted.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotReport {
    pub profiles: usize,
    pub providers: usize,
    pub models: usize,
    pub plugins: usize,
}

impl Snapshot {
    // Capture the current inventory of an adapter into a snapshot.
    pub async fn capture(adapter: &dyn HarnessAdapter) -> CoreResult<Snapshot> {
        let metadata = adapter.metadata();
        Ok(Snapshot {
            format: SNAPSHOT_FORMAT.to_string(),
            created: chrono::Utc::now().to_rfc3339(),
            adapter: metadata.id.clone(),
            adapter_version: metadata.version.clone(),
            profiles: adapter.profiles().await?,
            providers: adapter.providers().await?,
            models: adapter.models().await?,
            plugins: adapter.plugins().await?,
        })
    }

    // Apply this snapshot to a target adapter, merge-style. Every profile is
    // created, every provider and model upserted, and every plugin installed.
    // Existing items on the target are overwritten, never removed.
    //
    // The snapshot's `adapter` must match the target adapter's id: a snapshot
    // captured from one harness has a different configuration shape and must
    // not be applied to another.
    pub async fn apply(&self, adapter: &dyn HarnessAdapter) -> CoreResult<SnapshotReport> {
        let target = adapter.metadata();
        if self.adapter != target.id {
            return Err(CoreError::InvalidState(format!(
                "snapshot is for adapter '{}' but the active adapter is '{}'",
                self.adapter, target.id
            )));
        }

        let mut report = SnapshotReport::default();

        for profile in &self.profiles {
            adapter.create_profile(&profile.id).await?;
            report.profiles += 1;
        }
        for provider in &self.providers {
            adapter.upsert_provider(provider.clone()).await?;
            report.providers += 1;
        }
        for model in &self.models {
            let provider = model
                .provider
                .clone()
                .ok_or_else(|| CoreError::InvalidState("model has no provider".to_string()))?;
            adapter.upsert_model(&provider, model.clone()).await?;
            report.models += 1;
        }
        for plugin in &self.plugins {
            // Plugins without an attributable profile cannot be reinstalled.
            if plugin.profile.is_empty() {
                continue;
            }
            adapter.install_plugin(&plugin.profile, &plugin.id).await?;
            report.plugins += 1;
        }

        Ok(report)
    }
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
    use crate::testkit::FakeAdapter;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_DIR_SEQ: AtomicU64 = AtomicU64::new(0);

    fn temp_dir() -> PathBuf {
        let seq = TEST_DIR_SEQ.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!(
            "deepmate-snapshot-test-{}-{seq}",
            std::process::id()
        ))
    }

    #[tokio::test]
    async fn capture_and_apply_roundtrip() {
        let adapter = FakeAdapter::healthy();
        let snapshot = Snapshot::capture(&adapter).await.unwrap();
        assert_eq!(snapshot.adapter, "test");
        assert_eq!(snapshot.format, SNAPSHOT_FORMAT);
        assert_eq!(snapshot.profiles.len(), 1);
        assert_eq!(snapshot.providers.len(), 1);
        assert_eq!(snapshot.models.len(), 1);
        assert_eq!(snapshot.plugins.len(), 1);

        // Applying to the same adapter records the expected counts.
        let report = snapshot.apply(&adapter).await.unwrap();
        assert_eq!(report.profiles, 1);
        assert_eq!(report.providers, 1);
        assert_eq!(report.models, 1);
        assert_eq!(report.plugins, 1);
    }

    #[tokio::test]
    async fn apply_rejects_mismatched_adapter() {
        let snapshot = Snapshot::capture(&FakeAdapter::healthy()).await.unwrap();
        let other = FakeAdapter::new("other");
        let err = snapshot.apply(&other).await.unwrap_err();
        assert!(err.to_string().contains("adapter 'test'"));
    }

    #[test]
    fn store_roundtrip_and_list() {
        let dir = temp_dir();
        let store = SnapshotStore::new(&dir);
        assert!(store.list().unwrap().is_empty());

        let snapshot = Snapshot {
            format: SNAPSHOT_FORMAT.to_string(),
            created: "2026-08-26T00:00:00Z".to_string(),
            adapter: "test".to_string(),
            adapter_version: "0.1.0".to_string(),
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
}
