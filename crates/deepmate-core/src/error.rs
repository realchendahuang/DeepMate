use std::io;

// Errors produced by the DeepMate core.
#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("adapter not found: {0}")]
    AdapterNotFound(String),

    #[error("unsupported operation: {0}")]
    Unsupported(String),

    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    #[error("invalid state: {0}")]
    InvalidState(String),
}

pub type CoreResult<T> = Result<T, CoreError>;
