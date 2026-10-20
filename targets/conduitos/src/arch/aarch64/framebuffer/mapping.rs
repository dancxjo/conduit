//! Finite interpretation of actual EL1 write-translation observations.
pub const PAGE_BYTES: u64 = 4096;
pub const MAX_SCANOUT_BYTES: u64 = 8 * 1024 * 1024;
const NORMAL_NC: u8 = 0x44;
const PAR_ADDRESS: u64 = 0x000f_ffff_ffff_f000;

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
pub struct MappingEvidence {
    pub virtual_start: u64,
    pub physical_start: u64,
    pub byte_length: u64,
    pub checked_pages: u32,
    pub par_memory_attribute: u8,
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MappingRefusal {
    Extent,
    Translation,
    PhysicalMismatch,
    CachePolicy,
}
impl MappingRefusal {
    #[cfg_attr(test, allow(dead_code))]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Extent => "aarch64-framebuffer-mapping-extent-invalid",
            Self::Translation => "aarch64-framebuffer-not-root-writable",
            Self::PhysicalMismatch => "aarch64-framebuffer-physical-mapping-mismatch",
            Self::CachePolicy => "aarch64-framebuffer-cache-policy-unsupported",
        }
    }
}

/// No writes occur here. Each returned PAR must be an actual AT S1E1W result.
/// Only the initial firmware RAM scanout's Normal inner/outer NC policy is
/// admitted; a cached or Device mapping requires its own reviewed realization.
pub fn validate(
    virtual_start: u64,
    physical_start: u64,
    byte_length: u64,
    mut translate: impl FnMut(u64) -> u64,
) -> Result<MappingEvidence, MappingRefusal> {
    if byte_length == 0
        || byte_length > MAX_SCANOUT_BYTES
        || !virtual_start.is_multiple_of(PAGE_BYTES)
        || !physical_start.is_multiple_of(PAGE_BYTES)
        || virtual_start.checked_add(byte_length).is_none()
        || physical_start
            .checked_add(byte_length)
            .is_none_or(|end| end > (1_u64 << 48))
    {
        return Err(MappingRefusal::Extent);
    }
    let pages = byte_length.div_ceil(PAGE_BYTES);
    for page in 0..pages {
        let offset = page * PAGE_BYTES;
        let par = translate(virtual_start + offset);
        if par & 1 != 0 {
            return Err(MappingRefusal::Translation);
        }
        if par & PAR_ADDRESS != physical_start + offset {
            return Err(MappingRefusal::PhysicalMismatch);
        }
        if (par >> 56) as u8 != NORMAL_NC {
            return Err(MappingRefusal::CachePolicy);
        }
    }
    Ok(MappingEvidence {
        virtual_start,
        physical_start,
        byte_length,
        checked_pages: pages as u32,
        par_memory_attribute: NORMAL_NC,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn every_page_including_a_partial_last_page_needs_exact_writable_nc_translation() {
        let mut reads = 0;
        let result = validate(0xffff_0000_4000_0000, 0x4000_0000, 4097, |va| {
            reads += 1;
            (u64::from(NORMAL_NC) << 56) | (va & 0x0000_ffff_ffff_f000)
        })
        .unwrap();
        assert_eq!(reads, 2);
        assert_eq!(result.checked_pages, 2);
    }
    #[test]
    fn faults_wrong_physical_addresses_and_cached_mappings_remain_distinct() {
        for (par, error) in [
            (1, MappingRefusal::Translation),
            ((0x44 << 56) | 0x5000, MappingRefusal::PhysicalMismatch),
            ((0xff << 56) | 0x4000, MappingRefusal::CachePolicy),
        ] {
            assert_eq!(validate(0x8000, 0x4000, 4096, |_| par), Err(error));
        }
    }
    #[test]
    fn invalid_extents_refuse_before_a_translation_or_pixel_write() {
        for (va, pa, bytes) in [
            (0x8001, 0x4000, 4096),
            (0x8000, 0x4000, 0),
            (0x8000, 0x4000, MAX_SCANOUT_BYTES + 1),
            (u64::MAX - 4095, 0x4000, 8192),
        ] {
            assert_eq!(
                validate(va, pa, bytes, |_| panic!("invalid extent must not read")),
                Err(MappingRefusal::Extent)
            );
        }
    }
}
