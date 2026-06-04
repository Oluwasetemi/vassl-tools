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

/// The release channel for the current build.
///
/// Resolution order:
/// 1. `VASSL_CHANNEL` env var set at compile time (explicit CI override)
/// 2. Inferred from `CARGO_PKG_VERSION` using the pre-release suffix:
///    - `0.x.y`                  → Dev      (DB: `0-dev/`)
///    - `x.y.z-preview[.N]`      → Preview  (DB: `0-preview/`)
///    - `x.y.z-nightly[.N]`      → Nightly  (DB: `0-nightly/`)
///    - `x.y.z` (major ≥ 1)      → Stable   (DB: `0-stable/`)
///    - anything else             → Dev
pub static RELEASE_CHANNEL: LazyLock<ReleaseChannel> = LazyLock::new(|| {
    if let Some(ch) = option_env!("VASSL_CHANNEL") {
        return match ch {
            "stable"  => ReleaseChannel::Stable,
            "preview" => ReleaseChannel::Preview,
            "nightly" => ReleaseChannel::Nightly,
            _         => ReleaseChannel::Dev,
        };
    }
    let version = env!("CARGO_PKG_VERSION");
    let (base, pre) = match version.split_once('-') {
        Some((b, p)) => (b, Some(p)),
        None         => (version, None),
    };
    let major: u32 = base.split('.').next().and_then(|s| s.parse().ok()).unwrap_or(0);
    match pre {
        Some(p) if p.starts_with("preview") => ReleaseChannel::Preview,
        Some(p) if p.starts_with("nightly") => ReleaseChannel::Nightly,
        None if major >= 1                   => ReleaseChannel::Stable,
        _                                    => ReleaseChannel::Dev,
    }
});
