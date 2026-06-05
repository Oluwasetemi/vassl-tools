//! Minimal stub of the Zed `release_channel` crate.
//! Provides `ReleaseChannel` and `RELEASE_CHANNEL` for use in the `db` crate.

use std::sync::LazyLock;

/// A release channel for the application.
#[derive(Debug, Copy, Clone, PartialEq, Eq, Default)]
pub enum ReleaseChannel {
    /// Development builds (local cargo run / CI without a tag).
    #[default]
    Dev,
    /// Alpha builds — early access, may be unstable.
    Alpha,
    /// Beta builds — feature-complete, stabilisation phase.
    Beta,
    /// Nightly automated builds.
    Nightly,
    /// Preview / release-candidate builds.
    Preview,
    /// Stable production builds.
    Stable,
}

impl ReleaseChannel {
    /// Returns a short programmatic name for this channel.
    pub fn dev_name(&self) -> &'static str {
        match self {
            ReleaseChannel::Dev     => "dev",
            ReleaseChannel::Alpha   => "alpha",
            ReleaseChannel::Beta    => "beta",
            ReleaseChannel::Nightly => "nightly",
            ReleaseChannel::Preview => "preview",
            ReleaseChannel::Stable  => "stable",
        }
    }

    /// Display name shown to users (e.g. in the About dialog).
    pub fn display_name(&self) -> &'static str {
        match self {
            ReleaseChannel::Dev     => "Dev",
            ReleaseChannel::Alpha   => "Alpha",
            ReleaseChannel::Beta    => "Beta",
            ReleaseChannel::Nightly => "Nightly",
            ReleaseChannel::Preview => "Preview",
            ReleaseChannel::Stable  => "Stable",
        }
    }

    /// Base URL for the auto-update feed for this channel.
    ///
    /// Returns `None` for Dev/Nightly builds where auto-update is not
    /// expected.  The actual update check is not yet implemented; this
    /// provides the hook for a future updater.
    pub fn update_url(&self) -> Option<&'static str> {
        match self {
            ReleaseChannel::Alpha   => Some("https://releases.vassl.app/alpha/latest.json"),
            ReleaseChannel::Beta    => Some("https://releases.vassl.app/beta/latest.json"),
            ReleaseChannel::Preview => Some("https://releases.vassl.app/preview/latest.json"),
            ReleaseChannel::Stable  => Some("https://releases.vassl.app/stable/latest.json"),
            ReleaseChannel::Dev | ReleaseChannel::Nightly => None,
        }
    }
}

/// The release channel for the current build.
///
/// Resolution order:
/// 1. `VASSL_CHANNEL` env var set at compile time (explicit CI override)
/// 2. Inferred from `CARGO_PKG_VERSION` using the pre-release suffix:
///    - `0.x.y`                  → Dev      (DB: `0-dev/`)
///    - `x.y.z-alpha[.N]`        → Alpha
///    - `x.y.z-beta[.N]`         → Beta
///    - `x.y.z-preview[.N]`      → Preview
///    - `x.y.z-nightly[.N]`      → Nightly
///    - `x.y.z` (major ≥ 1)      → Stable
///    - anything else             → Dev
pub static RELEASE_CHANNEL: LazyLock<ReleaseChannel> = LazyLock::new(|| {
    if let Some(ch) = option_env!("VASSL_CHANNEL") {
        return match ch {
            "stable"  => ReleaseChannel::Stable,
            "preview" => ReleaseChannel::Preview,
            "beta"    => ReleaseChannel::Beta,
            "alpha"   => ReleaseChannel::Alpha,
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
        Some(p) if p.starts_with("alpha")   => ReleaseChannel::Alpha,
        Some(p) if p.starts_with("beta")    => ReleaseChannel::Beta,
        Some(p) if p.starts_with("preview") => ReleaseChannel::Preview,
        Some(p) if p.starts_with("nightly") => ReleaseChannel::Nightly,
        None if major >= 1                   => ReleaseChannel::Stable,
        _                                    => ReleaseChannel::Dev,
    }
});
