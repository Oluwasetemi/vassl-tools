use std::io::{Read as _, Write as _};
use std::path::{Path, PathBuf};
use std::sync::{Arc, atomic::{AtomicU8, Ordering}};

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

        let progress = Arc::new(AtomicU8::new(0));

        let task = cx.spawn({
            let progress = progress.clone();
            async move |this: gpui::WeakEntity<AutoUpdater>, cx| {
                // Run the blocking download on a real OS thread so we can
                // track byte progress without blocking the GPUI executor.
                let (result_tx, result_rx) =
                    std::sync::mpsc::sync_channel::<Result<(PathBuf, ReleaseInfo)>>(1);
                let p = progress.clone();
                std::thread::Builder::new()
                    .name("vassl-download".into())
                    .spawn(move || {
                        let _ = result_tx.send(download_release(&info, p));
                    })
                    .expect("spawn download thread");

                // Poll the progress counter at 200 ms intervals until the
                // thread signals completion.
                loop {
                    match result_rx.try_recv() {
                        Ok(result) => {
                            this.update(cx, |me, cx| {
                                me.status = match result {
                                    Ok((zip, _)) => UpdateStatus::ReadyToInstall(zip),
                                    Err(e)        => UpdateStatus::Error(e.to_string()),
                                };
                                cx.emit(AutoUpdateEvent::StatusChanged);
                                cx.notify();
                            }).ok();
                            break;
                        }
                        Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                            this.update(cx, |me, cx| {
                                me.status = UpdateStatus::Error("Download thread crashed".into());
                                cx.emit(AutoUpdateEvent::StatusChanged);
                                cx.notify();
                            }).ok();
                            break;
                        }
                        Err(std::sync::mpsc::TryRecvError::Empty) => {
                            let pct = progress.load(Ordering::Relaxed);
                            this.update(cx, |me, cx| {
                                me.status = UpdateStatus::Downloading { pct };
                                cx.notify();
                            }).ok();
                            cx.background_executor()
                                .timer(std::time::Duration::from_millis(200))
                                .await;
                        }
                    }
                }
            }
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
    tag_name:   String,
    draft:      bool,
    prerelease: bool,
    assets:     Vec<GhAsset>,
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

    // per_page=100 ensures we don't miss the latest after 30+ releases exist.
    let url = format!("https://api.github.com/repos/{GITHUB_REPO}/releases?per_page=100");
    let releases: Vec<GhRelease> = ureq::get(&url)
        .set("User-Agent", &format!("VASSL-Updater/{CURRENT_VERSION}"))
        .set("Accept", "application/vnd.github.v3+json")
        .call()
        .context("GitHub releases request failed (check network; unauthenticated API allows 60 req/hr per IP)")?
        .into_json()
        .context("Failed to parse GitHub releases JSON")?;

    let best = releases.iter()
        .filter(|r| {
            // For Stable channel, respect the GitHub prerelease flag as an extra guard.
            // Alpha/Beta/Preview identify themselves via tag suffix, so prerelease flag
            // is redundant for them but harmless to also check.
            let stable_ok = !matches!(channel, ReleaseChannel::Stable) || !r.prerelease;
            !r.draft && stable_ok && channel_matches(channel, &r.tag_name)
        })
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
        // Dev/Nightly channels have no update_url(), so check() exits early
        // before ever reaching this function. The false arms are unreachable
        // in practice but must be exhaustive.
        _ => false,
    }
}

fn download_release(info: &ReleaseInfo, progress: Arc<AtomicU8>) -> Result<(PathBuf, ReleaseInfo)> {
    let cache_dir = dirs::cache_dir()
        .unwrap_or_else(std::env::temp_dir)
        .join("com.kamalu.vassl")
        .join("update");
    std::fs::create_dir_all(&cache_dir)?;

    let dest = cache_dir.join(&info.asset_name);

    // HEAD the asset to get Content-Length before opening the file, so we can
    // skip re-downloading if a complete copy is already cached.
    let head = ureq::head(&info.download_url)
        .set("User-Agent", &format!("VASSL-Updater/{CURRENT_VERSION}"))
        .call().ok();
    let total_bytes: Option<u64> = head
        .as_ref()
        .and_then(|r| r.header("Content-Length"))
        .and_then(|s| s.parse().ok());

    if let (Some(total), Ok(meta)) = (total_bytes, std::fs::metadata(&dest)) {
        if meta.len() == total {
            progress.store(100, Ordering::Relaxed);
            return Ok((dest, info.clone()));
        }
    }

    let resp = ureq::get(&info.download_url)
        .set("User-Agent", &format!("VASSL-Updater/{CURRENT_VERSION}"))
        .call()
        .context("download request failed")?;

    // Re-read Content-Length from the GET response (HEAD may have differed).
    let total_bytes: Option<u64> = resp.header("Content-Length")
        .and_then(|s| s.parse().ok());

    let mut reader     = resp.into_reader();
    let mut file       = std::fs::File::create(&dest)?;
    let mut downloaded = 0u64;
    let mut buf        = [0u8; 65536];

    loop {
        let n = reader.read(&mut buf)?;
        if n == 0 { break; }
        file.write_all(&buf[..n])?;
        downloaded += n as u64;
        if let Some(total) = total_bytes {
            // Cap reported progress at 99 until the file is fully flushed.
            let pct = ((downloaded * 99) / total.max(1)).min(99) as u8;
            progress.store(pct, Ordering::Relaxed);
        }
        // When Content-Length is absent, pulse between 10-90 so the UI shows
        // activity rather than freezing at 0%.
        else {
            let pulse = (downloaded / (512 * 1024)) as u8; // tick every 512 KB
            progress.store(10 + (pulse % 80), Ordering::Relaxed);
        }
    }
    progress.store(100, Ordering::Relaxed);

    Ok((dest, info.clone()))
}

/// Extracts `zip_path`, applies the update in-place, and returns the path GPUI
/// should restart with (the updated .app bundle or executable).
fn apply_update(zip_path: &Path) -> Result<PathBuf> {
    let parent = zip_path.parent().context("zip has no parent dir")?;
    // Use a versioned extraction directory so a re-run never reads a
    // partial extraction left by a previous (failed) attempt.
    let extract_dir = parent.join("extracted");
    if extract_dir.exists() {
        std::fs::remove_dir_all(&extract_dir)?;
    }
    std::fs::create_dir_all(&extract_dir)?;

    let file    = std::fs::File::open(zip_path)?;
    let mut arc = zip::ZipArchive::new(file)?;
    arc.extract(&extract_dir)?;

    #[cfg(target_os = "macos")]
    let restart_path = apply_macos(&extract_dir)?;
    #[cfg(target_os = "windows")]
    let restart_path = apply_windows(&extract_dir)?;
    #[cfg(not(any(target_os = "macos", target_os = "windows")))]
    let restart_path: PathBuf = { anyhow::bail!("auto-update is not supported on this platform") };

    // Best-effort cleanup — leave no multi-hundred-MB extractions behind.
    let _ = std::fs::remove_dir_all(&extract_dir);
    let _ = std::fs::remove_file(zip_path);

    Ok(restart_path)
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

    // Discover the .app at the root of the extraction directory dynamically —
    // never hardcode the bundle name so a rename in Cargo.toml doesn't silently
    // fail here by falling through to the bare-binary path.
    let new_app: Option<PathBuf> = std::fs::read_dir(extract_dir)?
        .filter_map(|e| e.ok())
        .find(|e| e.path().extension().map_or(false, |x| x == "app"))
        .map(|e| e.path());

    let (copy_src, copy_dst) = match new_app {
        Some(app) => (app, app_bundle.clone()),
        None => {
            // Fallback: bare binary (shouldn't happen with our CI packaging).
            let new_bin = extract_dir.join("vassl");
            anyhow::ensure!(new_bin.exists(), "neither .app bundle nor bare 'vassl' binary found in extracted update");
            let inner = app_bundle.join("Contents/MacOS/vassl");
            (new_bin, inner)
        }
    };

    // Copy to destination — we can't use rename() because the cache dir and
    // /Applications may be on different filesystems (cross-device link error).
    if copy_dst.exists() {
        if copy_dst.is_dir() {
            std::fs::remove_dir_all(&copy_dst)?;
        } else {
            std::fs::remove_file(&copy_dst)?;
        }
    }
    if copy_src.is_dir() {
        copy_dir_all(&copy_src, &copy_dst)?;
    } else {
        std::fs::copy(&copy_src, &copy_dst)?;
    }

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

    // Safe three-step swap so a failure at any point leaves a launchable binary:
    //   1. Copy new binary to a staging path (current_exe is still intact here)
    //   2. Rename current → .bak  (path is now empty but staging exists)
    //   3. Rename staging → current  (fast same-filesystem rename, near-atomic)
    //
    // If step 1 fails: nothing changed, current_exe still runs.
    // If step 2 fails: staging exists but current_exe still runs.
    // If step 3 fails: user can manually rename .new → .exe to recover.
    let staging = current_exe.with_extension("exe.new");
    let backup  = current_exe.with_extension("exe.bak");

    // Remove stale staging/backup files from any previous attempt.
    if staging.exists() { let _ = std::fs::remove_file(&staging); }
    if backup.exists()  { let _ = std::fs::remove_file(&backup);  }

    std::fs::copy(&new_exe, &staging)?;
    std::fs::rename(&current_exe, &backup)?;
    std::fs::rename(&staging, &current_exe)?;

    Ok(current_exe)
}

/// Recursively copies a directory tree from `src` to `dst`, preserving symlinks.
/// macOS `.app` bundles contain symlinks (e.g. framework `Versions/Current`);
/// `std::fs::copy` would dereference them and create regular files, breaking the bundle.
fn copy_dir_all(src: &Path, dst: &Path) -> Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry   = entry?;
        let src_path = entry.path();
        let dst_path = dst.join(entry.file_name());
        // Use symlink_metadata so is_symlink() reports true for symlink entries
        // rather than the type of the symlink's target.
        let ty = entry.metadata()?.file_type();
        if ty.is_symlink() {
            #[cfg(target_os = "macos")]
            {
                let target = std::fs::read_link(&src_path)?;
                std::os::unix::fs::symlink(&target, &dst_path)?;
            }
            // Windows symlinks require elevated permissions; skip rather than fail.
            #[cfg(not(target_os = "macos"))]
            { std::fs::copy(&src_path, &dst_path)?; }
        } else if ty.is_dir() {
            copy_dir_all(&src_path, &dst_path)?;
        } else {
            std::fs::copy(&src_path, &dst_path)?;
        }
    }
    Ok(())
}

