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
    println!("cargo:rerun-if-changed=../../.git/HEAD");

    #[cfg(target_os = "windows")]
    {
        let mut res = winres::WindowsResource::new();
        res.set_icon("../../assets/icons/vassl.ico");
        res.set("ProductName", "VASSL");
        res.set("FileDescription", "VASSL — Video Access Security Solutions Ltd.");
        res.set("LegalCopyright", "Copyright \u{00a9} 2026 Video Access Security Solutions Ltd.");
        res.set("CompanyName", "Video Access Security Solutions Ltd.");
        res.compile().expect("failed to embed Windows resources");
        println!("cargo:rerun-if-changed=../../assets/icons/vassl.ico");
    }
}
