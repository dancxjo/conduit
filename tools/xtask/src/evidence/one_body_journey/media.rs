//! Bounded checks for gallery media and URL-safe retained asset names.
use std::{
    fs,
    path::{Component, Path},
};

pub(super) fn png(path: &Path) -> Result<bool, String> {
    Ok(fs::read(path)
        .map_err(|error| error.to_string())?
        .starts_with(b"\x89PNG\r\n\x1a\n"))
}

pub(super) fn wav(path: &Path) -> Result<bool, String> {
    let bytes = fs::read(path).map_err(|error| error.to_string())?;
    if bytes.len() < 44
        || !bytes.starts_with(b"RIFF")
        || &bytes[8..12] != b"WAVE"
        || u32::from_le_bytes(bytes[4..8].try_into().unwrap()) as usize != bytes.len() - 8
    {
        return Ok(false);
    }
    let mut offset = 12;
    let mut frame_bytes = None;
    let mut data_bytes = None;
    while offset + 8 <= bytes.len() {
        let size = u32::from_le_bytes(bytes[offset + 4..offset + 8].try_into().unwrap()) as usize;
        let start = offset + 8;
        let Some(end) = start.checked_add(size) else {
            return Ok(false);
        };
        if end > bytes.len() {
            return Ok(false);
        }
        match &bytes[offset..offset + 4] {
            b"fmt " if size >= 16 => {
                let format = u16::from_le_bytes(bytes[start..start + 2].try_into().unwrap());
                let channels = u16::from_le_bytes(bytes[start + 2..start + 4].try_into().unwrap());
                let sample_rate =
                    u32::from_le_bytes(bytes[start + 4..start + 8].try_into().unwrap());
                let byte_rate =
                    u32::from_le_bytes(bytes[start + 8..start + 12].try_into().unwrap());
                let align = u16::from_le_bytes(bytes[start + 12..start + 14].try_into().unwrap());
                let bits = u16::from_le_bytes(bytes[start + 14..start + 16].try_into().unwrap());
                if format != 1
                    || !(1..=2).contains(&channels)
                    || !(8000..=96000).contains(&sample_rate)
                    || bits != 16
                    || align != channels * 2
                    || byte_rate != sample_rate * u32::from(align)
                {
                    return Ok(false);
                }
                frame_bytes = Some(usize::from(align));
            }
            b"data" => data_bytes = Some(size),
            _ => {}
        }
        offset = end + (size & 1);
    }
    Ok(offset == bytes.len()
        && frame_bytes
            .is_some_and(|align| data_bytes.is_some_and(|size| size > 0 && size % align == 0)))
}

pub(super) fn digest(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(bytes))
}

pub(super) fn safe_asset_path(path: &Path) -> Result<String, String> {
    if path.is_absolute()
        || path
            .components()
            .any(|part| !matches!(part, Component::Normal(_)))
    {
        return Err("journey media path is not root-confined".into());
    }
    let value = path.to_str().ok_or("journey media path is not UTF-8")?;
    if value.is_empty()
        || value == "index.html"
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"-_. /".contains(&byte))
        || value.contains(' ')
        || value.starts_with('.')
    {
        return Err("journey media path contains an unsupported URL character".into());
    }
    Ok(value.to_owned())
}
