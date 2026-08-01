//! Bounded, non-interfering hosted Watch storage.
//!
//! The scheduler calls this boundary only after a real cord publication has
//! committed. Observation is infallible from the data plane's perspective:
//! an unavailable preview slot is reported as an observation gap and never
//! changes the cord offer, demand, pressure, or node outcome.

use std::mem::size_of;

use conduit_core::{
    Direction, EvidenceCursorStatus, ExecutionPlan, SemanticHash, Sensitivity, WatchRetention,
    WatchSubject, classify_evidence_cursor,
};
use sha2::{Digest as _, Sha256};

use crate::{HostValueStore, RuntimeError, RuntimeValue};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExactWatchSubject {
    Cord {
        cord: String,
    },
    NodePort {
        node: String,
        port: String,
        direction: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ExactWatchMaterial {
    Preview(Vec<u8>),
    Redacted,
    Absent,
}

/// One caller-owned value observation copied from fixed session storage.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactWatchObservation {
    pub cursor: u64,
    pub source_sequence: u64,
    pub tick: u64,
    pub watch_id: String,
    pub subject: ExactWatchSubject,
    pub value_handle: u64,
    pub accounted_bytes: u32,
    pub representation_id: String,
    pub representation_schema_version: u32,
    pub representation_semantic_hash: SemanticHash,
    pub sensitivity: Sensitivity,
    pub value_identity: Option<SemanticHash>,
    pub provenance: Option<SemanticHash>,
    pub content_hash: Option<SemanticHash>,
    pub original_bytes: u32,
    pub truncated: bool,
    pub gap_before: u64,
    pub material: ExactWatchMaterial,
}

/// A bounded read from one admitted Watch's retained window.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactWatchBatch {
    pub status: EvidenceCursorStatus,
    pub earliest_cursor: u64,
    pub next_cursor: u64,
    pub records: Vec<ExactWatchObservation>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ExactWatchUsage {
    pub admitted_slots: u32,
    pub attached_slots: u32,
    pub retained_observations: u64,
    pub retained_preview_bytes: u64,
    pub dropped_observations: u64,
    pub maximum_observations: u64,
    pub maximum_preview_bytes: u64,
}

#[derive(Clone, Copy)]
struct StoredWatchObservation {
    cursor: u64,
    source_sequence: u64,
    tick: u64,
    value_handle: u64,
    accounted_bytes: u32,
    sensitivity: Sensitivity,
    value_identity: Option<SemanticHash>,
    provenance: Option<SemanticHash>,
    content_hash: Option<SemanticHash>,
    original_bytes: u32,
    preview_len: u32,
    truncated: bool,
    redacted: bool,
    absent: bool,
    gap_before: u64,
}

struct HostedWatchSlot {
    id: String,
    subject: ExactWatchSubject,
    cord: usize,
    representation_id: String,
    representation_schema_version: u32,
    representation_semantic_hash: SemanticHash,
    maximum_preview_bytes: usize,
    maximum_history: usize,
    minimum_tick_interval: u64,
    retention: WatchRetention,
    sensitivity_ceiling: Sensitivity,
    attached: bool,
    next_cursor: u64,
    source_sequence: u64,
    last_observed_tick: Option<u64>,
    gap_before_next: u64,
    dropped_observations: u64,
    records: Vec<Option<StoredWatchObservation>>,
    previews: Vec<u8>,
}

pub(crate) struct HostedWatchRuntime {
    slots: Vec<HostedWatchSlot>,
}

impl HostedWatchRuntime {
    pub(crate) fn from_plan(plan: &ExecutionPlan<'_>) -> Result<Self, RuntimeError> {
        let mut slots = Vec::new();
        slots
            .try_reserve_exact(plan.watch_admissions.len())
            .map_err(|_| {
                RuntimeError::new("CND-SCH-005", "Watch admission storage allocation failed")
            })?;
        for admission in plan.watch_admissions {
            let (cord, subject) = match admission.subject {
                WatchSubject::Cord(id) => {
                    let cord = plan
                        .cords
                        .iter()
                        .position(|cord| cord.id == id)
                        .ok_or_else(|| {
                            RuntimeError::new(
                                "CND-WAT-002",
                                "Watch cord is absent from the exact plan",
                            )
                        })?;
                    (
                        cord,
                        ExactWatchSubject::Cord {
                            cord: id.as_str().to_owned(),
                        },
                    )
                }
                WatchSubject::NodePort {
                    node,
                    port,
                    direction,
                } => {
                    let cord = plan
                        .cords
                        .iter()
                        .position(|cord| {
                            let endpoint = if direction == Direction::Output {
                                cord.from
                            } else {
                                cord.to
                            };
                            endpoint.node == node
                                && endpoint.port == port
                                && endpoint.direction == direction
                        })
                        .ok_or_else(|| {
                            RuntimeError::new(
                                "CND-WAT-002",
                                "Watch node port is absent from the exact plan",
                            )
                        })?;
                    (
                        cord,
                        ExactWatchSubject::NodePort {
                            node: node.as_str().to_owned(),
                            port: port.as_str().to_owned(),
                            direction: direction.as_str().to_owned(),
                        },
                    )
                }
            };
            let maximum_history = usize::from(admission.maximum_history);
            let maximum_preview_bytes = usize::try_from(admission.maximum_preview_bytes)
                .map_err(|_| RuntimeError::new("CND-WAT-003", "Watch preview bound overflowed"))?;
            let preview_capacity = maximum_history
                .checked_mul(maximum_preview_bytes)
                .ok_or_else(|| RuntimeError::new("CND-WAT-003", "Watch storage overflowed"))?;
            let mut records = Vec::new();
            records.try_reserve_exact(maximum_history).map_err(|_| {
                RuntimeError::new("CND-SCH-005", "Watch record storage allocation failed")
            })?;
            records.resize(maximum_history, None);
            let mut previews = Vec::new();
            previews.try_reserve_exact(preview_capacity).map_err(|_| {
                RuntimeError::new("CND-SCH-005", "Watch preview storage allocation failed")
            })?;
            previews.resize(preview_capacity, 0);
            slots.push(HostedWatchSlot {
                id: admission.id.as_str().to_owned(),
                subject,
                cord,
                representation_id: admission.representation.id.as_str().to_owned(),
                representation_schema_version: admission.representation.schema_version,
                representation_semantic_hash: admission.representation.semantic_hash,
                maximum_preview_bytes,
                maximum_history,
                minimum_tick_interval: admission.minimum_tick_interval,
                retention: admission.retention,
                sensitivity_ceiling: admission.sensitivity_ceiling,
                attached: false,
                next_cursor: 0,
                source_sequence: 0,
                last_observed_tick: None,
                gap_before_next: 0,
                dropped_observations: 0,
                records,
                previews,
            });
        }
        Ok(Self { slots })
    }

    pub(crate) fn attach(&mut self, watch_id: &str) -> Result<(), RuntimeError> {
        let slot = self.slot_mut(watch_id)?;
        slot.attached = true;
        Ok(())
    }

    pub(crate) fn detach(&mut self, watch_id: &str) -> Result<(), RuntimeError> {
        let slot = self.slot_mut(watch_id)?;
        slot.attached = false;
        Ok(())
    }

    pub(crate) fn observe(
        &mut self,
        cord: usize,
        value: RuntimeValue,
        tick: u64,
        values: &HostValueStore,
    ) {
        for slot in self.slots.iter_mut().filter(|slot| slot.cord == cord) {
            let source_sequence = slot.source_sequence;
            slot.source_sequence = slot.source_sequence.saturating_add(1);
            if !slot.attached {
                continue;
            }
            if slot
                .last_observed_tick
                .is_some_and(|last| tick < last.saturating_add(slot.minimum_tick_interval))
            {
                slot.gap_before_next = slot.gap_before_next.saturating_add(1);
                slot.dropped_observations = slot.dropped_observations.saturating_add(1);
                continue;
            }
            // Latest, ring, and sample all use the same isolated fixed ring.
            // Their distinct plan identity controls history and rate bounds.
            debug_assert!(matches!(
                slot.retention,
                WatchRetention::Latest | WatchRetention::Ring | WatchRetention::Sample
            ));
            slot.last_observed_tick = Some(tick);
            let cursor = slot.next_cursor;
            slot.next_cursor = slot.next_cursor.saturating_add(1);
            let record_index = usize::try_from(cursor % slot.maximum_history as u64)
                .expect("Watch history index is bounded");
            let bytes = values.get(value.handle);
            let sensitivity = value.envelope.sensitivity;
            let redacted =
                sensitivity != Sensitivity::Public || sensitivity > slot.sensitivity_ceiling;
            let original_bytes = bytes
                .and_then(|bytes| u32::try_from(bytes.len()).ok())
                .unwrap_or(value.accounted_bytes);
            let preview_len = if redacted {
                0
            } else {
                bytes.map_or(0, |bytes| bytes.len().min(slot.maximum_preview_bytes))
            };
            if preview_len != 0 {
                let start = record_index * slot.maximum_preview_bytes;
                slot.previews[start..start + preview_len].copy_from_slice(
                    &bytes.expect("preview length requires material")[..preview_len],
                );
            }
            let content_hash = (!redacted)
                .then(|| bytes.map(|bytes| SemanticHash::from_bytes(Sha256::digest(bytes).into())))
                .flatten();
            slot.records[record_index] = Some(StoredWatchObservation {
                cursor,
                source_sequence,
                tick,
                value_handle: value.handle,
                accounted_bytes: value.accounted_bytes,
                sensitivity,
                value_identity: value.envelope.identity,
                provenance: value.envelope.provenance,
                content_hash,
                original_bytes,
                preview_len: u32::try_from(preview_len).expect("preview bound is u32"),
                truncated: !redacted && bytes.is_some_and(|bytes| preview_len < bytes.len()),
                redacted,
                absent: bytes.is_none(),
                gap_before: std::mem::take(&mut slot.gap_before_next),
            });
        }
    }

    pub(crate) fn read(
        &self,
        watch_id: &str,
        cursor: u64,
        maximum_records: u32,
    ) -> Result<ExactWatchBatch, RuntimeError> {
        if maximum_records == 0 {
            return Err(RuntimeError::new(
                "CND-WAT-003",
                "Watch read bound must be nonzero",
            ));
        }
        let slot = self.slot(watch_id)?;
        let retained = slot.next_cursor.min(slot.maximum_history as u64);
        let earliest_cursor = slot.next_cursor.saturating_sub(retained);
        let status =
            classify_evidence_cursor(cursor, earliest_cursor, slot.next_cursor).map_err(|_| {
                RuntimeError::new("CND-WAT-003", "Watch retained cursor is inconsistent")
            })?;
        let start = match status {
            EvidenceCursorStatus::Available => cursor,
            EvidenceCursorStatus::Gap { resume_at } => resume_at,
            EvidenceCursorStatus::Future { next_sequence } => next_sequence,
        };
        let end = start
            .saturating_add(u64::from(maximum_records))
            .min(slot.next_cursor);
        let count = usize::try_from(end.saturating_sub(start)).map_err(|_| {
            RuntimeError::new("CND-WAT-003", "Watch read bound exceeds the platform")
        })?;
        let mut records = Vec::new();
        records
            .try_reserve_exact(count)
            .map_err(|_| RuntimeError::new("CND-SCH-005", "Watch read allocation failed"))?;
        for retained_cursor in start..end {
            let record_index = usize::try_from(retained_cursor % slot.maximum_history as u64)
                .expect("Watch history index is bounded");
            let stored = slot.records[record_index].ok_or_else(|| {
                RuntimeError::new("CND-WAT-003", "Watch retained window is inconsistent")
            })?;
            if stored.cursor != retained_cursor {
                return Err(RuntimeError::new(
                    "CND-WAT-003",
                    "Watch retained cursor is inconsistent",
                ));
            }
            let material = if stored.redacted {
                ExactWatchMaterial::Redacted
            } else if stored.absent {
                ExactWatchMaterial::Absent
            } else {
                let preview_len = usize::try_from(stored.preview_len)
                    .expect("stored Watch preview length is bounded");
                let preview_start = record_index * slot.maximum_preview_bytes;
                let mut preview = Vec::new();
                preview.try_reserve_exact(preview_len).map_err(|_| {
                    RuntimeError::new("CND-SCH-005", "Watch read allocation failed")
                })?;
                preview
                    .extend_from_slice(&slot.previews[preview_start..preview_start + preview_len]);
                ExactWatchMaterial::Preview(preview)
            };
            records.push(ExactWatchObservation {
                cursor: stored.cursor,
                source_sequence: stored.source_sequence,
                tick: stored.tick,
                watch_id: slot.id.clone(),
                subject: slot.subject.clone(),
                value_handle: stored.value_handle,
                accounted_bytes: stored.accounted_bytes,
                representation_id: slot.representation_id.clone(),
                representation_schema_version: slot.representation_schema_version,
                representation_semantic_hash: slot.representation_semantic_hash,
                sensitivity: stored.sensitivity,
                value_identity: stored.value_identity,
                provenance: stored.provenance,
                content_hash: stored.content_hash,
                original_bytes: stored.original_bytes,
                truncated: stored.truncated,
                gap_before: stored.gap_before,
                material,
            });
        }
        Ok(ExactWatchBatch {
            status,
            earliest_cursor,
            next_cursor: end,
            records,
        })
    }

    pub(crate) fn usage(&self) -> ExactWatchUsage {
        self.slots.iter().fold(
            ExactWatchUsage {
                admitted_slots: u32::try_from(self.slots.len()).unwrap_or(u32::MAX),
                ..ExactWatchUsage::default()
            },
            |mut usage, slot| {
                usage.attached_slots += u32::from(slot.attached);
                let retained = slot.next_cursor.min(slot.maximum_history as u64);
                usage.retained_observations += retained;
                usage.retained_preview_bytes += slot
                    .records
                    .iter()
                    .flatten()
                    .map(|record| u64::from(record.preview_len))
                    .sum::<u64>();
                usage.dropped_observations += slot.dropped_observations;
                usage.maximum_observations += slot.maximum_history as u64;
                usage.maximum_preview_bytes +=
                    (slot.maximum_history as u64).saturating_mul(slot.maximum_preview_bytes as u64);
                usage
            },
        )
    }

    fn slot(&self, watch_id: &str) -> Result<&HostedWatchSlot, RuntimeError> {
        self.slots
            .iter()
            .find(|slot| slot.id == watch_id)
            .ok_or_else(|| RuntimeError::new("CND-WAT-002", "Watch is not admitted by this plan"))
    }

    fn slot_mut(&mut self, watch_id: &str) -> Result<&mut HostedWatchSlot, RuntimeError> {
        self.slots
            .iter_mut()
            .find(|slot| slot.id == watch_id)
            .ok_or_else(|| RuntimeError::new("CND-WAT-002", "Watch is not admitted by this plan"))
    }
}

pub(crate) fn planned_watch_memory_bytes(
    plan: &ExecutionPlan<'_>,
) -> Result<u64, crate::SchedulerError> {
    if plan.watch_admissions.is_empty() {
        return Ok(0);
    }
    let mut total = u64::try_from(size_of::<HostedWatchRuntime>())
        .map_err(|_| crate::SchedulerError::ArithmeticOverflow)?;
    for admission in plan.watch_admissions {
        let history = u64::from(admission.maximum_history);
        let preview = u64::from(admission.maximum_preview_bytes)
            .checked_mul(history)
            .ok_or(crate::SchedulerError::ArithmeticOverflow)?;
        let records = u64::try_from(size_of::<Option<StoredWatchObservation>>())
            .map_err(|_| crate::SchedulerError::ArithmeticOverflow)?
            .checked_mul(history)
            .ok_or(crate::SchedulerError::ArithmeticOverflow)?;
        let slot = u64::try_from(size_of::<HostedWatchSlot>())
            .map_err(|_| crate::SchedulerError::ArithmeticOverflow)?;
        let strings = u64::try_from(
            admission.id.as_str().len()
                + admission.representation.id.as_str().len()
                + match admission.subject {
                    WatchSubject::Cord(cord) => cord.as_str().len(),
                    WatchSubject::NodePort { node, port, .. } => {
                        node.as_str().len() + port.as_str().len()
                    }
                },
        )
        .map_err(|_| crate::SchedulerError::ArithmeticOverflow)?;
        total = total
            .checked_add(preview)
            .and_then(|total| total.checked_add(records))
            .and_then(|total| total.checked_add(slot))
            .and_then(|total| total.checked_add(strings))
            .ok_or(crate::SchedulerError::ArithmeticOverflow)?;
    }
    Ok(total)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RuntimeValueEnvelope;

    fn slot() -> HostedWatchSlot {
        HostedWatchSlot {
            id: "watch/test".to_owned(),
            subject: ExactWatchSubject::Cord {
                cord: "cord/test".to_owned(),
            },
            cord: 0,
            representation_id: "std/text".to_owned(),
            representation_schema_version: 0,
            representation_semantic_hash: SemanticHash::from_bytes([7; 32]),
            maximum_preview_bytes: 3,
            maximum_history: 2,
            minimum_tick_interval: 2,
            retention: WatchRetention::Ring,
            sensitivity_ceiling: Sensitivity::Restricted,
            attached: true,
            next_cursor: 0,
            source_sequence: 0,
            last_observed_tick: None,
            gap_before_next: 0,
            dropped_observations: 0,
            records: vec![None; 2],
            previews: vec![0; 6],
        }
    }

    #[test]
    fn fixed_watch_ring_reports_rate_and_retention_gaps_without_revealing_protected_values() {
        let mut watches = HostedWatchRuntime {
            slots: vec![slot()],
        };
        let mut values = HostValueStore::with_limits(32, 4).unwrap();
        let public_handle = values.store(b"hello").unwrap();
        let public = RuntimeValue {
            handle: public_handle,
            accounted_bytes: 5,
            envelope: RuntimeValueEnvelope::EMPTY,
        };

        watches.observe(0, public, 0, &values);
        watches.observe(0, public, 1, &values);
        watches.observe(0, public, 2, &values);
        watches.observe(0, public, 4, &values);

        let retained = watches.read("watch/test", 0, 8).unwrap();
        assert_eq!(retained.status, EvidenceCursorStatus::Gap { resume_at: 1 });
        assert_eq!(retained.records.len(), 2);
        assert_eq!(retained.records[0].gap_before, 1);
        assert_eq!(retained.records[0].source_sequence, 2);
        assert_eq!(
            retained.records[0].material,
            ExactWatchMaterial::Preview(b"hel".to_vec())
        );
        assert!(retained.records[0].truncated);
        assert!(retained.records[0].content_hash.is_some());

        let protected_handle = values.store(b"secret").unwrap();
        let mut protected_envelope = RuntimeValueEnvelope::EMPTY;
        protected_envelope.sensitivity = Sensitivity::Restricted;
        let protected = RuntimeValue {
            handle: protected_handle,
            accounted_bytes: 6,
            envelope: protected_envelope,
        };
        watches.observe(0, protected, 6, &values);
        let protected_batch = watches.read("watch/test", 3, 1).unwrap();
        assert_eq!(protected_batch.records.len(), 1);
        assert_eq!(
            protected_batch.records[0].material,
            ExactWatchMaterial::Redacted
        );
        assert_eq!(protected_batch.records[0].content_hash, None);
        assert_eq!(protected_batch.records[0].original_bytes, 6);

        watches.detach("watch/test").unwrap();
        watches.observe(0, public, 8, &values);
        assert_eq!(watches.usage().retained_observations, 2);
        assert_eq!(watches.usage().dropped_observations, 1);
    }

    #[test]
    fn watch_preview_owns_bounded_bytes_without_retaining_the_runtime_value() {
        let mut watches = HostedWatchRuntime {
            slots: vec![slot()],
        };
        let mut values = HostValueStore::with_limits(5, 1).unwrap();
        let source_handle = values.store(b"hello").unwrap();
        watches.observe(
            0,
            RuntimeValue {
                handle: source_handle,
                accounted_bytes: 5,
                envelope: RuntimeValueEnvelope::EMPTY,
            },
            0,
            &values,
        );

        values.begin_reconciliation();
        values.finish_reconciliation();
        assert_eq!(values.get(source_handle), None);
        assert_eq!(values.usage().resident_bytes, 0);

        let retained = watches.read("watch/test", 0, 1).unwrap();
        assert_eq!(retained.records.len(), 1);
        assert_eq!(
            retained.records[0].material,
            ExactWatchMaterial::Preview(b"hel".to_vec())
        );
        assert_eq!(watches.usage().retained_preview_bytes, 3);
    }
}
