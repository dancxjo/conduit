//! Finite append-only immutable NOR records. This owns no generation selection,
//! authority, erasure, reclamation, recording semantics or delivery cursor.
use crate::nor::{NorRefusal, NorStorage};
use conduit_core::semantic_digest;
pub const RECORD_UNIT_BYTES: usize = 256;
pub const RECORD_SLOT_BYTES: u32 = 512;
pub const MAXIMUM_RECORD_PAYLOAD_BYTES: usize = 200;
const HEADER: usize = 56;
const DATA_MAGIC: &[u8; 8] = b"CDNREC01";
const COMMIT_MAGIC: &[u8; 8] = b"CDNCMT01";
const HASH_DOMAIN: &str = "conduit.resource/nor-append-record@1";

/// Caller-selected content residence identity, not an authorization credential.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RecordResidenceId(pub [u8; 32]);
/// Physical record position within this exact finite residence, not an external
/// observation sequence or durable recording acknowledgement.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DurableRecordIndex(pub u32);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecordGap {
    /// No complete commit marker is readable: torn writes and damaged markers
    /// cannot be distinguished. Occupied bytes are preserved in either case.
    UnpublishedOrDamaged,
    Corrupt,
}
#[derive(Debug, PartialEq, Eq)]
pub enum RecordRead<'a> {
    Vacant {
        index: DurableRecordIndex,
    },
    Gap {
        index: DurableRecordIndex,
        reason: RecordGap,
    },
    Committed {
        index: DurableRecordIndex,
        payload: &'a [u8],
    },
}
/// A completed scan is not an exact-history acknowledgement. Every nonzero
/// damage/gap count must remain visible to the caller; it cannot acknowledge a
/// missing record merely because later committed records are recoverable.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RecordRecovery {
    pub committed: u32,
    pub unpublished_or_damaged: u32,
    pub corrupt: u32,
    /// Erased holes preceding the last occupied slot, never hidden as history.
    pub vacant_gaps: u32,
    pub next_index: u32,
    pub total_slots: u32,
}
#[derive(Debug, PartialEq, Eq)]
pub enum RecordResidenceRefusal {
    Geometry,
    Capacity,
    Payload,
    RecoveryRequired,
    ForeignResidence,
    Media(NorRefusal),
}
#[derive(Clone, Copy)]
enum State {
    Vacant,
    Gap(RecordGap),
    Committed(usize),
}
/// Two fixed unit buffers and exclusive owned storage. All work/extent bounds
/// are fixed before use. The caller separately admits actual media possession,
/// original authority and safe physical execution windows.
pub struct PreparedNorRecordResidence<S> {
    storage: S,
    identity: RecordResidenceId,
    offset: u32,
    slots: u32,
    next: u32,
    recovered: bool,
    data: [u8; RECORD_UNIT_BYTES],
    marker: [u8; RECORD_UNIT_BYTES],
}
impl<S: NorStorage> PreparedNorRecordResidence<S> {
    /// Pure geometry checks. Refusal returns original custody without media IO.
    pub fn prepare(
        storage: S,
        identity: RecordResidenceId,
        offset: u32,
        slots: u32,
    ) -> Result<Self, (S, RecordResidenceRefusal)> {
        if storage.program_unit() != RECORD_UNIT_BYTES as u32
            || slots == 0
            || !offset.is_multiple_of(RECORD_UNIT_BYTES as u32)
            || identity.0 == [0; 32]
        {
            return Err((storage, RecordResidenceRefusal::Geometry));
        }
        if slots
            .checked_mul(RECORD_SLOT_BYTES)
            .and_then(|bytes| offset.checked_add(bytes))
            .is_none_or(|end| end > storage.capacity())
        {
            return Err((storage, RecordResidenceRefusal::Capacity));
        }
        Ok(Self {
            storage,
            identity,
            offset,
            slots,
            next: 0,
            recovered: false,
            data: [0; RECORD_UNIT_BYTES],
            marker: [0; RECORD_UNIT_BYTES],
        })
    }
    /// Pure finite physical facts; none confer resource admission or authority.
    pub const fn residence_id(&self) -> RecordResidenceId {
        self.identity
    }
    pub const fn total_slots(&self) -> u32 {
        self.slots
    }
    pub const fn residence_bytes(&self) -> u32 {
        self.slots * RECORD_SLOT_BYTES
    }
    pub const fn recovered(&self) -> bool {
        self.recovered
    }
    pub const fn remaining_slots(&self) -> Option<u32> {
        if self.recovered {
            Some(self.slots - self.next)
        } else {
            None
        }
    }
    pub fn into_storage(self) -> S {
        self.storage
    }
    pub const fn maximum_recovery_reads(&self) -> u64 {
        self.slots as u64 * 2
    }
    pub const fn maximum_append_media_calls(&self) -> u32 {
        6
    }
    /// Full finite scan, never a repair. Cursor advances past every occupied
    /// slot, including an uncertain/corrupt tail. Foreign committed content
    /// refuses recovery rather than becoming permission to mix residences.
    pub fn recover(&mut self) -> Result<RecordRecovery, RecordResidenceRefusal> {
        self.recovered = false;
        let mut summary = RecordRecovery {
            total_slots: self.slots,
            ..RecordRecovery::default()
        };
        let mut pending_vacant = 0;
        for index in 0..self.slots {
            let state = self.load(index)?;
            if matches!(state, State::Vacant) {
                pending_vacant += 1;
                continue;
            }
            summary.vacant_gaps += pending_vacant;
            pending_vacant = 0;
            summary.next_index = index + 1;
            match state {
                State::Committed(_) => summary.committed += 1,
                State::Gap(RecordGap::UnpublishedOrDamaged) => summary.unpublished_or_damaged += 1,
                State::Gap(RecordGap::Corrupt) => summary.corrupt += 1,
                State::Vacant => unreachable!(),
            }
        }
        self.next = summary.next_index;
        self.recovered = true;
        Ok(summary)
    }
    /// Read exact committed bytes, or expose the selected physical gap. Incoming
    /// payload bytes never select the residence, cursor or execution authority.
    pub fn read(
        &mut self,
        index: DurableRecordIndex,
    ) -> Result<RecordRead<'_>, RecordResidenceRefusal> {
        match self.load(index.0)? {
            State::Vacant => Ok(RecordRead::Vacant { index }),
            State::Gap(reason) => Ok(RecordRead::Gap { index, reason }),
            State::Committed(length) => Ok(RecordRead::Committed {
                index,
                payload: &self.data[HEADER..HEADER + length],
            }),
        }
    }
    /// Success means verified committed media only, not a kernel commit or an
    /// external acknowledgment. Any media failure requires a new scan: a lost
    /// response can leave either a torn slot or a fully committed record.
    pub fn append(&mut self, payload: &[u8]) -> Result<DurableRecordIndex, RecordResidenceRefusal> {
        if payload.len() > MAXIMUM_RECORD_PAYLOAD_BYTES {
            return Err(RecordResidenceRefusal::Payload);
        }
        if !self.recovered {
            return Err(RecordResidenceRefusal::RecoveryRequired);
        }
        if self.next == self.slots {
            return Err(RecordResidenceRefusal::Capacity);
        }
        let index = self.next;
        self.recovered = false;
        // Recheck the exact selected slot before any program operation.
        if !matches!(self.load(index)?, State::Vacant) {
            return Err(RecordResidenceRefusal::RecoveryRequired);
        }
        self.data.fill(255);
        self.data[..8].copy_from_slice(DATA_MAGIC);
        self.data[8..40].copy_from_slice(&self.identity.0);
        self.data[40..44].copy_from_slice(&index.to_le_bytes());
        self.data[44..46].copy_from_slice(&(payload.len() as u16).to_le_bytes());
        self.data[46..HEADER].fill(0);
        self.data[HEADER..HEADER + payload.len()].copy_from_slice(payload);
        let at = self.address(index)?;
        self.storage
            .program(at, &self.data)
            .map_err(RecordResidenceRefusal::Media)?;
        self.storage
            .read(at, &mut self.marker)
            .map_err(RecordResidenceRefusal::Media)?;
        if self.marker != self.data {
            return Err(RecordResidenceRefusal::Media(NorRefusal::Corrupt));
        }
        self.marker.fill(0xa5);
        self.marker[..8].copy_from_slice(COMMIT_MAGIC);
        self.marker[8..40].copy_from_slice(&semantic_digest(HASH_DOMAIN, &self.data));
        self.marker[40..44].copy_from_slice(&index.to_le_bytes());
        self.storage
            .program(at + RECORD_UNIT_BYTES as u32, &self.marker)
            .map_err(RecordResidenceRefusal::Media)?;
        self.storage
            .read(at + RECORD_UNIT_BYTES as u32, &mut self.data)
            .map_err(RecordResidenceRefusal::Media)?;
        if self.data != self.marker {
            return Err(RecordResidenceRefusal::Media(NorRefusal::Corrupt));
        }
        self.next = index + 1;
        self.recovered = true;
        Ok(DurableRecordIndex(index))
    }
    fn address(&self, index: u32) -> Result<u32, RecordResidenceRefusal> {
        if index >= self.slots {
            return Err(RecordResidenceRefusal::Capacity);
        }
        // Constructor proved the entire finite address range without overflow.
        Ok(self.offset + index * RECORD_SLOT_BYTES)
    }
    fn load(&mut self, index: u32) -> Result<State, RecordResidenceRefusal> {
        let at = self.address(index)?;
        self.storage
            .read(at, &mut self.data)
            .map_err(RecordResidenceRefusal::Media)?;
        self.storage
            .read(at + RECORD_UNIT_BYTES as u32, &mut self.marker)
            .map_err(RecordResidenceRefusal::Media)?;
        if self
            .data
            .iter()
            .chain(self.marker.iter())
            .all(|b| *b == 255)
        {
            return Ok(State::Vacant);
        }
        if &self.marker[..8] != COMMIT_MAGIC || self.marker[44..].iter().any(|b| *b != 0xa5) {
            return Ok(State::Gap(RecordGap::UnpublishedOrDamaged));
        }
        if self.marker[8..40] != semantic_digest(HASH_DOMAIN, &self.data)
            || self.marker[40..44] != index.to_le_bytes()
        {
            return Ok(State::Gap(RecordGap::Corrupt));
        }
        let length = u16::from_le_bytes(self.data[44..46].try_into().unwrap()) as usize;
        if &self.data[..8] != DATA_MAGIC
            || self.data[40..44] != index.to_le_bytes()
            || length > MAXIMUM_RECORD_PAYLOAD_BYTES
            || self.data[46..HEADER].iter().any(|b| *b != 0)
            || self.data[HEADER + length..].iter().any(|b| *b != 255)
        {
            return Ok(State::Gap(RecordGap::Corrupt));
        }
        if self.data[8..40] != self.identity.0 {
            return Err(RecordResidenceRefusal::ForeignResidence);
        }
        Ok(State::Committed(length))
    }
}
#[cfg(test)]
mod tests;
