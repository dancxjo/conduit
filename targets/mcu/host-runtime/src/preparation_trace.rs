//! Fixed caller-supplied preparation/Play diagnostics, not live runtime truth.
//! Reused from the frozen MCU trace prototype; no firmware hook is installed.
//! These descriptive records are never an admission grant or an allocation bound.
//! Heap values precede an interruptible watermark scan: fields are not one
//! simultaneous observation. Lifetime peak is not a phase-local peak.

const PHASES: usize = 24;
const UNAVAILABLE: u32 = u32::MAX;
pub const PREPARATION_TRACE_ENCODED_BYTES: usize = 16 + PHASES * 20;

#[derive(Clone, Copy)]
#[repr(C)]
struct Record {
    phase: u32,
    heap_used: u32,
    lifetime_peak: u32,
    msp: u32,
    untouched_mark: u32,
}
impl Record {
    const EMPTY: Self = Self {
        phase: 0,
        heap_used: 0,
        lifetime_peak: 0,
        msp: 0,
        untouched_mark: UNAVAILABLE,
    };
}

pub struct PreparationTrace {
    records: [Record; PHASES],
    committed: u32,
    coarse_arena_bytes: u32,
}
impl PreparationTrace {
    pub const fn new() -> Self {
        Self {
            records: [Record::EMPTY; PHASES],
            committed: 0,
            coarse_arena_bytes: UNAVAILABLE,
        }
    }

    /// Retain once from the original live lease's bytes(), not allocator usage.
    /// This records coarse custody only; it creates no counter or authority.
    pub fn retain_arena(&mut self, bytes: usize) -> bool {
        let Ok(bytes) = u32::try_from(bytes) else {
            return false;
        };
        if bytes == UNAVAILABLE || self.coarse_arena_bytes != UNAVAILABLE {
            return false;
        }
        self.coarse_arena_bytes = bytes;
        true
    }

    /// Caller serializes access. The bitmap is committed after the full record.
    /// First observation wins; repeated phases cannot overwrite evidence.
    pub fn record(
        &mut self,
        phase: usize,
        heap_used: usize,
        lifetime_peak: usize,
        msp: usize,
        untouched_mark: Option<usize>,
    ) -> bool {
        if !(1..=PHASES).contains(&phase) {
            return false;
        }
        let bit = 1u32 << (phase - 1);
        if self.committed & bit != 0 {
            return false;
        }
        let (Ok(heap_used), Ok(lifetime_peak), Ok(msp)) = (
            u32::try_from(heap_used),
            u32::try_from(lifetime_peak),
            u32::try_from(msp),
        ) else {
            return false;
        };
        let untouched_mark = match untouched_mark {
            None => UNAVAILABLE,
            Some(value) => match u32::try_from(value) {
                Ok(value) if value != UNAVAILABLE => value,
                _ => return false,
            },
        };
        self.records[phase - 1] = Record {
            phase: phase as u32,
            heap_used,
            lifetime_peak,
            msp,
            untouched_mark,
        };
        self.committed |= bit;
        true
    }

    /// Version1: "CPTR", LE u32 version, committed bitmap, coarse arena bytes;
    /// then24 phase slots, each five LE u32 fields in Record order. Uncommitted
    /// slots are deterministic EMPTY; MAX denotes unavailable arena/watermark.
    /// Reject insufficient capacity before any write; leave extra bytes intact.
    pub fn encode_into(&self, output: &mut [u8]) -> Option<usize> {
        if output.len() < PREPARATION_TRACE_ENCODED_BYTES {
            return None;
        }
        output[..4].copy_from_slice(b"CPTR");
        let mut offset = 4;
        for word in [1, self.committed, self.coarse_arena_bytes] {
            output[offset..offset + 4].copy_from_slice(&word.to_le_bytes());
            offset += 4;
        }
        for record in &self.records {
            for word in [
                record.phase,
                record.heap_used,
                record.lifetime_peak,
                record.msp,
                record.untouched_mark,
            ] {
                output[offset..offset + 4].copy_from_slice(&word.to_le_bytes());
                offset += 4;
            }
        }
        Some(offset)
    }
}

impl Default for PreparationTrace {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[path = "preparation_trace/tests.rs"]
mod tests;
