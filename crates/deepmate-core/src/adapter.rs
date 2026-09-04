use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use specta::Type;

use crate::error::CoreResult;
use crate::model::{
    CompatReport, DoctorReport, MarketEntry, MarketSourceInfo, Model, Plugin, Profile, Provider,
    RuntimeStatus,
};

// Static metadata for an adapter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct AdapterMetadata {
    pub id: String,
    pub name: String,
    pub version: String,
}

// Capability flags an adapter declares.
//
// The UI and CLI use this to show only operations the active harness actually
// supports.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
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

    // Create or update a provider. When a provider with the same id already
    // exists its fields are overwritten; otherwise it is added.
    async fn upsert_provider(&self, _provider: Provider) -> CoreResult<()> {
        Err(crate::error::CoreError::Unsupported(format!(
            "{} does not implement upsert_provider()",
            self.metadata().id
        )))
    }

    // Remove a provider by id.
    async fn remove_provider(&self, _id: &str) -> CoreResult<()> {
        Err(crate::error::CoreError::Unsupported(format!(
            "{} does not implement remove_provider()",
            self.metadata().id
        )))
    }

    // Create or update a model within `provider`'s catalog. When a model with
    // the same id already exists its fields are overwritten; otherwise it is
    // added.
    async fn upsert_model(&self, _provider: &str, _model: Model) -> CoreResult<()> {
        Err(crate::error::CoreError::Unsupported(format!(
            "{} does not implement upsert_model()",
            self.metadata().id
        )))
    }

    // Remove a model by id from `provider`'s catalog.
    async fn remove_model(&self, _provider: &str, _id: &str) -> CoreResult<()> {
        Err(crate::error::CoreError::Unsupported(format!(
            "{} does not implement remove_model()",
            self.metadata().id
        )))
    }

    // Create a new profile with the given name.
    async fn create_profile(&self, _name: &str) -> CoreResult<()> {
        Err(crate::error::CoreError::Unsupported(format!(
            "{} does not implement create_profile()",
            self.metadata().id
        )))
    }

    // Remove a profile by name.
    async fn remove_profile(&self, _name: &str) -> CoreResult<()> {
        Err(crate::error::CoreError::Unsupported(format!(
            "{} does not implement remove_profile()",
            self.metadata().id
        )))
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

    // Run a plugin operation while streaming progress events to `tx`.
    //
    // The default implementation wraps the blocking `install_plugin` /
    // `remove_plugin` / `update_plugin` calls with `Started` and `Finished`
    // events, so every adapter produces a well-formed stream. Adapters that
    // can observe the harness process live (e.g. the DeepSeek Harness CLI)
    // override this to also emit `Line` events from the child's output.
    async fn stream_plugin_op(
        &self,
        profile: &str,
        kind: crate::model::PluginOpKind,
        target: Option<&str>,
        tx: tokio::sync::mpsc::Sender<crate::model::PluginOpEvent>,
    ) -> CoreResult<()> {
        use crate::model::{PluginOpEvent, PluginOpKind};
        let target = target.unwrap_or("").to_string();
        let _ = tx
            .send(PluginOpEvent::Started {
                op: kind,
                target: target.clone(),
            })
            .await;
        let result = match kind {
            PluginOpKind::Install => self.install_plugin(profile, &target).await,
            PluginOpKind::Remove => self.remove_plugin(profile, &target).await,
            PluginOpKind::Update => self.update_plugin(profile, Some(&target)).await,
        };
        match result {
            Ok(()) => {
                let _ = tx
                    .send(PluginOpEvent::Finished {
                        ok: true,
                        detail: None,
                    })
                    .await;
                Ok(())
            }
            Err(e) => {
                let _ = tx
                    .send(PluginOpEvent::Finished {
                        ok: false,
                        detail: Some(e.to_string()),
                    })
                    .await;
                Err(e)
            }
        }
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

    // Check whether a market package is compatible with the active harness.
    //
    // `spec` is the same package-name-with-optional-range syntax accepted by
    // `install_plugin`. Adapters without a compatibility contract report
    // `Unsupported`; callers treat that like an `Unknown` verdict.
    async fn plugin_compat(&self, _spec: &str) -> CoreResult<CompatReport> {
        Err(crate::error::CoreError::Unsupported(format!(
            "{} does not implement plugin_compat()",
            self.metadata().id
        )))
    }

    async fn doctor(&self) -> CoreResult<DoctorReport>;
}
