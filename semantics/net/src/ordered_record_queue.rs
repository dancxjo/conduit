//! Preallocated FIFO state for ordered framed records.

use alloc::vec::Vec;

use crate::{
    decode_typed_record, TypedRecordFrameRefusal, MAXIMUM_TYPED_RECORD_FRAME_BYTES,
    TYPED_RECORD_FRAME_HEADER_BYTES,
};

pub const MAXIMUM_ORDERED_RECORD_QUEUE_ITEMS: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq)]
struct RecordSlot {
    sequence: u64,
    bytes: Vec<u8>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct QueuedRecordRef<'a> {
    pub sequence: u64,
    pub frame: &'a [u8],
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum OrderedRecordQueueRefusal {
    InvalidLimits,
    InvalidFrame(TypedRecordFrameRefusal),
    FrameTooLarge,
    Full,
    Closed,
    SequenceExhausted,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundedOrderedRecordQueue {
    slots: Vec<RecordSlot>,
    maximum_frame_bytes: usize,
    head: usize,
    length: usize,
    next_sequence: u64,
    input_closed: bool,
}

impl BoundedOrderedRecordQueue {
    pub fn new(
        maximum_items: usize,
        maximum_frame_bytes: usize,
        first_sequence: u64,
    ) -> Result<Self, OrderedRecordQueueRefusal> {
        if maximum_items == 0
            || maximum_items > MAXIMUM_ORDERED_RECORD_QUEUE_ITEMS
            || !(TYPED_RECORD_FRAME_HEADER_BYTES..=MAXIMUM_TYPED_RECORD_FRAME_BYTES)
                .contains(&maximum_frame_bytes)
        {
            return Err(OrderedRecordQueueRefusal::InvalidLimits);
        }
        let mut slots = Vec::with_capacity(maximum_items);
        for _ in 0..maximum_items {
            slots.push(RecordSlot {
                sequence: 0,
                bytes: Vec::with_capacity(maximum_frame_bytes),
            });
        }
        Ok(Self {
            slots,
            maximum_frame_bytes,
            head: 0,
            length: 0,
            next_sequence: first_sequence,
            input_closed: false,
        })
    }

    pub fn enqueue(&mut self, frame: &[u8]) -> Result<u64, OrderedRecordQueueRefusal> {
        if self.input_closed {
            return Err(OrderedRecordQueueRefusal::Closed);
        }
        if frame.len() > self.maximum_frame_bytes {
            return Err(OrderedRecordQueueRefusal::FrameTooLarge);
        }
        decode_typed_record(frame).map_err(OrderedRecordQueueRefusal::InvalidFrame)?;
        if self.length == self.slots.len() {
            return Err(OrderedRecordQueueRefusal::Full);
        }
        let following_sequence = self
            .next_sequence
            .checked_add(1)
            .ok_or(OrderedRecordQueueRefusal::SequenceExhausted)?;
        let tail = (self.head + self.length) % self.slots.len();
        let slot = &mut self.slots[tail];
        slot.sequence = self.next_sequence;
        slot.bytes.clear();
        slot.bytes.extend_from_slice(frame);
        self.length += 1;
        let admitted = self.next_sequence;
        self.next_sequence = following_sequence;
        Ok(admitted)
    }

    pub fn dequeue(&mut self) -> Option<QueuedRecordRef<'_>> {
        if self.length == 0 {
            return None;
        }
        let index = self.head;
        self.head = (self.head + 1) % self.slots.len();
        self.length -= 1;
        let slot = &self.slots[index];
        Some(QueuedRecordRef {
            sequence: slot.sequence,
            frame: &slot.bytes,
        })
    }

    pub fn close_input(&mut self) {
        self.input_closed = true;
    }

    pub const fn len(&self) -> usize {
        self.length
    }

    pub const fn is_empty(&self) -> bool {
        self.length == 0
    }

    pub const fn is_input_closed(&self) -> bool {
        self.input_closed
    }

    pub const fn is_terminal(&self) -> bool {
        self.input_closed && self.length == 0
    }

    pub fn slot_capacity(&self, index: usize) -> Option<usize> {
        self.slots.get(index).map(|slot| slot.bytes.capacity())
    }
}
