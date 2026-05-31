//! Minimal stub of the Zed `release_channel` crate.
//! Provides `ReleaseChannel` and `RELEASE_CHANNEL` for use in the `db` crate.

use std::sync::LazyLock;

/// A release channel for the application.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub enum ReleaseChannel {
    /// Development builds.
    #[default]
    Dev,
    /// Nightly builds.
    Nightly,
    /// Preview / beta builds.
    Preview,
    /// Stable production builds.
    Stable,
}

impl ReleaseChannel {
    /// Returns a short programmatic name for this channel.
    pub fn dev_name(&self) -> &'static str {
        match self {
            ReleaseChannel::Dev => "dev",
            ReleaseChannel::Nightly => "nightly",
            ReleaseChannel::Preview => "preview",
            ReleaseChannel::Stable => "stable",
        }
    }
}

/// The release channel for the current build. Defaults to `Dev`.
pub static RELEASE_CHANNEL: LazyLock<ReleaseChannel> =
    LazyLock::new(|| ReleaseChannel::Dev);
