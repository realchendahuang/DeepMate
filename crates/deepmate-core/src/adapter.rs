use async_trait::async_trait;
use serde::{Deserialize, Serialize};

use crate::error::CoreResult;
use crate::model::{
    DoctorReport, MarketEntry, MarketSourceInfo, Model, Plugin, Profile, Provider, RuntimeStatus,
};

// Static metadata for an adapter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterMetadata {
    pub id: String,
    pub name: String,
    pub version: String,
}

// Capability flags an adapter declares.
//
// The UI and CLI use this to show only operations the active harness actually
// supports.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdapterCapabilities {
    pub runtime: bool,
    pub profiles: bool,
    pub providers: bool,
    pub models: bool,
    pub plugins: bool,
    pub marketplace: bool,
    pub skills: bool,
    pub mcp: bool,
    pub snapshots: bool,
}

// Result of a harness detection attempt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Detection {
    pub found: bool,
    pub harness: Option<crate::model::HarnessInfo>,
    pub detail: Option<String>,
}

// The central adapter contract.
//
// Harness-specific behavior belongs behind this trait. The core, UI and CLI
// should not depend on DeepSeek Harness file paths or commands directly.
#[async_trait]
pub trait HarnessAdapter: Send + Sync {
    fn metadata(&self) -> AdapterMetadata;

    fn capabilities(&self) -> AdapterCapabilities;

    async fn detect(&self) -> CoreResult<Detection>;

    async fn status(&self) -> CoreResult<RuntimeStatus>;

    async fn start(&self) -> CoreResult<()> {
        Err(crate::error::CoreError::Unsupported(format!(
            "{} does not implement start()",
            self.metadata().id
        )))
    }

    async fn stop(&self) -> CoreResult<()> {
        Err(crate::error::CoreError::Unsupported(format!(
            "{} does not implement stop()",
            self.metadata().id
        )))
    }

    async fn restart(&self) -> CoreResult<()> {
        Err(crate::error::CoreError::Unsupported(format!(
            "{} does not implement restart()",
            self.metadata().id
        )))
    }

    async fn open_ui(&self) -> CoreResult<()> {
        Err(crate::error::CoreError::Unsupported(format!(
            "{} does not implement open_ui()",
            self.metadata().id
        )))
    }

    async fn profiles(&self) -> CoreResult<Vec<Profile>> {
        Ok(Vec::new())
    }

    async fn providers(&self) -> CoreResult<Vec<Provider>> {
        Ok(Vec::new())
    }

    async fn models(&self) -> CoreResult<Vec<Model>> {
        Ok(Vec::new())
    }

    async fn plugins(&self) -> CoreResult<Vec<Plugin>> {
        Ok(Vec::new())
    }

    // Install one plugin into a profile. `spec` is a package name with an
    // optional version range, in the harness's own plugin-install syntax.
    async fn install_plugin(&self, _profile: &str, _spec: &str) -> CoreResult<()> {
        Err(crate::error::CoreError::Unsupported(format!(
            "{} does not implement install_plugin()",
            self.metadata().id
        )))
    }

    // Remove one installed plugin from a profile.
    async fn remove_plugin(&self, _profile: &str, _id: &str) -> CoreResult<()> {
        Err(crate::error::CoreError::Unsupported(format!(
            "{} does not implement remove_plugin()",
            self.metadata().id
        )))
    }

    // Update the plugins of a profile; a single package when `id` is given,
    // every plugin otherwise.
    async fn update_plugin(&self, _profile: &str, _id: Option<&str>) -> CoreResult<()> {
        Err(crate::error::CoreError::Unsupported(format!(
            "{} does not implement update_plugin()",
            self.metadata().id
        )))
    }

    // Search the market for plugins matching `query`.
    async fn search_plugins(&self, _query: &str) -> CoreResult<Vec<MarketEntry>> {
        Err(crate::error::CoreError::Unsupported(format!(
            "{} does not implement search_plugins()",
            self.metadata().id
        )))
    }

    // List the market sources this adapter can discover plugins from.
    async fn market_sources(&self) -> CoreResult<Vec<MarketSourceInfo>> {
        Ok(Vec::new())
    }

    async fn doctor(&self) -> CoreResult<DoctorReport>;
}
