// DeepMate core domain types and data layer.
//
// This crate intentionally has no UI, no CLI and no platform-specific code.
// It is the shared foundation used by the desktop app and the CLI: normalized
// domain models, the on-disk data layout, configuration, snapshots, backups
// and release-update helpers. Harness-specific behavior lives in the
// the harness service crate, never here.

pub mod backup;
pub mod data;
pub mod error;
pub mod fsutil;
pub mod model;
pub mod snapshot;
pub mod update;

pub use backup::ConfigBackup;
pub use data::{ActionRecord, Config, DataLayout, History};
pub use error::{CoreError, CoreResult};
pub use fsutil::{quarantine, write_atomic, write_atomic_string};
pub use model::{
    CheckStatus, CompatReport, CompatStatus, Detection, DoctorCheck, DoctorReport, FixReport,
    HarnessInfo, Model, Plugin, Profile, Provider, RuntimeStatus, RuntimeStatusKind,
};
pub use snapshot::{
    validate_snapshot_name, Snapshot, SnapshotReport, SnapshotStore, SNAPSHOT_FORMAT,
};
pub use update::{
    is_newer_version, parse_checksum_file, parse_checksum_for, pick_release_asset, target_triple,
    verify_sha256, AssetKind, ReleaseAsset,
};
