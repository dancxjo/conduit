//! Finite cache-line custody for stores to the observed firmware scanout.
use super::mapping::MAX_SCANOUT_BYTES;
use crate::display::DisplayError;
use core::ops::Range;

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
pub struct CachePolicy {
    pub ctr_el0: u64,
    pub cache_line_bytes: u16,
    pub clean_to_point_of_coherency: bool,
    virtual_start: u64,
    byte_length: u64,
}
impl CachePolicy {
    pub fn prepare(start: u64, bytes: u64, attribute: u8, ctr: u64) -> Result<Self, &'static str> {
        if bytes == 0 || bytes > MAX_SCANOUT_BYTES || start.checked_add(bytes).is_none() {
            return Err("aarch64-framebuffer-cache-extent-invalid");
        }
        let line = match attribute {
            0x44 => 0,
            0xff => 4_u64 << ((ctr >> 16) & 15),
            _ => return Err("aarch64-framebuffer-cache-policy-unsupported"),
        };
        // Whole-line ownership keeps maintenance inside this exact scanout.
        if line != 0
            && (!(16..=256).contains(&line)
                || !start.is_multiple_of(line)
                || !bytes.is_multiple_of(line))
        {
            return Err("aarch64-framebuffer-cache-geometry-unsupported");
        }
        Ok(Self {
            ctr_el0: ctr,
            cache_line_bytes: line as u16,
            clean_to_point_of_coherency: line != 0,
            virtual_start: start,
            byte_length: bytes,
        })
    }
    /// Validate the complete maintenance envelope before any pixel store.
    pub fn lines(&self, offset: usize, bytes: usize) -> Result<Option<Range<u64>>, DisplayError> {
        let end = (offset as u64)
            .checked_add(bytes as u64)
            .filter(|end| *end <= self.byte_length)
            .ok_or(DisplayError::InvalidExtent)?;
        if bytes == 0 || !self.clean_to_point_of_coherency {
            return Ok(None);
        }
        let line = u64::from(self.cache_line_bytes);
        let begin = (offset as u64) & !(line - 1);
        let end = end
            .checked_add(line - 1)
            .ok_or(DisplayError::InvalidExtent)?
            & !(line - 1);
        Ok(Some(self.virtual_start + begin..self.virtual_start + end))
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cacheable_writes_clean_each_touched_line_without_leaving_the_base() {
        let cache = CachePolicy::prepare(0x8000, 4096, 0xff, 4 << 16).unwrap();
        assert_eq!(cache.cache_line_bytes, 64);
        assert_eq!(cache.lines(60, 8).unwrap(), Some(0x8000..0x8080));
        assert_eq!(cache.lines(4092, 4).unwrap(), Some(0x8fc0..0x9000));
        assert!(cache.lines(4092, 8).is_err());
        assert!(cache.lines(usize::MAX, 8).is_err());
        assert_eq!(cache.lines(4096, 0).unwrap(), None);
    }
    #[test]
    fn noncacheable_memory_needs_only_the_completion_barrier() {
        let cache = CachePolicy::prepare(0x8000, 4096, 0x44, 0).unwrap();
        assert!(!cache.clean_to_point_of_coherency);
        assert_eq!(cache.lines(0, 4096).unwrap(), None);
        assert!(cache.lines(0, 4097).is_err());
    }
    #[test]
    fn unsupported_or_partial_line_geometry_refuses_before_any_store() {
        for (start, bytes, attr, ctr) in [
            (0x8000, 4096, 0x00, 4 << 16),
            (0x8000, 4096, 0xff, 15 << 16),
            (0x8004, 4096, 0xff, 4 << 16),
            (0x8000, 4092, 0xff, 4 << 16),
            (0x8000, MAX_SCANOUT_BYTES + 1, 0xff, 4 << 16),
        ] {
            assert!(CachePolicy::prepare(start, bytes, attr, ctr).is_err());
        }
    }
}
