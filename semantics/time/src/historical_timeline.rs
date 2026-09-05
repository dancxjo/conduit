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
    InvalidSnapshot,
    AccountingOverflow,
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
                    self.evict_oldest()?;
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

    fn evict_oldest(&mut self) -> Result<(), HistoricalTimelineRefusal> {
        let oldest = self.slots[self.head]
            .entry
            .as_ref()
            .expect("a nonempty timeline has an oldest entry");
        let updated_gap = match self.retention_gap {
            Some(gap) => HistoricalRetentionGap {
                first_sequence: gap.first_sequence,
                last_sequence: oldest.sequence,
                entries: gap
                    .entries
                    .checked_add(1)
                    .ok_or(HistoricalTimelineRefusal::AccountingOverflow)?,
                referenced_bytes: gap
                    .referenced_bytes
                    .checked_add(oldest.value.extent.bytes)
                    .ok_or(HistoricalTimelineRefusal::AccountingOverflow)?,
            },
            None => HistoricalRetentionGap {
                first_sequence: oldest.sequence,
                last_sequence: oldest.sequence,
                entries: 1,
                referenced_bytes: oldest.value.extent.bytes,
            },
        };
        let evicted = self.slots[self.head]
            .entry
            .take()
            .expect("the preflighted oldest entry remains present");
        self.referenced_bytes -= evicted.value.extent.bytes;
        self.head = (self.head + 1) % self.slots.len();
        self.length -= 1;
        self.retention_gap = Some(updated_gap);
        Ok(())
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

    /// Project only the currently retained identities and original event
    /// coordinates. Any retention gap remains separately inspectable and is
    /// never converted into a replay event.
    pub fn replay_metadata(&self) -> Vec<crate::HistoricalReplayEntry> {
        let mut replay = Vec::with_capacity(self.length);
        for index in 0..self.length {
            let entry = self
                .entry(index)
                .expect("a retained timeline index names one exact entry");
            replay.push(crate::HistoricalReplayEntry {
                identity: entry.identity.clone(),
                event_time: entry.event_time.clone(),
            });
        }
        replay
    }

    pub(crate) fn snapshot_configuration(
        &self,
    ) -> (
        &KindId,
        &str,
        TemporalScale,
        usize,
        u64,
        HistoricalOverflowPolicy,
        u64,
        u64,
    ) {
        (
            &self.value_profile,
            &self.clock_basis,
            self.time_scale,
            self.slots.len(),
            self.maximum_referenced_bytes,
            self.overflow,
            self.next_sequence,
            self.clear_revision,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn restore(
        value_profile: KindId,
        clock_basis: &str,
        time_scale: TemporalScale,
        maximum_entries: usize,
        maximum_referenced_bytes: u64,
        overflow: HistoricalOverflowPolicy,
        next_sequence: u64,
        clear_revision: u64,
        retention_gap: Option<HistoricalRetentionGap>,
        entries: Vec<HistoricalTimelineEntry>,
    ) -> Result<Self, HistoricalTimelineRefusal> {
        let first_sequence = entries
            .first()
            .map_or(next_sequence, |entry| entry.sequence);
        let mut timeline = Self::new(
            value_profile,
            clock_basis,
            time_scale,
            maximum_entries,
            maximum_referenced_bytes,
            overflow,
            first_sequence,
        )?;
        let mut prior_sequence = None;
        for entry in entries {
            if prior_sequence.is_some_and(|prior| entry.sequence <= prior)
                || entry.sequence >= next_sequence
            {
                return Err(HistoricalTimelineRefusal::InvalidSnapshot);
            }
            timeline.validate_entry(&entry.identity, &entry.event_time, &entry.value)?;
            if timeline.length == timeline.slots.len()
                || timeline.referenced_bytes + entry.value.extent.bytes
                    > timeline.maximum_referenced_bytes
            {
                return Err(HistoricalTimelineRefusal::InvalidSnapshot);
            }
            let sequence = entry.sequence;
            timeline.referenced_bytes += entry.value.extent.bytes;
            timeline.slots[timeline.length].entry = Some(entry);
            timeline.length += 1;
            prior_sequence = Some(sequence);
        }
        let invalid_gap = retention_gap.is_some_and(|gap| {
            overflow != HistoricalOverflowPolicy::EvictOldestWithGap
                || gap.entries == 0
                || gap.first_sequence > gap.last_sequence
                || gap.last_sequence >= next_sequence
                || timeline
                    .entry(0)
                    .is_some_and(|first| gap.last_sequence >= first.sequence)
        });
        if invalid_gap {
            return Err(HistoricalTimelineRefusal::InvalidSnapshot);
        }
        timeline.next_sequence = next_sequence;
        timeline.clear_revision = clear_revision;
        timeline.retention_gap = retention_gap;
        Ok(timeline)
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
            port_id: port_id("command"),
            value_kind: value_kind(crate::HISTORICAL_TIMELINE_COMMAND_INFO_ID),
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
