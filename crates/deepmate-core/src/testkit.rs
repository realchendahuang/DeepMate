// Test doubles and helpers for writing adapter/core tests without a real
// harness installed.
//
// This module is intentionally public: CLI integration tests and local debug
// commands can use the same fake adapter to get deterministic behaviour.

use async_trait::async_trait;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use crate::adapter::{AdapterCapabilities, AdapterMetadata, Detection, HarnessAdapter};
use crate::error::{CoreError, CoreResult};
use crate::model::{
    CheckStatus, DoctorCheck, DoctorReport, HarnessInfo, MarketEntry, MarketSource,
    MarketSourceInfo, Model, Plugin, Profile, Provider, RuntimeStatus, RuntimeStatusKind,
};

static NEXT_PID: AtomicU32 = AtomicU32::new(4200);

// A configurable in-memory adapter for tests and local debugging.
//
// Use FakeAdapter::healthy() or FakeAdapter::unhealthy() for quick setups,
// or mutate the public fields for custom scenarios.
#[derive(Debug, Clone)]
pub struct FakeAdapter {
    pub id: String,
    pub name: String,
    pub version: String,
    pub found: bool,
    pub status: RuntimeStatus,
    pub capabilities: AdapterCapabilities,
    pub doctor: DoctorReport,
    pub start_error: Option<String>,
    pub stop_error: Option<String>,
    pub restart_error: Option<String>,
    pub open_error: Option<String>,
    pub install_error: Option<String>,
    pub remove_error: Option<String>,
    pub update_error: Option<String>,
    pub search_error: Option<String>,
    pub upsert_provider_error: Option<String>,
    pub remove_provider_error: Option<String>,
    pub upsert_model_error: Option<String>,
    pub remove_model_error: Option<String>,
    pub create_profile_error: Option<String>,
    pub remove_profile_error: Option<String>,
    // Call recordings for plugin lifecycle operations, as
    // "<profile>:<spec-or-id>" lines (update without an id records the bare
    // profile). Shared so the tests can inspect them through &self.
    pub installed: Arc<Mutex<Vec<String>>>,
    pub removed: Arc<Mutex<Vec<String>>>,
    pub updated: Arc<Mutex<Vec<String>>>,
    // Call recordings for configuration edits, as "<profile>:<name>" or
    // "<provider>:<model-or-id>". Shared so the tests can inspect them.
    pub created_profiles: Arc<Mutex<Vec<String>>>,
    pub removed_profiles: Arc<Mutex<Vec<String>>>,
    pub upserted_providers: Arc<Mutex<Vec<String>>>,
    pub removed_providers: Arc<Mutex<Vec<String>>>,
    pub upserted_models: Arc<Mutex<Vec<String>>>,
    pub removed_models: Arc<Mutex<Vec<String>>>,
    pub market_entries: Vec<MarketEntry>,
    pub market_sources: Vec<MarketSourceInfo>,
}

impl FakeAdapter {
    pub fn new(id: &str) -> Self {
        Self {
            id: id.to_string(),
            name: format!("Fake {id}"),
            version: "0.1.0-test".to_string(),
            found: true,
            status: RuntimeStatus::installed(),
            capabilities: AdapterCapabilities {
                runtime: true,
                profiles: true,
                providers: true,
                models: true,
                plugins: true,
                marketplace: true,
                snapshots: true,
                ..Default::default()
            },
            doctor: DoctorReport {
                adapter_id: id.to_string(),
                checks: vec![DoctorCheck {
                    id: "fake.healthy".to_string(),
                    status: CheckStatus::Pass,
                    summary: "fake adapter is healthy".to_string(),
                    details: None,
                    suggested_action: None,
                }],
            },
            start_error: None,
            stop_error: None,
            restart_error: None,
            open_error: None,
            install_error: None,
            remove_error: None,
            update_error: None,
            search_error: None,
            upsert_provider_error: None,
            remove_provider_error: None,
            upsert_model_error: None,
            remove_model_error: None,
            create_profile_error: None,
            remove_profile_error: None,
            installed: Arc::new(Mutex::new(Vec::new())),
            removed: Arc::new(Mutex::new(Vec::new())),
            updated: Arc::new(Mutex::new(Vec::new())),
            created_profiles: Arc::new(Mutex::new(Vec::new())),
            removed_profiles: Arc::new(Mutex::new(Vec::new())),
            upserted_providers: Arc::new(Mutex::new(Vec::new())),
            removed_providers: Arc::new(Mutex::new(Vec::new())),
            upserted_models: Arc::new(Mutex::new(Vec::new())),
            removed_models: Arc::new(Mutex::new(Vec::new())),
            market_entries: vec![MarketEntry {
                id: "fake-market-plugin".to_string(),
                name: "Fake Market Plugin".to_string(),
                description: Some("A fake market entry".to_string()),
                version: Some("2.0.0".to_string()),
                source: MarketSource::Community,
                repository: Some("https://example.com/repo".to_string()),
                publisher: Some("fake-publisher".to_string()),
                updated: None,
            }],
            market_sources: vec![
                MarketSourceInfo {
                    id: "curated".to_string(),
                    name: "Curated".to_string(),
                    description: "fake curated source".to_string(),
                    source: MarketSource::Curated,
                },
                MarketSourceInfo {
                    id: "community".to_string(),
                    name: "Community".to_string(),
                    description: "fake community source".to_string(),
                    source: MarketSource::Community,
                },
            ],
        }
    }

    // A fake adapter that reports a healthy installed runtime.
    pub fn healthy() -> Self {
        Self::new("test")
    }

    // A fake adapter that reports a missing runtime and a failing doctor.
    pub fn unhealthy() -> Self {
        let mut adapter = Self::new("test");
        adapter.found = false;
        adapter.status = RuntimeStatus {
            kind: RuntimeStatusKind::Error,
            pid: None,
            message: Some("harness not found".to_string()),
        };
        adapter.doctor = DoctorReport {
            adapter_id: adapter.id.clone(),
            checks: vec![DoctorCheck {
                id: "fake.runtime".to_string(),
                status: CheckStatus::Fail,
                summary: "harness binary was not found".to_string(),
                details: Some("fake detail".to_string()),
                suggested_action: Some("install the harness or use --adapter test".to_string()),
            }],
        };
        adapter
    }

    // A fake adapter that reports a running runtime with a synthetic PID.
    pub fn running() -> Self {
        let pid = NEXT_PID.fetch_add(1, Ordering::Relaxed);
        let mut adapter = Self::new("test");
        adapter.status = RuntimeStatus::running(pid);
        adapter
    }

    fn unsupported(&self, op: &str) -> CoreError {
        CoreError::Unsupported(format!("{}.{} is disabled in this fake", self.id, op))
    }
}

#[async_trait]
impl HarnessAdapter for FakeAdapter {
    fn metadata(&self) -> AdapterMetadata {
        AdapterMetadata {
            id: self.id.clone(),
            name: self.name.clone(),
            version: self.version.clone(),
        }
    }

    fn capabilities(&self) -> AdapterCapabilities {
        self.capabilities.clone()
    }

    async fn detect(&self) -> CoreResult<Detection> {
        Ok(Detection {
            found: self.found,
            harness: self.found.then(|| HarnessInfo {
                id: self.id.clone(),
                name: self.name.clone(),
                version: Some("9.9.9-test".to_string()),
                adapter_version: self.version.clone(),
            }),
            detail: self.found.then(|| "fake detection succeeded".to_string()),
        })
    }

    async fn status(&self) -> CoreResult<RuntimeStatus> {
        Ok(self.status.clone())
    }

    async fn start(&self) -> CoreResult<()> {
        match &self.start_error {
            Some(message) => Err(CoreError::InvalidState(message.clone())),
            None => Ok(()),
        }
    }

    async fn stop(&self) -> CoreResult<()> {
        match &self.stop_error {
            Some(message) => Err(CoreError::InvalidState(message.clone())),
            None => Ok(()),
        }
    }

    async fn restart(&self) -> CoreResult<()> {
        match &self.restart_error {
            Some(message) => Err(CoreError::InvalidState(message.clone())),
            None => Ok(()),
        }
    }

    async fn open_ui(&self) -> CoreResult<()> {
        match &self.open_error {
            Some(message) => Err(CoreError::InvalidState(message.clone())),
            None => Ok(()),
        }
    }

    async fn profiles(&self) -> CoreResult<Vec<Profile>> {
        Ok(vec![Profile {
            id: "default".to_string(),
            name: "Default".to_string(),
            description: Some("Fake default profile".to_string()),
        }])
    }

    async fn providers(&self) -> CoreResult<Vec<Provider>> {
        Ok(vec![Provider {
            id: "demo".to_string(),
            name: "Demo".to_string(),
            kind: "openai-compatible".to_string(),
            api: Some("openai-completions".to_string()),
            base_url: Some("https://example.com/v1".to_string()),
            api_key_env: Some("DEMO_API_KEY".to_string()),
            compat: None,
        }])
    }

    async fn models(&self) -> CoreResult<Vec<Model>> {
        Ok(vec![Model {
            id: "demo-chat".to_string(),
            name: "Demo Chat".to_string(),
            provider: Some("demo".to_string()),
            context_window: Some(8192),
            max_tokens: Some(2048),
            input: None,
            reasoning_efforts: None,
            compat: None,
        }])
    }

    async fn upsert_provider(&self, provider: Provider) -> CoreResult<()> {
        if let Some(message) = &self.upsert_provider_error {
            return Err(CoreError::InvalidState(message.clone()));
        }
        self.upserted_providers
            .lock()
            .unwrap()
            .push(format!("{}:{}", provider.id, provider.name));
        Ok(())
    }

    async fn remove_provider(&self, id: &str) -> CoreResult<()> {
        if let Some(message) = &self.remove_provider_error {
            return Err(CoreError::InvalidState(message.clone()));
        }
        self.removed_providers.lock().unwrap().push(id.to_string());
        Ok(())
    }

    async fn upsert_model(&self, provider: &str, model: Model) -> CoreResult<()> {
        if let Some(message) = &self.upsert_model_error {
            return Err(CoreError::InvalidState(message.clone()));
        }
        self.upserted_models
            .lock()
            .unwrap()
            .push(format!("{provider}:{}", model.id));
        Ok(())
    }

    async fn remove_model(&self, provider: &str, id: &str) -> CoreResult<()> {
        if let Some(message) = &self.remove_model_error {
            return Err(CoreError::InvalidState(message.clone()));
        }
        self.removed_models
            .lock()
            .unwrap()
            .push(format!("{provider}:{id}"));
        Ok(())
    }

    async fn create_profile(&self, name: &str) -> CoreResult<()> {
        if let Some(message) = &self.create_profile_error {
            return Err(CoreError::InvalidState(message.clone()));
        }
        self.created_profiles.lock().unwrap().push(name.to_string());
        Ok(())
    }

    async fn remove_profile(&self, name: &str) -> CoreResult<()> {
        if let Some(message) = &self.remove_profile_error {
            return Err(CoreError::InvalidState(message.clone()));
        }
        self.removed_profiles.lock().unwrap().push(name.to_string());
        Ok(())
    }

    async fn plugins(&self) -> CoreResult<Vec<Plugin>> {
        Ok(vec![Plugin {
            id: "fake-plugin".to_string(),
            name: "Fake Plugin".to_string(),
            version: Some("1.0.0".to_string()),
            enabled: true,
            profile: "default".to_string(),
            latest: None,
            outdated: false,
        }])
    }

    async fn install_plugin(&self, profile: &str, spec: &str) -> CoreResult<()> {
        if let Some(message) = &self.install_error {
            return Err(CoreError::InvalidState(message.clone()));
        }
        self.installed
            .lock()
            .unwrap()
            .push(format!("{profile}:{spec}"));
        Ok(())
    }

    async fn remove_plugin(&self, profile: &str, id: &str) -> CoreResult<()> {
        if let Some(message) = &self.remove_error {
            return Err(CoreError::InvalidState(message.clone()));
        }
        self.removed.lock().unwrap().push(format!("{profile}:{id}"));
        Ok(())
    }

    async fn update_plugin(&self, profile: &str, id: Option<&str>) -> CoreResult<()> {
        if let Some(message) = &self.update_error {
            return Err(CoreError::InvalidState(message.clone()));
        }
        match id {
            Some(id) => self.updated.lock().unwrap().push(format!("{profile}:{id}")),
            None => self.updated.lock().unwrap().push(profile.to_string()),
        }
        Ok(())
    }

    async fn search_plugins(&self, query: &str) -> CoreResult<Vec<MarketEntry>> {
        if let Some(message) = &self.search_error {
            return Err(CoreError::InvalidState(message.clone()));
        }
        Ok(self
            .market_entries
            .iter()
            .filter(|entry| entry.id.contains(query))
            .cloned()
            .collect())
    }

    async fn market_sources(&self) -> CoreResult<Vec<MarketSourceInfo>> {
        Ok(self.market_sources.clone())
    }

    async fn doctor(&self) -> CoreResult<DoctorReport> {
        if self.doctor.adapter_id.is_empty() {
            return Err(self.unsupported("doctor"));
        }
        Ok(self.doctor.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::AdapterRegistry;

    #[tokio::test]
    async fn fake_adapter_reports_healthy_detection() {
        let adapter = FakeAdapter::healthy();
        let detection = adapter.detect().await.unwrap();
        assert!(detection.found);
        assert_eq!(detection.harness.unwrap().id, "test");
    }

    #[tokio::test]
    async fn registry_returns_adapter_by_id() {
        let mut registry = AdapterRegistry::new();
        registry.register(Box::new(FakeAdapter::healthy()));
        assert_eq!(registry.len(), 1);
        assert!(registry.get("test").is_some());
        assert!(registry.get("missing").is_none());
    }
}
