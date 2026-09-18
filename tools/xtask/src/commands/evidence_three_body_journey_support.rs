//! Bounded document and identity validation for three-Body Journey evidence.

use serde::Deserialize;
use std::path::{Component, Path};

use super::MAXIMUM_DOCUMENT_BYTES;

pub(super) fn read_bounded_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, String> {
    let bytes = std::fs::read(path).map_err(|error| format!("read {}: {error}", path.display()))?;
    if bytes.is_empty() || bytes.len() > MAXIMUM_DOCUMENT_BYTES {
        return Err(format!(
            "{} violates the document byte bound",
            path.display()
        ));
    }
    serde_json::from_slice(&bytes).map_err(|error| format!("decode {}: {error}", path.display()))
}

pub(super) fn valid_identity(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 256
}

pub(super) fn valid_narrative(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 1_024
}

pub(super) fn valid_commit(value: &str) -> bool {
    value.len() == 40 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

pub(super) fn valid_sha256(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..].bytes().all(|byte| byte.is_ascii_hexdigit())
}

pub(super) fn validate_relative_path(path: &Path) -> Result<(), String> {
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::CurDir
                    | Component::ParentDir
                    | Component::RootDir
                    | Component::Prefix(_)
            )
        })
    {
        return Err("Journey artifact path must be root-confined and relative".into());
    }
    Ok(())
}
