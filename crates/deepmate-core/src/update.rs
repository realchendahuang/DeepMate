// Update availability comparison.
//
// A small, pure helper shared by surfaces that check for new DeepMate
// releases — the desktop update check today, `deepmate doctor` later. Only
// the comparison lives here; the network fetch belongs to the caller.
//
// The self-update flow also lives here as pure logic: release assets are
// picked by target triple, and downloaded artifacts are verified against the
// published `.sha256` before anything is opened or replaced.

use std::path::Path;

use semver::Version;
use sha2::{Digest, Sha256};

use crate::error::{CoreError, CoreResult};

// True when `latest` is a strictly newer version than `current`.
//
// Both sides are compared as semver versions, so `0.10.0` beats `0.9.9`.
// A leading `v` is tolerated on either side because release tags are
// commonly written as `v0.6.0`. Any unparsable value is treated as "not
// newer": an update check must never claim an upgrade it cannot prove.
pub fn is_newer_version(latest: &str, current: &str) -> bool {
    let latest = parse_version(latest);
    let current = parse_version(current);
    match (latest, current) {
        (Some(latest), Some(current)) => latest > current,
        _ => false,
    }
}

fn parse_version(value: &str) -> Option<Version> {
    let trimmed = value.trim();
    let without_v = trimmed.strip_prefix('v').unwrap_or(trimmed);
    Version::parse(without_v).ok()
}

// The Rust target triple of the running binary, matching the release asset
// naming convention (`deepmate-<version>-<triple>.<ext>`).
pub fn target_triple() -> String {
    let arch = std::env::consts::ARCH;
    match std::env::consts::OS {
        "macos" => format!("{arch}-apple-darwin"),
        "linux" => format!("{arch}-unknown-linux-gnu"),
        "windows" => format!("{arch}-pc-windows-msvc"),
        os => format!("{arch}-unknown-{os}"),
    }
}

// Which release artifact a surface consumes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssetKind {
    // The desktop bundle; opened in the platform installer.
    Dmg,
    // The CLI archive; extracted for a self-replacement.
    Tarball,
}

impl AssetKind {
    fn matches(self, asset_name: &str, triple: &str) -> bool {
        let suffix = match self {
            AssetKind::Dmg => format!("-{triple}.dmg"),
            AssetKind::Tarball => format!("-{triple}.tar.gz"),
        };
        asset_name.ends_with(&suffix)
    }

    // The published checksum sits next to the asset as `<name>.sha256`.
    fn checksum_name(self, asset_name: &str) -> String {
        format!("{asset_name}.sha256")
    }
}

// One release asset offered by GitHub: its file name and download URL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReleaseAsset {
    pub name: String,
    pub url: String,
}

// The asset of a release matching the running platform, plus its checksum
// sidecar. `None` means the release carries no artifact for this triple —
// the local host only ever produces macOS arm64, so other platforms must
// fail with a clear signal instead of downloading the wrong file.
pub fn pick_release_asset(
    assets: &[ReleaseAsset],
    triple: &str,
    kind: AssetKind,
) -> Option<(ReleaseAsset, ReleaseAsset)> {
    let asset = assets
        .iter()
        .find(|asset| kind.matches(&asset.name, triple))?;
    let checksum_name = kind.checksum_name(&asset.name);
    let checksum = assets
        .iter()
        .find(|asset| asset.name == checksum_name)
        .cloned()?;
    Some((asset.clone(), checksum))
}

// The hex digest a published `.sha256` file declares. The format is
// `<hash>  <filename>` (two spaces, `shasum -a 256` output); anything after
// the first whitespace-delimited token is ignored.
pub fn parse_checksum_file(text: &str) -> Option<String> {
    let token = text.split_whitespace().next()?;
    let hex = token.to_lowercase();
    if hex.len() == 64 && hex.chars().all(|c| c.is_ascii_hexdigit()) {
        Some(hex)
    } else {
        None
    }
}

// The lowercase SHA-256 hex digest of a file, streamed.
pub fn sha256_file(path: &Path) -> CoreResult<String> {
    use std::io::Read;
    let mut file = std::fs::File::open(path)
        .map_err(|err| CoreError::InvalidState(format!("cannot open {path:?}: {err}")))?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|err| CoreError::InvalidState(format!("cannot read {path:?}: {err}")))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

// Verify a downloaded artifact against the expected hex digest.
pub fn verify_sha256(path: &Path, expected: &str) -> CoreResult<()> {
    let actual = sha256_file(path)?;
    if actual == expected.to_lowercase() {
        Ok(())
    } else {
        Err(crate::error::CoreError::InvalidState(format!(
            "checksum mismatch for {}: expected {expected}, got {actual}",
            path.display()
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::{
        is_newer_version, parse_checksum_file, pick_release_asset, target_triple, verify_sha256,
        AssetKind, ReleaseAsset,
    };

    #[test]
    fn newer_version_is_reported() {
        assert!(is_newer_version("0.6.0", "0.5.0"));
    }

    #[test]
    fn leading_v_is_tolerated_on_either_side() {
        assert!(is_newer_version("v0.6.0", "0.5.0"));
        assert!(is_newer_version("0.6.0", "v0.5.0"));
    }

    #[test]
    fn equal_or_older_versions_are_not_newer() {
        assert!(!is_newer_version("0.6.0", "0.6.0"));
        assert!(!is_newer_version("0.5.0", "0.6.0"));
    }

    #[test]
    fn comparison_is_numeric_not_lexicographic() {
        assert!(is_newer_version("0.10.0", "0.9.9"));
    }

    #[test]
    fn unparsable_values_are_never_newer() {
        assert!(!is_newer_version("garbage", "0.5.0"));
        assert!(!is_newer_version("0.6.0", ""));
        assert!(!is_newer_version("", "0.5.0"));
    }

    fn fixture_assets() -> Vec<ReleaseAsset> {
        let base = "https://example.com";
        vec![
            ReleaseAsset {
                name: "deepmate-0.7.0-aarch64-apple-darwin.dmg".to_string(),
                url: format!("{base}/deepmate-0.7.0-aarch64-apple-darwin.dmg"),
            },
            ReleaseAsset {
                name: "deepmate-0.7.0-aarch64-apple-darwin.dmg.sha256".to_string(),
                url: format!("{base}/deepmate-0.7.0-aarch64-apple-darwin.dmg.sha256"),
            },
            ReleaseAsset {
                name: "deepmate-0.7.0-aarch64-apple-darwin.tar.gz".to_string(),
                url: format!("{base}/deepmate-0.7.0-aarch64-apple-darwin.tar.gz"),
            },
            ReleaseAsset {
                name: "deepmate-0.7.0-aarch64-apple-darwin.tar.gz.sha256".to_string(),
                url: format!("{base}/deepmate-0.7.0-aarch64-apple-darwin.tar.gz.sha256"),
            },
        ]
    }

    #[test]
    fn assets_are_picked_by_triple_and_kind() {
        let assets = fixture_assets();
        let triple = "aarch64-apple-darwin";
        let (dmg, dmg_sum) =
            pick_release_asset(&assets, triple, AssetKind::Dmg).expect("dmg asset");
        assert!(dmg.name.ends_with(".dmg"));
        assert!(dmg_sum.name.ends_with(".dmg.sha256"));

        let (tar, tar_sum) =
            pick_release_asset(&assets, triple, AssetKind::Tarball).expect("tarball asset");
        assert!(tar.name.ends_with(".tar.gz"));
        assert!(tar_sum.name.ends_with(".tar.gz.sha256"));
    }

    #[test]
    fn unknown_triples_have_no_asset() {
        assert!(pick_release_asset(
            &fixture_assets(),
            "x86_64-unknown-linux-gnu",
            AssetKind::Dmg
        )
        .is_none());
    }

    #[test]
    fn a_missing_checksum_sidecar_is_no_asset() {
        let mut assets = fixture_assets();
        assets.retain(|asset| !asset.name.ends_with(".dmg.sha256"));
        assert!(pick_release_asset(&assets, "aarch64-apple-darwin", AssetKind::Dmg).is_none());
    }

    #[test]
    fn target_triple_matches_the_asset_naming() {
        let triple = target_triple();
        assert!(
            pick_release_asset(&fixture_assets(), &triple, AssetKind::Tarball).is_some()
                == (triple == "aarch64-apple-darwin")
        );
    }

    #[test]
    fn checksum_files_are_parsed_conservatively() {
        assert_eq!(
            parse_checksum_file("76d118ccbe166092fece8d8b6b65837a5ab54bb44bf88fd294e21e8f25fff44e  deepmate-0.7.0-aarch64-apple-darwin.dmg\n"),
            Some("76d118ccbe166092fece8d8b6b65837a5ab54bb44bf88fd294e21e8f25fff44e".to_string())
        );
        assert_eq!(parse_checksum_file("not-a-hash"), None);
        assert_eq!(parse_checksum_file(""), None);
    }

    #[test]
    fn checksum_verification_accepts_and_rejects() {
        let dir = std::env::temp_dir().join(format!("deepmate-update-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("artifact");
        std::fs::write(&path, b"deepmate").unwrap();
        let digest = super::sha256_file(&path).unwrap();
        verify_sha256(&path, &digest).unwrap();
        assert!(verify_sha256(&path, &"0".repeat(64)).is_err());
    }
}
