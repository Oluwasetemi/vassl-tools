use std::path::PathBuf;

/// Minimal subset of Zed's `PathExt` needed by sqlez.
pub trait PathExt {
    /// Converts a raw byte slice to `Self`.
    fn try_from_bytes(bytes: &[u8]) -> anyhow::Result<Self>
    where
        Self: Sized;
}

impl PathExt for PathBuf {
    fn try_from_bytes(bytes: &[u8]) -> anyhow::Result<PathBuf> {
        let s = std::str::from_utf8(bytes)?;
        Ok(PathBuf::from(s))
    }
}
