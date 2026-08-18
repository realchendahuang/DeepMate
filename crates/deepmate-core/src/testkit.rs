// Test doubles and helpers for writing adapter/core tests without a real
// harness installed.
//
// This module is intentionally public: CLI integration tests and local debug
// commands can use the same fake adapter to get deterministic behaviour.

use async_trait::async_trait;
use std::sync::atomic::{AtomicU32, Ordering};

use crate::adapter::{AdapterCapabilities, AdapterMetadata, Detection, HarnessAdapter};
use crate::error::{CoreError, CoreResult};
use crate::model::{
    CheckStatus, DoctorCheck, DoctorReport, HarnessInfo, Model, Plugin, Profile, Provider,
    RuntimeStatus, RuntimeStatusKind,
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
        }])
    }

    async fn models(&self) -> CoreResult<Vec<Model>> {
        Ok(vec![Model {
            id: "demo-chat".to_string(),
            name: "Demo Chat".to_string(),
            provider: Some("demo".to_string()),
        }])
    }

    async fn plugins(&self) -> CoreResult<Vec<Plugin>> {
        Ok(vec![Plugin {
            id: "fake-plugin".to_string(),
            name: "Fake Plugin".to_string(),
            version: Some("1.0.0".to_string()),
            enabled: true,
        }])
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
