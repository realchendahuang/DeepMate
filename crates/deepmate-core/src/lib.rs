// DeepMate core domain types, adapter contract and registry.
//
// This crate intentionally has no UI, no CLI and no platform-specific code.
// It is the shared foundation used by the desktop app, CLI and adapters.

pub mod adapter;
pub mod backup;
pub mod data;
pub mod error;
pub mod model;
pub mod registry;
pub mod snapshot;
pub mod testkit;
pub mod update;

pub use adapter::{AdapterCapabilities, AdapterMetadata, Detection, HarnessAdapter};
pub use backup::ConfigBackup;
pub use data::{ActionRecord, Config, DataLayout, History};
pub use error::{CoreError, CoreResult};
pub use model::{
    CheckStatus, CompatReport, CompatStatus, DoctorCheck, DoctorReport, HarnessInfo, Model, Plugin,
    Profile, Provider, RuntimeStatus, RuntimeStatusKind,
};
pub use registry::AdapterRegistry;
pub use snapshot::{Snapshot, SnapshotReport, SnapshotStore};
pub use update::is_newer_version;
