//! A finite physical read range, not a read grant, recording cursor or ACK.
use super::*;

/// Captures a recovered residence's immutable prefix without borrowing its
/// storage between calls. The sole owner can continue appending between reads.
/// Only one existing record buffer is borrowed per call; no batch is copied.
/// Root must separately admit media work, authorize reads from the actual
/// initialized owner and authenticate any exported bytes. Matching IDs alone
/// establish none of that custody. This range is deliberately not serializable.
pub struct RecordReadWindow {
    residence: RecordResidenceId,
    first: u32,
    end: u32,
    next: u32,
}

impl RecordReadWindow {
    pub const fn first(&self) -> DurableRecordIndex {
        DurableRecordIndex(self.first)
    }
    /// Exclusive physical upper bound captured before any read, never extended
    /// by subsequent appends. It is not a producer observation sequence.
    pub const fn end(&self) -> DurableRecordIndex {
        DurableRecordIndex(self.end)
    }
    pub const fn remaining(&self) -> u32 {
        self.end - self.next
    }
    /// Total successful-window scan ceiling. A failed call consumes no cursor
    /// but can consume work; retries need their own admitted attempt budget.
    pub const fn maximum_media_reads(&self) -> u64 {
        (self.end - self.first) as u64 * 2
    }
}

impl<S: NorStorage> PreparedNorRecordResidence<S> {
    /// Preparation without IO/allocation. Select at most `maximum_records` from
    /// the actual recovered tail. Empty available ranges are explicit, but a
    /// zero work bound or a start beyond that tail refuses.
    pub fn capture_read_window(
        &self,
        first: DurableRecordIndex,
        maximum_records: u32,
    ) -> Result<RecordReadWindow, RecordResidenceRefusal> {
        if !self.recovered {
            return Err(RecordResidenceRefusal::RecoveryRequired);
        }
        if maximum_records == 0 || first.0 > self.next {
            return Err(RecordResidenceRefusal::Capacity);
        }
        // Subtraction precedes addition, so even a maximal requested bound
        // cannot overflow or exceed the independently checked residence.
        let count = maximum_records.min(self.next - first.0);
        Ok(RecordReadWindow {
            residence: self.identity,
            first: first.0,
            end: first.0 + count,
            next: first.0,
        })
    }

    /// Read exactly one position through the SAME exclusive media owner.
    /// Exposes gaps/vacancies rather than skipping them. A media refusal leaves
    /// the local range position unchanged; no erasure, durable cursor or
    /// acknowledgment is performed. Returned bytes describe committed MEDIA,
    /// not a producer kernel commit or acceptance by a remote recorder.
    pub fn read_window_next<'a>(
        &'a mut self,
        window: &mut RecordReadWindow,
    ) -> Result<Option<RecordRead<'a>>, RecordResidenceRefusal> {
        if window.residence != self.identity {
            return Err(RecordResidenceRefusal::ForeignResidence);
        }
        if !self.recovered {
            return Err(RecordResidenceRefusal::RecoveryRequired);
        }
        if window.end > self.next {
            return Err(RecordResidenceRefusal::Capacity);
        }
        if window.next == window.end {
            return Ok(None);
        }
        let record = self.read(DurableRecordIndex(window.next))?;
        window.next += 1;
        Ok(Some(record))
    }
}
