//! Which release to offer, and which of its assets.

use crate::github::{Asset, Release};
use crate::install::Install;
use semver::Version;

/// A newer release than the running one.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Update {
    /// The newest eligible release.
    pub release: Release,
    /// Its asset for this install, when the release has one; `None` means the notes can be
    /// shown but nothing can be applied (no asset for this platform, or an install kind that
    /// cannot be replaced).
    pub asset: Option<Asset>,
    /// The `SHA256SUMS` asset of the release; required before anything is applied.
    pub checksums: Option<Asset>,
    /// Every eligible release newer than the running one, newest first (the first is
    /// `release`), for showing all the notes the user has not seen.
    pub notes: Vec<Release>,
}

impl Update {
    pub fn tag(&self) -> &str {
        &self.release.tag
    }

    /// Whether Update can download and apply: an asset, a checksum file, and an install kind
    /// that can be swapped.
    pub fn applicable(&self) -> bool {
        self.asset.is_some() && self.checksums.is_some()
    }
}

/// The newest release that is newer than `current`, with its asset for `install`.
///
/// Pre-releases are offered when the running build is itself a pre-release or when
/// `include_prerelease` is set; a stable build otherwise only sees stable releases.
pub fn find_update(
    releases: &[Release],
    current: &Version,
    include_prerelease: bool,
    install: &Install,
) -> Option<Update> {
    let prereleases_ok = include_prerelease || !current.pre.is_empty();
    let mut newer: Vec<&Release> = releases
        .iter()
        .filter(|r| r.version > *current)
        .filter(|r| prereleases_ok || (!r.prerelease && r.version.pre.is_empty()))
        .collect();
    newer.sort_by(|a, b| b.version.cmp(&a.version));
    newer.dedup_by(|a, b| a.version == b.version);
    let release = (*newer.first()?).clone();
    let asset = install
        .can_apply()
        .then(|| install.asset_name(&release.tag))
        .flatten()
        .and_then(|n| release.asset(&n).cloned());
    let checksums = release.asset(crate::CHECKSUMS_ASSET).cloned();
    Some(Update {
        asset,
        checksums,
        notes: newer.into_iter().cloned().collect(),
        release,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::github::parse_releases;

    fn v(s: &str) -> Version {
        Version::parse(s).expect(s)
    }

    fn list() -> Vec<Release> {
        parse_releases(crate::github::SAMPLE).expect("sample")
    }

    #[test]
    fn offers_the_newest_prerelease_to_a_prerelease_build_with_all_unseen_notes() {
        let portable = Install::WindowsPortable { exe: "x".into() };
        let u = find_update(&list(), &v("0.1.0-alpha.1"), false, &portable).expect("update");
        assert_eq!(u.tag(), "v0.1.0-alpha.3");
        let notes: Vec<&str> = u.notes.iter().map(|r| r.tag.as_str()).collect();
        assert_eq!(notes, ["v0.1.0-alpha.3", "v0.1.0-alpha.2"]);
        assert_eq!(
            u.asset.as_ref().map(|a| a.name.as_str()),
            Some("OpenStitch-v0.1.0-alpha.3-windows-x86_64.zip")
        );
        assert!(u.checksums.is_some() && u.applicable());
        let same = find_update(&list(), &v("0.1.0-alpha.3"), false, &portable);
        assert_eq!(same, None, "already the newest");
        let newer = find_update(&list(), &v("0.1.0-alpha.9"), false, &portable);
        assert_eq!(newer, None, "running something newer than any release");
    }

    #[test]
    fn stable_builds_only_see_stable_releases_unless_asked() {
        let msi = Install::WindowsMsi { exe: "x".into() };
        let mut rel = list();
        assert_eq!(find_update(&rel, &v("0.0.9"), false, &msi), None);
        let u = find_update(&rel, &v("0.0.9"), true, &msi).expect("opted in");
        assert_eq!(u.tag(), "v0.1.0-alpha.3");
        assert!(u.asset.as_ref().is_some_and(|a| a.name.ends_with(".msi")));
        let mut stable = rel[1].clone();
        stable.tag = "v0.1.0".into();
        stable.version = v("0.1.0");
        stable.prerelease = false;
        stable.assets.clear();
        rel.push(stable);
        let u = find_update(&rel, &v("0.0.9"), false, &msi).expect("stable");
        assert_eq!(u.tag(), "v0.1.0");
        assert_eq!(
            u.notes.len(),
            1,
            "pre-releases are not in a stable user's notes"
        );
        assert!(u.asset.is_none() && u.checksums.is_none() && !u.applicable());
        let u = find_update(&rel, &v("0.1.0-alpha.2"), false, &msi).expect("alpha sees stable");
        assert_eq!(u.tag(), "v0.1.0");
        let notes: Vec<&str> = u.notes.iter().map(|r| r.tag.as_str()).collect();
        assert_eq!(notes, ["v0.1.0", "v0.1.0-alpha.3"]);
    }

    #[test]
    fn installs_without_an_asset_get_notes_only() {
        for i in [
            Install::Flatpak,
            Install::Unsupported("arm".into()),
            Install::Development,
        ] {
            let u = find_update(&list(), &v("0.1.0-alpha.2"), false, &i).expect("update");
            assert_eq!(u.asset, None, "{i:?}");
            assert!(!u.applicable());
        }
        let appimage = Install::AppImage { image: "x".into() };
        let u = find_update(&list(), &v("0.1.0-alpha.2"), false, &appimage).expect("update");
        assert!(u.asset.is_some_and(|a| a.name.ends_with(".AppImage")));
    }
}
