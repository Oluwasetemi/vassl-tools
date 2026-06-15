fn main() {
    // Guard: a release build must have a recognisable pre-release or stable
    // version suffix so the auto-updater channel resolves to something other
    // than Dev. Catching this at build time prevents a repeat of the silent
    // "Already up to date" bug where Dev channel short-circuits update checks.
    let profile = std::env::var("PROFILE").unwrap_or_default();
    if profile == "release" {
        let version = std::env::var("CARGO_PKG_VERSION").unwrap_or_default();
        let pre = version.split_once('-').map(|(_, p)| p).unwrap_or("");
        let known = ["alpha", "beta", "preview", "nightly"];
        let major: u32 = version
            .split('.')
            .next()
            .and_then(|s| s.parse().ok())
            .unwrap_or(0);
        let is_stable = pre.is_empty() && major >= 1;
        let is_known_pre = known.iter().any(|k| pre.starts_with(k));
        if !is_stable && !is_known_pre {
            panic!(
                "Release build has version `{version}` which resolves to Dev \
                 channel — auto-updates will be silently disabled. \
                 Set a recognised pre-release suffix (alpha/beta/preview/nightly) \
                 or bump major to ≥ 1 for stable."
            );
        }
    }

    // Bake the short git commit hash into the binary so the About dialog can
    // display it without requiring a live git installation at runtime.
    let hash = std::process::Command::new("git")
        .args(["rev-parse", "--short", "HEAD"])
        .output()
        .ok()
        .and_then(|o| String::from_utf8(o.stdout).ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "unknown".to_string());

    println!("cargo:rustc-env=VASSL_GIT_COMMIT={hash}");
    println!("cargo:rerun-if-changed=../../.git/HEAD");

    #[cfg(target_os = "windows")]
    {
        let mut res = winres::WindowsResource::new();
        res.set_icon("../../assets/icons/vassl.ico");
        res.set("ProductName", "VASSL");
        res.set(
            "FileDescription",
            "VASSL — Video Access Security Solutions Ltd.",
        );
        res.set(
            "LegalCopyright",
            "Copyright \u{00a9} 2026 Video Access Security Solutions Ltd.",
        );
        res.set("CompanyName", "Video Access Security Solutions Ltd.");

        // Encode a monotonically increasing 4th version component so Windows
        // FILEVERSION comparisons work correctly across pre-release stages.
        // winres strips the pre-release suffix, making every 0.1.0-* build
        // appear as 0.1.0.0 — causing MajorUpgrade to miss previous installs
        // and leave stale binaries on disk alongside the new one.
        //
        // Encoding:  stage * 1000 + N
        //   alpha.N   → 1000 + N   (alpha.15 → 1015)
        //   beta.N    → 2000 + N   (beta.4   → 2004)
        //   preview.N → 3000 + N
        //   stable    → 4000
        let pkg_ver = std::env::var("CARGO_PKG_VERSION").unwrap_or_default();
        let build_num: u64 = if let Some((_, pre)) = pkg_ver.split_once('-') {
            let (stage, n_str) = pre.split_once('.').unwrap_or((pre, "0"));
            let n: u64 = n_str.parse().unwrap_or(0);
            match stage {
                "alpha"   => 1000 + n,
                "beta"    => 2000 + n,
                "preview" => 3000 + n,
                "rc"      => 3500 + n,
                _         => n,
            }
        } else {
            4000 // stable
        };
        // Pack Major.Minor.Patch.Build — each field is 16 bits
        let file_ver: u64 = (0u64 << 48) | (1u64 << 32) | (0u64 << 16) | build_num;
        res.set_version_info(winres::VersionInfo::FILEVERSION, file_ver);
        res.set_version_info(winres::VersionInfo::PRODUCTVERSION, file_ver);

        res.compile().expect("failed to embed Windows resources");
        println!("cargo:rerun-if-changed=../../assets/icons/vassl.ico");
    }
}
