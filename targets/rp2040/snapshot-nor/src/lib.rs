#![no_std]
//! Physical immutable-slot adapter, not a residence allocator or permission.
//! Root separately supplies admitted custody, native capability and execution
//! windows. No constructor reads, programs, erases, publishes Ready or Starts.
use conduit_resource_snapshot::nor::NorRefusal;
use conduit_rp2040_flash_layout::DURABLE;

pub const PROGRAM_BYTES: u32 = 256;

/// Descriptive subrange of the checked durable region; no physical possession.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Slot {
    physical_offset: u32,
    bytes: u32,
}
impl Slot {
    /// Offset is relative to DURABLE, never an absolute XIP address. Adjacent
    /// slots can share erase blocks: this adapter exposes no erasure operation.
    pub fn new(relative_offset: u32, bytes: u32) -> Result<Self, NorRefusal> {
        if bytes == 0
            || !relative_offset.is_multiple_of(PROGRAM_BYTES)
            || !bytes.is_multiple_of(PROGRAM_BYTES)
        {
            return Err(NorRefusal::Geometry);
        }
        let physical_offset = DURABLE
            .offset()
            .checked_add(relative_offset)
            .ok_or(NorRefusal::Geometry)?;
        if !DURABLE.contains(physical_offset, bytes) {
            return Err(NorRefusal::Geometry);
        }
        Ok(Self {
            physical_offset,
            bytes,
        })
    }
    pub fn bytes(self) -> u32 {
        self.bytes
    }
    pub fn physical_offset(self) -> u32 {
        self.physical_offset
    }
    #[cfg(any(feature = "rp2040", test))]
    fn at(self, offset: u32, length: usize) -> Result<u32, NorRefusal> {
        let length = u32::try_from(length).map_err(|_| NorRefusal::Geometry)?;
        if offset
            .checked_add(length)
            .is_none_or(|end| end > self.bytes)
        {
            return Err(NorRefusal::Geometry);
        }
        self.physical_offset
            .checked_add(offset)
            .ok_or(NorRefusal::Geometry)
    }
    #[cfg(any(feature = "rp2040", test))]
    fn program_at(self, offset: u32, length: usize) -> Result<u32, NorRefusal> {
        if !offset.is_multiple_of(PROGRAM_BYTES) || length != PROGRAM_BYTES as usize {
            return Err(NorRefusal::Geometry);
        }
        self.at(offset, length)
    }
}

#[cfg(feature = "rp2040")]
mod hardware;
#[cfg(feature = "rp2040")]
pub use hardware::{Device, SnapshotNor};

#[cfg(test)]
mod tests;
