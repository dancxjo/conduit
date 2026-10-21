//! Closed finite memory profiles for two x86_64 service address spaces.
//!
//! These ranges do not install mappings or establish isolation. A backend must
//! map image segments RX/RO, private ranges RW+NX and leave every gap unmapped.
use crate::domain_image::{DomainImage, ImageRefusal};

pub const PAGE_BYTES: u64 = 4096;
pub const COMPOSITOR_PIXEL_BYTES: u64 = 8 * 1024 * 1024;
/// One physical slot per profile. This is not an arbitrary service allocator.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ServiceProfile {
    Compositor,
    Draft,
}
impl ServiceProfile {
    pub const fn layout(self) -> ServiceMemoryLayout {
        match self {
            Self::Compositor => ServiceMemoryLayout::compositor(),
            Self::Draft => ServiceMemoryLayout::draft(),
        }
    }
}

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
    profile: ServiceProfile,
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
        Self::from_sizes(ServiceProfile::Compositor, sizes)
    }
    /// A bounded draft-state ceiling, not proof that a full Crèche implementation fits.
    pub const fn draft() -> Self {
        Self::from_sizes(
            ServiceProfile::Draft,
            [
                256 * 1024,
                PAGE_BYTES,
                16 * 1024,
                64 * 1024,
                256 * 1024,
                0,
                0,
            ],
        )
    }
    const fn from_sizes(profile: ServiceProfile, sizes: [u64; 7]) -> Self {
        let mut ranges = [MemoryRange { start: 0, bytes: 0 }; 7];
        let mut start = 0x0040_0000;
        let mut index = 0;
        while index < ranges.len() {
            if sizes[index] != 0 {
                ranges[index] = MemoryRange {
                    start,
                    bytes: sizes[index],
                };
                start += sizes[index] + PAGE_BYTES;
            }
            index += 1;
        }
        Self { profile, ranges }
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
    pub const fn pixels(self) -> Option<MemoryRange> {
        match self.profile {
            ServiceProfile::Compositor => Some(self.ranges[5]),
            ServiceProfile::Draft => None,
        }
    }
    /// Peak replacement/upload storage, separately charged from retained pixels.
    pub const fn scratch(self) -> Option<MemoryRange> {
        match self.profile {
            ServiceProfile::Compositor => Some(self.ranges[6]),
            ServiceProfile::Draft => None,
        }
    }
    pub const fn profile(self) -> ServiceProfile {
        self.profile
    }
    pub const fn range_count(self) -> usize {
        match self.profile {
            ServiceProfile::Compositor => 7,
            ServiceProfile::Draft => 5,
        }
    }
    pub fn active_ranges(&self) -> &[MemoryRange] {
        &self.ranges[..self.range_count()]
    }
    pub const fn end(self) -> u64 {
        self.ranges[self.range_count() - 1].end()
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
        for (index, range) in self.ranges.into_iter().enumerate() {
            if index >= self.range_count() {
                if range.start != 0 || range.bytes != 0 {
                    return Err(ImageRefusal::InvalidMapping);
                }
                continue;
            }
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
        Self::for_profile(bytes, ServiceProfile::Compositor)
    }
    pub fn for_profile(bytes: &'a [u8], profile: ServiceProfile) -> Result<Self, ImageRefusal> {
        let layout = profile.layout();
        layout.validate()?;
        let image =
            DomainImage::parse_in_window(bytes, 62, layout.image().start, layout.image().bytes)?;
        Ok(Self { image, layout })
    }
    pub const fn profile(&self) -> ServiceProfile {
        self.layout.profile()
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
