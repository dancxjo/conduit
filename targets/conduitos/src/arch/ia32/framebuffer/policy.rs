//! Finite IA-32 identity-map and cache-completion prerequisites.
#![cfg_attr(test, allow(dead_code))]
use crate::display::{DisplayError, DisplayFormat};
use core::ops::Range;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ScanoutPolicy {
    pub physical_start: u32,
    pub byte_length: u32,
    pub cache_line_bytes: u16,
}
impl ScanoutPolicy {
    pub fn prepare(
        start: u32,
        bytes: u32,
        cr0: u32,
        selectors: [u16; 4],
        cpuid_edx: u32,
        cpuid_ebx: u32,
    ) -> Result<Self, &'static str> {
        if cr0 & 1 == 0 || cr0 & (1 << 31) != 0 || selectors != [8, 16, 16, 16] {
            return Err("ia32-scanout-identity-regime-unsupported");
        }
        // CLFSH and SSE2 (MFENCE) are independently advertised.
        if cpuid_edx & ((1 << 19) | (1 << 26)) != (1 << 19) | (1 << 26) {
            return Err("ia32-scanout-store-completion-unsupported");
        }
        let line = ((cpuid_ebx >> 8) & 255) as u16 * 8;
        if !(16..=256).contains(&line) || !line.is_power_of_two() {
            return Err("ia32-scanout-cache-line-unsupported");
        }
        if start == 0
            || bytes == 0
            || bytes > 8 * 1024 * 1024
            || !start.is_multiple_of(u32::from(line))
            || !bytes.is_multiple_of(u32::from(line))
            || u64::from(start) + u64::from(bytes) > 1_u64 << 32
        {
            return Err("ia32-scanout-cache-extent-invalid");
        }
        Ok(Self {
            physical_start: start,
            byte_length: bytes,
            cache_line_bytes: line,
        })
    }
    pub fn lines(
        &self,
        format: DisplayFormat,
        x: u32,
        y: u32,
        pixels: usize,
    ) -> Result<Option<Range<u64>>, DisplayError> {
        let pixels = u32::try_from(pixels).map_err(|_| DisplayError::InvalidExtent)?;
        if y >= format.height || x.checked_add(pixels).is_none_or(|end| end > format.width) {
            return Err(DisplayError::InvalidExtent);
        }
        if pixels == 0 {
            return Ok(None);
        }
        let offset = u64::from(y) * u64::from(format.pitch) + u64::from(x) * 4;
        let end = offset + u64::from(pixels) * 4;
        if end > u64::from(self.byte_length) {
            return Err(DisplayError::BufferTooSmall);
        }
        let line = u64::from(self.cache_line_bytes);
        let start = u64::from(self.physical_start);
        Ok(Some(
            start + (offset / line) * line..start + end.div_ceil(line) * line,
        ))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    const FEATURES: u32 = (1 << 19) | (1 << 26);
    fn policy() -> ScanoutPolicy {
        ScanoutPolicy::prepare(0x80000000, 1920000, 1, [8, 16, 16, 16], FEATURES, 8 << 8).unwrap()
    }
    fn format() -> DisplayFormat {
        DisplayFormat {
            width: 800,
            height: 600,
            pitch: 3200,
            bits_per_pixel: 32,
            red_shift: 16,
            green_shift: 8,
            blue_shift: 0,
        }
    }
    #[test]
    fn inherited_paging_wrong_segments_and_missing_cpu_features_refuse() {
        for (cr0, selectors, features) in [
            (0, [8, 16, 16, 16], FEATURES),
            (1 | (1 << 31), [8, 16, 16, 16], FEATURES),
            (1, [8, 16, 16, 24], FEATURES),
            (1, [8, 16, 16, 16], 1 << 26),
            (1, [8, 16, 16, 16], 1 << 19),
        ] {
            assert!(
                ScanoutPolicy::prepare(0x80000000, 1920000, cr0, selectors, features, 8 << 8)
                    .is_err()
            );
        }
    }
    #[test]
    fn unsupported_lines_and_unowned_partial_lines_refuse() {
        for line_units in [0, 1, 3, 64] {
            assert!(
                ScanoutPolicy::prepare(
                    0x80000000,
                    1920000,
                    1,
                    [8, 16, 16, 16],
                    FEATURES,
                    line_units << 8
                )
                .is_err()
            );
        }
        for (start, bytes) in [
            (4, 1920000),
            (0x80000000, 1920004),
            (0xffffffc0, 128),
            (0x80000000, 9 * 1024 * 1024),
        ] {
            assert!(
                ScanoutPolicy::prepare(start, bytes, 1, [8, 16, 16, 16], FEATURES, 8 << 8).is_err()
            );
        }
    }
    #[test]
    fn row_flushes_cover_changed_lines_and_stay_in_the_owned_extent() {
        let policy = policy();
        let start = u64::from(policy.physical_start);
        assert_eq!(
            policy.lines(format(), 15, 0, 2).unwrap(),
            Some(start..start + 128)
        );
        assert_eq!(
            policy.lines(format(), 0, 599, 800).unwrap(),
            Some(start + 1916800..start + 1920000)
        );
        assert_eq!(policy.lines(format(), 800, 0, 0).unwrap(), None);
        for (x, y, pixels) in [(800, 0, 1), (0, 600, 1), (799, 0, 2), (u32::MAX, 0, 1)] {
            assert!(policy.lines(format(), x, y, pixels).is_err());
        }
    }
}
