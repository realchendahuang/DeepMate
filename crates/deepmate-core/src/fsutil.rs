// Filesystem helpers shared by the core's persistent stores.
//
// DeepMate-owned files are small but load-bearing: a half-written config or
// registry must never be observable, and a file that fails to parse must be
// recoverable rather than silently destroyed.

use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::error::{CoreError, CoreResult};

// Write `contents` to `path` atomically: a sibling temporary file is written,
// flushed to disk, then renamed over the target. Readers therefore never see
// a partially written file, and a crash mid-write leaves the previous
// contents intact.
pub fn write_atomic(path: &Path, contents: &[u8]) -> CoreResult<()> {
    let parent = path.parent().filter(|p| !p.as_os_str().is_empty());
    if let Some(parent) = parent {
        fs::create_dir_all(parent).map_err(|err| CoreError::io_at(parent, err))?;
    }
    let temp = temp_sibling(path);
    let result = (|| -> CoreResult<()> {
        let mut file = fs::File::create(&temp).map_err(|err| CoreError::io_at(&temp, err))?;
        file.write_all(contents)
            .map_err(|err| CoreError::io_at(&temp, err))?;
        file.sync_all()
            .map_err(|err| CoreError::io_at(&temp, err))?;
        Ok(())
    })();
    if let Err(err) = result {
        let _ = fs::remove_file(&temp);
        return Err(err);
    }
    fs::rename(&temp, path).map_err(|err| {
        let _ = fs::remove_file(&temp);
        CoreError::io_at(path, err)
    })?;
    // Best effort: make the rename itself durable. Directory fsync is not
    // available everywhere, so a failure here is not an error.
    if let Some(parent) = parent {
        if let Ok(dir) = fs::File::open(parent) {
            let _ = dir.sync_all();
        }
    }
    Ok(())
}

pub fn write_atomic_string(path: &Path, contents: &str) -> CoreResult<()> {
    write_atomic(path, contents.as_bytes())
}

// A unique sibling path used as the temporary target of an atomic write.
fn temp_sibling(path: &Path) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|span| span.as_nanos())
        .unwrap_or(0);
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("file");
    path.with_file_name(format!(".{name}.tmp-{}-{stamp}", std::process::id()))
}

// Move `path` aside to a timestamped sibling (`<name>.invalid-<stamp>`),
// returning the new location. Corrupted files are quarantined instead of
// being overwritten or deleted, so the previous contents remain recoverable
// while the app continues from a fresh file.
pub fn quarantine(path: &Path) -> CoreResult<PathBuf> {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|span| span.as_secs())
        .unwrap_or(0);
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("file");
    let target = path.with_file_name(format!("{name}.invalid-{stamp}"));
    fs::rename(path, &target).map_err(|err| CoreError::io_at(path, err))?;
    Ok(target)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static TEST_DIR_SEQ: AtomicU64 = AtomicU64::new(0);

    fn temp_dir() -> PathBuf {
        let seq = TEST_DIR_SEQ.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("deepmate-fsutil-test-{}-{seq}", std::process::id()))
    }

    #[test]
    fn atomic_write_replaces_contents_and_creates_parents() {
        let dir = temp_dir();
        let path = dir.join("nested").join("file.txt");
        write_atomic_string(&path, "first").unwrap();
        write_atomic_string(&path, "second").unwrap();
        assert_eq!(fs::read_to_string(&path).unwrap(), "second");
        // No temporary residue is left behind.
        let leftovers: Vec<_> = fs::read_dir(path.parent().unwrap())
            .unwrap()
            .flatten()
            .filter(|entry| entry.file_name().to_string_lossy().starts_with('.'))
            .collect();
        assert!(
            leftovers.is_empty(),
            "temp files left behind: {leftovers:?}"
        );
    }

    #[test]
    fn quarantine_moves_the_file_and_keeps_its_bytes() {
        let dir = temp_dir();
        fs::create_dir_all(&dir).unwrap();
        let path = dir.join("config.toml");
        fs::write(&path, "broken [").unwrap();
        let moved = quarantine(&path).unwrap();
        assert!(!path.exists());
        assert_eq!(fs::read_to_string(&moved).unwrap(), "broken [");
        assert!(moved
            .file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("config.toml.invalid-"));
    }
}
