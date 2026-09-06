//! Preallocated content backing for one finite, refusal-policy history.
//!
//! This is storage machinery, not an operation driver or a scheduler. The
//! caller supplies admitted metadata and content; the existing history owns
//! profile, clock, ordering, and sequence validation. Every accepted sample is
//! retained: no coalescing, downsampling, or implicit replacement occurs.

use conduit_core::{BoundedResourceRef, KindId, TemporalInstant, TemporalScale};
use conduit_kernel::{HostedValueStore, ValueRef, ValueStorage};
use conduit_time::{
    BoundedHistoricalTimeline, HistoricalEntryOrigin, HistoricalOverflowPolicy,
    HistoricalTimelineEntry, HistoricalTimelineRefusal,
};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum HistoryContentRefusal {
    ExtentMismatch,
    DuplicateResourceVersion,
    History(HistoricalTimelineRefusal),
}

/// Content and its ordered semantic index have one lifetime. Construction
/// reserves all item and byte storage; append consumes caller-owned metadata
/// without allocating. Reading a retained entry grants no external authority.
pub struct HostedHistoryContent {
    timeline: BoundedHistoricalTimeline,
    storage: HostedValueStore,
    values: Vec<ValueRef>,
    maximum_bytes: usize,
    maximum_entries: usize,
}

impl HostedHistoryContent {
    pub fn new(
        value_profile: KindId,
        clock_basis: &str,
        time_scale: TemporalScale,
        maximum_entries: usize,
        maximum_bytes: usize,
        first_sequence: u64,
    ) -> Result<Self, HistoricalTimelineRefusal> {
        let timeline = BoundedHistoricalTimeline::new(
            value_profile,
            clock_basis,
            time_scale,
            maximum_entries,
            maximum_bytes as u64,
            HistoricalOverflowPolicy::Refuse,
            first_sequence,
        )?;
        let storage = HostedValueStore::new(
            u16::try_from(maximum_entries).map_err(|_| HistoricalTimelineRefusal::InvalidLimits)?,
            u32::try_from(maximum_bytes).map_err(|_| HistoricalTimelineRefusal::InvalidLimits)?,
            u32::try_from(maximum_bytes).map_err(|_| HistoricalTimelineRefusal::InvalidLimits)?,
        )
        .map_err(|_| HistoricalTimelineRefusal::InvalidLimits)?;
        Ok(Self {
            timeline,
            storage,
            values: Vec::with_capacity(maximum_entries),
            maximum_bytes,
            maximum_entries,
        })
    }

    pub fn append(
        &mut self,
        identity: String,
        event_time: TemporalInstant,
        origin: HistoricalEntryOrigin,
        reference: BoundedResourceRef,
        content: &[u8],
    ) -> Result<u64, HistoryContentRefusal> {
        if reference.extent.bytes != content.len() as u64 {
            return Err(HistoryContentRefusal::ExtentMismatch);
        }
        // A resource version cannot silently acquire a second backing value.
        for index in 0..self.timeline.len() {
            let retained = &self.timeline.entry(index).unwrap().value;
            if retained.identity == reference.identity
                && retained.lifetime.version == reference.lifetime.version
            {
                return Err(HistoryContentRefusal::DuplicateResourceVersion);
            }
        }
        // Preflight physical storage before touching either the index or bytes.
        if self.values.len() == self.maximum_entries {
            return Err(HistoryContentRefusal::History(
                HistoricalTimelineRefusal::Full,
            ));
        }
        if content.len() > self.maximum_bytes {
            return Err(HistoryContentRefusal::History(
                HistoricalTimelineRefusal::EntryExceedsByteLimit,
            ));
        }
        if content.len() > self.maximum_bytes - self.storage.used_bytes() as usize {
            return Err(HistoryContentRefusal::History(
                HistoricalTimelineRefusal::ByteCapacityExceeded,
            ));
        }
        // Refusal-policy append performs every semantic check before mutation.
        // Once it succeeds, the preflighted copies below cannot refuse or grow.
        let sequence = self
            .timeline
            .append(identity, event_time, origin, reference)
            .map_err(HistoryContentRefusal::History)?;
        let stored = self
            .storage
            .store(content)
            .expect("exclusive storage has preflighted item, value, and byte capacity");
        self.values.push(stored);
        Ok(sequence)
    }

    pub fn entry(&self, index: usize) -> Option<(&HistoricalTimelineEntry, &[u8])> {
        let entry = self.timeline.entry(index)?;
        Some((
            entry,
            self.storage
                .get(self.values[index])
                .expect("retained history owns its stored value until clear"),
        ))
    }

    pub const fn timeline(&self) -> &BoundedHistoricalTimeline {
        &self.timeline
    }

    pub fn clear(&mut self) -> Result<(), HistoricalTimelineRefusal> {
        self.timeline.clear()?;
        self.storage.clear();
        self.values.clear();
        Ok(())
    }
}

#[cfg(test)]
#[path = "hosted_history_tests.rs"]
mod tests;
