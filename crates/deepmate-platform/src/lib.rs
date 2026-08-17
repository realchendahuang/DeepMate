// Platform abstraction for DeepMate.
//
// OS-specific behavior should stay behind this boundary so core business logic
// remains portable and easy to test.

use std::path::Path;
use std::process::Command;

// Errors produced by platform services.
#[derive(Debug, thiserror::Error)]
pub enum PlatformError {
    #[error("failed to open {target}: {source}")]
    Open {
        target: String,
        #[source]
        source: std::io::Error,
    },

    #[error("unsupported platform operation: {0}")]
    Unsupported(String),
}

pub type PlatformResult<T> = Result<T, PlatformError>;

// Platform services used by adapters and core.
pub trait PlatformService: Send + Sync {
    fn name(&self) -> &'static str;

    fn open_url(&self, url: &str) -> PlatformResult<()>;

    fn open_path(&self, path: &Path) -> PlatformResult<()>;
}

// Production implementation for the current OS.
#[derive(Debug, Default, Clone, Copy)]
pub struct SystemPlatform;

impl SystemPlatform {
    fn open_command(&self, target: &str) -> PlatformResult<()> {
        #[cfg(target_os = "macos")]
        let status = Command::new("open").arg(target).status();

        #[cfg(target_os = "windows")]
        let status = Command::new("cmd")
            .args(["/C", "start", "", target])
            .status();

        #[cfg(all(unix, not(target_os = "macos")))]
        let status = Command::new("xdg-open").arg(target).status();

        #[cfg(not(any(target_os = "macos", target_os = "windows", unix)))]
        let status = Err(std::io::Error::new(
            std::io::ErrorKind::Unsupported,
            "no opener configured for this platform",
        ));

        match status {
            Ok(status) if status.success() => Ok(()),
            Ok(status) => Err(PlatformError::Unsupported(format!(
                "opener exited with {status:?}"
            ))),
            Err(source) => Err(PlatformError::Open {
                target: target.to_string(),
                source,
            }),
        }
    }
}

impl PlatformService for SystemPlatform {
    fn name(&self) -> &'static str {
        if cfg!(target_os = "macos") {
            "macos"
        } else if cfg!(target_os = "windows") {
            "windows"
        } else {
            "linux"
        }
    }

    fn open_url(&self, url: &str) -> PlatformResult<()> {
        self.open_command(url)
    }

    fn open_path(&self, path: &Path) -> PlatformResult<()> {
        self.open_command(&path.to_string_lossy())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_platform_has_a_name() {
        let platform = SystemPlatform;
        assert!(!platform.name().is_empty());
    }
}
