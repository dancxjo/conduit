//! One actual flash owner, with six checked disjoint Root integrity regions.
//! Region handles expose no erase operation or address outside their partition.
use conduit_environment_runtime::display_evidence_journal::{Refusal, Storage};
use core::cell::RefCell;
use embassy_sync::blocking_mutex::{raw::CriticalSectionRawMutex, Mutex};
use embedded_storage::nor_flash::{NorFlash, ReadNorFlash};
use esp_storage::FlashStorage;
use static_cell::StaticCell;
// The autonomous appliance preserves the full signed history geometry while
// retaining a smaller, disjoint display-evidence region. Other profiles retain
// their original partition contract; a mismatched table is refused at boot.
#[cfg(feature = "local-appliance")]
const EVIDENCE_START: u32 = 0x347000;
#[cfg(feature = "local-appliance")]
const EVIDENCE_LENGTH: u32 = 0x1000;
#[cfg(not(feature = "local-appliance"))]
const EVIDENCE_START: u32 = 0x290000;
#[cfg(not(feature = "local-appliance"))]
const EVIDENCE_LENGTH: u32 = 0xa8000;
#[cfg(feature = "local-appliance")]
const EPOCHS_START: u32 = 0x348000;
#[cfg(not(feature = "local-appliance"))]
const EPOCHS_START: u32 = 0x338000;
#[cfg(feature = "local-appliance")]
const NETWORK_START: u32 = 0x3e8000;
#[cfg(not(feature = "local-appliance"))]
const NETWORK_START: u32 = 0x3d8000;
#[cfg(feature = "local-appliance")]
const BODY_START: u32 = 0x3f0000;
#[cfg(not(feature = "local-appliance"))]
const BODY_START: u32 = 0x3e0000;
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
    /// Original CRC-only legacy region is absent in the fresh appliance profile.
    pub observations: Option<Region>,
    /// Five additional immutable signed Boot epochs; no erase entrance.
    pub observation_epochs: [Region; 5],
    pub credentials: CredentialRegion,
    pub network_recovery: NetworkRegion,
    pub body_state: BodyRegion,
}
/// Only this narrow credential partition supports bounded rotating-sector erase.
pub struct CredentialRegion(Region);
/// Append-only recovery evidence; no erase entrance is exposed.
pub struct NetworkRegion(Region);
/// Exclusive Body/key checkpoint custody, with exactly two bounded erase banks.
pub struct BodyRegion(Region);
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
    let mut counts = [0; 6];
    for part in table.iter() {
        let start = part.offset();
        let end = start
            .checked_add(part.len())
            .ok_or("evidence-partition-overflow")?;
        let expected = if part.label().starts_with(b"evidence\0") {
            Some((0, EVIDENCE_START, EVIDENCE_LENGTH))
        } else if part.label().starts_with(b"observations\0") {
            Some((1, 0x3f0000, 0x10000))
        } else if part.label().starts_with(b"credentials\0") {
            Some((2, 0xd000, 0x2000))
        } else if part.label().starts_with(b"net_recovery\0") {
            Some((3, NETWORK_START, 0x8000))
        } else if part.label().starts_with(b"obs_epochs\0") {
            Some((5, EPOCHS_START, 0xa0000))
        } else if part.label().starts_with(b"body_state\0") {
            Some((4, BODY_START, 0x10000))
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
        } else if (start < 0x400000 && end > EVIDENCE_START) || (start < 0xf000 && end > 0xd000) {
            return Err("evidence-partition-overlap");
        }
    }
    let legacy_count = if cfg!(feature = "local-appliance") {
        0
    } else {
        1
    };
    if counts != [1, legacy_count, 1, 1, 1, 1] {
        return Err("evidence-partition-unavailable");
    }
    let base = FLASH
        .try_init(Mutex::new(RefCell::new(flash)))
        .ok_or("evidence-flash-already-owned")?;
    Ok(Regions {
        evidence: Region {
            base,
            start: EVIDENCE_START,
            length: EVIDENCE_LENGTH,
        },
        #[cfg(not(feature = "local-appliance"))]
        observations: Some(Region {
            base,
            start: 0x3f0000,
            length: 0x10000,
        }),
        #[cfg(feature = "local-appliance")]
        observations: None,
        observation_epochs: core::array::from_fn(|index| Region {
            base,
            start: EPOCHS_START + (index as u32) * 0x20000,
            length: 0x20000,
        }),
        credentials: CredentialRegion(Region {
            base,
            start: 0xd000,
            length: 0x2000,
        }),
        network_recovery: NetworkRegion(Region {
            base,
            start: NETWORK_START,
            length: 0x8000,
        }),
        body_state: BodyRegion(Region {
            base,
            start: BODY_START,
            length: 0x10000,
        }),
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

impl crate::native_credentials::journal::Storage for CredentialRegion {
    fn capacity(&self) -> u32 {
        self.0.length
    }
    fn read(
        &mut self,
        at: u32,
        output: &mut [u8],
    ) -> Result<(), crate::native_credentials::journal::Refusal> {
        Storage::read(&mut self.0, at, output)
            .map_err(|_| crate::native_credentials::journal::Refusal::Media)
    }
    fn program(
        &mut self,
        at: u32,
        bytes: &[u8],
    ) -> Result<(), crate::native_credentials::journal::Refusal> {
        Storage::program(&mut self.0, at, bytes)
            .map_err(|_| crate::native_credentials::journal::Refusal::Media)
    }
    fn erase_sector(&mut self, at: u32) -> Result<(), crate::native_credentials::journal::Refusal> {
        use crate::native_credentials::journal::Refusal as E;
        if at != 0 && at != 4096 {
            return Err(E::Geometry);
        }
        let start = self.0.bound(at, 4096).map_err(|_| E::Geometry)?;
        self.0.base.lock(|base| {
            NorFlash::erase(&mut *base.borrow_mut(), start, start + 4096).map_err(|_| E::Media)
        })
    }
}

impl BodyRegion {
    pub fn capacity(&self) -> u32 {
        self.0.length
    }
    pub fn read(&mut self, at: u32, out: &mut [u8]) -> Result<(), Refusal> {
        Storage::read(&mut self.0, at, out)
    }
    pub fn program(&mut self, at: u32, bytes: &[u8]) -> Result<(), Refusal> {
        Storage::program(&mut self.0, at, bytes)
    }
    pub fn erase_bank(&mut self, bank: u8) -> Result<(), Refusal> {
        if bank > 1 {
            return Err(Refusal::Geometry);
        }
        let start = self.0.bound(u32::from(bank) * 0x8000, 0x8000)?;
        // Exclusive BodyRegion custody prevents same-bank interleaving. Keep
        // the leading commit sector first, but release the flash critical
        // section between sectors so an active radio/pulse interrupt can run.
        for offset in (0..0x8000).step_by(4096) {
            self.0.base.lock(|base| {
                let mut flash = base.borrow_mut();
                NorFlash::erase(&mut *flash, start + offset, start + offset + 4096)
                    .map_err(|_| Refusal::Media)
            })?;
        }
        Ok(())
    }
}
impl Storage for NetworkRegion {
    fn capacity(&self) -> u32 {
        self.0.length
    }
    fn read(&mut self, at: u32, out: &mut [u8]) -> Result<(), Refusal> {
        Storage::read(&mut self.0, at, out)
    }
    fn program(&mut self, at: u32, bytes: &[u8]) -> Result<(), Refusal> {
        Storage::program(&mut self.0, at, bytes)
    }
}
