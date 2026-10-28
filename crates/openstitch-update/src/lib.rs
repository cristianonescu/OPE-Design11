//! Self-update for the OpenStitch editor (design: `docs/research/updater.md`).
//!
//! The editor asks GitHub for the project's releases, compares the newest one with the tag it
//! was built from, picks the asset that matches how it is installed (portable zip or MSI on
//! Windows, the `.app` zip on macOS, the AppImage or the tarball on Linux), downloads it,
//! checks its SHA-256 against the release's `SHA256SUMS`, swaps it into place and restarts.
//!
//! This crate has no UI and no global state. Every network call goes through a [`Client`] whose
//! base URL and host allow-list can point at a local mock server, so the tests never touch the
//! network. Nothing here runs unless the build carries a release tag
//! ([`version::current`]) and the install kind is one that can be updated ([`Install`]).

mod apply;
mod checksums;
mod client;
mod github;
mod install;
mod select;
mod version;

pub use apply::{
    ApplyError, Launch, Staged, cleanup_previous, download_dir, launch, restart, stage, swap,
};
pub use checksums::{Checksums, Sha256};
pub use client::{Client, DownloadError, Fetched, MAX_ASSET_BYTES, MAX_TEXT_BYTES};
pub use github::{Asset, Release};
pub use install::{Context, Install, Os, classify, detect};
pub use select::{Update, find_update};
pub use version::{Current, current, is_release_tag, parse_tag};

/// The GitHub repository releases come from, as `owner/name`.
pub const REPO: &str = "cristianonescu/OpenStitch";
/// The GitHub REST API.
pub const API_BASE: &str = "https://api.github.com";
/// Release assets are downloaded from here (GitHub redirects to its object store).
pub const DOWNLOAD_PREFIX: &str = "https://github.com/cristianonescu/OpenStitch/releases/download/";
/// The checksum file `release.yml` publishes with every release.
pub const CHECKSUMS_ASSET: &str = "SHA256SUMS";
/// Environment variables that point the client at a local stand-in for GitHub (the API base
/// and the download prefix), for end-to-end tests of a real build; both must be set.
pub const API_ENV: &str = "OPENSTITCH_UPDATE_API";
pub const DOWNLOADS_ENV: &str = "OPENSTITCH_UPDATE_DOWNLOADS";
