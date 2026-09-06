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

    // The pid of the process listening on `port`, when resolvable. Used to
    // manage a harness started outside DeepMate: the port it serves is the
    // only reliable handle on it. Best effort — None when the OS tooling is
    // absent or nothing listens.
    fn find_listener_pid(&self, port: u16) -> Option<u32>;
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

    fn find_listener_pid(&self, port: u16) -> Option<u32> {
        #[cfg(not(target_os = "windows"))]
        {
            // lsof prints one pid per matching socket (IPv4 and IPv6 each
            // yield a line); dedupe and keep the last.
            let output = Command::new("lsof")
                .args(["-nP", &format!("-iTCP:{port}"), "-sTCP:LISTEN", "-t"])
                .output()
                .ok()?;
            if !output.status.success() {
                return None;
            }
            let mut pids: Vec<u32> = String::from_utf8_lossy(&output.stdout)
                .lines()
                .map(str::trim)
                .filter_map(|line| line.parse().ok())
                .collect();
            pids.dedup();
            pids.into_iter().next_back()
        }
        #[cfg(target_os = "windows")]
        {
            // netstat lines read: "TCP 127.0.0.1:3080 0.0.0.0:0 LISTENING 11635".
            let output = Command::new("netstat")
                .args(["-ano", "-p", "tcp"])
                .output()
                .ok()?;
            let needle = format!(":{port}");
            String::from_utf8_lossy(&output.stdout)
                .lines()
                .filter(|line| line.contains(&needle) && line.contains("LISTENING"))
                .filter_map(|line| line.split_whitespace().last()?.parse().ok())
                .next_back()
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

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn resolves_the_pid_owning_a_listening_port() {
        // lsof is the resolver; skip where it is absent.
        if Command::new("lsof").arg("-v").output().is_err() {
            eprintln!("lsof unavailable; skipping");
            return;
        }
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        assert_eq!(
            SystemPlatform.find_listener_pid(port),
            Some(std::process::id())
        );
    }
}
