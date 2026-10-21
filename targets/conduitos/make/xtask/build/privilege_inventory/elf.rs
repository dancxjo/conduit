//! Bounded ELF64 load-segment observations, not installed mapping evidence.
use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Segment {
    offset: u64,
    virtual_address: u64,
    physical_address: u64,
    file_bytes: u64,
    memory_bytes: u64,
    flags: u32,
}
#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Image {
    offset_in_root: usize,
    bytes: usize,
    sha256: String,
    entry: u64,
    load_segments: Vec<Segment>,
}
fn number(bytes: &[u8], offset: usize, size: usize) -> Result<u64, String> {
    let end = offset.checked_add(size).ok_or("ELF offset overflow")?;
    let slice = bytes.get(offset..end).ok_or("truncated ELF")?;
    Ok(slice.iter().rev().fold(0, |v, b| (v << 8) | u64::from(*b)))
}
fn decode(bytes: &[u8], offset: usize) -> Result<Image, String> {
    if bytes.get(..6) != Some(b"\x7fELF\x02\x01") {
        return Err("expected little-endian ELF64".into());
    }
    if number(bytes, 18, 2)? != 62 || number(bytes, 16, 2)? != 2 {
        return Err("expected x86_64 executable image".into());
    }
    let ph = number(bytes, 32, 8)? as usize;
    let sh = number(bytes, 40, 8)? as usize;
    let pn = number(bytes, 56, 2)? as usize;
    let sn = number(bytes, 60, 2)? as usize;
    if number(bytes, 54, 2)? != 56 || number(bytes, 58, 2)? != 64 || pn > 64 || sn > 4096 {
        return Err("unsupported ELF table bounds".into());
    }
    let mut end = sh.checked_add(sn * 64).ok_or("ELF section overflow")?;
    let mut segments = Vec::new();
    for i in 0..pn {
        let at = ph.checked_add(i * 56).ok_or("ELF program overflow")?;
        if number(bytes, at, 4)? != 1 {
            continue;
        }
        let segment = Segment {
            offset: number(bytes, at + 8, 8)?,
            virtual_address: number(bytes, at + 16, 8)?,
            physical_address: number(bytes, at + 24, 8)?,
            file_bytes: number(bytes, at + 32, 8)?,
            memory_bytes: number(bytes, at + 40, 8)?,
            flags: number(bytes, at + 4, 4)? as u32,
        };
        if segment.file_bytes > segment.memory_bytes {
            return Err("ELF filesz exceeds memsz".into());
        }
        segments.push(segment);
    }
    for i in 0..sn {
        let at = sh.checked_add(i * 64).ok_or("ELF section overflow")?;
        if number(bytes, at + 4, 4)? != 8 {
            let section_end = number(bytes, at + 24, 8)?
                .checked_add(number(bytes, at + 32, 8)?)
                .ok_or("ELF section extent overflow")?;
            end = end.max(usize::try_from(section_end).map_err(|_| "ELF extent overflow")?);
        }
    }
    if segments.is_empty() || end < 64 || end > bytes.len() {
        return Err("invalid ELF image extent".into());
    }
    for segment in &segments {
        if segment
            .offset
            .checked_add(segment.file_bytes)
            .is_none_or(|n| n > end as u64)
        {
            return Err("ELF segment outside image".into());
        }
    }
    Ok(Image {
        offset_in_root: offset,
        bytes: end,
        sha256: super::digest(&bytes[..end]),
        entry: number(bytes, 24, 8)?,
        load_segments: segments,
    })
}
pub(super) fn observe(bytes: &[u8]) -> Result<Vec<Image>, String> {
    let mut images = vec![decode(bytes, 0)?];
    let mut offset = 64;
    while offset + 64 <= bytes.len() {
        if bytes.get(offset..offset + 6) == Some(b"\x7fELF\x02\x01") {
            if let Ok(image) = decode(&bytes[offset..], offset) {
                offset += image.bytes;
                images.push(image);
                if images.len() > 17 {
                    return Err("embedded ELF inventory exceeds 16 images".into());
                }
                continue;
            }
        }
        offset += 1;
    }
    Ok(images)
}
