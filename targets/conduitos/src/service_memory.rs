//! Admission contract for an x86_64 compositor address space.
//!
//! These ranges do not install mappings or establish isolation. A backend must
//! map image segments RX/RO, private ranges RW+NX and leave every gap unmapped.
use crate::domain_image::{DomainImage, ImageRefusal};

pub const PAGE_BYTES: u64 = 4096;
pub const COMPOSITOR_PIXEL_BYTES: u64 = 8 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MemoryRange {
    start: u64,
    bytes: u64,
}
impl MemoryRange {
    pub const fn start(self) -> u64 {
        self.start
    }
    pub const fn bytes(self) -> u64 {
        self.bytes
    }
    pub const fn end(self) -> u64 {
        self.start + self.bytes
    }
}

/// Finite user memory, excluding Root-owned page tables and trap stacks.
/// Construction is closed: callers cannot admit arbitrary addresses or sizes.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ServiceMemoryLayout {
    ranges: [MemoryRange; 7],
}

impl ServiceMemoryLayout {
    pub const fn compositor() -> Self {
        let sizes = [
            2 * 1024 * 1024,
            PAGE_BYTES,
            64 * 1024,
            64 * 1024,
            256 * 1024,
            COMPOSITOR_PIXEL_BYTES,
            COMPOSITOR_PIXEL_BYTES,
        ];
        let mut ranges = [MemoryRange { start: 0, bytes: 0 }; 7];
        let mut start = 0x0040_0000;
        let mut index = 0;
        while index < ranges.len() {
            ranges[index] = MemoryRange {
                start,
                bytes: sizes[index],
            };
            start += sizes[index] + PAGE_BYTES;
            index += 1;
        }
        Self { ranges }
    }
    pub const fn image(self) -> MemoryRange {
        self.ranges[0]
    }
    pub const fn exchange(self) -> MemoryRange {
        self.ranges[1]
    }
    pub const fn output(self) -> MemoryRange {
        self.ranges[2]
    }
    pub const fn stack(self) -> MemoryRange {
        self.ranges[3]
    }
    /// Ceiling for retained-core metadata; no Face/GraphicsScene fit is claimed.
    pub const fn metadata(self) -> MemoryRange {
        self.ranges[4]
    }
    pub const fn pixels(self) -> MemoryRange {
        self.ranges[5]
    }
    /// Peak replacement/upload storage, separately charged from retained pixels.
    pub const fn scratch(self) -> MemoryRange {
        self.ranges[6]
    }
    pub const fn mapped_bytes(self) -> u64 {
        let mut bytes = 0;
        let mut index = 0;
        while index < self.ranges.len() {
            bytes += self.ranges[index].bytes;
            index += 1;
        }
        bytes
    }

    /// Recheck arithmetic at the admission boundary before parsing the image.
    /// Guard gaps include one page below and above the complete user extent.
    fn validate(self) -> Result<(), ImageRefusal> {
        let mut previous_end = self
            .image()
            .start
            .checked_sub(PAGE_BYTES)
            .ok_or(ImageRefusal::InvalidMapping)?;
        for range in self.ranges {
            let end = range
                .start
                .checked_add(range.bytes)
                .ok_or(ImageRefusal::InvalidMapping)?;
            if range.bytes == 0
                || !range.bytes.is_multiple_of(PAGE_BYTES)
                || !range.start.is_multiple_of(PAGE_BYTES)
                || previous_end
                    .checked_add(PAGE_BYTES)
                    .is_none_or(|minimum| range.start < minimum)
                || end
                    .checked_add(PAGE_BYTES)
                    .is_none_or(|guard_end| guard_end > 0x0000_8000_0000_0000)
            {
                return Err(ImageRefusal::InvalidMapping);
            }
            previous_end = end;
        }
        Ok(())
    }
}

/// An immutable ELF admitted against the fixed service profile. Writable ELF
/// segments are forbidden; all mutable state comes from separate private grants.
/// The x86_64 backend must charge its page tables and trap stack in addition to
/// `mapped_bytes`; neither is a user-visible allocation from this descriptor.
pub struct ServiceImage<'a> {
    image: DomainImage<'a>,
    layout: ServiceMemoryLayout,
}
impl<'a> ServiceImage<'a> {
    pub fn compositor(bytes: &'a [u8]) -> Result<Self, ImageRefusal> {
        let layout = ServiceMemoryLayout::compositor();
        layout.validate()?;
        let image =
            DomainImage::parse_in_window(bytes, 62, layout.image().start, layout.image().bytes)?;
        Ok(Self { image, layout })
    }
    pub fn image(&self) -> &DomainImage<'a> {
        &self.image
    }
    pub const fn layout(&self) -> ServiceMemoryLayout {
        self.layout
    }
}

#[cfg(test)]
#[path = "service_memory/tests.rs"]
mod tests;
