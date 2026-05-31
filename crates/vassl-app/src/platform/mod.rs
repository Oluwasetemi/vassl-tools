#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "windows")]
mod windows;

pub fn app_name() -> &'static str {
    "VASSL"
}
