//! Minimal stub of the Zed `zed_env_vars` crate.
//! Provides `ZED_STATELESS` env-var flag used by the `db` crate.

use std::sync::LazyLock;

/// When `true`, all database connections fall back to in-memory storage.
/// Set the `ZED_STATELESS` environment variable to enable this.
pub static ZED_STATELESS: LazyLock<bool> = LazyLock::new(|| {
    std::env::var("ZED_STATELESS")
        .map(|v| !v.is_empty() && v != "0" && v.to_ascii_lowercase() != "false")
        .unwrap_or(false)
});
