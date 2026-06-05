fn main() {
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
    // Re-run whenever the HEAD pointer changes (new commit, branch switch).
    println!("cargo:rerun-if-changed=../../.git/HEAD");
}
