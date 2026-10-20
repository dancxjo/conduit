//! Finite preparation-only catalogue reads, including absent-name observations.
//!
//! A trace belongs to one cold check/expansion and its internal catalogue clones.
//! Cached results retain the immutable finished read set, never this recorder.
//! Exhaustion or an unfinished publication requires whole-context invalidation.
use alloc::{collections::BTreeSet, string::String, vec::Vec};
use core::sync::atomic::{AtomicBool, AtomicU32, AtomicU8, AtomicUsize, Ordering};

const MAX_READS: usize = 4096;
const MAX_KEY_BYTES: usize = 128 * 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum CatalogueTable {
    Signature,
    Fore,
    VariadicFore,
}
impl CatalogueTable {
    fn code(self) -> u8 {
        match self {
            Self::Signature => 1,
            Self::Fore => 2,
            Self::VariadicFore => 3,
        }
    }
    fn from_code(code: u8) -> Option<Self> {
        match code {
            1 => Some(Self::Signature),
            2 => Some(Self::Fore),
            3 => Some(Self::VariadicFore),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct CatalogueRead {
    pub table: CatalogueTable,
    pub key: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ReadTraceFallback {
    Capacity,
    Unfinished,
}

struct Slot {
    ready: AtomicBool,
    table: AtomicU8,
    offset: AtomicU32,
    length: AtomicU32,
}
impl Slot {
    fn empty() -> Self {
        Self {
            ready: AtomicBool::new(false),
            table: AtomicU8::new(0),
            offset: AtomicU32::new(0),
            length: AtomicU32::new(0),
        }
    }
}

/// Atomic storage preserves catalogue Send/Sync without a lock or unsafe code.
/// Slots and key bytes are allocated once before checking, never per lookup.
/// Any publication not fully observed causes conservative fallback, not a hit.
pub(crate) struct ReadTrace {
    slots: Vec<Slot>,
    bytes: Vec<AtomicU8>,
    next: AtomicUsize,
    used: AtomicUsize,
    overflow: AtomicBool,
}
impl ReadTrace {
    pub fn new() -> Self {
        Self::with_limits(MAX_READS, MAX_KEY_BYTES)
    }
    fn with_limits(reads: usize, bytes: usize) -> Self {
        assert!(reads <= MAX_READS && bytes <= MAX_KEY_BYTES);
        Self {
            slots: (0..reads).map(|_| Slot::empty()).collect(),
            bytes: (0..bytes).map(|_| AtomicU8::new(0)).collect(),
            next: AtomicUsize::new(0),
            used: AtomicUsize::new(0),
            overflow: AtomicBool::new(false),
        }
    }
    pub fn record(&self, table: CatalogueTable, key: &str) {
        if self.overflow.load(Ordering::Relaxed) {
            return;
        }
        let slot = self
            .next
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |next| {
                (next < self.slots.len()).then(|| next + 1)
            });
        let bytes = self
            .used
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |used| {
                used.checked_add(key.len())
                    .filter(|end| *end <= self.bytes.len())
            });
        let (Ok(slot), Ok(offset)) = (slot, bytes) else {
            self.overflow.store(true, Ordering::Release);
            return;
        };
        for (destination, byte) in self.bytes[offset..offset + key.len()]
            .iter()
            .zip(key.bytes())
        {
            destination.store(byte, Ordering::Relaxed);
        }
        let slot = &self.slots[slot];
        slot.table.store(table.code(), Ordering::Relaxed);
        slot.offset.store(offset as u32, Ordering::Relaxed);
        slot.length.store(key.len() as u32, Ordering::Relaxed);
        slot.ready.store(true, Ordering::Release);
    }
    /// Close after the synchronous checker returns. Keys retain exact UTF-8,
    /// not hashes; a prior miss remains a dependency on that absent key.
    pub fn finish(&self) -> Result<Vec<CatalogueRead>, ReadTraceFallback> {
        if self.overflow.load(Ordering::Acquire) {
            return Err(ReadTraceFallback::Capacity);
        }
        let count = self.next.load(Ordering::Acquire);
        let mut reads = BTreeSet::new();
        for slot in &self.slots[..count] {
            if !slot.ready.load(Ordering::Acquire) {
                return Err(ReadTraceFallback::Unfinished);
            }
            let table = CatalogueTable::from_code(slot.table.load(Ordering::Relaxed))
                .ok_or(ReadTraceFallback::Unfinished)?;
            let offset = slot.offset.load(Ordering::Relaxed) as usize;
            let length = slot.length.load(Ordering::Relaxed) as usize;
            let bytes = self.bytes[offset..offset + length]
                .iter()
                .map(|byte| byte.load(Ordering::Relaxed))
                .collect::<Vec<_>>();
            let key = String::from_utf8(bytes).map_err(|_| ReadTraceFallback::Unfinished)?;
            reads.insert(CatalogueRead { table, key });
        }
        // A concurrent overflow cannot turn an incomplete family into a hit.
        if self.overflow.load(Ordering::Acquire) {
            return Err(ReadTraceFallback::Capacity);
        }
        Ok(reads.into_iter().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::sync::Arc;

    #[test]
    fn exact_absent_names_and_table_identity_survive_deduplication() {
        let trace = ReadTrace::new();
        trace.record(CatalogueTable::Signature, "private/α");
        trace.record(CatalogueTable::Signature, "private/α");
        trace.record(CatalogueTable::Fore, "private/α");
        trace.record(CatalogueTable::Signature, "not-installed/yet");
        let reads = trace.finish().unwrap();
        assert_eq!(reads.len(), 3);
        assert!(reads.iter().any(|read| read.key == "not-installed/yet"));
        assert_eq!(
            reads.iter().filter(|read| read.key == "private/α").count(),
            2
        );
    }
    #[test]
    fn slot_or_byte_pressure_requires_whole_context_fallback() {
        let slots = ReadTrace::with_limits(1, 64);
        slots.record(CatalogueTable::Signature, "one");
        slots.record(CatalogueTable::Signature, "two");
        assert_eq!(slots.finish(), Err(ReadTraceFallback::Capacity));
        let bytes = ReadTrace::with_limits(8, 2);
        bytes.record(CatalogueTable::Fore, "long-key");
        assert_eq!(bytes.finish(), Err(ReadTraceFallback::Capacity));
    }
    #[test]
    fn unfinished_lookup_cannot_be_silently_dropped() {
        let trace = ReadTrace::new();
        trace.next.store(1, Ordering::Relaxed);
        assert_eq!(trace.finish(), Err(ReadTraceFallback::Unfinished));
    }
    #[test]
    fn shared_internal_clones_preserve_every_completed_read() {
        let trace = Arc::new(ReadTrace::new());
        let threads = (0..4)
            .map(|thread| {
                let trace = trace.clone();
                std::thread::spawn(move || {
                    for index in 0..64 {
                        trace.record(
                            CatalogueTable::VariadicFore,
                            &alloc::format!("{thread}/{index}"),
                        );
                    }
                })
            })
            .collect::<Vec<_>>();
        for thread in threads {
            thread.join().unwrap();
        }
        assert_eq!(trace.finish().unwrap().len(), 256);
    }
}
