//! Append-only local evidence custody over an exclusively owned NOR region.
//! The caller binds the region to exact Body/Boot/Play scope and admits backing
//! before Play. This module neither acquires storage authority nor erases it.
//! CRC detects torn/corrupt media; it is not authentication. Original evidence
//! validation and signed export remain required at the existing boundaries.
use super::display_evidence_compact as compact;
const COMMIT: [u8; 4] = *b"done";
const PREFIX: usize = 104;
pub const WORKSPACE_BYTES: usize = PREFIX + super::display_evidence::MAXIMUM_BYTES + 8;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    Media,
    Geometry,
    Scope,
    Corrupt,
    Capacity,
    Torn,
}
/// One initialized, exclusive, finite region. Offsets are relative to this
/// region, never the whole flash. Program must complete synchronously, respect
/// NOR 1→0 transitions, and preserve previously programmed words on failure.
/// Read must observe completed media writes, not an uncommitted write cache.
pub trait Storage {
    fn capacity(&self) -> u32;
    fn read(&mut self, offset: u32, output: &mut [u8]) -> Result<(), Refusal>;
    fn program(&mut self, offset: u32, bytes: &[u8]) -> Result<(), Refusal>;
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Committed {
    sequence: u32,
    offset: u32,
    extent: u32,
}
impl Committed {
    pub fn sequence(&self) -> u32 {
        self.sequence
    }
    pub fn offset(&self) -> u32 {
        self.offset
    }
    pub fn extent(&self) -> u32 {
        self.extent
    }
}
pub struct Replay {
    offset: u32,
    sequence: u32,
}
pub struct Journal<'a, S> {
    storage: S,
    basis: &'a [u8],
    first: u32,
    cursor: u32,
    count: u32,
    maximum_records: u32,
    torn: bool,
}
fn aligned(n: usize) -> usize {
    (n + 3) & !3
}
fn crc(bytes: &[u8]) -> u32 {
    let mut crc = !0u32;
    for &byte in bytes {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb88320 & 0u32.wrapping_sub(crc & 1));
        }
    }
    !crc
}
fn u32_at(bytes: &[u8], at: usize) -> u32 {
    u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap())
}
fn seal(bytes: &mut [u8], end: usize) {
    let digest = crc(&bytes[..end]);
    bytes[end..end + 4].copy_from_slice(&digest.to_le_bytes());
    bytes[end + 4..end + 8].copy_from_slice(&COMMIT);
}
fn valid(bytes: &[u8], end: usize) -> bool {
    bytes[end + 4..end + 8] == COMMIT && u32_at(bytes, end) == crc(&bytes[..end])
}
fn verify(storage: &mut impl Storage, offset: u32, bytes: &[u8]) -> Result<(), Refusal> {
    let mut read = [0; 64];
    for (index, chunk) in bytes.chunks(64).enumerate() {
        storage.read(offset + (index * 64) as u32, &mut read[..chunk.len()])?;
        if &read[..chunk.len()] != chunk {
            return Err(Refusal::Corrupt);
        }
    }
    Ok(())
}
impl<'a, S: Storage> Journal<'a, S> {
    /// Initialize only a verified erased region. No old evidence is overwritten.
    /// Scope is the canonical fixed digests of Body, Boot and active Play.
    pub fn create(
        mut storage: S,
        scope: &[u8; 96],
        basis: &'a [u8],
        maximum_records: u32,
        work: &mut [u8; WORKSPACE_BYTES],
    ) -> Result<Self, Refusal> {
        let end = Self::header_end(basis, maximum_records)?;
        if storage.capacity() < (end + 8) as u32 || !storage.capacity().is_multiple_of(4) {
            return Err(Refusal::Geometry);
        }
        let mut read = [0; 64];
        for offset in (0..storage.capacity()).step_by(64) {
            let n = (storage.capacity() - offset).min(64) as usize;
            storage.read(offset, &mut read[..n])?;
            if read[..n].iter().any(|b| *b != 255) {
                return Err(Refusal::Corrupt);
            }
        }
        work[..end + 8].fill(255);
        work[..4].copy_from_slice(b"JEV1");
        work[4..100].copy_from_slice(scope);
        work[100..104].copy_from_slice(&(basis.len() as u32).to_le_bytes());
        work[PREFIX..PREFIX + basis.len()].copy_from_slice(basis);
        seal(work, end);
        storage.program(0, &work[..end + 4])?;
        storage.program((end + 4) as u32, &COMMIT)?;
        verify(&mut storage, 0, &work[..end + 8])?;
        let first = (end + 8) as u32;
        Ok(Self {
            storage,
            basis,
            first,
            cursor: first,
            count: 0,
            maximum_records,
            torn: false,
        })
    }
    fn header_end(basis: &[u8], maximum_records: u32) -> Result<usize, Refusal> {
        if basis.is_empty()
            || basis.len() > super::display_evidence::MAXIMUM_BYTES
            || maximum_records == 0
        {
            return Err(Refusal::Geometry);
        }
        Ok(PREFIX + aligned(basis.len()))
    }
    /// Recover only the committed prefix. An interrupted tail seals append
    /// capacity until the owner explicitly selects another erased region.
    pub fn recover(
        mut storage: S,
        scope: &[u8; 96],
        basis: &'a [u8],
        maximum_records: u32,
        work: &mut [u8; WORKSPACE_BYTES],
    ) -> Result<Self, Refusal> {
        let end = Self::header_end(basis, maximum_records)?;
        if storage.capacity() < (end + 8) as u32
            || !storage.capacity().is_multiple_of(4)
            || storage.capacity() > u32::MAX - WORKSPACE_BYTES as u32
        {
            return Err(Refusal::Geometry);
        }
        storage.read(0, &mut work[..end + 8])?;
        if !valid(work, end) {
            return Err(Refusal::Corrupt);
        }
        if &work[..4] != b"JEV1"
            || &work[4..100] != scope
            || u32_at(work, 100) as usize != basis.len()
            || &work[PREFIX..PREFIX + basis.len()] != basis
        {
            return Err(Refusal::Scope);
        }
        let first = (end + 8) as u32;
        let mut journal = Self {
            storage,
            basis,
            first,
            cursor: first,
            count: 0,
            maximum_records,
            torn: false,
        };
        while journal.cursor + 8 <= journal.storage.capacity() {
            journal.storage.read(journal.cursor, &mut work[..8])?;
            if work[..8].iter().all(|b| *b == 255) {
                // A torn program may leave its leading word erased. Refuse
                // reuse if ANY later byte was programmed before power loss.
                let mut read = [0; 64];
                for offset in (journal.cursor..journal.storage.capacity()).step_by(64) {
                    let n = (journal.storage.capacity() - offset).min(64) as usize;
                    journal.storage.read(offset, &mut read[..n])?;
                    if read[..n].iter().any(|b| *b != 255) {
                        journal.torn = true;
                        break;
                    }
                }
                break;
            }
            let encoded = usize::from(u16::from_le_bytes([work[2], work[3]]));
            let end = 8 + aligned(encoded);
            if &work[..2] != b"JR"
                || encoded < 2
                || end + 8 > WORKSPACE_BYTES
                || journal.cursor.saturating_add((end + 8) as u32) > journal.storage.capacity()
            {
                journal.torn = true;
                break;
            }
            journal.storage.read(journal.cursor, &mut work[..end + 8])?;
            if work[end + 4..end + 8] != COMMIT {
                journal.torn = true;
                break;
            }
            if !valid(work, end) || u32_at(work, 4) != journal.count {
                return Err(Refusal::Corrupt);
            }
            journal.count = journal.count.checked_add(1).ok_or(Refusal::Capacity)?;
            if journal.count > maximum_records {
                return Err(Refusal::Capacity);
            }
            journal.cursor += (end + 8) as u32;
        }
        Ok(journal)
    }
    pub fn committed_count(&self) -> u32 {
        self.count
    }
    pub fn interrupted_tail(&self) -> bool {
        self.torn
    }
    pub fn append(
        &mut self,
        page: &[u8],
        work: &mut [u8; WORKSPACE_BYTES],
    ) -> Result<Committed, Refusal> {
        if self.torn {
            return Err(Refusal::Torn);
        }
        let encoded = compact::encoded_len(self.basis, page).map_err(|_| Refusal::Geometry)?;
        let end = 8 + aligned(encoded);
        if self.count == self.maximum_records
            || self.cursor.saturating_add((end + 8) as u32) > self.storage.capacity()
        {
            return Err(Refusal::Capacity);
        }
        work[..end + 8].fill(255);
        work[..2].copy_from_slice(b"JR");
        work[2..4].copy_from_slice(&(encoded as u16).to_le_bytes());
        work[4..8].copy_from_slice(&self.count.to_le_bytes());
        compact::encode(self.basis, page, &mut work[8..8 + encoded])
            .map_err(|_| Refusal::Geometry)?;
        seal(work, end);
        // Any uncertain media outcome keeps the live page with its producer.
        self.torn = true;
        self.storage.program(self.cursor, &work[..end + 4])?;
        self.storage
            .program(self.cursor + (end + 4) as u32, &COMMIT)?;
        verify(&mut self.storage, self.cursor, &work[..end + 8])?;
        let committed = Committed {
            sequence: self.count,
            offset: self.cursor,
            extent: (end + 8) as u32,
        };
        self.count += 1;
        self.cursor += committed.extent;
        self.torn = false;
        Ok(committed)
    }
    pub fn replay_cursor(&self) -> Replay {
        Replay {
            offset: self.first,
            sequence: 0,
        }
    }
    /// Read only records from this recovered prefix. Physical offsets and CRC
    /// never substitute for the outer signed identity.
    pub fn replay(
        &mut self,
        cursor: &mut Replay,
        output: &mut [u8],
        work: &mut [u8; WORKSPACE_BYTES],
    ) -> Result<Option<usize>, Refusal> {
        if cursor.offset == self.cursor && cursor.sequence == self.count {
            return Ok(None);
        }
        if cursor.offset < self.first
            || cursor.offset >= self.cursor
            || cursor.sequence >= self.count
        {
            return Err(Refusal::Geometry);
        }
        self.storage.read(cursor.offset, &mut work[..8])?;
        let n = usize::from(u16::from_le_bytes([work[2], work[3]]));
        let end = 8 + aligned(n);
        if &work[..2] != b"JR"
            || u32_at(work, 4) != cursor.sequence
            || n < 2
            || end + 8 > WORKSPACE_BYTES
            || cursor.offset.saturating_add((end + 8) as u32) > self.cursor
        {
            return Err(Refusal::Corrupt);
        }
        self.storage.read(cursor.offset, &mut work[..end + 8])?;
        if !valid(work, end) {
            return Err(Refusal::Corrupt);
        }
        let length =
            compact::decode(self.basis, &work[8..8 + n], output).map_err(|error| match error {
                compact::Refusal::Capacity => Refusal::Capacity,
                _ => Refusal::Corrupt,
            })?;
        cursor.offset += (end + 8) as u32;
        cursor.sequence += 1;
        Ok(Some(length))
    }
    pub fn into_storage(self) -> S {
        self.storage
    }
}
