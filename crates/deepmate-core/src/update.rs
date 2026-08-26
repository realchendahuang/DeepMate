// Update availability comparison.
//
// A small, pure helper shared by surfaces that check for new DeepMate
// releases — the desktop update check today, `deepmate doctor` later. Only
// the comparison lives here; the network fetch belongs to the caller.

use semver::Version;

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

#[cfg(test)]
mod tests {
    use super::is_newer_version;

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
}
