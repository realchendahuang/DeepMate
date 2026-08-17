// DeepMate core domain types, adapter contract and registry.
//
// This crate intentionally has no UI, no CLI and no platform-specific code.
// It is the shared foundation used by the desktop app, CLI and adapters.

pub mod adapter;
pub mod error;
pub mod model;
pub mod registry;
pub mod testkit;

pub use adapter::{AdapterCapabilities, AdapterMetadata, Detection, HarnessAdapter};
pub use error::{CoreError, CoreResult};
pub use model::{
    CheckStatus, DoctorCheck, DoctorReport, HarnessInfo, Model, Plugin, Profile, Provider,
    RuntimeStatus, RuntimeStatusKind,
};
pub use registry::AdapterRegistry;
