use super::*;
use conduit_resource_snapshot::nor::NorStorage;
use conduit_rp2040_flash_layout::FLASH_BYTES;
use embassy_rp::{
    flash::{Blocking, Flash},
    peripherals::FLASH,
};

const _: () = assert!(embassy_rp::flash::PAGE_SIZE == PROGRAM_BYTES as usize);
const _: () = assert!(embassy_rp::flash::READ_SIZE == 1);

pub type Device = Flash<'static, FLASH, Blocking, { FLASH_BYTES as usize }>;
#[repr(align(4))]
struct Page([u8; PROGRAM_BYTES as usize]);

/// Owns the actual sole FLASH peripheral and bounded RAM programming buffer.
/// Embassy's pinned blocking driver executes from RAM while pausing core 1,
/// excluding interrupts and waiting for XIP DMA. Root must also admit the
/// blocking work/latency and retain current capability/Play authority. This
/// adapter cannot manufacture those facts from a Slot or a resource descriptor.
pub struct SnapshotNor {
    flash: Device,
    slot: Slot,
    page: Page,
    failed: bool,
}
impl SnapshotNor {
    pub fn new(flash: Device, slot: Slot) -> Self {
        Self {
            flash,
            slot,
            page: Page([255; PROGRAM_BYTES as usize]),
            failed: false,
        }
    }
    /// Native retirement must finish before Root withdraws this physical owner
    /// for another residence. Returning possession does not grant a new Play.
    pub fn into_flash(self) -> Device {
        self.flash
    }
    pub fn slot(&self) -> Slot {
        self.slot
    }
}
impl NorStorage for SnapshotNor {
    fn capacity(&self) -> u32 {
        self.slot.bytes()
    }
    fn program_unit(&self) -> u32 {
        PROGRAM_BYTES
    }
    fn read(&mut self, offset: u32, bytes: &mut [u8]) -> Result<(), NorRefusal> {
        let physical = self.slot.at(offset, bytes.len())?;
        self.flash
            .blocking_read(physical, bytes)
            .map_err(|_| NorRefusal::Media)
    }
    fn program(&mut self, offset: u32, bytes: &[u8]) -> Result<(), NorRefusal> {
        let physical = self.slot.program_at(offset, bytes.len())?;
        if self.failed {
            return Err(NorRefusal::Media);
        }
        // Always a full aligned page in retained RAM: no HAL partial-page pad,
        // read/modify/erase/write path, flash-backed source or hidden 4KiB buffer.
        self.page.0.copy_from_slice(bytes);
        if self.flash.blocking_write(physical, &self.page.0).is_err() {
            self.failed = true;
            return Err(NorRefusal::Media);
        }
        Ok(())
    }
}
