use std::path::{Path, PathBuf};

use anyhow::{Context as _, Result};
#[cfg(target_os = "windows")]
use anyhow::bail;
use gpui::{Context, EventEmitter, Task};
use release_channel::{RELEASE_CHANNEL, ReleaseChannel};
use semver::Version;
use serde::Deserialize;

const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
const GITHUB_REPO: &str = "Oluwasetemi/vassl-tools";

// ── Platform asset name ────────────────────────────────────────────────────

#[cfg(all(target_os = "macos", target_arch = "aarch64"))]
const PLATFORM_ASSET: &str = "VASSL-macos-arm64.zip";

#[cfg(all(target_os = "macos", target_arch = "x86_64"))]
const PLATFORM_ASSET: &str = "VASSL-macos-x86_64.zip";

#[cfg(target_os = "windows")]
const PLATFORM_ASSET: &str = "VASSL-windows-x86_64.zip";

#[cfg(not(any(target_os = "macos", target_os = "windows")))]
const PLATFORM_ASSET: &str = "unsupported";

// ── Public types ───────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct ReleaseInfo {
    pub version:      String,
    pub asset_name:   String,
    pub download_url: String,
}

#[derive(Debug, Clone, Default)]
pub enum UpdateStatus {
    #[default]
    Idle,
    Checking,
    UpToDate,
    Available(ReleaseInfo),
    Downloading { pct: u8 },
    ReadyToInstall(PathBuf),
    Installing,
    Error(String),
}

pub enum AutoUpdateEvent {
    StatusChanged,
}
impl EventEmitter<AutoUpdateEvent> for AutoUpdater {}

// ── AutoUpdater entity ─────────────────────────────────────────────────────

pub struct AutoUpdater {
    pub status:  UpdateStatus,
    _task: Option<Task<()>>,
}

impl AutoUpdater {
    pub fn new() -> Self {
        Self { status: UpdateStatus::Idle, _task: None }
    }

    /// Trigger an update check in the background.
    /// No-ops for Dev/Nightly channels.
    pub fn check(&mut self, cx: &mut Context<Self>) {
        if RELEASE_CHANNEL.update_url().is_none() {
            self.status = UpdateStatus::UpToDate;
            cx.notify();
            return;
        }
        self.status = UpdateStatus::Checking;
        cx.notify();

        let task = cx.spawn(async move |this: gpui::WeakEntity<AutoUpdater>, cx| {
            let result = cx.background_executor()
                .spawn(async { query_latest_release() })
                .await;

            this.update(cx, |me, cx| {
                me.status = match result {
                    Ok(None)       => UpdateStatus::UpToDate,
                    Ok(Some(info)) => UpdateStatus::Available(info),
                    Err(e)         => UpdateStatus::Error(e.to_string()),
                };
                cx.emit(AutoUpdateEvent::StatusChanged);
                cx.notify();
            }).ok();
        });
        self._task = Some(task);
    }

    /// Start downloading the release asset.
    pub fn download(&mut self, info: ReleaseInfo, cx: &mut Context<Self>) {
        self.status = UpdateStatus::Downloading { pct: 0 };
        cx.notify();

        let task = cx.spawn(async move |this: gpui::WeakEntity<AutoUpdater>, cx| {
            let result = cx.background_executor()
                .spawn(async move { download_release(&info) })
                .await;

            this.update(cx, |me, cx| {
                me.status = match result {
                    Ok((zip, _info)) => UpdateStatus::ReadyToInstall(zip),
                    Err(e)           => UpdateStatus::Error(e.to_string()),
                };
                cx.emit(AutoUpdateEvent::StatusChanged);
                cx.notify();
            }).ok();
        });
        self._task = Some(task);
    }

    /// Extract the downloaded zip and relaunch the updated binary.
    pub fn install_and_restart(&mut self, zip: PathBuf, cx: &mut Context<Self>) {
        self.status = UpdateStatus::Installing;
        cx.notify();

        let task = cx.spawn(async move |this: gpui::WeakEntity<AutoUpdater>, cx| {
            let result = cx.background_executor()
                .spawn(async move { apply_update(&zip) })
                .await;

            match result {
                Ok(restart_path) => {
                    // Set the restart path so GPUI relaunches the updated binary, then quit
                    // gracefully — flushing writes and closing windows before exit.
                    cx.update(|cx| {
                        cx.set_restart_path(restart_path);
                        cx.quit();
                    });
                }
                Err(e) => {
                    this.update(cx, |me, cx| {
                        me.status = UpdateStatus::Error(e.to_string());
                        cx.emit(AutoUpdateEvent::StatusChanged);
                        cx.notify();
                    }).ok();
                }
            }
        });
        self._task = Some(task);
    }

}

// ── GitHub API types ───────────────────────────────────────────────────────

#[derive(Deserialize)]
struct GhRelease {
    tag_name: String,
    draft:    bool,
    assets:   Vec<GhAsset>,
}

#[derive(Deserialize)]
struct GhAsset {
    name:                 String,
    browser_download_url: String,
}

// ── Background work ────────────────────────────────────────────────────────

fn query_latest_release() -> Result<Option<ReleaseInfo>> {
    if PLATFORM_ASSET == "unsupported" {
        return Ok(None);
    }

    let current = Version::parse(CURRENT_VERSION)
        .unwrap_or_else(|_| Version::new(0, 0, 0));

    let channel = &*RELEASE_CHANNEL;

    let url = format!("https://api.github.com/repos/{GITHUB_REPO}/releases");
    let releases: Vec<GhRelease> = ureq::get(&url)
        .set("User-Agent", &format!("VASSL-Updater/{CURRENT_VERSION}"))
        .set("Accept", "application/vnd.github.v3+json")
        .call()
        .context("GitHub releases request failed")?
        .into_json()
        .context("Failed to parse GitHub releases JSON")?;

    let best = releases.iter()
        .filter(|r| !r.draft && channel_matches(channel, &r.tag_name))
        .filter_map(|r| {
            let v = r.tag_name.strip_prefix('v').unwrap_or(&r.tag_name);
            Version::parse(v).ok().map(|ver| (ver, r))
        })
        .max_by(|(a, _), (b, _)| a.cmp(b));

    let Some((latest_ver, release)) = best else { return Ok(None); };

    if latest_ver <= current { return Ok(None); }

    let asset = release.assets.iter()
        .find(|a| a.name == PLATFORM_ASSET)
        .ok_or_else(|| anyhow::anyhow!(
            "release {} has no asset {PLATFORM_ASSET}", release.tag_name
        ))?;

    Ok(Some(ReleaseInfo {
        version:      latest_ver.to_string(),
        asset_name:   PLATFORM_ASSET.to_string(),
        download_url: asset.browser_download_url.clone(),
    }))
}

fn channel_matches(channel: &ReleaseChannel, tag: &str) -> bool {
    let v = tag.strip_prefix('v').unwrap_or(tag);
    match channel {
        ReleaseChannel::Alpha   => v.contains("-alpha"),
        ReleaseChannel::Beta    => v.contains("-beta"),
        ReleaseChannel::Preview => v.contains("-preview") || v.contains("-rc"),
        ReleaseChannel::Stable  => !v.contains('-'),
        _ => false,
    }
}

fn download_release(info: &ReleaseInfo) -> Result<(PathBuf, ReleaseInfo)> {
    let cache_dir = dirs::cache_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("com.kamalu.vassl")
        .join("update");
    std::fs::create_dir_all(&cache_dir)?;

    let dest = cache_dir.join(&info.asset_name);

    let resp = ureq::get(&info.download_url)
        .set("User-Agent", &format!("VASSL-Updater/{CURRENT_VERSION}"))
        .call()
        .context("download request failed")?;

    let mut reader = resp.into_reader();
    let mut file   = std::fs::File::create(&dest)?;
    std::io::copy(&mut reader, &mut file)?;

    Ok((dest, info.clone()))
}

/// Extracts `zip_path`, applies the update in-place, and returns the path GPUI
/// should restart with (the updated .app bundle or executable).
fn apply_update(zip_path: &Path) -> Result<PathBuf> {
    let extract_dir = zip_path.parent()
        .context("zip has no parent dir")?
        .join("extracted");
    std::fs::create_dir_all(&extract_dir)?;

    let file    = std::fs::File::open(zip_path)?;
    let mut arc = zip::ZipArchive::new(file)?;
    arc.extract(&extract_dir)?;

    #[cfg(target_os = "macos")]
    { return apply_macos(&extract_dir); }
    #[cfg(target_os = "windows")]
    { return apply_windows(&extract_dir); }
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    { bail!("auto-update is not supported on this platform") }
}

#[cfg(target_os = "macos")]
fn apply_macos(extract_dir: &Path) -> Result<PathBuf> {
    use std::os::unix::fs::PermissionsExt as _;

    let current_exe = std::env::current_exe()?;

    // Walk up from the binary to find the .app bundle root.
    let app_bundle = current_exe
        .ancestors()
        .find(|p| p.extension().map_or(false, |e| e == "app"))
        .map(|p| p.to_path_buf())
        .unwrap_or_else(|| current_exe.clone());

    // Prefer the .app bundle from the zip; fall back to bare binary (x86_64 asset).
    let new_app = extract_dir.join("VASSL.app");
    let (copy_src, copy_dst) = if new_app.exists() {
        (new_app, app_bundle.clone())
    } else {
        let new_bin = extract_dir.join("vassl");
        let inner   = app_bundle.join("Contents/MacOS/vassl");
        (new_bin, inner)
    };

    // Replace the destination atomically while the app is still running.
    // Using a staging temp path avoids a partially-written bundle if we're interrupted.
    let staging = copy_dst.with_extension("app.update");
    if staging.exists() { std::fs::remove_dir_all(&staging)?; }
    std::fs::rename(&copy_src, &staging)?;
    if copy_dst.exists() { std::fs::remove_dir_all(&copy_dst)?; }
    std::fs::rename(&staging, &copy_dst)?;

    // Mark the binary executable (zip may strip the bit).
    let bin_path = app_bundle.join("Contents/MacOS/vassl");
    if bin_path.exists() {
        std::fs::set_permissions(&bin_path, std::fs::Permissions::from_mode(0o755))?;
    }

    Ok(app_bundle)
}

#[cfg(target_os = "windows")]
fn apply_windows(extract_dir: &Path) -> Result<PathBuf> {
    let current_exe = std::env::current_exe()?;
    let new_exe     = extract_dir.join("vassl.exe");

    if !new_exe.exists() {
        bail!("vassl.exe not found in extracted update");
    }

    // On Windows the running exe is locked, so we rename it out of the way and
    // copy the new one in. A cleanup of the old .bak happens on next launch.
    let backup = current_exe.with_extension("exe.bak");
    if backup.exists() { std::fs::remove_file(&backup)?; }
    std::fs::rename(&current_exe, &backup)?;
    std::fs::copy(&new_exe, &current_exe)?;

    Ok(current_exe)
}

