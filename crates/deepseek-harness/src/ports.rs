// DeepMate-owned scenario port assignments (`state/ports.json`).
//
// Web-surface scenarios each need a fixed port so their URL survives
// DeepMate restarts and the engine keeps running independently. The default
// `web` scenario owns 3080; later web scenarios get 3081, 3082, … — the
// smallest port that is neither already assigned nor currently listened on.
// Task-surface scenarios never take a port. The registry lives under the
// DeepMate data directory, never in the harness home, mirroring the
// disabled-plugin registry.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use deepmate_core::error::{CoreError, CoreResult};

const STATE_DIR: &str = "state";
const FILENAME: &str = "ports.json";
const SCHEMA: u32 = 1;

// The port the default web scenario owns, matching the engine's own default.
pub const DEFAULT_WEB_PORT: u16 = 3080;
// The first port handed to non-default web scenarios.
pub(crate) const FIRST_SCENARIO_PORT: u16 = 3081;

#[derive(Debug, Clone, Default)]
pub struct PortRegistry {
    path: Option<PathBuf>,
}

impl PortRegistry {
    pub fn new(data_root: Option<PathBuf>) -> Self {
        Self {
            path: data_root.map(|root| root.join(STATE_DIR).join(FILENAME)),
        }
    }

    // The port assigned to a scenario, when it has one.
    pub fn port(&self, profile: &str) -> CoreResult<Option<u16>> {
        Ok(self.load()?.ports.get(profile).copied())
    }

    // Assign a port to a scenario. `preferred` wins when it is free; the
    // default `web` scenario falls back to `DEFAULT_WEB_PORT`. Otherwise the
    // smallest unassigned and unlistened port from `FIRST_SCENARIO_PORT` up
    // is handed out. `listening` reports whether a port is currently taken
    // by a live process, so an externally started engine is respected.
    pub fn assign(
        &self,
        profile: &str,
        preferred: Option<u16>,
        listening: impl Fn(u16) -> bool,
    ) -> CoreResult<u16> {
        let Some(path) = &self.path else {
            return Err(CoreError::InvalidState(
                "no data directory; cannot assign a scenario port".to_string(),
            ));
        };
        let mut file = self.load()?;
        if let Some(existing) = file.ports.get(profile) {
            return Ok(*existing);
        }
        // A port is unavailable when another scenario already owns it or a
        // live process listens on it. A `preferred` port that fails either
        // check is skipped: handing it out anyway makes two engines fight
        // over one port, and the loser only fails after the boot timeout.
        let unavailable =
            |port: u16| file.ports.values().any(|assigned| *assigned == port) || listening(port);
        let mut candidate = match preferred {
            Some(port) if !unavailable(port) => port,
            _ if profile == "web" && preferred.is_none() => DEFAULT_WEB_PORT,
            _ => FIRST_SCENARIO_PORT,
        };
        while unavailable(candidate) {
            candidate += 1;
        }
        file.ports.insert(profile.to_string(), candidate);
        self.write(path, &file)?;
        Ok(candidate)
    }

    // Drop a scenario's port assignment (on stop), freeing it for reuse.
    pub fn release(&self, profile: &str) -> CoreResult<()> {
        let Some(path) = &self.path else {
            return Ok(());
        };
        let mut file = self.load()?;
        if file.ports.remove(profile).is_none() {
            return Ok(());
        }
        self.write(path, &file)
    }

    fn load(&self) -> CoreResult<PortsFile> {
        let Some(path) = &self.path else {
            return Ok(PortsFile::default());
        };
        let text = match std::fs::read_to_string(path) {
            Ok(text) => text,
            Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
                return Ok(PortsFile::default())
            }
            Err(err) => return Err(err.into()),
        };
        let file: PortsFile = serde_json::from_str(&text).map_err(|err| {
            CoreError::InvalidState(format!("invalid port registry {}: {err}", path.display()))
        })?;
        if file.schema > SCHEMA {
            return Err(CoreError::InvalidState(format!(
                "port registry schema {} is newer than this build supports ({SCHEMA})",
                file.schema
            )));
        }
        Ok(file)
    }

    fn write(&self, path: &Path, file: &PortsFile) -> CoreResult<()> {
        let text = serde_json::to_string_pretty(file).map_err(|err| {
            CoreError::InvalidState(format!("failed to serialize port registry: {err}"))
        })?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, text)?;
        Ok(())
    }
}

#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
struct PortsFile {
    schema: u32,
    ports: BTreeMap<String, u16>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_registry() -> (PortRegistry, PathBuf) {
        // Tests run concurrently in one process: a wall-clock name can collide
        // and let two tests share (and truncate) the same ports.json.
        static SEQ: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let seq = SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("deepmate-ports-test-{}-{seq}", std::process::id()));
        (PortRegistry::new(Some(dir.clone())), dir)
    }

    #[test]
    fn web_defaults_to_3080_and_others_climb() {
        let (registry, dir) = temp_registry();
        let free = |_port: u16| false;
        let web = registry.assign("web", None, free).unwrap();
        assert_eq!(web, DEFAULT_WEB_PORT);
        let coding = registry.assign("coding", None, free).unwrap();
        assert_eq!(coding, FIRST_SCENARIO_PORT);
        let nightly = registry.assign("nightly", None, free).unwrap();
        assert_eq!(nightly, FIRST_SCENARIO_PORT + 1);
        // The assignment is sticky across loads.
        assert_eq!(registry.port("coding").unwrap(), Some(FIRST_SCENARIO_PORT));
        // A listening port is skipped; 3081 is both assigned and listened on,
        // 3082 is already assigned, so the next free candidate is 3083.
        let busy = registry
            .assign("busy", None, |port| port == FIRST_SCENARIO_PORT)
            .unwrap();
        assert_eq!(busy, FIRST_SCENARIO_PORT + 2);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn preferred_port_wins_when_free() {
        let (registry, dir) = temp_registry();
        let free = |_port: u16| false;
        let port = registry.assign("coding", Some(4200), free).unwrap();
        assert_eq!(port, 4200);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn preferred_port_is_skipped_when_owned_by_another_scenario() {
        let (registry, dir) = temp_registry();
        let free = |_port: u16| false;
        registry.assign("coding", Some(4200), free).unwrap();
        // The port is taken: the second scenario must get the next free one
        // instead of double-booking 4200.
        let port = registry.assign("daily", Some(4200), free).unwrap();
        assert_eq!(port, FIRST_SCENARIO_PORT);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn preferred_port_is_skipped_when_listening() {
        let (registry, dir) = temp_registry();
        let listening = |port: u16| port == 4200;
        let port = registry.assign("coding", Some(4200), listening).unwrap();
        assert_eq!(port, FIRST_SCENARIO_PORT);
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn release_frees_the_port_for_reuse() {
        let (registry, dir) = temp_registry();
        let free = |_port: u16| false;
        registry.assign("coding", None, free).unwrap();
        registry.release("coding").unwrap();
        let next = registry.assign("daily", None, free).unwrap();
        assert_eq!(next, FIRST_SCENARIO_PORT);
        // Releasing an unassigned scenario is a no-op.
        registry.release("absent").unwrap();
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn no_data_root_refuses_assignment() {
        let registry = PortRegistry::new(None);
        let free = |_port: u16| false;
        assert!(registry.assign("coding", None, free).is_err());
        assert_eq!(registry.port("coding").unwrap(), None);
    }
}
