//! Exclusive Root integrity backing, never a general flash-writing offer.
//! Only the checked dedicated partition is addressable; no erase entrance.
use conduit_environment_runtime::display_evidence_journal::{Refusal, Storage};
use embedded_storage::nor_flash::{NorFlash, ReadNorFlash};
use esp_storage::FlashStorage;
const START: u32 = 0x290000;
const LENGTH: u32 = 0x170000;
#[repr(align(4))]
struct Aligned([u8; 64]);
pub struct Region {
    flash: FlashStorage<'static>,
}
impl Region {
    #[inline(never)]
    pub fn prepare(flash: esp_hal::peripherals::FLASH<'static>) -> Result<Self, &'static str> {
        let mut flash = FlashStorage::new(flash);
        if ReadNorFlash::capacity(&flash) != 0x400000 {
            return Err("evidence-flash-geometry");
        }
        let mut bytes = alloc::boxed::Box::new([0u8; 3072]);
        let table =
            esp_bootloader_esp_idf::partitions::read_partition_table(&mut flash, &mut bytes[..])
                .map_err(|_| "evidence-partition-table")?;
        let mut count = 0;
        for part in table.iter() {
            let start = part.offset();
            let end = start
                .checked_add(part.len())
                .ok_or("evidence-partition-overflow")?;
            if part.label().starts_with(b"evidence\0") {
                if start != START
                    || part.len() != LENGTH
                    || part.raw_type() != 1
                    || part.raw_subtype() != 6
                    || part.flags() != 0
                {
                    return Err("evidence-partition-geometry");
                }
                count += 1;
            } else if start < START + LENGTH && end > START {
                return Err("evidence-partition-overlap");
            }
        }
        if count != 1 {
            return Err("evidence-partition-unavailable");
        }
        Ok(Self { flash })
    }
    fn bound(&self, at: u32, n: usize) -> Result<u32, Refusal> {
        let n = u32::try_from(n).map_err(|_| Refusal::Geometry)?;
        if !at.is_multiple_of(4)
            || !n.is_multiple_of(4)
            || at.checked_add(n).is_none_or(|end| end > LENGTH)
        {
            return Err(Refusal::Geometry);
        }
        Ok(START + at)
    }
}
impl Storage for Region {
    fn capacity(&self) -> u32 {
        LENGTH
    }
    fn read(&mut self, at: u32, output: &mut [u8]) -> Result<(), Refusal> {
        let at = self.bound(at, output.len())?;
        let mut aligned = Aligned([0; 64]);
        for (i, chunk) in output.chunks_mut(64).enumerate() {
            ReadNorFlash::read(
                &mut self.flash,
                at + (i * 64) as u32,
                &mut aligned.0[..chunk.len()],
            )
            .map_err(|_| Refusal::Media)?;
            chunk.copy_from_slice(&aligned.0[..chunk.len()]);
        }
        Ok(())
    }
    fn program(&mut self, at: u32, bytes: &[u8]) -> Result<(), Refusal> {
        let at = self.bound(at, bytes.len())?;
        let mut aligned = Aligned([0; 64]);
        for (i, chunk) in bytes.chunks(64).enumerate() {
            aligned.0[..chunk.len()].copy_from_slice(chunk);
            NorFlash::write(
                &mut self.flash,
                at + (i * 64) as u32,
                &aligned.0[..chunk.len()],
            )
            .map_err(|_| Refusal::Media)?;
        }
        Ok(())
    }
}
