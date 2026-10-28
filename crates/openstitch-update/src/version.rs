//! The running version and the tag grammar.
//!
//! The workspace version (`CARGO_PKG_VERSION`, `0.1.0`) carries no pre-release, so a release
//! build is identified by the tag it was built from, which `build-executables.yml` passes to
//! cargo as `OPENSTITCH_RELEASE_TAG`. A build without a release tag (a local `cargo build`, a
//! `main-<sha>` or `pr<N>` label) has no [`Current`] and never checks for updates.

use semver::Version;

/// The release build this is: its tag and the version parsed from it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Current {
    /// `v0.1.0-alpha.3`
    pub tag: String,
    pub version: Version,
}

/// Is `tag` a release tag (`vX.Y.Z`, optionally `-alpha.N`, `-beta.N` or `-rc.N`), as
/// `release.yml` accepts them?
pub fn is_release_tag(tag: &str) -> bool {
    parse_tag(tag).is_some()
}

/// The version a release tag names; `None` for anything that is not a release tag.
pub fn parse_tag(tag: &str) -> Option<Version> {
    let rest = tag.strip_prefix('v')?;
    let (core, pre) = match rest.split_once('-') {
        Some((c, p)) => (c, Some(p)),
        None => (rest, None),
    };
    let mut parts = core.split('.');
    let mut nums = [0u64; 3];
    for n in &mut nums {
        *n = plain_number(parts.next()?)?;
    }
    if parts.next().is_some() {
        return None;
    }
    if let Some(pre) = pre {
        let (kind, n) = pre.split_once('.')?;
        if !["alpha", "beta", "rc"].contains(&kind) {
            return None;
        }
        plain_number(n)?;
    }
    Version::parse(rest).ok()
}

/// A decimal number without sign, spaces or leading zeros (other than `0` itself).
fn plain_number(s: &str) -> Option<u64> {
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) || (s.len() > 1 && s.starts_with('0'))
    {
        return None;
    }
    s.parse().ok()
}

/// The release this binary was built from, or `None` when the build carried no release tag.
pub fn current() -> Option<Current> {
    from_tag(option_env!("OPENSTITCH_RELEASE_TAG"))
}

fn from_tag(tag: Option<&str>) -> Option<Current> {
    let tag = tag?.trim();
    let version = parse_tag(tag)?;
    Some(Current {
        tag: tag.to_string(),
        version,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(tag: &str) -> Version {
        parse_tag(tag).expect(tag)
    }

    #[test]
    fn prerelease_ordering_follows_semver() {
        assert!(v("v0.1.0-alpha.3") < v("v0.1.0-alpha.4"));
        assert!(
            v("v0.1.0-alpha.9") < v("v0.1.0-alpha.10"),
            "numeric, not lexical"
        );
        assert!(v("v0.1.0-alpha.4") < v("v0.1.0-beta.1"));
        assert!(v("v0.1.0-beta.1") < v("v0.1.0-rc.1"));
        assert!(v("v0.1.0-rc.1") < v("v0.1.0"));
        assert!(v("v0.1.0") < v("v0.1.1"));
        assert!(v("v0.1.1") < v("v0.2.0-alpha.1"));
        assert!(v("v0.9.9") < v("v1.0.0"));
    }

    #[test]
    fn only_release_tags_parse() {
        for ok in [
            "v0.1.0",
            "v0.1.0-alpha.3",
            "v10.2.33-rc.12",
            "v0.0.0-beta.0",
        ] {
            assert!(is_release_tag(ok), "{ok}");
        }
        for bad in [
            "0.1.0",
            "v0.1",
            "v0.1.0.1",
            "v0.1.0-alpha",
            "v0.1.0-alpha.",
            "v0.1.0-alpha.x",
            "v0.1.0-gamma.1",
            "v0.1.0-alpha.1.2",
            "v01.1.0",
            "v0.1.0+build",
            "main-abc123",
            "pr12-34",
            "",
            "v",
            "v-1.0.0",
            " v0.1.0",
        ] {
            assert!(!is_release_tag(bad), "{bad:?} must not parse");
        }
    }

    #[test]
    fn current_needs_a_release_tag() {
        assert_eq!(from_tag(None), None);
        assert_eq!(from_tag(Some("")), None);
        assert_eq!(from_tag(Some("main-f584502")), None);
        assert_eq!(from_tag(Some("pr76-1200")), None);
        let c = from_tag(Some(" v0.1.0-alpha.3\n")).expect("tag");
        assert_eq!(c.tag, "v0.1.0-alpha.3");
        assert_eq!(c.version, Version::parse("0.1.0-alpha.3").expect("semver"));
    }
}
