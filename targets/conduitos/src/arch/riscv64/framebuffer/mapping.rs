//! Bounded Sv39 interpretation; the caller supplies checked table reads.
const PAGE: u64 = 4096;
const PPN: u64 = (1 << 44) - 1;
const ALLOWED: u64 = (1 << 54) - 1;
const MAX_BYTES: u64 = 8 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
pub struct MappingEvidence {
    pub virtual_start: u64,
    pub physical_start: u64,
    pub byte_length: u64,
    pub checked_pages: u32,
    pub descriptors_read: u32,
    pub satp: u64,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Refusal {
    Extent,
    Regime,
    Table,
    Permission,
    PhysicalMismatch,
    MemoryAttribute,
}
impl Refusal {
    #[cfg_attr(test, allow(dead_code))]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Extent => "riscv64-framebuffer-mapping-extent-invalid",
            Self::Regime => "riscv64-framebuffer-sv39-required",
            Self::Table => "riscv64-framebuffer-table-invalid",
            Self::Permission => "riscv64-framebuffer-not-supervisor-writable",
            Self::PhysicalMismatch => "riscv64-framebuffer-physical-mapping-mismatch",
            Self::MemoryAttribute => "riscv64-framebuffer-memory-attribute-unsupported",
        }
    }
}
fn canonical(address: u64) -> bool {
    let high = address >> 39;
    high == if address & (1 << 38) == 0 {
        0
    } else {
        (1 << 25) - 1
    }
}
/// No writes or speculative pointer dereferences occur here. `read` must only
/// return an actual descriptor after validating the table's physical custody.
/// The fixed profile accepts Sv39 with PBMT=0 (inherited PMA), no Svnapot, and
/// already accessed/dirty supervisor-writable mappings. It never sets A/D bits.
pub fn validate(
    virtual_start: u64,
    physical_start: u64,
    byte_length: u64,
    satp: u64,
    mut read: impl FnMut(u64) -> Result<u64, Refusal>,
) -> Result<MappingEvidence, Refusal> {
    let end = virtual_start
        .checked_add(byte_length)
        .ok_or(Refusal::Extent)?;
    if byte_length == 0
        || byte_length > MAX_BYTES
        || !virtual_start.is_multiple_of(PAGE)
        || !physical_start.is_multiple_of(PAGE)
        || !canonical(virtual_start)
        || !canonical(end - 1)
        || physical_start
            .checked_add(byte_length)
            .is_none_or(|end| end > (1 << 56))
    {
        return Err(Refusal::Extent);
    }
    if satp >> 60 != 8 || satp & PPN == 0 {
        return Err(Refusal::Regime);
    }
    let pages = byte_length.div_ceil(PAGE);
    let mut descriptors = 0;
    for page in 0..pages {
        let virtual_address = virtual_start + page * PAGE;
        let mut table = (satp & PPN) << 12;
        let mut translated = None;
        for level in (0..3).rev() {
            let shift = 12 + level * 9;
            let index = (virtual_address >> shift) & 511;
            let entry = read(table + index * 8)?;
            descriptors += 1;
            if entry & !ALLOWED != 0 {
                return Err(Refusal::MemoryAttribute);
            }
            if entry & 1 == 0 || entry & 6 == 4 {
                return Err(Refusal::Table);
            }
            let base = ((entry >> 10) & PPN) << 12;
            if entry & 10 != 0 {
                // R+W, A+D, and U=0. X is observed but does not grant extra
                // writer authority; executable inherited mappings remain Root.
                if entry & (2 | 4 | 16 | 64 | 128) != 2 | 4 | 64 | 128 {
                    return Err(Refusal::Permission);
                }
                let offset_mask = (1 << shift) - 1;
                if base & offset_mask != 0 {
                    return Err(Refusal::Table);
                }
                translated = Some(base | (virtual_address & offset_mask));
                break;
            }
            if level == 0 || base == 0 || entry & (16 | 64 | 128) != 0 {
                return Err(Refusal::Table);
            }
            table = base;
        }
        if translated != Some(physical_start + page * PAGE) {
            return Err(Refusal::PhysicalMismatch);
        }
    }
    Ok(MappingEvidence {
        virtual_start,
        physical_start,
        byte_length,
        checked_pages: pages as u32,
        descriptors_read: descriptors,
        satp,
    })
}
#[cfg(test)]
#[path = "mapping_tests.rs"]
mod tests;
