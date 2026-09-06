//! Portable storage boundary for complete bounded historical snapshots.

use alloc::{string::String, vec::Vec};

use crate::{
    decode_historical_timeline, encode_historical_timeline_into, BoundedHistoricalTimeline,
    HistoricalTimelineCodecRefusal, MAXIMUM_HISTORICAL_TIMELINE_SNAPSHOT_BYTES,
};

pub const MAXIMUM_HISTORICAL_STORE_KEY_BYTES: usize = 128;
pub const MAXIMUM_DETERMINISTIC_SNAPSHOT_RECORDS: usize = 8;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HistoricalStoreRefusal {
    InvalidLimits,
    EmptyKey,
    KeyTooLarge,
    Unavailable,
    Missing,
    RecordTooLarge,
    QuotaExhausted,
    Snapshot(HistoricalTimelineCodecRefusal),
    CorruptSnapshot(HistoricalTimelineCodecRefusal),
}

pub trait HistoricalSnapshotStore {
    fn write_snapshot(&mut self, key: &str, snapshot: &[u8]) -> Result<(), HistoricalStoreRefusal>;

    fn read_snapshot<'a>(&'a self, key: &str) -> Result<&'a [u8], HistoricalStoreRefusal>;

    fn delete_snapshot(&mut self, key: &str) -> Result<(), HistoricalStoreRefusal>;
}

pub fn retain_historical_timeline<S: HistoricalSnapshotStore>(
    store: &mut S,
    key: &str,
    timeline: &BoundedHistoricalTimeline,
    scratch: &mut [u8],
) -> Result<usize, HistoricalStoreRefusal> {
    let written = encode_historical_timeline_into(timeline, scratch)
        .map_err(HistoricalStoreRefusal::Snapshot)?;
    store.write_snapshot(key, &scratch[..written])?;
    Ok(written)
}

pub fn reload_historical_timeline<S: HistoricalSnapshotStore>(
    store: &S,
    key: &str,
) -> Result<BoundedHistoricalTimeline, HistoricalStoreRefusal> {
    let snapshot = store.read_snapshot(key)?;
    decode_historical_timeline(snapshot).map_err(HistoricalStoreRefusal::CorruptSnapshot)
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SnapshotSlot {
    key: String,
    bytes: Vec<u8>,
}

/// Allocation-stable deterministic realization after construction. It is a
/// fixture, not a claim of durable platform persistence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundedMemorySnapshotStore {
    slots: Vec<SnapshotSlot>,
    maximum_record_bytes: usize,
    maximum_total_bytes: usize,
    retained_bytes: usize,
    available: bool,
}

impl BoundedMemorySnapshotStore {
    pub fn new(
        maximum_records: usize,
        maximum_record_bytes: usize,
        maximum_total_bytes: usize,
    ) -> Result<Self, HistoricalStoreRefusal> {
        if maximum_records == 0
            || maximum_records > MAXIMUM_DETERMINISTIC_SNAPSHOT_RECORDS
            || maximum_record_bytes == 0
            || maximum_record_bytes > MAXIMUM_HISTORICAL_TIMELINE_SNAPSHOT_BYTES
            || maximum_total_bytes < maximum_record_bytes
            || maximum_total_bytes
                > MAXIMUM_DETERMINISTIC_SNAPSHOT_RECORDS
                    * MAXIMUM_HISTORICAL_TIMELINE_SNAPSHOT_BYTES
        {
            return Err(HistoricalStoreRefusal::InvalidLimits);
        }
        let mut slots = Vec::with_capacity(maximum_records);
        for _ in 0..maximum_records {
            slots.push(SnapshotSlot {
                key: String::with_capacity(MAXIMUM_HISTORICAL_STORE_KEY_BYTES),
                bytes: Vec::with_capacity(maximum_record_bytes),
            });
        }
        Ok(Self {
            slots,
            maximum_record_bytes,
            maximum_total_bytes,
            retained_bytes: 0,
            available: true,
        })
    }

    pub fn set_available(&mut self, available: bool) {
        self.available = available;
    }

    pub const fn retained_bytes(&self) -> usize {
        self.retained_bytes
    }

    pub const fn is_available(&self) -> bool {
        self.available
    }

    pub fn allocated_capacities(&self, index: usize) -> Option<(usize, usize)> {
        self.slots
            .get(index)
            .map(|slot| (slot.key.capacity(), slot.bytes.capacity()))
    }

    fn validate_key(key: &str) -> Result<(), HistoricalStoreRefusal> {
        if key.is_empty() {
            return Err(HistoricalStoreRefusal::EmptyKey);
        }
        if key.len() > MAXIMUM_HISTORICAL_STORE_KEY_BYTES {
            return Err(HistoricalStoreRefusal::KeyTooLarge);
        }
        Ok(())
    }
}

impl HistoricalSnapshotStore for BoundedMemorySnapshotStore {
    fn write_snapshot(&mut self, key: &str, snapshot: &[u8]) -> Result<(), HistoricalStoreRefusal> {
        Self::validate_key(key)?;
        if !self.available {
            return Err(HistoricalStoreRefusal::Unavailable);
        }
        if snapshot.len() > self.maximum_record_bytes {
            return Err(HistoricalStoreRefusal::RecordTooLarge);
        }
        let existing = self.slots.iter().position(|slot| slot.key == key);
        let target = existing.or_else(|| self.slots.iter().position(|slot| slot.key.is_empty()));
        let Some(target) = target else {
            return Err(HistoricalStoreRefusal::QuotaExhausted);
        };
        let prior = self.slots[target].bytes.len();
        let next_total = self
            .retained_bytes
            .checked_sub(prior)
            .and_then(|value| value.checked_add(snapshot.len()))
            .ok_or(HistoricalStoreRefusal::QuotaExhausted)?;
        if next_total > self.maximum_total_bytes {
            return Err(HistoricalStoreRefusal::QuotaExhausted);
        }
        let slot = &mut self.slots[target];
        slot.key.clear();
        slot.key.push_str(key);
        slot.bytes.clear();
        slot.bytes.extend_from_slice(snapshot);
        self.retained_bytes = next_total;
        Ok(())
    }

    fn read_snapshot<'a>(&'a self, key: &str) -> Result<&'a [u8], HistoricalStoreRefusal> {
        Self::validate_key(key)?;
        if !self.available {
            return Err(HistoricalStoreRefusal::Unavailable);
        }
        self.slots
            .iter()
            .find(|slot| slot.key == key)
            .map(|slot| slot.bytes.as_slice())
            .ok_or(HistoricalStoreRefusal::Missing)
    }

    fn delete_snapshot(&mut self, key: &str) -> Result<(), HistoricalStoreRefusal> {
        Self::validate_key(key)?;
        if !self.available {
            return Err(HistoricalStoreRefusal::Unavailable);
        }
        let slot = self
            .slots
            .iter_mut()
            .find(|slot| slot.key == key)
            .ok_or(HistoricalStoreRefusal::Missing)?;
        self.retained_bytes -= slot.bytes.len();
        slot.key.clear();
        slot.bytes.clear();
        Ok(())
    }
}
