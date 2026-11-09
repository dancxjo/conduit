//! One exclusive immutable NOR slot. Erasure and slot allocation belong to the
//! externally admitted residence owner; this primitive can do neither.
use crate::{PreparedSnapshotRecord, SnapshotRefusal, MAXIMUM_RECORD_BYTES};
use conduit_core::{semantic_digest, AuthorityBinding};
const HEADER: usize = 44;
const MAGIC: &[u8; 8] = b"CDNOR001";
const MAXIMUM_PROGRAM_UNIT: usize = 256;

#[derive(Debug, PartialEq, Eq)]
pub enum NorRefusal {
    Geometry,
    Capacity,
    Media,
    Occupied,
    Unpublished,
    Corrupt,
    AlreadyAttempted,
    Record(SnapshotRefusal),
}
impl From<SnapshotRefusal> for NorRefusal {
    fn from(value: SnapshotRefusal) -> Self {
        Self::Record(value)
    }
}
/// Exclusive logical slot, with offsets relative to that slot. Programs are
/// exactly one aligned unit (1..=256 bytes, power of two). Each operation must
/// finish synchronously on actual media before returning; reads observe those
/// writes. Interrupted programs may change only their addressed bytes, only
/// from 1 to 0, and must preserve all previously completed units. Providers must
/// separately own/fund safe execution windows, including XIP/multicore hazards.
/// There is deliberately no erase method: adjacent slots may share erase blocks.
pub trait NorStorage {
    fn capacity(&self) -> u32;
    fn program_unit(&self) -> u32;
    fn read(&mut self, offset: u32, bytes: &mut [u8]) -> Result<(), NorRefusal>;
    fn program(&mut self, offset: u32, bytes: &[u8]) -> Result<(), NorRefusal>;
}
/// Publication-only specialization retains one program unit, not a read record.
/// Prepared before Play. This owns no
/// resource grant, generation allocator, catalog, retry or delivery cursor.
/// Publication success means an exact committed media record, not a kernel
/// committed output, member delivery acknowledgement or permission to reclaim.
pub struct PreparedNorSnapshotWriter<S> {
    storage: S,
    capacity: usize,
    unit: usize,
    attempted: bool,
    work: [u8; MAXIMUM_PROGRAM_UNIT],
}
impl<S: NorStorage> PreparedNorSnapshotWriter<S> {
    /// Pure geometry preparation; custody is returned intact on refusal. Slots
    /// need program alignment only, never exclusive erase-block ownership.
    pub fn prepare(storage: S) -> Result<Self, (S, NorRefusal)> {
        let capacity = storage.capacity() as usize;
        let unit = storage.program_unit() as usize;
        if unit == 0
            || !unit.is_power_of_two()
            || unit > MAXIMUM_PROGRAM_UNIT
            || !capacity.is_multiple_of(unit)
            || capacity < HEADER + 2 * unit
        {
            return Err((storage, NorRefusal::Geometry));
        }
        Ok(Self {
            storage,
            capacity,
            unit,
            attempted: false,
            work: [0; MAXIMUM_PROGRAM_UNIT],
        })
    }
    /// Pure preparation geometry; this grants no publication or erase access.
    pub fn fits_record(&self, bytes: usize) -> bool {
        bytes <= MAXIMUM_RECORD_BYTES
            && (HEADER + bytes).div_ceil(self.unit) * self.unit <= self.capacity - self.unit
    }
    pub fn slot_bytes(&self) -> usize {
        self.capacity
    }
    /// Maximum media calls: erased-slot scan, record programs and verification
    /// reads, plus final marker program/read. Root funds the selected slot/work.
    pub fn maximum_publish_calls(&self) -> usize {
        self.capacity.div_ceil(MAXIMUM_PROGRAM_UNIT)
            + 2 * (HEADER + MAXIMUM_RECORD_BYTES).div_ceil(self.unit)
            + 2
    }
    pub fn into_storage(self) -> S {
        self.storage
    }
    pub fn publish(
        &mut self,
        record: &mut PreparedSnapshotRecord,
        authority: &AuthorityBinding,
        content: &[u8],
    ) -> Result<(), NorRefusal> {
        if self.attempted {
            return Err(NorRefusal::AlreadyAttempted);
        }
        // Binding/content/capacity refusal precedes any storage operation.
        let bytes = record.publication(authority, content)?;
        let end = (HEADER + bytes.len()).div_ceil(self.unit) * self.unit;
        let commit = self.capacity - self.unit;
        if !self.fits_record(bytes.len()) {
            return Err(NorRefusal::Capacity);
        }
        self.attempted = true;
        for at in (0..self.capacity).step_by(MAXIMUM_PROGRAM_UNIT) {
            let n = (self.capacity - at).min(MAXIMUM_PROGRAM_UNIT);
            self.storage.read(at as u32, &mut self.work[..n])?;
            if self.work[..n].iter().any(|v| *v != 255) {
                return Err(NorRefusal::Occupied);
            }
        }
        let mut header = [0; HEADER];
        header[..8].copy_from_slice(MAGIC);
        header[8..12].copy_from_slice(&(bytes.len() as u32).to_le_bytes());
        header[12..].copy_from_slice(&semantic_digest("conduit.resource/nor-record@1", bytes));
        for at in (0..end).step_by(self.unit) {
            self.work[..self.unit].fill(255);
            for i in 0..self.unit {
                let offset = at + i;
                if offset < HEADER {
                    self.work[i] = header[offset];
                } else if offset - HEADER < bytes.len() {
                    self.work[i] = bytes[offset - HEADER];
                }
            }
            self.storage.program(at as u32, &self.work[..self.unit])?;
        }
        // Read actual media back before publishing. Include padding in the
        // exact comparison; an acknowledged faulty program cannot publish.
        for at in (0..end).step_by(self.unit) {
            self.storage.read(at as u32, &mut self.work[..self.unit])?;
            for i in 0..self.unit {
                let offset = at + i;
                let expected = if offset < HEADER {
                    header[offset]
                } else if offset - HEADER < bytes.len() {
                    bytes[offset - HEADER]
                } else {
                    255
                };
                if self.work[i] != expected {
                    return Err(NorRefusal::Corrupt);
                }
            }
        }
        // This unit is never programmed until every record unit has completed
        // and its exact contents have been verified from actual media.
        self.work[..self.unit].fill(0xa5);
        self.storage
            .program(commit as u32, &self.work[..self.unit])?;
        self.storage
            .read(commit as u32, &mut self.work[..self.unit])?;
        if self.work[..self.unit].iter().any(|v| *v != 0xa5) {
            return Err(NorRefusal::Corrupt);
        }
        Ok(())
    }
}

/// Compatibility reader/publication owner. The immutable NOR format and all
/// publication logic are shared with the smaller publication-only writer.
pub struct PreparedNorSnapshot<S> {
    writer: PreparedNorSnapshotWriter<S>,
    record: [u8; MAXIMUM_RECORD_BYTES],
}
impl<S: NorStorage> PreparedNorSnapshot<S> {
    pub fn prepare(storage: S) -> Result<Self, (S, NorRefusal)> {
        PreparedNorSnapshotWriter::prepare(storage).map(Self::from_writer)
    }
    /// Add explicitly funded read workspace to the same exclusive writer.
    pub fn from_writer(writer: PreparedNorSnapshotWriter<S>) -> Self {
        Self {
            writer,
            record: [0; MAXIMUM_RECORD_BYTES],
        }
    }
    /// Discard read workspace while preserving actual slot/attempt custody.
    pub fn into_writer(self) -> PreparedNorSnapshotWriter<S> {
        self.writer
    }
    pub fn fits_record(&self, bytes: usize) -> bool {
        self.writer.fits_record(bytes)
    }
    pub fn slot_bytes(&self) -> usize {
        self.writer.slot_bytes()
    }
    pub fn maximum_publish_calls(&self) -> usize {
        self.writer.maximum_publish_calls()
    }
    pub fn into_storage(self) -> S {
        self.writer.into_storage()
    }
    pub fn publish(
        &mut self,
        record: &mut PreparedSnapshotRecord,
        authority: &AuthorityBinding,
        content: &[u8],
    ) -> Result<(), NorRefusal> {
        self.writer.publish(record, authority, content)
    }
    /// Reopen requires a fresh current-Boot codec/authority. A lost publication
    /// response may leave a readable committed record; occupied bytes alone do
    /// not establish success. This method never repairs or discards a tail.
    pub fn read<'a>(
        &'a mut self,
        record: &PreparedSnapshotRecord,
        authority: &AuthorityBinding,
    ) -> Result<&'a [u8], NorRefusal> {
        record.check_read_binding(authority)?;
        self.writer.storage.read(
            (self.writer.capacity - self.writer.unit) as u32,
            &mut self.writer.work[..self.writer.unit],
        )?;
        if self.writer.work[..self.writer.unit]
            .iter()
            .any(|v| *v != 0xa5)
        {
            return Err(NorRefusal::Unpublished);
        }
        let mut header = [0; HEADER];
        self.writer.storage.read(0, &mut header)?;
        let length = u32::from_le_bytes(header[8..12].try_into().unwrap()) as usize;
        if &header[..8] != MAGIC
            || length > MAXIMUM_RECORD_BYTES
            || HEADER + length > self.writer.capacity - self.writer.unit
        {
            return Err(NorRefusal::Corrupt);
        }
        self.writer
            .storage
            .read(HEADER as u32, &mut self.record[..length])?;
        if header[12..] != semantic_digest("conduit.resource/nor-record@1", &self.record[..length])
        {
            return Err(NorRefusal::Corrupt);
        }
        Ok(record.restore(authority, &self.record[..length])?)
    }
}

#[cfg(test)]
mod tests;
