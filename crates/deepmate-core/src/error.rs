use std::io;
use std::path::Path;

// Errors produced by the DeepMate core. The payload of every variant carries
// the full human-readable message so Display stays the single source of
// truth for prose; the variant itself classifies the failure mode.
#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("unsupported operation: {0}")]
    Unsupported(String),

    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    // The same failure as `Io`, but carrying the path that failed. Reported
    // by the data stores so the message names the file the user must look at
    // instead of a bare OS errno.
    #[error("I/O error at {path}: {source}")]
    IoAt {
        path: String,
        #[source]
        source: io::Error,
    },

    // Something the caller asked for does not exist: the harness CLI, a
    // plugin, a profile.
    #[error("{0}")]
    NotFound(String),

    // Launching the harness child process failed before it could run.
    #[error("{0}")]
    SpawnFailed(String),

    // A bounded operation (scenario boot, plugin op, task run) hit its
    // deadline and was stopped.
    #[error("{0}")]
    Timeout(String),

    // The harness child ran and failed: nonzero exit or a lost wait.
    #[error("{0}")]
    CommandFailed(String),

    // A remote request (market search, curated list, registry) failed to
    // complete: connection, timeout or a non-success HTTP status.
    #[error("{0}")]
    Network(String),

    #[error("invalid state: {0}")]
    InvalidState(String),
}

impl CoreError {
    // A stable snake_case classification of the failure. The desktop layer
    // serializes this alongside the message (`{"code":..,"message":..}`) so
    // the frontend can map exact codes to localized text instead of
    // pattern-matching prose.
    pub fn code(&self) -> &'static str {
        match self {
            Self::Unsupported(_) => "unsupported",
            Self::Io(_) | Self::IoAt { .. } => "io",
            Self::NotFound(_) => "not_found",
            Self::SpawnFailed(_) => "spawn_failed",
            Self::Timeout(_) => "timeout",
            Self::CommandFailed(_) => "command_failed",
            Self::Network(_) => "network",
            Self::InvalidState(_) => "invalid_state",
        }
    }

    // Attach a path to a raw I/O failure so the message says which file the
    // operation touched.
    pub fn io_at(path: impl AsRef<Path>, source: io::Error) -> Self {
        Self::IoAt {
            path: path.as_ref().display().to_string(),
            source,
        }
    }
}

pub type CoreResult<T> = Result<T, CoreError>;
