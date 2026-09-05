//! Typed finite historical indexes over separately realized resource content.

use alloc::{string::String, vec::Vec};
use conduit_core::{BoundedResourceRef, KindId, TemporalInstant, TemporalScale};

pub const MAXIMUM_HISTORICAL_TIMELINE_ENTRIES: usize = 64;
pub const MAXIMUM_HISTORICAL_ENTRY_IDENTITY_BYTES: usize = 128;
pub const MAXIMUM_HISTORICAL_REFERENCED_BYTES: u64 = 64 * 1024 * 1024;

pub const HISTORICAL_TIMELINE_KIND: &str = "history/bounded-typed";
pub const HISTORICAL_TIMELINE_CONTRACT_REVISION: &str = "conduit.history/bounded-typed@1";

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum HistoricalEntryOrigin {
    MachineObservation,
    OperatorAuthored,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoricalTimelineEntry {
    pub sequence: u64,
    pub identity: String,
    pub event_time: TemporalInstant,
    pub origin: HistoricalEntryOrigin,
    pub value: BoundedResourceRef,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum HistoricalOverflowPolicy {
    Refuse,
    EvictOldestWithGap,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct HistoricalRetentionGap {
    pub first_sequence: u64,
    pub last_sequence: u64,
    pub entries: u64,
    pub referenced_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TimelineSlot {
    entry: Option<HistoricalTimelineEntry>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum HistoricalTimelineRefusal {
    InvalidLimits,
    InvalidValueProfile,
    InvalidEntryIdentity,
    InvalidEventTime,
    IncomparableEventTime,
    ReorderedEventTime,
    InvalidResource,
    WrongValueProfile,
    EntryExceedsByteLimit,
    Full,
    ByteCapacityExceeded,
    SequenceExhausted,
    UnknownSequence,
}

/// An allocation-stable ring after construction. Values are exact resource
/// references: storage realization and permission remain outside this history.
pub struct BoundedHistoricalTimeline {
    value_profile: KindId,
    clock_basis: String,
    time_scale: TemporalScale,
    slots: Vec<TimelineSlot>,
    maximum_referenced_bytes: u64,
    overflow: HistoricalOverflowPolicy,
    head: usize,
    length: usize,
    referenced_bytes: u64,
    next_sequence: u64,
    retention_gap: Option<HistoricalRetentionGap>,
    clear_revision: u64,
}

impl BoundedHistoricalTimeline {
    pub fn new(
        value_profile: KindId,
        clock_basis: &str,
        time_scale: TemporalScale,
        maximum_entries: usize,
        maximum_referenced_bytes: u64,
        overflow: HistoricalOverflowPolicy,
        first_sequence: u64,
    ) -> Result<Self, HistoricalTimelineRefusal> {
        if value_profile.as_str().is_empty() {
            return Err(HistoricalTimelineRefusal::InvalidValueProfile);
        }
        if clock_basis.is_empty()
            || clock_basis.len() > conduit_core::MAXIMUM_TEMPORAL_IDENTITY_BYTES
            || maximum_entries == 0
            || maximum_entries > MAXIMUM_HISTORICAL_TIMELINE_ENTRIES
            || maximum_referenced_bytes == 0
            || maximum_referenced_bytes > MAXIMUM_HISTORICAL_REFERENCED_BYTES
        {
            return Err(HistoricalTimelineRefusal::InvalidLimits);
        }
        let mut slots = Vec::with_capacity(maximum_entries);
        for _ in 0..maximum_entries {
            slots.push(TimelineSlot { entry: None });
        }
        Ok(Self {
            value_profile,
            clock_basis: String::from(clock_basis),
            time_scale,
            slots,
            maximum_referenced_bytes,
            overflow,
            head: 0,
            length: 0,
            referenced_bytes: 0,
            next_sequence: first_sequence,
            retention_gap: None,
            clear_revision: 0,
        })
    }

    pub fn append(
        &mut self,
        identity: String,
        event_time: TemporalInstant,
        origin: HistoricalEntryOrigin,
        value: BoundedResourceRef,
    ) -> Result<u64, HistoricalTimelineRefusal> {
        self.validate_entry(&identity, &event_time, &value)?;
        let bytes = value.extent.bytes;
        if bytes > self.maximum_referenced_bytes {
            return Err(HistoricalTimelineRefusal::EntryExceedsByteLimit);
        }
        let following = self
            .next_sequence
            .checked_add(1)
            .ok_or(HistoricalTimelineRefusal::SequenceExhausted)?;
        match self.overflow {
            HistoricalOverflowPolicy::Refuse => {
                if self.length == self.slots.len() {
                    return Err(HistoricalTimelineRefusal::Full);
                }
                if self.referenced_bytes + bytes > self.maximum_referenced_bytes {
                    return Err(HistoricalTimelineRefusal::ByteCapacityExceeded);
                }
            }
            HistoricalOverflowPolicy::EvictOldestWithGap => {
                while self.length == self.slots.len()
                    || self.referenced_bytes + bytes > self.maximum_referenced_bytes
                {
                    self.evict_oldest();
                }
            }
        }
        let tail = (self.head + self.length) % self.slots.len();
        self.slots[tail].entry = Some(HistoricalTimelineEntry {
            sequence: self.next_sequence,
            identity,
            event_time,
            origin,
            value,
        });
        self.length += 1;
        self.referenced_bytes += bytes;
        let sequence = self.next_sequence;
        self.next_sequence = following;
        Ok(sequence)
    }

    fn validate_entry(
        &self,
        identity: &str,
        event_time: &TemporalInstant,
        value: &BoundedResourceRef,
    ) -> Result<(), HistoricalTimelineRefusal> {
        if identity.is_empty() || identity.len() > MAXIMUM_HISTORICAL_ENTRY_IDENTITY_BYTES {
            return Err(HistoricalTimelineRefusal::InvalidEntryIdentity);
        }
        event_time
            .validate()
            .map_err(|_| HistoricalTimelineRefusal::InvalidEventTime)?;
        if event_time.clock_basis != self.clock_basis || event_time.scale != self.time_scale {
            return Err(HistoricalTimelineRefusal::IncomparableEventTime);
        }
        if self
            .entry(self.length.saturating_sub(1))
            .is_some_and(|last| event_time.ticks < last.event_time.ticks)
        {
            return Err(HistoricalTimelineRefusal::ReorderedEventTime);
        }
        value
            .validate()
            .map_err(|_| HistoricalTimelineRefusal::InvalidResource)?;
        if value.content_profile != self.value_profile {
            return Err(HistoricalTimelineRefusal::WrongValueProfile);
        }
        Ok(())
    }

    fn evict_oldest(&mut self) {
        let evicted = self.slots[self.head]
            .entry
            .take()
            .expect("a nonempty timeline has an oldest entry");
        self.referenced_bytes -= evicted.value.extent.bytes;
        self.head = (self.head + 1) % self.slots.len();
        self.length -= 1;
        match &mut self.retention_gap {
            Some(gap) => {
                gap.last_sequence = evicted.sequence;
                gap.entries += 1;
                gap.referenced_bytes += evicted.value.extent.bytes;
            }
            None => {
                self.retention_gap = Some(HistoricalRetentionGap {
                    first_sequence: evicted.sequence,
                    last_sequence: evicted.sequence,
                    entries: 1,
                    referenced_bytes: evicted.value.extent.bytes,
                });
            }
        }
    }

    pub fn remove(
        &mut self,
        sequence: u64,
    ) -> Result<HistoricalTimelineEntry, HistoricalTimelineRefusal> {
        let retained_index = (0..self.length)
            .find(|index| {
                self.entry(*index)
                    .is_some_and(|entry| entry.sequence == sequence)
            })
            .ok_or(HistoricalTimelineRefusal::UnknownSequence)?;
        let physical = (self.head + retained_index) % self.slots.len();
        let removed = self.slots[physical].entry.take().unwrap();
        self.referenced_bytes -= removed.value.extent.bytes;
        for index in retained_index..self.length - 1 {
            let from = (self.head + index + 1) % self.slots.len();
            let to = (self.head + index) % self.slots.len();
            self.slots[to].entry = self.slots[from].entry.take();
        }
        self.length -= 1;
        Ok(removed)
    }

    pub fn clear(&mut self) -> Result<(), HistoricalTimelineRefusal> {
        self.clear_revision = self
            .clear_revision
            .checked_add(1)
            .ok_or(HistoricalTimelineRefusal::SequenceExhausted)?;
        for slot in &mut self.slots {
            slot.entry = None;
        }
        self.head = 0;
        self.length = 0;
        self.referenced_bytes = 0;
        self.retention_gap = None;
        Ok(())
    }

    pub fn entry(&self, retained_index: usize) -> Option<&HistoricalTimelineEntry> {
        if retained_index >= self.length {
            return None;
        }
        self.slots[(self.head + retained_index) % self.slots.len()]
            .entry
            .as_ref()
    }

    pub const fn len(&self) -> usize {
        self.length
    }
    pub const fn is_empty(&self) -> bool {
        self.length == 0
    }
    pub const fn referenced_bytes(&self) -> u64 {
        self.referenced_bytes
    }
    pub const fn retention_gap(&self) -> Option<HistoricalRetentionGap> {
        self.retention_gap
    }
    pub const fn clear_revision(&self) -> u64 {
        self.clear_revision
    }
}

#[cfg(feature = "form-catalog")]
pub fn historical_timeline_kind_definition() -> conduit_form::KindDefinition {
    use alloc::{string::ToString, vec};
    use conduit_core::{
        kind_id, port_id, ConfigurationValue, KindContractRevision, PortDescriptor, PortDirection,
        PortTemporal, StructuredInfoType,
    };
    use conduit_form::{ConfigurationField, ConfigurationRule};
    let value_kind = |identity| {
        StructuredInfoType::leaf(kind_id(identity))
            .expect("reviewed history value identity")
            .profile()
            .expect("reviewed history value profile")
            .value_kind()
            .clone()
    };
    conduit_form::KindDefinition {
        kind_id: kind_id(HISTORICAL_TIMELINE_KIND),
        kind_contract_revision: KindContractRevision::from(HISTORICAL_TIMELINE_CONTRACT_REVISION),
        inputs: alloc::vec![PortDescriptor {
            port_id: port_id("entry"),
            value_kind: value_kind("history/typed-entry@1"),
            direction: PortDirection::Input,
            temporal: PortTemporal::Flow { closes: true }
        }],
        outputs: alloc::vec![PortDescriptor {
            port_id: port_id("timeline"),
            value_kind: value_kind("history/typed-timeline@1"),
            direction: PortDirection::Output,
            temporal: PortTemporal::Value
        }],
        configuration: vec![
            ConfigurationField {
                key: "value-profile".to_string(),
                default_value: ConfigurationValue::Text("value/text@1".to_string()),
                validation: ConfigurationRule::TextBytes { maximum: 128 },
            },
            ConfigurationField {
                key: "maximum-entries".to_string(),
                default_value: ConfigurationValue::U64(16),
                validation: ConfigurationRule::U64Range {
                    minimum: 1,
                    maximum: MAXIMUM_HISTORICAL_TIMELINE_ENTRIES as u64,
                },
            },
            ConfigurationField {
                key: "maximum-referenced-bytes".to_string(),
                default_value: ConfigurationValue::U64(1_048_576),
                validation: ConfigurationRule::U64Range {
                    minimum: 1,
                    maximum: MAXIMUM_HISTORICAL_REFERENCED_BYTES,
                },
            },
            ConfigurationField {
                key: "overflow-policy".to_string(),
                default_value: ConfigurationValue::Text("refuse".to_string()),
                validation: ConfigurationRule::TextOneOf {
                    values: vec!["refuse".to_string(), "evict-oldest-with-gap".to_string()],
                },
            },
        ],
    }
}
