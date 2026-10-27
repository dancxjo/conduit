//! CPU prototype only; deliberately not linked into firmware or a public route.
//! Media transactions do not establish membership or a closed journal. Trusted
//! callers must verify those separately before `begin`; recovery additionally
//! verifies the retained Root endorsement against independently restored keys.
//! No raw flash address or arbitrary erase entrance escapes this affine owner.
pub(crate) const SECTOR: usize = 4096;
pub(crate) const DATA: usize = 30 * SECTOR;
pub(crate) const TOTAL: usize = 32 * SECTOR;
pub(crate) const ACCEPTANCE: usize = 416;
pub(crate) const RECORD: usize = 448;
pub(crate) const WORK: usize = 536;
const PAYLOAD: usize = 24;
const CRC: usize = 440;
const COMMIT: usize = 444;
const MAGIC: &[u8; 8] = b"OCUST001";
const DONE: &[u8; 4] = b"done";
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Error {
    Media,
    Corrupt,
    UnknownMetadata,
    Region,
    Authentication,
    NotClosed,
    Phase,
    Occupied,
    Exhausted,
}
/// Implemented only by a uniquely owned whole physical epoch. Sector methods
/// cannot address another epoch or any Body/key/network/credential partition.
pub(crate) trait WholeEpoch {
    fn identity(&self) -> [u8; 32];
    fn read(&mut self, at: usize, out: &mut [u8]) -> Result<(), Error>;
    fn program_marker(&mut self, bank: u8, at: usize, bytes: &[u8]) -> Result<(), Error>;
    fn erase_marker(&mut self, bank: u8) -> Result<(), Error>;
    fn erase_data(&mut self, sector: u8) -> Result<(), Error>;
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Reclaiming = 1,
    Reclaimed = 2,
}
#[derive(Clone, Copy)]
struct Stamp {
    bank: u8,
    generation: u64,
    phase: Phase,
}
pub(crate) struct Custody<S> {
    storage: S,
    latest: Option<Stamp>,
}
/// Only complete erase readback plus committed Reclaimed can construct this.
pub(crate) struct ErasedData<S>(S);
impl<S> ErasedData<S> {
    pub(crate) fn into_storage(self) -> S {
        self.0
    }
}
fn checksum(bytes: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &byte in bytes {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb88320 & 0u32.wrapping_sub(crc & 1));
        }
    }
    !crc
}
fn bank_at(bank: u8) -> usize {
    DATA + usize::from(bank) * SECTOR
}
impl<S: WholeEpoch> Custody<S> {
    /// Never turns unknown/torn first-format metadata into fresh writable media.
    #[allow(clippy::result_large_err)]
    pub(crate) fn open(
        mut storage: S,
        work: &mut [u8; WORK],
        verify: impl Fn(&[u8; ACCEPTANCE]) -> bool,
    ) -> Result<Self, (S, Error)> {
        let result = (|| {
            let mut latest: Option<Stamp> = None;
            let mut dirty = false;
            for bank in 0..2 {
                let mut occupied = false;
                let mut extra = false;
                let mut bytes = [0; 64];
                for at in (0..SECTOR).step_by(64) {
                    storage.read(bank_at(bank) + at, &mut bytes)?;
                    occupied |= bytes.iter().any(|b| *b != 255);
                    if at >= RECORD {
                        extra |= bytes.iter().any(|b| *b != 255);
                    }
                }
                dirty |= occupied;
                storage.read(bank_at(bank), &mut work[..RECORD])?;
                if &work[COMMIT..RECORD] != DONE {
                    continue;
                }
                if extra
                    || &work[..8] != MAGIC
                    || work[20..24] != [0; 4]
                    || u32::from_le_bytes(work[CRC..COMMIT].try_into().unwrap())
                        != checksum(&work[..CRC])
                {
                    return Err(Error::Corrupt);
                }
                let acceptance: &[u8; ACCEPTANCE] = work[PAYLOAD..CRC].try_into().unwrap();
                if acceptance[..32] != storage.identity() {
                    return Err(Error::Region);
                }
                if !verify(acceptance) {
                    return Err(Error::Authentication);
                }
                let generation = u64::from_le_bytes(work[8..16].try_into().unwrap());
                let phase = match u32::from_le_bytes(work[16..20].try_into().unwrap()) {
                    1 => Phase::Reclaiming,
                    2 => Phase::Reclaimed,
                    _ => return Err(Error::Corrupt),
                };
                if generation == 0 {
                    return Err(Error::Corrupt);
                }
                let stamp = Stamp {
                    bank,
                    generation,
                    phase,
                };
                if let Some(previous) = latest {
                    let (old, new) = if previous.generation < generation {
                        (previous, stamp)
                    } else {
                        (stamp, previous)
                    };
                    if old.generation.checked_add(1) != Some(new.generation)
                        || old.phase == new.phase
                    {
                        return Err(Error::Corrupt);
                    }
                    if new.phase == Phase::Reclaimed {
                        let mut a = [0; 64];
                        let mut b = [0; 64];
                        for at in (0..ACCEPTANCE).step_by(64) {
                            let n = (ACCEPTANCE - at).min(64);
                            storage.read(bank_at(old.bank) + PAYLOAD + at, &mut a[..n])?;
                            storage.read(bank_at(new.bank) + PAYLOAD + at, &mut b[..n])?;
                            if a[..n] != b[..n] {
                                return Err(Error::Corrupt);
                            }
                        }
                    }
                    latest = Some(new);
                } else {
                    latest = Some(stamp);
                }
            }
            if latest.is_none() && dirty {
                return Err(Error::UnknownMetadata);
            }
            Ok(latest)
        })();
        match result {
            Ok(latest) => Ok(Self { storage, latest }),
            Err(error) => Err((storage, error)),
        }
    }
    /// Trusted closure must consume/check actual immutable journal custody,
    /// exact signed scope/forms/count/content, and opaque admitted-message proof.
    /// The prototype's closure boundary grants no remote or lifecycle authority.
    #[allow(clippy::result_large_err)]
    pub(crate) fn begin(
        mut self,
        acceptance: &[u8; ACCEPTANCE],
        work: &mut [u8; WORK],
        verify: impl Fn(&[u8; ACCEPTANCE]) -> bool,
        closed: impl FnOnce(&mut S, &[u8; ACCEPTANCE]) -> Result<(), Error>,
    ) -> Result<ErasedData<S>, (Self, Error)> {
        let result = (|| {
            if self.latest.is_some_and(|s| s.phase != Phase::Reclaimed) {
                return Err(Error::Phase);
            }
            self.latest
                .map_or(0, |s| s.generation)
                .checked_add(2)
                .ok_or(Error::Exhausted)?;
            if acceptance[..32] != self.storage.identity() {
                return Err(Error::Region);
            }
            if !verify(acceptance) {
                return Err(Error::Authentication);
            }
            closed(&mut self.storage, acceptance)?;
            work[PAYLOAD..CRC].copy_from_slice(acceptance);
            self.publish(Phase::Reclaiming, work)?;
            self.complete(work)
        })();
        match result {
            Ok(()) => Ok(ErasedData(self.storage)),
            Err(error) => Err((self, error)),
        }
    }
    #[allow(clippy::result_large_err)]
    pub(crate) fn resume(mut self, work: &mut [u8; WORK]) -> Result<ErasedData<S>, (Self, Error)> {
        let result = (|| {
            let stamp = self.latest.ok_or(Error::Phase)?;
            if stamp.phase != Phase::Reclaiming {
                return Err(Error::Phase);
            }
            stamp.generation.checked_add(1).ok_or(Error::Exhausted)?;
            self.storage
                .read(bank_at(stamp.bank), &mut work[..RECORD])?;
            self.complete(work)
        })();
        match result {
            Ok(()) => Ok(ErasedData(self.storage)),
            Err(error) => Err((self, error)),
        }
    }
    #[allow(clippy::result_large_err)]
    pub(crate) fn release(mut self) -> Result<ErasedData<S>, (Self, Error)> {
        let result = if self.latest.is_some_and(|s| s.phase == Phase::Reclaimed) {
            self.erased()
        } else {
            Err(Error::Phase)
        };
        match result {
            Ok(()) => Ok(ErasedData(self.storage)),
            Err(error) => Err((self, error)),
        }
    }
    #[cfg(test)] // Snapshot extraction is never a production custody escape.
    pub(crate) fn into_storage(self) -> S {
        self.storage
    }
    fn erased(&mut self) -> Result<(), Error> {
        let mut bytes = [0; 64];
        for at in (0..DATA).step_by(64) {
            self.storage.read(at, &mut bytes)?;
            if bytes.iter().any(|b| *b != 255) {
                return Err(Error::Occupied);
            }
        }
        Ok(())
    }
    fn complete(&mut self, work: &mut [u8; WORK]) -> Result<(), Error> {
        for sector in 0..(DATA / SECTOR) {
            self.storage.erase_data(sector as u8)?;
        }
        self.erased()?;
        self.publish(Phase::Reclaimed, work)
    }
    fn publish(&mut self, phase: Phase, work: &mut [u8; WORK]) -> Result<(), Error> {
        let generation = self
            .latest
            .map_or(Some(1), |s| s.generation.checked_add(1))
            .ok_or(Error::Exhausted)?;
        let bank = self.latest.map_or(0, |s| 1 - s.bank);
        let mut old_commit = [0; 4];
        self.storage.read(bank_at(bank) + COMMIT, &mut old_commit)?;
        if &old_commit == DONE {
            // Invalidate obsolete COMMIT before sector erase. An interrupted
            // inactive-bank erase cannot masquerade as committed corrupt data.
            self.storage.program_marker(bank, COMMIT, &[0; 4])?;
            self.storage.read(bank_at(bank) + COMMIT, &mut old_commit)?;
            if old_commit != [0; 4] {
                return Err(Error::Media);
            }
        }
        self.storage.erase_marker(bank)?;
        let mut erased = [0; 64];
        for at in (0..SECTOR).step_by(64) {
            self.storage.read(bank_at(bank) + at, &mut erased)?;
            if erased.iter().any(|byte| *byte != 255) {
                return Err(Error::Media);
            }
        }
        work[..8].copy_from_slice(MAGIC);
        work[8..16].copy_from_slice(&generation.to_le_bytes());
        work[16..20].copy_from_slice(&(phase as u32).to_le_bytes());
        work[20..24].fill(0);
        let crc = checksum(&work[..CRC]);
        work[CRC..COMMIT].copy_from_slice(&crc.to_le_bytes());
        self.storage.program_marker(bank, 0, &work[..COMMIT])?;
        let mut read = [0; 64];
        for at in (0..COMMIT).step_by(64) {
            let n = (COMMIT - at).min(64);
            self.storage.read(bank_at(bank) + at, &mut read[..n])?;
            if read[..n] != work[at..at + n] {
                return Err(Error::Media);
            }
        }
        self.storage.program_marker(bank, COMMIT, DONE)?;
        self.storage.read(bank_at(bank) + COMMIT, &mut old_commit)?;
        if &old_commit != DONE {
            return Err(Error::Media);
        }
        self.latest = Some(Stamp {
            bank,
            generation,
            phase,
        });
        Ok(())
    }
}
