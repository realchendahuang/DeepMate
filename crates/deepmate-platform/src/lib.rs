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

    // Terminate a process by pid, asking it to exit (SIGTERM on unix,
    // `taskkill /F` on windows).
    fn kill_process(&self, pid: u32) -> PlatformResult<()>;

    // Terminate a process by pid without giving it a chance to clean up.
    // Used to escalate after `kill_process` was ignored.
    fn kill_process_force(&self, pid: u32) -> PlatformResult<()>;

    // Whether a process with this pid currently exists. Callers use it to
    // wait for a stop to take effect and to notice a stale pid file; the
    // answer is a snapshot, so a pid can disappear immediately afterwards.
    fn process_alive(&self, pid: u32) -> bool;

    // The command line of a running process, when the OS reports it. Callers
    // compare it against the harness they expect before signalling a pid they
    // read from a file, so a recycled pid never gets killed by mistake.
    fn process_command(&self, pid: u32) -> Option<String>;

    // The pid of the process listening on `port`, when resolvable. Used to
    // manage a harness started outside DeepMate: the port it serves is the
    // only reliable handle on it. Best effort — None when the OS tooling is
    // absent or nothing listens.
    fn find_listener_pid(&self, port: u16) -> Option<u32>;
}

// Whether a command line looks like the harness (or a DeepMate surface owning
// it). Shared by every caller that must decide whether a pid read from a file
// or a port is safe to signal or report.
pub fn looks_like_harness(command: &str) -> bool {
    let lowered = command.to_lowercase();
    ["dsh", "deepseek-harness", "deepmate"]
        .iter()
        .any(|needle| lowered.contains(needle))
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

    fn kill_process_force(&self, pid: u32) -> PlatformResult<()> {
        #[cfg(target_os = "windows")]
        let status = Command::new("taskkill")
            .args(["/PID", &pid.to_string(), "/F"])
            .status();

        #[cfg(not(target_os = "windows"))]
        let status = Command::new("kill").args(["-9", &pid.to_string()]).status();

        match status {
            Ok(status) if status.success() => Ok(()),
            Ok(status) => Err(PlatformError::Unsupported(format!(
                "kill -9 exited with {status:?}"
            ))),
            Err(source) => Err(PlatformError::Kill { pid, source }),
        }
    }

    fn process_alive(&self, pid: u32) -> bool {
        if pid == 0 {
            return false;
        }
        #[cfg(unix)]
        {
            // Signal 0 performs the permission and existence checks without
            // delivering a signal. EPERM means the process exists but belongs
            // to someone else, which still counts as alive.
            let result = unsafe { libc::kill(pid as i32, 0) };
            if result == 0 {
                return true;
            }
            std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
        }
        #[cfg(target_os = "windows")]
        {
            let Ok(output) = Command::new("tasklist")
                .args(["/FI", &format!("PID eq {pid}"), "/NH"])
                .output()
            else {
                return false;
            };
            let text = String::from_utf8_lossy(&output.stdout);
            text.contains(&pid.to_string())
        }
    }

    fn process_command(&self, pid: u32) -> Option<String> {
        if pid == 0 {
            return None;
        }
        #[cfg(target_os = "windows")]
        {
            let output = Command::new("wmic")
                .args([
                    "process",
                    "where",
                    &format!("ProcessId={pid}"),
                    "get",
                    "CommandLine",
                    "/value",
                ])
                .output()
                .ok()?;
            let text = String::from_utf8_lossy(&output.stdout);
            let command = text
                .lines()
                .find_map(|line| line.strip_prefix("CommandLine="))?
                .trim()
                .to_string();
            (!command.is_empty()).then_some(command)
        }
        #[cfg(not(target_os = "windows"))]
        {
            let output = Command::new("ps")
                .args(["-o", "command=", "-p", &pid.to_string()])
                .output()
                .ok()?;
            if !output.status.success() {
                return None;
            }
            let text = String::from_utf8_lossy(&output.stdout).trim().to_string();
            (!text.is_empty()).then_some(text)
        }
    }

    // The pid of a process listening on `port`, when it can be resolved
    // unambiguously.
    //
    // `Some(pid)` is a promise: the caller kills this pid to stop whatever
    // serves the port, so the resolver must never guess. A port shared by
    // several processes (SO_REUSEPORT, or an unrelated squatter next to the
    // harness) is reported as unresolved rather than as an arbitrary
    // candidate.
    fn find_listener_pid(&self, port: u16) -> Option<u32> {
        #[cfg(not(target_os = "windows"))]
        {
            // lsof prints one pid per matching socket (IPv4 and IPv6 each
            // yield a line), so the same pid usually repeats.
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
            pids.sort_unstable();
            pids.dedup();
            match pids.as_slice() {
                [pid] => Some(*pid),
                _ => None,
            }
        }
        #[cfg(target_os = "windows")]
        {
            // netstat lines read: "TCP 127.0.0.1:3080 0.0.0.0:0 LISTENING 11635".
            let output = Command::new("netstat")
                .args(["-ano", "-p", "tcp"])
                .output()
                .ok()?;
            let mut pids: Vec<u32> = String::from_utf8_lossy(&output.stdout)
                .lines()
                .filter_map(|line| {
                    let fields: Vec<&str> = line.split_whitespace().collect();
                    // Match the local-address column exactly: a substring
                    // search would treat `:30800` as a hit for `:3080`.
                    let local = fields.get(1)?;
                    let matches_port = local
                        .rsplit_once(':')
                        .and_then(|(_, candidate)| candidate.parse::<u16>().ok())
                        == Some(port);
                    if !matches_port || !line.contains("LISTENING") {
                        return None;
                    }
                    fields.last()?.parse().ok()
                })
                .collect();
            pids.sort_unstable();
            pids.dedup();
            match pids.as_slice() {
                [pid] => Some(*pid),
                _ => None,
            }
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

    #[cfg(not(target_os = "windows"))]
    #[test]
    fn an_idle_port_resolves_to_nothing() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        assert_eq!(SystemPlatform.find_listener_pid(port), None);
    }
}
