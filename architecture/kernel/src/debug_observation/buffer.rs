use core::mem::size_of;

use super::{
    DebugEventKind, DebugExecutionIdentity, DebugObservationGap, DebugObservationInput,
    DebugObservationRecord, DebugObservationRefusal, DEBUG_OBSERVATION_SCHEMA_VERSION,
    MAX_DEBUG_VALUE_PREVIEW_BYTES,
};

pub struct DebugObservationBuffer<const RECORDS: usize> {
    execution: DebugExecutionIdentity,
    records: [Option<DebugObservationRecord>; RECORDS],
    head: usize,
    len: usize,
    item_capacity: usize,
    byte_capacity: u32,
    maximum_preview_bytes: u8,
    used_bytes: u32,
    next_sequence: u64,
    last_host_sequences: [Option<(u16, u64)>; RECORDS],
    dropped_records: u64,
}

impl<const RECORDS: usize> DebugObservationBuffer<RECORDS> {
    pub fn new(
        execution: DebugExecutionIdentity,
        item_capacity: u16,
        byte_capacity: u32,
        maximum_preview_bytes: u8,
    ) -> Result<Self, DebugObservationRefusal> {
        let item_capacity = usize::from(item_capacity);
        let record_bytes = u32::try_from(size_of::<DebugObservationRecord>())
            .map_err(|_| DebugObservationRefusal::InvalidBounds)?;
        let physical_bytes = u32::try_from(item_capacity)
            .ok()
            .and_then(|count| count.checked_mul(record_bytes))
            .ok_or(DebugObservationRefusal::InvalidBounds)?;
        if RECORDS == 0
            || item_capacity == 0
            || item_capacity > RECORDS
            || byte_capacity < record_bytes
            || byte_capacity > physical_bytes
            || usize::from(maximum_preview_bytes) > MAX_DEBUG_VALUE_PREVIEW_BYTES
        {
            return Err(DebugObservationRefusal::InvalidBounds);
        }
        Ok(Self {
            execution,
            records: [None; RECORDS],
            head: 0,
            len: 0,
            item_capacity,
            byte_capacity,
            maximum_preview_bytes,
            used_bytes: 0,
            next_sequence: 0,
            last_host_sequences: [None; RECORDS],
            dropped_records: 0,
        })
    }

    pub const fn execution(&self) -> DebugExecutionIdentity {
        self.execution
    }

    pub fn len(&self) -> usize {
        self.len
    }

    pub fn is_empty(&self) -> bool {
        self.len == 0
    }

    pub const fn item_capacity(&self) -> usize {
        self.item_capacity
    }

    pub const fn byte_capacity(&self) -> u32 {
        self.byte_capacity
    }

    pub const fn used_bytes(&self) -> u32 {
        self.used_bytes
    }

    pub fn record(&self, index: usize) -> Option<&DebugObservationRecord> {
        if index >= self.len {
            return None;
        }
        self.records[(self.head + index) % self.item_capacity].as_ref()
    }

    pub fn latest(&self) -> Option<&DebugObservationRecord> {
        self.len.checked_sub(1).and_then(|index| self.record(index))
    }

    pub fn gap(&self) -> Option<DebugObservationGap> {
        if self.dropped_records == 0 {
            return None;
        }
        Some(DebugObservationGap {
            dropped_records: self.dropped_records,
            first_retained_sequence: self
                .record(0)
                .map_or(self.next_sequence, |record| record.sequence),
        })
    }

    pub fn admit(
        &mut self,
        input: DebugObservationInput<'_>,
    ) -> Result<DebugObservationRecord, DebugObservationRefusal> {
        if input.execution != self.execution {
            return Err(DebugObservationRefusal::StaleExecution);
        }
        if matches!(input.kind, DebugEventKind::Unsupported(_)) {
            return Err(DebugObservationRefusal::UnsupportedEventKind);
        }
        if input.kind == DebugEventKind::Fault && input.fault_code.is_none() {
            return Err(DebugObservationRefusal::InvalidFault);
        }
        if matches!(
            input.kind,
            DebugEventKind::ValueSent | DebugEventKind::ValueReceived
        ) && input.value.is_none()
            && input.type_identity.is_none()
        {
            return Err(DebugObservationRefusal::InvalidValueObservation);
        }
        self.admit_host_sequence(input.host, input.host_sequence)?;
        let sequence = self.next_sequence;
        self.next_sequence = self
            .next_sequence
            .checked_add(1)
            .ok_or(DebugObservationRefusal::InvalidSequence)?;
        let mut preview = [0; MAX_DEBUG_VALUE_PREVIEW_BYTES];
        let value = input.value.unwrap_or(&[]);
        let preview_len = value
            .len()
            .min(usize::from(self.maximum_preview_bytes))
            .min(MAX_DEBUG_VALUE_PREVIEW_BYTES);
        preview[..preview_len].copy_from_slice(&value[..preview_len]);
        let record = DebugObservationRecord {
            schema_version: DEBUG_OBSERVATION_SCHEMA_VERSION,
            execution: input.execution,
            sequence,
            host_sequence: input.host_sequence,
            host: input.host,
            form: input.form,
            subject: input.subject,
            related_subject: input.related_subject,
            kind: input.kind,
            type_identity: input.type_identity,
            value_bytes: u32::try_from(value.len()).unwrap_or(u32::MAX),
            preview_len: u8::try_from(preview_len).unwrap_or(u8::MAX),
            preview_truncated: preview_len < value.len(),
            preview,
            fault_code: input.fault_code,
            causal_parent_sequence: input.causal_parent_sequence,
            invocation_sequence: input.invocation_sequence,
        };
        record.validate_for(self.execution, self.maximum_preview_bytes)?;
        self.push(record);
        Ok(record)
    }

    fn admit_host_sequence(
        &mut self,
        host: u16,
        sequence: u64,
    ) -> Result<(), DebugObservationRefusal> {
        if let Some((_, prior)) = self
            .last_host_sequences
            .iter_mut()
            .flatten()
            .find(|(known, _)| *known == host)
        {
            if sequence <= *prior {
                return Err(DebugObservationRefusal::InvalidSequence);
            }
            *prior = sequence;
            return Ok(());
        }
        let slot = self
            .last_host_sequences
            .iter_mut()
            .find(|slot| slot.is_none())
            .ok_or(DebugObservationRefusal::InvalidBounds)?;
        *slot = Some((host, sequence));
        Ok(())
    }

    fn push(&mut self, record: DebugObservationRecord) {
        let charge = u32::try_from(size_of::<DebugObservationRecord>()).unwrap_or(u32::MAX);
        let capacity_by_bytes = usize::try_from(self.byte_capacity / charge).unwrap_or(0);
        let retained_capacity = self.item_capacity.min(capacity_by_bytes);
        if self.len == retained_capacity {
            self.records[self.head] = None;
            self.head = (self.head + 1) % self.item_capacity;
            self.len -= 1;
            self.used_bytes -= charge;
            self.dropped_records = self.dropped_records.saturating_add(1);
        }
        let index = (self.head + self.len) % self.item_capacity;
        self.records[index] = Some(record);
        self.len += 1;
        self.used_bytes += charge;
    }
}
