// Platform abstraction for DeepMate.
//
// OS-specific behavior should stay behind this boundary so core business logic
// remains portable and easy to test.

use std::path::{Path, PathBuf};
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

    #[error("failed to kill process {pid}: {source}")]
    Kill {
        pid: u32,
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

    // The DeepMate data directory for this platform.
    //
    // DEEPMATE_DATA_DIR overrides the OS convention, which keeps setups
    // portable and tests deterministic.
    fn data_dir(&self) -> PlatformResult<PathBuf>;

    // Terminate a process by pid.
    fn kill_process(&self, pid: u32) -> PlatformResult<()>;
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

    fn data_dir(&self) -> PlatformResult<PathBuf> {
        if let Ok(dir) = std::env::var("DEEPMATE_DATA_DIR") {
            if !dir.is_empty() {
                return Ok(PathBuf::from(dir));
            }
        }
        #[cfg(target_os = "macos")]
        {
            if let Some(home) = std::env::var_os("HOME") {
                return Ok(PathBuf::from(home).join("Library/Application Support/DeepMate"));
            }
        }
        #[cfg(target_os = "windows")]
        {
            if let Some(appdata) = std::env::var_os("APPDATA") {
                return Ok(PathBuf::from(appdata).join("DeepMate"));
            }
        }
        #[cfg(all(unix, not(target_os = "macos")))]
        {
            if let Some(xdg) = std::env::var_os("XDG_DATA_HOME") {
                return Ok(PathBuf::from(xdg).join("deepmate"));
            }
            if let Some(home) = std::env::var_os("HOME") {
                return Ok(PathBuf::from(home).join(".local/share/deepmate"));
            }
        }
        Err(PlatformError::Unsupported(
            "no data directory convention for this platform".to_string(),
        ))
    }

    fn kill_process(&self, pid: u32) -> PlatformResult<()> {
        #[cfg(target_os = "windows")]
        let status = Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/F"])
            .status();

        #[cfg(not(target_os = "windows"))]
        let status = Command::new("kill").arg(pid.to_string()).status();

        match status {
            Ok(status) if status.success() => Ok(()),
            Ok(status) => Err(PlatformError::Unsupported(format!(
                "kill exited with {status:?}"
            ))),
            Err(source) => Err(PlatformError::Kill { pid, source }),
        }
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

    #[test]
    fn data_dir_honors_override() {
        let previous = std::env::var("DEEPMATE_DATA_DIR").ok();
        std::env::set_var("DEEPMATE_DATA_DIR", "/tmp/deepmate-test-data");
        let dir = SystemPlatform.data_dir().unwrap();
        assert_eq!(dir, PathBuf::from("/tmp/deepmate-test-data"));
        match previous {
            Some(value) => std::env::set_var("DEEPMATE_DATA_DIR", value),
            None => std::env::remove_var("DEEPMATE_DATA_DIR"),
        }
    }
}
