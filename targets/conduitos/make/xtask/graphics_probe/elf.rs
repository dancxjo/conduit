//! Read-only locations in the exact retained kernel, never a guest authority grant.
const MAXIMUM_ELF_BYTES: usize = 64 * 1024 * 1024;
const MAXIMUM_PROGRAM_HEADERS: usize = 32;
pub const FRAMEBUFFER_REQUEST_ID: [u64; 4] = [
    0xc7b1_dd30_df4c_8b88,
    0x0a82_e883_a194_f07b,
    0x9d58_27dc_d881_dd75,
    0xa314_8604_f6fa_b11b,
];

fn bytes<const N: usize>(image: &[u8], offset: usize) -> Result<[u8; N], &'static str> {
    let end = offset.checked_add(N).ok_or("elf-offset-overflow")?;
    image
        .get(offset..end)
        .ok_or("elf-truncated")?
        .try_into()
        .map_err(|_| "elf-truncated")
}
fn u16_at(image: &[u8], offset: usize) -> Result<u16, &'static str> {
    Ok(u16::from_le_bytes(bytes(image, offset)?))
}
fn u32_at(image: &[u8], offset: usize) -> Result<u32, &'static str> {
    Ok(u32::from_le_bytes(bytes(image, offset)?))
}
fn u64_at(image: &[u8], offset: usize) -> Result<u64, &'static str> {
    Ok(u64::from_le_bytes(bytes(image, offset)?))
}
fn format(image: &[u8]) -> Result<u8, &'static str> {
    if image.len() > MAXIMUM_ELF_BYTES {
        return Err("elf-byte-bound");
    }
    if image.get(..4) != Some(b"\x7fELF") || image.get(5) != Some(&1) {
        return Err("elf-format-unsupported");
    }
    match image.get(4) {
        Some(1 | 2) => Ok(image[4]),
        _ => Err("elf-class-unsupported"),
    }
}
pub fn entry_point(image: &[u8]) -> Result<u64, &'static str> {
    match format(image)? {
        1 => Ok(u64::from(u32_at(image, 24)?)),
        2 => u64_at(image, 24),
        _ => unreachable!(),
    }
}

/// Locate the complete 48-byte Limine request in exactly one file-backed LOAD.
/// Debugger reads must check its ID again in guest memory before using a response.
pub fn framebuffer_request(image: &[u8]) -> Result<u64, &'static str> {
    if format(image)? != 2 {
        return Err("limine-probe-requires-elf64");
    }
    let id: Vec<_> = FRAMEBUFFER_REQUEST_ID
        .into_iter()
        .flat_map(u64::to_le_bytes)
        .collect();
    let mut matches = image
        .windows(id.len())
        .enumerate()
        .filter(|(_, bytes)| *bytes == id);
    let offset = matches.next().ok_or("framebuffer-request-not-in-image")?.0;
    if matches.next().is_some() {
        return Err("framebuffer-request-ambiguous");
    }
    let table = usize::try_from(u64_at(image, 32)?).map_err(|_| "elf-offset-overflow")?;
    let stride = usize::from(u16_at(image, 54)?);
    let count = usize::from(u16_at(image, 56)?);
    if stride < 56 || count == 0 || count > MAXIMUM_PROGRAM_HEADERS {
        return Err("elf-program-header-bound");
    }
    let mut address = None;
    for index in 0..count {
        let header = table
            .checked_add(index.checked_mul(stride).ok_or("elf-offset-overflow")?)
            .ok_or("elf-offset-overflow")?;
        let _: [u8; 56] = bytes(image, header)?;
        if u32_at(image, header)? != 1 {
            continue;
        }
        let start =
            usize::try_from(u64_at(image, header + 8)?).map_err(|_| "elf-offset-overflow")?;
        let extent =
            usize::try_from(u64_at(image, header + 32)?).map_err(|_| "elf-offset-overflow")?;
        let end = start.checked_add(extent).ok_or("elf-offset-overflow")?;
        if end > image.len() {
            return Err("elf-segment-truncated");
        }
        if u64_at(image, header + 40)? < extent as u64 {
            return Err("elf-segment-memory-extent");
        }
        if start <= offset
            && offset
                .checked_add(48)
                .is_some_and(|request_end| request_end <= end)
        {
            let virtual_address = u64_at(image, header + 16)?
                .checked_add((offset - start) as u64)
                .ok_or("elf-address-overflow")?;
            virtual_address
                .checked_add(48)
                .ok_or("elf-address-overflow")?;
            if address.replace(virtual_address).is_some() {
                return Err("elf-request-load-ambiguous");
            }
        }
    }
    address.ok_or("framebuffer-request-not-file-backed")
}

#[cfg(test)]
mod tests {
    use super::*;
    fn image() -> Vec<u8> {
        let mut image = vec![0; 256];
        image[..6].copy_from_slice(b"\x7fELF\x02\x01");
        image[24..32].copy_from_slice(&0xffff_ffff_8000_0000_u64.to_le_bytes());
        image[32..40].copy_from_slice(&64_u64.to_le_bytes());
        image[54..56].copy_from_slice(&56_u16.to_le_bytes());
        image[56..58].copy_from_slice(&1_u16.to_le_bytes());
        image[64..68].copy_from_slice(&1_u32.to_le_bytes());
        image[72..80].copy_from_slice(&128_u64.to_le_bytes());
        image[80..88].copy_from_slice(&0xffff_ffff_8071_0000_u64.to_le_bytes());
        image[96..104].copy_from_slice(&128_u64.to_le_bytes());
        image[104..112].copy_from_slice(&128_u64.to_le_bytes());
        for (index, word) in FRAMEBUFFER_REQUEST_ID.iter().enumerate() {
            image[160 + index * 8..168 + index * 8].copy_from_slice(&word.to_le_bytes());
        }
        image
    }
    #[test]
    fn locates_request_and_entry_independently() {
        let image = image();
        assert_eq!(entry_point(&image), Ok(0xffff_ffff_8000_0000));
        assert_eq!(framebuffer_request(&image), Ok(0xffff_ffff_8071_0020));
    }
    #[test]
    fn every_truncated_prefix_refuses_without_panicking() {
        let image = image();
        for length in 0..image.len() {
            assert!(
                framebuffer_request(&image[..length]).is_err(),
                "prefix {length}"
            );
        }
    }
    #[test]
    fn duplicate_ids_and_overlapping_loads_are_distinct_refusals() {
        let mut duplicate = image();
        let id = duplicate[160..192].to_vec();
        duplicate[208..240].copy_from_slice(&id);
        assert_eq!(
            framebuffer_request(&duplicate),
            Err("framebuffer-request-ambiguous")
        );
        let mut overlap = image();
        overlap[56..58].copy_from_slice(&2_u16.to_le_bytes());
        let header = overlap[64..120].to_vec();
        overlap[120..176].copy_from_slice(&header);
        for (i, word) in FRAMEBUFFER_REQUEST_ID.iter().enumerate() {
            overlap[184 + i * 8..192 + i * 8].copy_from_slice(&word.to_le_bytes());
        }
        assert_eq!(
            framebuffer_request(&overlap),
            Err("elf-request-load-ambiguous")
        );
    }
    #[test]
    fn rejects_unbacked_requests_header_budgets_and_address_overflow() {
        let mut unbacked = image();
        unbacked[96..104].copy_from_slice(&40_u64.to_le_bytes());
        assert_eq!(
            framebuffer_request(&unbacked),
            Err("framebuffer-request-not-file-backed")
        );
        let mut excessive = image();
        excessive[56..58].copy_from_slice(&33_u16.to_le_bytes());
        assert_eq!(
            framebuffer_request(&excessive),
            Err("elf-program-header-bound")
        );
        let mut overflow = image();
        overflow[80..88].copy_from_slice(&u64::MAX.to_le_bytes());
        assert_eq!(framebuffer_request(&overflow), Err("elf-address-overflow"));
    }
    #[test]
    fn elf32_entry_is_usable_for_a_multiboot_debugger_checkpoint() {
        let mut image = vec![0; 52];
        image[..6].copy_from_slice(b"\x7fELF\x01\x01");
        image[24..28].copy_from_slice(&0x10_0020_u32.to_le_bytes());
        assert_eq!(entry_point(&image), Ok(0x10_0020));
        assert_eq!(
            framebuffer_request(&image),
            Err("limine-probe-requires-elf64")
        );
    }
}
