//! The bounded stored-ZIP carrier shared by native and embedded Seed installation.
//! This parses structure and CRCs, not admission or execution authority.
use std::collections::BTreeSet;
const MAXIMUM_FILES: usize = 17;
const MAXIMUM_PACKAGE_BYTES: usize = 64 * 1024 * 1024;
const LOCAL_FILE: u32 = 0x0403_4b50;
const CENTRAL_FILE: u32 = 0x0201_4b50;
const END: u32 = 0x0605_4b50;

/// One bounded carrier entry; owned payload bytes are cleared when released.
pub struct JoiningZipEntry {
    pub name: String,
    pub bytes: Vec<u8>,
}

impl Drop for JoiningZipEntry {
    fn drop(&mut self) {
        self.bytes.fill(0);
    }
}

/// Check the exact stored-ZIP layout and CRCs within the joining carrier bounds.
/// Artifact identity, construction and admission remain the caller's obligations.
pub fn read_stored_joining_zip(bytes: &[u8]) -> Result<Vec<JoiningZipEntry>, String> {
    if bytes.len() > MAXIMUM_PACKAGE_BYTES
        || bytes.len() < 22
        || u32_at(bytes, bytes.len() - 22)? != END
    {
        return Err("native ZIP has no exact terminal directory".into());
    }
    let end = bytes.len() - 22;
    let count = usize::from(u16_at(bytes, end + 10)?);
    let central_bytes = usize_at_u32(bytes, end + 12)?;
    let central_offset = usize_at_u32(bytes, end + 16)?;
    if u16_at(bytes, end + 4)? != 0
        || u16_at(bytes, end + 6)? != 0
        || usize::from(u16_at(bytes, end + 8)?) != count
        || u16_at(bytes, end + 20)? != 0
        || !(2..=MAXIMUM_FILES).contains(&count)
        || central_offset.checked_add(central_bytes) != Some(end)
    {
        return Err("native ZIP terminal directory is malformed or unbounded".into());
    }
    let mut cursor = central_offset;
    let mut expected_local = 0_usize;
    let mut names = BTreeSet::new();
    let mut entries = Vec::with_capacity(count);
    for _ in 0..count {
        if u32_at(bytes, cursor)? != CENTRAL_FILE {
            return Err("native ZIP central entry is malformed".into());
        }
        let flags = u16_at(bytes, cursor + 8)?;
        let method = u16_at(bytes, cursor + 10)?;
        let expected_crc = u32_at(bytes, cursor + 16)?;
        let compressed = usize_at_u32(bytes, cursor + 20)?;
        let size = usize_at_u32(bytes, cursor + 24)?;
        let name_len = usize::from(u16_at(bytes, cursor + 28)?);
        let extra_len = usize::from(u16_at(bytes, cursor + 30)?);
        let comment_len = usize::from(u16_at(bytes, cursor + 32)?);
        let local_offset = usize_at_u32(bytes, cursor + 42)?;
        let name = name_at(bytes, cursor + 46, name_len)?;
        cursor = cursor
            .checked_add(46 + name_len + extra_len + comment_len)
            .ok_or("native ZIP central extent overflow")?;
        if flags != 0x0800
            || method != 0
            || compressed != size
            || local_offset != expected_local
            || !names.insert(name.clone())
            || u32_at(bytes, local_offset)? != LOCAL_FILE
            || u16_at(bytes, local_offset + 6)? != flags
            || u16_at(bytes, local_offset + 8)? != method
            || u32_at(bytes, local_offset + 14)? != expected_crc
            || usize_at_u32(bytes, local_offset + 18)? != size
            || usize_at_u32(bytes, local_offset + 22)? != size
        {
            return Err("native ZIP entry layout is inconsistent".into());
        }
        let local_name_len = usize::from(u16_at(bytes, local_offset + 26)?);
        let local_extra_len = usize::from(u16_at(bytes, local_offset + 28)?);
        if name_at(bytes, local_offset + 30, local_name_len)? != name {
            return Err("native ZIP local name differs from its central entry".into());
        }
        let data_offset = local_offset
            .checked_add(30 + local_name_len + local_extra_len)
            .ok_or("native ZIP local extent overflow")?;
        let data_end = data_offset
            .checked_add(size)
            .filter(|end| *end <= central_offset)
            .ok_or("native ZIP payload exceeds its local region")?;
        let payload = &bytes[data_offset..data_end];
        if crc32(payload) != expected_crc {
            return Err("native ZIP entry failed CRC-32".into());
        }
        entries.push(JoiningZipEntry {
            name,
            bytes: payload.to_vec(),
        });
        expected_local = data_end;
    }
    if cursor != end || expected_local != central_offset {
        return Err("native ZIP has unaccounted bytes".into());
    }
    Ok(entries)
}

fn name_at(bytes: &[u8], offset: usize, length: usize) -> Result<String, String> {
    let end = offset
        .checked_add(length)
        .filter(|end| *end <= bytes.len())
        .ok_or("native ZIP entry name exceeds the package")?;
    let name = std::str::from_utf8(&bytes[offset..end])
        .map_err(|_| "native ZIP entry name is not UTF-8")?;
    if name.is_empty()
        || name.len() > 256
        || name.contains(['/', '\\'])
        || matches!(name, "." | "..")
    {
        return Err("native ZIP entry name is unsafe".into());
    }
    Ok(name.into())
}

fn u16_at(bytes: &[u8], offset: usize) -> Result<u16, String> {
    let value = bytes
        .get(offset..offset + 2)
        .ok_or("native ZIP field exceeds the package")?;
    Ok(u16::from_le_bytes([value[0], value[1]]))
}

fn u32_at(bytes: &[u8], offset: usize) -> Result<u32, String> {
    let value = bytes
        .get(offset..offset + 4)
        .ok_or("native ZIP field exceeds the package")?;
    Ok(u32::from_le_bytes([value[0], value[1], value[2], value[3]]))
}

fn usize_at_u32(bytes: &[u8], offset: usize) -> Result<usize, String> {
    usize::try_from(u32_at(bytes, offset)?).map_err(|_| "native ZIP extent is unsupported".into())
}

fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = u32::MAX;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb8_8320 & 0_u32.wrapping_sub(crc & 1));
        }
    }
    !crc
}

#[cfg(test)]
#[path = "joining_zip/tests.rs"]
mod tests;
