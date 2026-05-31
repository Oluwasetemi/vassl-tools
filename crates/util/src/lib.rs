/// Minimal stub of the Zed `util` crate.
/// Provides only the surface area needed by sqlez and db.

pub mod paths;

/// `maybe!({ block })` — wraps a block in an immediately-called closure.
/// Supports plain, async, and async move blocks.
///
/// Inside the block, `?` can propagate `Option::None` or `Result::Err`.
///
/// This matches the semantics of `gpui_util::maybe!` exactly.
#[macro_export]
macro_rules! maybe {
    ($block:block) => {
        (|| $block)()
    };
    (async $block:block) => {
        (async || $block)()
    };
    (async move $block:block) => {
        (async move || $block)()
    };
}

/// Extension trait that provides `.log_err()` on `Result` values.
pub trait ResultExt<E> {
    type Ok;
    fn log_err(self) -> Option<Self::Ok>;
    fn warn_on_err(self) -> Option<Self::Ok>;
}

impl<T, E: std::fmt::Display> ResultExt<E> for Result<T, E> {
    type Ok = T;

    fn log_err(self) -> Option<T> {
        match self {
            Ok(v) => Some(v),
            Err(e) => {
                log::error!("{}", e);
                None
            }
        }
    }

    fn warn_on_err(self) -> Option<T> {
        match self {
            Ok(v) => Some(v),
            Err(e) => {
                log::warn!("{}", e);
                None
            }
        }
    }
}
