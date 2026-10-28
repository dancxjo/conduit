//! Affine reclamation transactions for actual immutable observation epochs.
//! Media transactions do not establish membership or a closed journal. Trusted
//! callers must verify those separately before `begin`; recovery additionally
//! verifies the retained Root endorsement against independently restored keys.
//! No raw flash address or arbitrary erase entrance escapes this affine owner.
use sha2::{Digest, Sha256};
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
    /// Offset of the journal view inside this actual whole physical epoch.
    /// Trusted region adapters override this after sealing empty prefixes.
    fn data_offset(&self) -> usize {
        0
    }
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
    acceptance: [u8; 32],
}
pub(crate) struct Custody<S> {
    storage: S,
    latest: Option<Stamp>,
}
/// Boot classification transfers the same affine owner. Retained is existing
/// data custody only; it does not claim erased backing or fresh write authority.
pub(crate) enum Opened<S> {
    PendingReclaim(Custody<S>),
    Retained(S),
    Erased(ErasedData<S>),
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
#[path = "native_observed_layout.rs"]
pub(crate) mod layout;
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
                    acceptance: Sha256::digest(acceptance).into(),
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
    /// A prior durable layout witness can recognize interrupted FIRST metadata
    /// formatting only. Existing committed corruption never takes this path.
    #[allow(clippy::result_large_err)]
    pub(crate) fn open_established(
        storage: S,
        witness: layout::EstablishedLayout,
        work: &mut [u8; WORK],
        verify: impl Fn(&[u8; ACCEPTANCE]) -> bool,
    ) -> Result<Self, (S, Error)> {
        if !witness.matches(&storage) {
            return Err((storage, Error::Region));
        }
        match Self::open(storage, work, verify) {
            Err((mut storage, Error::UnknownMetadata)) => {
                match Self::first_format_footprint(&mut storage) {
                    Ok(()) => Ok(Self {
                        storage,
                        latest: None,
                    }),
                    Err(error) => Err((storage, error)),
                }
            }
            other => other,
        }
    }
    // First publication can touch only bank zero, generation one, Reclaiming.
    // A durable layout receipt cannot reset counters or reinterpret later torn
    // metadata as a new journal. Only programmed expected bytes or erased bytes
    // are accepted in the fixed header and physical-region binding.
    fn first_format_footprint(storage: &mut S) -> Result<(), Error> {
        let mut chunk = [0; 64];
        for (bank, start) in [(0, RECORD), (1, 0)] {
            for at in (start..SECTOR).step_by(64) {
                let n = (SECTOR - at).min(64);
                storage.read(bank_at(bank) + at, &mut chunk[..n])?;
                if chunk[..n].iter().any(|byte| *byte != 255) {
                    return Err(Error::UnknownMetadata);
                }
            }
        }
        let mut expected = [0; 24];
        expected[..8].copy_from_slice(MAGIC);
        expected[8..16].copy_from_slice(&1u64.to_le_bytes());
        expected[16..20].copy_from_slice(&(Phase::Reclaiming as u32).to_le_bytes());
        storage.read(bank_at(0), &mut chunk[..24])?;
        if chunk[..24]
            .iter()
            .zip(expected)
            .any(|(actual, expected)| *actual != 255 && *actual != expected)
        {
            return Err(Error::UnknownMetadata);
        }
        storage.read(bank_at(0) + PAYLOAD, &mut chunk[..32])?;
        if chunk[..32]
            .iter()
            .zip(storage.identity())
            .any(|(actual, expected)| *actual != 255 && *actual != expected)
        {
            return Err(Error::Region);
        }
        Ok(())
    }
    #[allow(clippy::result_large_err)]
    pub(crate) fn classify(mut self) -> Result<Opened<S>, (Self, Error)> {
        match self.latest.map(|stamp| stamp.phase) {
            Some(Phase::Reclaiming) => Ok(Opened::PendingReclaim(self)),
            None => Ok(Opened::Retained(self.storage)),
            Some(Phase::Reclaimed) => match self.erased() {
                Ok(()) => Ok(Opened::Erased(ErasedData(self.storage))),
                Err(Error::Occupied) => Ok(Opened::Retained(self.storage)),
                Err(error) => Err((self, error)),
            },
        }
    }
    /// Trusted closure must consume/check actual immutable journal custody,
    /// exact signed scope/forms/count/content, and opaque admitted-message proof.
    /// The prototype's closure boundary grants no remote or lifecycle authority.
    #[allow(clippy::result_large_err)]
    pub(crate) fn begin(
        self,
        acceptance: &[u8; ACCEPTANCE],
        work: &mut [u8; WORK],
        verify: impl Fn(&[u8; ACCEPTANCE]) -> bool,
        closed: impl FnOnce(&mut S, &[u8; ACCEPTANCE]) -> Result<(), Error>,
    ) -> Result<ErasedData<S>, (Self, Error)> {
        let job = self.begin_staged(acceptance, work, verify, closed)?;
        job.complete(work)
    }
    /// Commit the exact authenticated acceptance before any data erase. This
    /// metadata operation requires its own caller scheduling admission; returning
    /// a job establishes no timing, execution, membership or erase-window grant.
    #[allow(clippy::result_large_err)]
    pub(crate) fn begin_staged(
        mut self,
        acceptance: &[u8; ACCEPTANCE],
        work: &mut [u8; WORK],
        verify: impl Fn(&[u8; ACCEPTANCE]) -> bool,
        closed: impl FnOnce(&mut S, &[u8; ACCEPTANCE]) -> Result<(), Error>,
    ) -> Result<PendingReclaim<S>, (Self, Error)> {
        let result = (|| {
            self.check_begin(acceptance, verify, closed)?;
            work[PAYLOAD..CRC].copy_from_slice(acceptance);
            self.publish(Phase::Reclaiming, work)
        })();
        match result {
            Ok(()) => Ok(PendingReclaim {
                custody: self,
                next: 0,
            }),
            Err(error) => Err((self, error)),
        }
    }
    /// Authenticate and retain the actual closed owner before scheduling any
    /// metadata write. The trusted `closed` callback must inspect read-only;
    /// this method itself issues no flash writes or erased-data witness.
    #[allow(clippy::result_large_err)]
    pub(crate) fn prepare_begin(
        mut self,
        acceptance: &[u8; ACCEPTANCE],
        verify: impl Fn(&[u8; ACCEPTANCE]) -> bool,
        closed: impl FnOnce(&mut S, &[u8; ACCEPTANCE]) -> Result<(), Error>,
    ) -> Result<PreparedReclaim<S>, (Self, Error)> {
        match self.check_begin(acceptance, verify, closed) {
            Ok(()) => Ok(PreparedReclaim {
                custody: self,
                acceptance: *acceptance,
            }),
            Err(error) => Err((self, error)),
        }
    }
    fn check_begin(
        &mut self,
        acceptance: &[u8; ACCEPTANCE],
        verify: impl Fn(&[u8; ACCEPTANCE]) -> bool,
        closed: impl FnOnce(&mut S, &[u8; ACCEPTANCE]) -> Result<(), Error>,
    ) -> Result<(), Error> {
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
        closed(&mut self.storage, acceptance)
    }
    #[allow(clippy::result_large_err)]
    pub(crate) fn resume(self, work: &mut [u8; WORK]) -> Result<ErasedData<S>, (Self, Error)> {
        self.resume_staged(work)?.complete(work)
    }
    /// Resume only an already authenticated committed Reclaiming owner. Sector
    /// progress is deliberately not trusted across reboot: all steps restart.
    #[allow(clippy::result_large_err)]
    pub(crate) fn resume_staged(
        mut self,
        work: &mut [u8; WORK],
    ) -> Result<PendingReclaim<S>, (Self, Error)> {
        let result = (|| {
            let stamp = self.latest.ok_or(Error::Phase)?;
            if stamp.phase != Phase::Reclaiming {
                return Err(Error::Phase);
            }
            stamp.generation.checked_add(1).ok_or(Error::Exhausted)?;
            self.read_reclaiming(work)
        })();
        match result {
            Ok(()) => Ok(PendingReclaim {
                custody: self,
                next: 0,
            }),
            Err(error) => Err((self, error)),
        }
    }
    fn read_reclaiming(&mut self, work: &mut [u8; WORK]) -> Result<(), Error> {
        let stamp = self.latest.ok_or(Error::Phase)?;
        if stamp.phase != Phase::Reclaiming {
            return Err(Error::Phase);
        }
        self.storage
            .read(bank_at(stamp.bank), &mut work[..RECORD])?;
        if &work[..8] != MAGIC
            || u64::from_le_bytes(work[8..16].try_into().unwrap()) != stamp.generation
            || u32::from_le_bytes(work[16..20].try_into().unwrap()) != Phase::Reclaiming as u32
            || work[20..24] != [0; 4]
            || &work[COMMIT..RECORD] != DONE
            || u32::from_le_bytes(work[CRC..COMMIT].try_into().unwrap()) != checksum(&work[..CRC])
            || <[u8; 32]>::from(Sha256::digest(&work[PAYLOAD..CRC])) != stamp.acceptance
        {
            return Err(Error::Corrupt);
        }
        Ok(())
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
            acceptance: Sha256::digest(&work[PAYLOAD..CRC]).into(),
        });
        Ok(())
    }
}

/// Accepted, closed media whose Reclaiming marker has not yet been committed.
/// Only consuming commit may produce PendingReclaim; cancellation preserves
/// the original retained custody and cannot issue a writable/erased owner.
pub(crate) struct PreparedReclaim<S> {
    custody: Custody<S>,
    acceptance: [u8; ACCEPTANCE],
}
impl<S: WholeEpoch> PreparedReclaim<S> {
    #[allow(clippy::result_large_err)]
    pub(crate) fn commit(
        mut self,
        work: &mut [u8; WORK],
    ) -> Result<PendingReclaim<S>, (Self, Error)> {
        work[PAYLOAD..CRC].copy_from_slice(&self.acceptance);
        match self.custody.publish(Phase::Reclaiming, work) {
            Ok(()) => Ok(PendingReclaim {
                custody: self.custody,
                next: 0,
            }),
            Err(error) => Err((self, error)),
        }
    }
    pub(crate) fn into_custody(self) -> Custody<S> {
        self.custody
    }
}

/// Affine progress only. It neither exposes storage nor substitutes for an
/// actual scheduler/provider-idle witness. Each operation must be admitted by
/// its caller separately; no shared work buffer is retained between turns.
pub(crate) struct PendingReclaim<S> {
    custody: Custody<S>,
    next: u8,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ReclaimStep {
    EraseSector(u8),
    VerifySector(u8),
    Publish,
}
impl<S: WholeEpoch> PendingReclaim<S> {
    pub(crate) fn next_step(&self) -> ReclaimStep {
        let sectors = (DATA / SECTOR) as u8;
        if self.next < sectors {
            ReclaimStep::EraseSector(self.next)
        } else if self.next < 2 * sectors {
            ReclaimStep::VerifySector(self.next - sectors)
        } else {
            ReclaimStep::Publish
        }
    }
    /// At most one 4 KiB data erase plus its full readback, or one 4 KiB
    /// verification pass. Errors retain this exact owner and retry position.
    /// A second verification sweep preserves the former post-erase full check.
    pub(crate) fn step(&mut self) -> Result<(), Error> {
        let sector = match self.next_step() {
            ReclaimStep::EraseSector(sector) => {
                self.custody.storage.erase_data(sector)?;
                sector
            }
            ReclaimStep::VerifySector(sector) => sector,
            ReclaimStep::Publish => return Err(Error::Phase),
        };
        let mut bytes = [0; 64];
        for at in (0..SECTOR).step_by(bytes.len()) {
            self.custody
                .storage
                .read(usize::from(sector) * SECTOR + at, &mut bytes)?;
            if bytes.iter().any(|byte| *byte != 255) {
                return Err(Error::Occupied);
            }
        }
        self.next += 1;
        Ok(())
    }
    /// Only all completed data steps may publish Reclaimed. This separately
    /// bounded metadata operation erases one marker sector and commits last.
    #[allow(clippy::result_large_err)]
    pub(crate) fn publish(mut self, work: &mut [u8; WORK]) -> Result<ErasedData<S>, (Self, Error)> {
        let result = (|| {
            if self.next_step() != ReclaimStep::Publish {
                return Err(Error::Phase);
            }
            self.custody.read_reclaiming(work)?;
            self.custody.publish(Phase::Reclaimed, work)
        })();
        match result {
            Ok(()) => Ok(ErasedData(self.custody.storage)),
            Err(error) => Err((self, error)),
        }
    }
    /// Return the same durable pending custody, never an erased/writeable view.
    pub(crate) fn into_custody(self) -> Custody<S> {
        self.custody
    }
    #[allow(clippy::result_large_err)]
    fn complete(mut self, work: &mut [u8; WORK]) -> Result<ErasedData<S>, (Custody<S>, Error)> {
        while self.next_step() != ReclaimStep::Publish {
            if let Err(error) = self.step() {
                return Err((self.into_custody(), error));
            }
        }
        self.publish(work)
            .map_err(|(job, error)| (job.into_custody(), error))
    }
}
