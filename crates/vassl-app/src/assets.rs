use gpui::{AssetSource, Result, SharedString};
use rust_embed::RustEmbed;
use std::borrow::Cow;

#[derive(RustEmbed)]
#[folder = "../../assets"]
#[exclude = "*.db"]
#[exclude = "*.sql"]
#[exclude = "*.xls"]
#[exclude = "*.xlsx"]
#[exclude = ".DS_Store"]
pub struct VasslAssets;

impl AssetSource for VasslAssets {
    fn load(&self, path: &str) -> Result<Option<Cow<'static, [u8]>>> {
        Ok(Self::get(path).map(|f| f.data))
    }

    fn list(&self, path: &str) -> Result<Vec<SharedString>> {
        Ok(Self::iter()
            .filter(|p| p.starts_with(path))
            .map(|p| SharedString::from(p.into_owned()))
            .collect())
    }
}
