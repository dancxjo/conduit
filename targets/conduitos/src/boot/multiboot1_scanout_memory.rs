//! Pinned Multiboot memory-map checks before claiming firmware scanout memory.
//! Discovery does not itself grant display ownership or permit writes.
#![cfg_attr(test, allow(dead_code))]
use super::multiboot1_framebuffer::FramebufferFacts;
use core::ops::Range;

const ENTRY_BYTES: usize = 24;
const MAX_ENTRIES: usize = 256; // Limine v12.5.2 MEMMAP_MAX.
const MAX_BYTES: usize = ENTRY_BYTES * MAX_ENTRIES;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScanoutMemoryEvidence {
    pub memory_map_entries: u16,
    pub covered_by_reserved_entry: bool,
}

/// Check the pinned loader's raw E820-compatible map and Root-owned extents.
/// A framebuffer may occupy a reserved region or a hole outside reported RAM;
/// neither case admits overlap with usable/ACPI/bad/unknown memory or owners.
///
/// # Safety
/// The pointer and advertised memory map must be readable identity-mapped
/// bootloader storage from the matching Multiboot entry. No writes occur.
pub unsafe fn validate_multiboot1_scanout_memory(
    magic: u32,
    information: u32,
    framebuffer: &FramebufferFacts,
    owned: &[Range<u64>],
) -> Result<ScanoutMemoryEvidence, &'static str> {
    if magic != 0x2bad_b002 || information == 0 || information.checked_add(118).is_none() {
        return Err("ia32-scanout-memory-information-invalid");
    }
    let pointer = information as usize as *const u8;
    let word = |offset| unsafe { pointer.add(offset).cast::<u32>().read_unaligned() };
    if word(0) & (1 << 6) == 0 {
        return Err("ia32-scanout-memory-map-absent");
    }
    let length = word(44) as usize;
    let address = word(48);
    if length == 0
        || length > MAX_BYTES
        || address == 0
        || address.checked_add(length as u32).is_none()
    {
        return Err("ia32-scanout-memory-map-invalid");
    }
    let extent = extent(framebuffer)?;
    if overlaps(
        &extent,
        &(u64::from(information)..u64::from(information) + 118),
    ) || overlaps(
        &extent,
        &(u64::from(address)..u64::from(address) + length as u64),
    ) {
        return Err("ia32-scanout-overlaps-boot-information");
    }
    let mut bytes = [0_u8; MAX_BYTES];
    unsafe {
        core::ptr::copy_nonoverlapping(address as usize as *const u8, bytes.as_mut_ptr(), length)
    };
    validate(&bytes[..length], extent, owned)
}

fn extent(framebuffer: &FramebufferFacts) -> Result<Range<u64>, &'static str> {
    let start = u64::from(framebuffer.physical_start);
    let end = start
        .checked_add(u64::from(framebuffer.byte_len))
        .ok_or("ia32-scanout-memory-extent-invalid")?;
    if start == 0 || end <= start || end > 1_u64 << 32 {
        return Err("ia32-scanout-memory-extent-invalid");
    }
    Ok(start..end)
}

fn validate(
    bytes: &[u8],
    framebuffer: Range<u64>,
    owned: &[Range<u64>],
) -> Result<ScanoutMemoryEvidence, &'static str> {
    if bytes.is_empty() || bytes.len() > MAX_BYTES || !bytes.len().is_multiple_of(ENTRY_BYTES) {
        return Err("ia32-scanout-memory-map-invalid");
    }
    if owned
        .iter()
        .any(|range| range.end <= range.start || overlaps(&framebuffer, range))
    {
        return Err("ia32-scanout-overlaps-owned-storage");
    }
    let mut reserved = false;
    for entry in bytes.chunks_exact(ENTRY_BYTES) {
        let word = |offset| u32::from_le_bytes(entry[offset..offset + 4].try_into().unwrap());
        if word(0) != 20 {
            return Err("ia32-scanout-memory-entry-unsupported");
        }
        let start = u64::from_le_bytes(entry[4..12].try_into().unwrap());
        let length = u64::from_le_bytes(entry[12..20].try_into().unwrap());
        let end = start
            .checked_add(length)
            .filter(|end| *end > start)
            .ok_or("ia32-scanout-memory-entry-invalid")?;
        if overlaps(&framebuffer, &(start..end)) {
            if word(20) != 2 {
                return Err("ia32-scanout-overlaps-nonreserved-memory");
            }
            reserved |= start <= framebuffer.start && end >= framebuffer.end;
        }
    }
    Ok(ScanoutMemoryEvidence {
        memory_map_entries: (bytes.len() / ENTRY_BYTES) as u16,
        covered_by_reserved_entry: reserved,
    })
}
fn overlaps(left: &Range<u64>, right: &Range<u64>) -> bool {
    left.start < right.end && right.start < left.end
}

#[cfg(test)]
mod tests {
    use super::*;
    fn entry(start: u64, length: u64, kind: u32) -> [u8; 24] {
        let mut bytes = [0; 24];
        bytes[..4].copy_from_slice(&20_u32.to_le_bytes());
        bytes[4..12].copy_from_slice(&start.to_le_bytes());
        bytes[12..20].copy_from_slice(&length.to_le_bytes());
        bytes[20..].copy_from_slice(&kind.to_le_bytes());
        bytes
    }
    #[test]
    fn reserved_scanout_and_unreported_mmio_remain_distinct() {
        let reserved = entry(0x8000_0000, 0x100_0000, 2);
        assert!(
            validate(&reserved, 0x8000_0000..0x8040_0000, &[])
                .unwrap()
                .covered_by_reserved_entry
        );
        let ram = entry(0, 0x2000_0000, 1);
        assert!(
            !validate(&ram, 0x8000_0000..0x8040_0000, &[])
                .unwrap()
                .covered_by_reserved_entry
        );
        assert!(validate(&ram, 0x8000_0000..0x8040_0000, &[0x100000..0x400000]).is_ok());
    }
    #[test]
    fn every_nonreserved_overlap_or_owned_extent_refuses() {
        for kind in [1, 3, 4, 5, 0, 99] {
            assert!(
                validate(
                    &entry(0x8000_0000, 4096, kind),
                    0x8000_0000..0x8040_0000,
                    &[]
                )
                .is_err()
            );
        }
        let map = entry(0, 0x2000_0000, 1);
        assert!(validate(&map, 0x8000_0000..0x8040_0000, &[0x8000_1000..0x8000_2000]).is_err());
        assert!(validate(&map, 0x8000_0000..0x8040_0000, &[4..4]).is_err());
    }
    #[test]
    fn malformed_or_conflicting_maps_do_not_hide_ram() {
        for map in [
            vec![],
            vec![0; 23],
            entry(u64::MAX, 4096, 2).to_vec(),
            entry(0, 0, 2).to_vec(),
        ] {
            assert!(validate(&map, 0x8000_0000..0x8040_0000, &[]).is_err());
        }
        let map = [
            entry(0x8000_0000, 0x100_0000, 2),
            entry(0x8000_0000, 4096, 1),
        ]
        .concat();
        assert!(validate(&map, 0x8000_0000..0x8040_0000, &[]).is_err());
        assert!(validate(&vec![0; MAX_BYTES + 24], 0x8000_0000..0x8040_0000, &[]).is_err());
    }
}
