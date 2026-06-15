/// Converts a captured `Keystroke` into the string format `KeyBinding::new` accepts.
///
/// `Keystroke::unparse()` emits platform-specific names ("cmd-" on macOS, "win-" on Windows,
/// "super-" on Linux). `KeyBinding::new` uses the cross-platform alias "secondary-".
/// This converts between them so a captured keystroke round-trips correctly.
pub fn normalize_for_keybinding(ks: &gpui::Keystroke) -> String {
    let s = ks.unparse();
    for prefix in &["cmd-", "win-", "super-"] {
        if let Some(rest) = s.strip_prefix(prefix) {
            return format!("secondary-{rest}");
        }
    }
    s
}

/// Renders a keystroke string (e.g. `"secondary-shift-a"`) as a human-readable label.
///
/// On macOS: `"⌘⇧A"`. On Windows: `"⊞shift-A"`. Falls back to the raw string on parse error.
pub fn format_keystroke(raw: &str) -> String {
    gpui::Keystroke::parse(raw)
        .map(|ks| ks.to_string())
        .unwrap_or_else(|_| raw.to_string())
}

/// Returns all remappable app-level bindings as (action_name, default_keystroke, human_label).
pub fn default_app_bindings() -> Vec<(&'static str, &'static str, &'static str)> {
    vec![
        ("vassl::OpenInventory", "secondary-1", "Open Inventory"),
        ("vassl::OpenQuotations", "secondary-2", "Open Quotations"),
        ("vassl::OpenPriceBook", "secondary-3", "Open Price Book"),
        ("vassl::OpenSuppliers", "secondary-4", "Open Suppliers"),
        ("vassl::OpenAuditLog", "secondary-shift-a", "Open Audit Log"),
        ("vassl::OpenSettings", "secondary-,", "Open Settings"),
        (
            "vassl::OpenGlobalSearch",
            "secondary-shift-f",
            "Global Search",
        ),
        (
            "vassl::FocusSearch",
            "secondary-f",
            "Focus Search / Command Palette",
        ),
        ("vassl::NewRecord", "secondary-n", "New Record"),
        (
            "vassl::IncreaseFontSize",
            "secondary-=",
            "Increase Font Size",
        ),
        (
            "vassl::DecreaseFontSize",
            "secondary--",
            "Decrease Font Size",
        ),
    ]
}
