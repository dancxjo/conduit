//! One actual flash owner, with two checked disjoint Root integrity regions.
//! Region handles expose no erase operation or address outside their partition.
use conduit_environment_runtime::display_evidence_journal::{Refusal, Storage};
use core::cell::RefCell;
use embassy_sync::blocking_mutex::{raw::CriticalSectionRawMutex, Mutex};
use embedded_storage::nor_flash::{NorFlash, ReadNorFlash};
use esp_storage::FlashStorage;
use static_cell::StaticCell;
type Base = Mutex<CriticalSectionRawMutex, RefCell<FlashStorage<'static>>>;
static FLASH: StaticCell<Base> = StaticCell::new();
#[repr(align(4))]
struct Aligned([u8; 64]);
pub struct Region {
    base: &'static Base,
    start: u32,
    length: u32,
}
pub struct Regions {
    pub evidence: Region,
    pub observations: Region,
}
/// Consume the actual FLASH peripheral exactly once. Narrow handles are issued
/// only after checking the real table, including all overlapping partitions.
#[inline(never)]
pub fn prepare(flash: esp_hal::peripherals::FLASH<'static>) -> Result<Regions, &'static str> {
    let mut flash = FlashStorage::new(flash);
    if ReadNorFlash::capacity(&flash) != 0x400000 {
        return Err("evidence-flash-geometry");
    }
    let mut bytes = alloc::boxed::Box::new([0u8; 3072]);
    let table =
        esp_bootloader_esp_idf::partitions::read_partition_table(&mut flash, &mut bytes[..])
            .map_err(|_| "evidence-partition-table")?;
    let mut counts = [0; 2];
    for part in table.iter() {
        let start = part.offset();
        let end = start
            .checked_add(part.len())
            .ok_or("evidence-partition-overflow")?;
        let expected = if part.label().starts_with(b"evidence\0") {
            Some((0, 0x290000, 0x160000))
        } else if part.label().starts_with(b"observations\0") {
            Some((1, 0x3f0000, 0x10000))
        } else {
            None
        };
        if let Some((index, offset, length)) = expected {
            if start != offset
                || part.len() != length
                || part.raw_type() != 1
                || part.raw_subtype() != 6
                || part.flags() != 0
            {
                return Err("evidence-partition-geometry");
            }
            counts[index] += 1;
        } else if start < 0x400000 && end > 0x290000 {
            return Err("evidence-partition-overlap");
        }
    }
    if counts != [1, 1] {
        return Err("evidence-partition-unavailable");
    }
    let base = FLASH
        .try_init(Mutex::new(RefCell::new(flash)))
        .ok_or("evidence-flash-already-owned")?;
    Ok(Regions {
        evidence: Region {
            base,
            start: 0x290000,
            length: 0x160000,
        },
        observations: Region {
            base,
            start: 0x3f0000,
            length: 0x10000,
        },
    })
}
impl Region {
    fn bound(&self, at: u32, n: usize) -> Result<u32, Refusal> {
        let n = u32::try_from(n).map_err(|_| Refusal::Geometry)?;
        if !at.is_multiple_of(4)
            || !n.is_multiple_of(4)
            || at.checked_add(n).is_none_or(|end| end > self.length)
        {
            return Err(Refusal::Geometry);
        }
        Ok(self.start + at)
    }
}
impl Storage for Region {
    fn capacity(&self) -> u32 {
        self.length
    }
    fn read(&mut self, at: u32, output: &mut [u8]) -> Result<(), Refusal> {
        let at = self.bound(at, output.len())?;
        self.base.lock(|base| {
            let mut flash = base.borrow_mut();
            let mut aligned = Aligned([0; 64]);
            for (i, chunk) in output.chunks_mut(64).enumerate() {
                ReadNorFlash::read(
                    &mut *flash,
                    at + (i * 64) as u32,
                    &mut aligned.0[..chunk.len()],
                )
                .map_err(|_| Refusal::Media)?;
                chunk.copy_from_slice(&aligned.0[..chunk.len()]);
            }
            Ok(())
        })
    }
    fn program(&mut self, at: u32, bytes: &[u8]) -> Result<(), Refusal> {
        let at = self.bound(at, bytes.len())?;
        self.base.lock(|base| {
            let mut flash = base.borrow_mut();
            let mut aligned = Aligned([0; 64]);
            for (i, chunk) in bytes.chunks(64).enumerate() {
                aligned.0[..chunk.len()].copy_from_slice(chunk);
                NorFlash::write(&mut *flash, at + (i * 64) as u32, &aligned.0[..chunk.len()])
                    .map_err(|_| Refusal::Media)?;
            }
            Ok(())
        })
    }
}
