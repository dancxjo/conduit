//! Bounded interpretation of the reviewed four-level LoongArch page regime.
//! This pure leaf neither reads CSRs/table memory nor grants scanout custody.
const PAGE: u64 = 4096;
const ADDRESS: u64 = 0x0000_ffff_ffff_f000;
const PWCL: u64 = 12 | (9 << 5) | (21 << 10) | (9 << 15) | (30 << 20) | (9 << 25);
const PWCH: u64 = 39 | (9 << 6);
const MAX_BYTES: u64 = 8 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
pub struct Regime {
    pub crmd: u64,
    pub pwcl: u64,
    pub pwch: u64,
    pub stlbps: u64,
    pub pgdh: u64,
    pub direct_windows: [u64; 4],
    pub physical_bits: u8,
    pub virtual_bits: u8,
    pub reduced_address_config: u64,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
pub struct MappingEvidence {
    pub virtual_start: u64,
    pub physical_start: u64,
    pub byte_length: u64,
    pub checked_pages: u32,
    pub descriptors_read: u32,
    pub memory_access_type: u8,
    pub regime: Regime,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Refusal {
    Extent,
    Regime,
    DirectWindow,
    Table,
    Permission,
    PhysicalMismatch,
    MemoryAttribute,
}
impl Refusal {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Extent => "loongarch64-framebuffer-mapping-extent-invalid",
            Self::Regime => "loongarch64-framebuffer-paging-regime-unsupported",
            Self::DirectWindow => "loongarch64-framebuffer-direct-window-unsupported",
            Self::Table => "loongarch64-framebuffer-table-invalid",
            Self::Permission => "loongarch64-framebuffer-not-plv0-writable",
            Self::PhysicalMismatch => "loongarch64-framebuffer-physical-mapping-mismatch",
            Self::MemoryAttribute => "loongarch64-framebuffer-memory-attribute-unsupported",
        }
    }
}
fn canonical_high(address: u64) -> bool {
    address >> 47 == (1 << 17) - 1
}
/// Only the observed high-half 48-bit, four-level, 4KiB regime is eligible.
/// `read` must separately prove complete table-page custody before each actual
/// descriptor read. V, D and software W must already permit Root writes; this
/// validator never modifies a page table or services a dirty/access fault.
pub fn validate(
    virtual_start: u64,
    physical_start: u64,
    byte_length: u64,
    regime: Regime,
    mut read: impl FnMut(u64) -> Result<u64, Refusal>,
) -> Result<MappingEvidence, Refusal> {
    let end = virtual_start
        .checked_add(byte_length)
        .ok_or(Refusal::Extent)?;
    if byte_length == 0
        || byte_length > MAX_BYTES
        || !virtual_start.is_multiple_of(PAGE)
        || !physical_start.is_multiple_of(PAGE)
        || !canonical_high(virtual_start)
        || !canonical_high(end - 1)
    {
        return Err(Refusal::Extent);
    }
    if regime.virtual_bits != 48
        || regime.reduced_address_config != 0
        || !(32..=48).contains(&regime.physical_bits)
        || regime.crmd & 0x1fb != 0xb0
        || regime.pwcl != PWCL
        || regime.pwch != PWCH
        || regime.stlbps != 12
        || regime.pgdh == 0
        || regime.pgdh & !ADDRESS != 0
    {
        return Err(Refusal::Regime);
    }
    let physical_limit = 1_u64 << regime.physical_bits;
    if physical_start
        .checked_add(byte_length)
        .is_none_or(|last| last > physical_limit)
        || regime.pgdh >= physical_limit
    {
        return Err(Refusal::Extent);
    }
    if regime
        .direct_windows
        .iter()
        .any(|window| window & 1 != 0 && window >> 60 == virtual_start >> 60)
    {
        return Err(Refusal::DirectWindow);
    }
    let pages = byte_length.div_ceil(PAGE);
    let mut descriptors = 0;
    let mut attribute = None;
    for page in 0..pages {
        let va = virtual_start + page * PAGE;
        let mut table = regime.pgdh;
        let mut translated = None;
        for level in (0..4).rev() {
            let shift = 12 + 9 * level;
            let entry = read(table + ((va >> shift) & 511) * 8)?;
            descriptors += 1;
            if entry == 0 {
                return Err(Refusal::Table);
            }
            if entry & 1 == 0 {
                if level == 0 || entry & !ADDRESS != 0 || entry >= physical_limit {
                    return Err(Refusal::Table);
                }
                table = entry;
                continue;
            }
            // The software P/W flags are distinct from final TLB fields.
            // Upper NR/NX/RPLV bits are observed but do not authorize a write.
            if entry & !(ADDRESS | 0x1ff | (7 << 61)) != 0 {
                return Err(Refusal::Table);
            }
            if entry & (1 | 2 | 12 | 256) != 1 | 2 | 256 {
                return Err(Refusal::Permission);
            }
            let mat = ((entry >> 4) & 3) as u8;
            if mat == 3 || attribute.is_some_and(|previous| previous != mat) {
                return Err(Refusal::MemoryAttribute);
            }
            let mut base = entry & ADDRESS;
            if level != 0 {
                if level == 3 || entry & (1 << 6) == 0 {
                    return Err(Refusal::Table);
                }
                // Huge-page HGLOBAL occupies bit12, not a physical address bit.
                base &= !(1 << 12);
            }
            let offset_mask = (1 << shift) - 1;
            if base & offset_mask != 0 || base >= physical_limit {
                return Err(Refusal::Table);
            }
            translated = Some(base | (va & offset_mask));
            attribute = Some(mat);
            break;
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
        memory_access_type: attribute.ok_or(Refusal::MemoryAttribute)?,
        regime,
    })
}
#[cfg(test)]
#[path = "mapping_tests.rs"]
mod tests;
