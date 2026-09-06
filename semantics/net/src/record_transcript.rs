//! Preallocated finite history for typed-record and terminal events.

use alloc::vec::Vec;

use crate::{
    decode_typed_record, TypedRecordFrameRefusal, MAXIMUM_TYPED_RECORD_FRAME_BYTES,
    TYPED_RECORD_FRAME_HEADER_BYTES,
};

pub const MAXIMUM_RECORD_TRANSCRIPT_ITEMS: usize = 32;
pub const MAXIMUM_RECORD_TRANSCRIPT_BYTES: usize =
    MAXIMUM_RECORD_TRANSCRIPT_ITEMS * MAXIMUM_TYPED_RECORD_FRAME_BYTES;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum RecordTranscriptDirection {
    Sent,
    Received,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum RecordTranscriptTerminal {
    Completed,
    Disconnected,
    TimedOut,
    Refused(u16),
    Failed(u16),
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum StoredTranscriptEvent {
    Empty,
    Record(RecordTranscriptDirection),
    Terminal(RecordTranscriptTerminal),
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TranscriptSlot {
    sequence: u64,
    event: StoredTranscriptEvent,
    bytes: Vec<u8>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum RecordTranscriptEventRef<'a> {
    Record {
        direction: RecordTranscriptDirection,
        frame: &'a [u8],
    },
    Terminal(RecordTranscriptTerminal),
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct RecordTranscriptEntryRef<'a> {
    pub sequence: u64,
    pub event: RecordTranscriptEventRef<'a>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum RecordTranscriptRefusal {
    InvalidLimits,
    InvalidFrame(TypedRecordFrameRefusal),
    FrameTooLarge,
    EventExceedsByteLimit,
    SequenceExhausted,
}

/// Finite, oldest-first retained history. Retention pressure evicts complete
/// oldest events; it never truncates or partially retains a typed record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundedRecordTranscript {
    slots: Vec<TranscriptSlot>,
    maximum_frame_bytes: usize,
    maximum_retained_bytes: usize,
    head: usize,
    length: usize,
    retained_bytes: usize,
    next_sequence: u64,
    retention_gap: u64,
}

impl BoundedRecordTranscript {
    pub fn new(
        maximum_items: usize,
        maximum_frame_bytes: usize,
        maximum_retained_bytes: usize,
        first_sequence: u64,
    ) -> Result<Self, RecordTranscriptRefusal> {
        if maximum_items == 0
            || maximum_items > MAXIMUM_RECORD_TRANSCRIPT_ITEMS
            || !(TYPED_RECORD_FRAME_HEADER_BYTES..=MAXIMUM_TYPED_RECORD_FRAME_BYTES)
                .contains(&maximum_frame_bytes)
            || maximum_retained_bytes < maximum_frame_bytes
            || maximum_retained_bytes > MAXIMUM_RECORD_TRANSCRIPT_BYTES
        {
            return Err(RecordTranscriptRefusal::InvalidLimits);
        }
        let mut slots = Vec::with_capacity(maximum_items);
        for _ in 0..maximum_items {
            slots.push(TranscriptSlot {
                sequence: 0,
                event: StoredTranscriptEvent::Empty,
                bytes: Vec::with_capacity(maximum_frame_bytes),
            });
        }
        Ok(Self {
            slots,
            maximum_frame_bytes,
            maximum_retained_bytes,
            head: 0,
            length: 0,
            retained_bytes: 0,
            next_sequence: first_sequence,
            retention_gap: 0,
        })
    }

    pub fn record(
        &mut self,
        direction: RecordTranscriptDirection,
        frame: &[u8],
    ) -> Result<u64, RecordTranscriptRefusal> {
        if frame.len() > self.maximum_frame_bytes {
            return Err(RecordTranscriptRefusal::FrameTooLarge);
        }
        decode_typed_record(frame).map_err(RecordTranscriptRefusal::InvalidFrame)?;
        if frame.len() > self.maximum_retained_bytes {
            return Err(RecordTranscriptRefusal::EventExceedsByteLimit);
        }
        self.append(StoredTranscriptEvent::Record(direction), frame)
    }

    pub fn terminal(
        &mut self,
        terminal: RecordTranscriptTerminal,
    ) -> Result<u64, RecordTranscriptRefusal> {
        self.append(StoredTranscriptEvent::Terminal(terminal), &[])
    }

    fn append(
        &mut self,
        event: StoredTranscriptEvent,
        bytes: &[u8],
    ) -> Result<u64, RecordTranscriptRefusal> {
        let following_sequence = self
            .next_sequence
            .checked_add(1)
            .ok_or(RecordTranscriptRefusal::SequenceExhausted)?;
        while self.length == self.slots.len()
            || self.retained_bytes + bytes.len() > self.maximum_retained_bytes
        {
            self.evict_oldest();
        }
        let tail = (self.head + self.length) % self.slots.len();
        let slot = &mut self.slots[tail];
        slot.sequence = self.next_sequence;
        slot.event = event;
        slot.bytes.clear();
        slot.bytes.extend_from_slice(bytes);
        self.length += 1;
        self.retained_bytes += bytes.len();
        let admitted = self.next_sequence;
        self.next_sequence = following_sequence;
        Ok(admitted)
    }

    fn evict_oldest(&mut self) {
        let slot = &mut self.slots[self.head];
        self.retained_bytes -= slot.bytes.len();
        slot.bytes.clear();
        slot.event = StoredTranscriptEvent::Empty;
        self.head = (self.head + 1) % self.slots.len();
        self.length -= 1;
        self.retention_gap += 1;
    }

    pub fn entry(&self, retained_index: usize) -> Option<RecordTranscriptEntryRef<'_>> {
        if retained_index >= self.length {
            return None;
        }
        let slot = &self.slots[(self.head + retained_index) % self.slots.len()];
        let event = match slot.event {
            StoredTranscriptEvent::Record(direction) => RecordTranscriptEventRef::Record {
                direction,
                frame: &slot.bytes,
            },
            StoredTranscriptEvent::Terminal(terminal) => {
                RecordTranscriptEventRef::Terminal(terminal)
            }
            StoredTranscriptEvent::Empty => return None,
        };
        Some(RecordTranscriptEntryRef {
            sequence: slot.sequence,
            event,
        })
    }

    pub const fn len(&self) -> usize {
        self.length
    }

    pub const fn is_empty(&self) -> bool {
        self.length == 0
    }

    pub const fn retained_bytes(&self) -> usize {
        self.retained_bytes
    }

    pub const fn retention_gap(&self) -> u64 {
        self.retention_gap
    }

    pub fn slot_capacity(&self, index: usize) -> Option<usize> {
        self.slots.get(index).map(|slot| slot.bytes.capacity())
    }
}
