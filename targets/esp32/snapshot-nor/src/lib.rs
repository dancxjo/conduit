//! Exclusive physical NOR slot for Blue's checked durable partition. Geometry
//! is not a grant: Root must retain the actual flash owner and admit the snapshot
//! operation separately. No erase, allocation, Ready offer or grant issuer.
//!
//! Integration gate: a current admitted native operation must exclusively own
//! the flash execution window. HAL cache/interrupt protection alone does not
//! exclude an in-flight DHT pulse capture or TM1637 transaction. The Root must
//! serialize those physical operations before calling this adapter; geometry,
//! constructors and receipt bytes supply no such permission.
#![no_std]
use conduit_resource_snapshot::nor::{NorRefusal, NorStorage};
use embedded_storage::nor_flash::{NorFlash, ReadNorFlash};
pub const FLASH_BYTES: u32 = 4 * 1024 * 1024;
pub const DURABLE_START: u32 = 0x348000;
pub const DURABLE_END: u32 = 0x3e8000;
pub const PROGRAM_BYTES: usize = 256;
/// Independently checked physical selection, not possession or authority. Slots
/// may share erase blocks; neither this type nor the adapter can erase them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SlotGeometry {
    offset: u32,
    bytes: u32,
}
impl SlotGeometry {
    pub fn checked(offset: u32, bytes: u32) -> Result<Self, NorRefusal> {
        if offset < DURABLE_START
            || !offset.is_multiple_of(PROGRAM_BYTES as u32)
            || bytes < 3 * PROGRAM_BYTES as u32
            || !bytes.is_multiple_of(PROGRAM_BYTES as u32)
            || offset
                .checked_add(bytes)
                .is_none_or(|end| end > DURABLE_END)
        {
            return Err(NorRefusal::Geometry);
        }
        Ok(Self { offset, bytes })
    }
    pub const fn offset(self) -> u32 {
        self.offset
    }
    pub const fn bytes(self) -> u32 {
        self.bytes
    }
}
#[repr(C, align(4))]
struct Aligned([u32; PROGRAM_BYTES / 4]);
impl Aligned {
    fn bytes_mut(&mut self) -> &mut [u8; PROGRAM_BYTES] {
        // SAFETY: owned aligned mutable RAM; u32 has no invalid bit patterns,
        // and the byte array has exactly the same extent as its backing words.
        unsafe { &mut *(self.0.as_mut_ptr().cast::<[u8; PROGRAM_BYTES]>()) }
    }
}
/// Sole storage owner, including one funded 256-byte aligned bounce buffer.
/// The actual Root must keep this private beneath its admitted operation owner;
/// receiving resource identities cannot construct it. Preparation neither checks
/// erased media nor writes anything. The shared snapshot mechanism does both
/// publication validation and erased-slot verification at the admitted call.
pub struct SnapshotNor<F> {
    flash: F,
    slot: SlotGeometry,
    bounce: Aligned,
}
impl<F: NorFlash> SnapshotNor<F> {
    /// Custody is returned unchanged on geometry/capacity refusal. Only the real
    /// FlashStorage owner may be supplied by target construction. The generic
    /// parameter permits CPU contract tests; it does not publish a provider.
    pub fn prepare(flash: F, slot: SlotGeometry) -> Result<Self, (F, NorRefusal)> {
        if F::READ_SIZE != 4 || F::WRITE_SIZE != 4 || F::ERASE_SIZE != 4096 {
            return Err((flash, NorRefusal::Geometry));
        }
        if flash.capacity() != FLASH_BYTES as usize {
            return Err((flash, NorRefusal::Capacity));
        }
        Ok(Self {
            flash,
            slot,
            bounce: Aligned([0; PROGRAM_BYTES / 4]),
        })
    }
    pub fn into_flash(self) -> F {
        self.flash
    }
    fn absolute(&self, offset: u32, bytes: usize) -> Result<u32, NorRefusal> {
        let bytes = u32::try_from(bytes).map_err(|_| NorRefusal::Capacity)?;
        if offset
            .checked_add(bytes)
            .is_none_or(|end| end > self.slot.bytes)
        {
            return Err(NorRefusal::Capacity);
        }
        self.slot
            .offset
            .checked_add(offset)
            .ok_or(NorRefusal::Capacity)
    }
}
impl<F: NorFlash> NorStorage for SnapshotNor<F> {
    fn capacity(&self) -> u32 {
        self.slot.bytes
    }
    fn program_unit(&self) -> u32 {
        PROGRAM_BYTES as u32
    }
    fn read(&mut self, offset: u32, bytes: &mut [u8]) -> Result<(), NorRefusal> {
        let mut at = self.absolute(offset, bytes.len())?;
        let mut done = 0;
        while done < bytes.len() {
            let aligned = at & !3;
            let skip = (at - aligned) as usize;
            let count = (bytes.len() - done).min(PROGRAM_BYTES - skip);
            let extent = (skip + count).next_multiple_of(4);
            let bounce = self.bounce.bytes_mut();
            ReadNorFlash::read(&mut self.flash, aligned, &mut bounce[..extent])
                .map_err(|_| NorRefusal::Media)?;
            bytes[done..done + count].copy_from_slice(&bounce[skip..skip + count]);
            done += count;
            at += count as u32;
        }
        Ok(())
    }
    fn program(&mut self, offset: u32, bytes: &[u8]) -> Result<(), NorRefusal> {
        if bytes.len() != PROGRAM_BYTES || !offset.is_multiple_of(PROGRAM_BYTES as u32) {
            return Err(NorRefusal::Geometry);
        }
        let absolute = self.absolute(offset, bytes.len())?;
        let bounce = self.bounce.bytes_mut();
        bounce.copy_from_slice(bytes);
        // NorFlash::write ONLY. Storage::write performs hidden erase/rewrite and
        // must never be used here. Aligned RAM avoids HAL's 4KiB stack fallback.
        NorFlash::write(&mut self.flash, absolute, bounce).map_err(|_| NorRefusal::Media)
    }
}
#[cfg(target_arch = "xtensa")]
pub mod hardware {
    use super::*;
    pub type Esp32SnapshotNor<'d> = SnapshotNor<esp_storage::FlashStorage<'d>>;
    /// Consume the actual HAL FLASH owner after bootstrap's boot-only read.
    /// Default esp-storage critical-section and OtherCoreRunning refusal remain
    /// enabled: no unchecked multicore_ignore/auto-parking path is selected.
    /// Caller must fund ROM operation latency/XIP exclusion before publication,
    /// and prove no conflicting GPIO transaction is in flight. Neither this
    /// constructor nor HAL critical sections establish that admitted window.
    ///
    /// # Panics
    /// The underlying FlashStorage constructor can panic if its boot-header
    /// ROM read fails. It must run once with the original HAL FLASH owner,
    /// during checked Root construction before any writable offer is exposed.
    pub fn prepare<'d>(
        flash: esp_hal::peripherals::FLASH<'d>,
        slot: SlotGeometry,
    ) -> Result<Esp32SnapshotNor<'d>, (esp_storage::FlashStorage<'d>, NorRefusal)> {
        SnapshotNor::prepare(esp_storage::FlashStorage::new(flash), slot)
    }
}
#[cfg(test)]
extern crate std;
#[cfg(test)]
mod tests;
