//! Allocation-free consumer cursor for an already decoded finite read exchange.
//!
//! The caller independently authenticates the residence/generation, typed Fore
//! and producer, and funds requests, retries and transport. This cursor supplies
//! no media ownership, generation proof, recording commitment or retirement ACK.
use crate::read_result::{GapReason, ReadResult, MAXIMUM_RECORD_BYTES};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecordScanRefusal {
    InvalidBound,
    Complete,
    WrongIndex,
    InvalidExtent,
    RecordTooLarge,
    RecordBoundExceeded,
    TailRegressed,
    UnexpectedEnd,
    Overflow,
    Cancelled,
    Busy,
    RecoveryRequired,
    Unavailable,
}

/// One borrowed record or explicit physical gap; neither is an observation ID.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecordScanItem<'a> {
    Recorded { index: u32, record: &'a [u8] },
    Gap { index: u32, reason: GapReason },
    End,
}

/// Pins the first accepted physical tail. Later appends cannot extend this scan.
/// Not serializable: persistence/epoch selection belongs to the history owner.
#[derive(Debug, PartialEq, Eq)]
pub struct RecordScanCursor {
    first: u32,
    next: u32,
    maximum_records: u32,
    captured_end: Option<u32>,
    observed_end: u32,
}
impl RecordScanCursor {
    /// A zero record allowance refuses. An empty captured prefix is still valid.
    /// `first + maximum_records` must fit u32. The first response must fit that
    /// allowance; no prefix is silently omitted or called complete history.
    pub const fn new(first: u32, maximum_records: u32) -> Result<Self, RecordScanRefusal> {
        if maximum_records == 0 {
            return Err(RecordScanRefusal::InvalidBound);
        }
        if first.checked_add(maximum_records).is_none() {
            return Err(RecordScanRefusal::Overflow);
        }
        Ok(Self {
            first,
            next: first,
            maximum_records,
            captured_end: None,
            observed_end: first,
        })
    }
    pub const fn next_index(&self) -> Option<u32> {
        match self.captured_end {
            Some(end) if self.next == end => None,
            _ => Some(self.next),
        }
    }
    pub const fn captured_end(&self) -> Option<u32> {
        self.captured_end
    }
    pub const fn remaining(&self) -> Option<u32> {
        match self.captured_end {
            Some(end) => Some(end - self.next),
            None => None,
        }
    }
    fn check_tail(&self, end: u32) -> Result<(), RecordScanRefusal> {
        if end < self.observed_end {
            return Err(RecordScanRefusal::TailRegressed);
        }
        if self.captured_end.is_none() && end - self.first > self.maximum_records {
            return Err(RecordScanRefusal::RecordBoundExceeded);
        }
        Ok(())
    }
    /// Consume only the exact outstanding index. Every refusal leaves the cursor
    /// unchanged. Busy/retry attempts still consume the caller's own work budget.
    /// Returned payload borrows the response; this cursor retains no payload.
    pub fn accept<'a>(
        &mut self,
        requested_index: u32,
        response: ReadResult<'a>,
    ) -> Result<RecordScanItem<'a>, RecordScanRefusal> {
        use RecordScanRefusal as R;
        let expected = self.next_index().ok_or(R::Complete)?;
        if requested_index != expected {
            return Err(R::WrongIndex);
        }
        let (index, end, item) = match response {
            ReadResult::Recorded {
                index,
                captured_end,
                record,
            } => {
                if record.len() > MAXIMUM_RECORD_BYTES {
                    return Err(R::RecordTooLarge);
                }
                (
                    index,
                    captured_end,
                    RecordScanItem::Recorded { index, record },
                )
            }
            ReadResult::Gap {
                index,
                captured_end,
                reason,
            } => (index, captured_end, RecordScanItem::Gap { index, reason }),
            ReadResult::End(end) => {
                if end != expected {
                    return Err(R::UnexpectedEnd);
                }
                self.check_tail(end)?;
                // An admitted scan never requests its pinned ceiling. Therefore
                // End is accepted only as the initial empty range, not early EOF.
                if self.captured_end.is_some() {
                    return Err(R::UnexpectedEnd);
                }
                self.captured_end = Some(end);
                self.observed_end = end;
                return Ok(RecordScanItem::End);
            }
            ReadResult::Cancelled => return Err(R::Cancelled),
            ReadResult::Busy => return Err(R::Busy),
            ReadResult::RecoveryRequired => return Err(R::RecoveryRequired),
            ReadResult::Unavailable => return Err(R::Unavailable),
        };
        if index != expected {
            return Err(R::WrongIndex);
        }
        if index >= end {
            return Err(R::InvalidExtent);
        }
        self.check_tail(end)?;
        let next = index.checked_add(1).ok_or(R::Overflow)?;
        self.captured_end.get_or_insert(end);
        self.observed_end = end;
        self.next = next;
        Ok(item)
    }
}

#[cfg(test)]
#[path = "read_scan/tests.rs"]
mod tests;
