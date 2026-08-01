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

use crate::{ExactRunIdentity, HostValueStore, RuntimeError, RuntimeValue};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExactWatchOperation {
    Attach,
    Read,
    Detach,
}

/// Fresh host authority observation for one exact Watch control operation.
/// The plan owns immutable identities; active/revoked/expired status is
/// re-observed at each attach, read, and detach boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactWatchUseAuthority {
    pub operation: ExactWatchOperation,
    pub operator_id: String,
    pub control_grant_hash: SemanticHash,
    pub control_grant_active: bool,
    pub run_id: String,
    pub plan_epoch: u64,
    pub watch_id: String,
    pub lease_id: String,
    pub lease_epoch: u64,
    pub lease_available: bool,
    pub reveal_grant_hash: Option<SemanticHash>,
    pub reveal_grant_active: bool,
    pub time_basis: String,
    pub validated_at_tick: u64,
    pub valid_until_tick: u64,
}

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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactWatchTimestamp {
    pub clock_domain: String,
    pub tick: i64,
    pub uncertainty_ticks: u64,
}

/// One caller-owned value observation copied from fixed session storage.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactWatchObservation {
    pub cursor: u64,
    pub source_sequence: u64,
    pub tick: u64,
    pub watch_id: String,
    pub subject: ExactWatchSubject,
    /// Host that committed the observed publication. For a cross-host cord
    /// this is the writer/producing host, never the Patchbay reader.
    pub producing_host: String,
    pub host_observation: String,
    pub time_basis: String,
    /// Scheduler ticks are observed in the producing host's pinned local time
    /// basis, so no cross-clock conversion uncertainty has been introduced.
    pub clock_uncertainty_ticks: u64,
    pub value_timestamps: Vec<ExactWatchTimestamp>,
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
    timestamp_count: u8,
    timestamps: [crate::RuntimeTimestamp; conduit_core::MAX_VALUE_CLOCK_DOMAINS],
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
    operator: String,
    control_grant_hash: SemanticHash,
    lease: String,
    reveal_grant_hash: Option<SemanticHash>,
    cord: usize,
    producing_host: String,
    host_observation: String,
    time_basis: String,
    clock_domains: Vec<String>,
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
            let producing_node = plan
                .nodes
                .iter()
                .find(|node| node.instance == plan.cords[cord].from.node)
                .ok_or_else(|| {
                    RuntimeError::new(
                        "CND-WAT-002",
                        "Watch producing node is absent from the exact plan",
                    )
                })?;
            let host_observation = plan
                .host_observations
                .iter()
                .find(|observation| observation.id == producing_node.host_observation)
                .ok_or_else(|| {
                    RuntimeError::new(
                        "CND-WAT-002",
                        "Watch producing host observation is absent from the exact plan",
                    )
                })?;
            let maximum_history = usize::from(admission.maximum_history);
            let clock_domains = plan
                .value_envelopes
                .iter()
                .find(|policy| policy.cord == plan.cords[cord].id)
                .map_or_else(Vec::new, |policy| {
                    policy
                        .clock_domains
                        .iter()
                        .map(|domain| domain.as_str().to_owned())
                        .collect()
                });
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
                operator: admission.operator.as_str().to_owned(),
                control_grant_hash: admission.control_grant_hash,
                lease: admission.lease.as_str().to_owned(),
                reveal_grant_hash: admission.reveal_grant_hash,
                cord,
                producing_host: producing_node.host.as_str().to_owned(),
                host_observation: host_observation.id.as_str().to_owned(),
                time_basis: host_observation.time_basis.as_str().to_owned(),
                clock_domains,
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

    pub(crate) fn attach(
        &mut self,
        run: &ExactRunIdentity,
        watch_id: &str,
        authority: &ExactWatchUseAuthority,
    ) -> Result<(), RuntimeError> {
        let slot = self.slot_mut(watch_id)?;
        validate_watch_authority(run, slot, ExactWatchOperation::Attach, authority)?;
        slot.attached = true;
        Ok(())
    }

    pub(crate) fn detach(
        &mut self,
        run: &ExactRunIdentity,
        watch_id: &str,
        authority: &ExactWatchUseAuthority,
    ) -> Result<(), RuntimeError> {
        let slot = self.slot_mut(watch_id)?;
        validate_watch_authority(run, slot, ExactWatchOperation::Detach, authority)?;
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
                timestamp_count: value.envelope.timestamp_count,
                timestamps: value.envelope.timestamps,
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
        run: &ExactRunIdentity,
        watch_id: &str,
        cursor: u64,
        maximum_records: u32,
        authority: &ExactWatchUseAuthority,
    ) -> Result<ExactWatchBatch, RuntimeError> {
        if maximum_records == 0 {
            return Err(RuntimeError::new(
                "CND-WAT-003",
                "Watch read bound must be nonzero",
            ));
        }
        let slot = self.slot(watch_id)?;
        validate_watch_authority(run, slot, ExactWatchOperation::Read, authority)?;
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
                producing_host: slot.producing_host.clone(),
                host_observation: slot.host_observation.clone(),
                time_basis: slot.time_basis.clone(),
                clock_uncertainty_ticks: 0,
                value_timestamps: stored.timestamps[..usize::from(stored.timestamp_count)]
                    .iter()
                    .map(|timestamp| ExactWatchTimestamp {
                        clock_domain: slot.clock_domains[usize::from(timestamp.domain_index)]
                            .clone(),
                        tick: timestamp.tick,
                        uncertainty_ticks: timestamp.uncertainty_ticks,
                    })
                    .collect(),
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

fn validate_watch_authority(
    run: &ExactRunIdentity,
    slot: &HostedWatchSlot,
    operation: ExactWatchOperation,
    authority: &ExactWatchUseAuthority,
) -> Result<(), RuntimeError> {
    let reveal_matches = match slot.reveal_grant_hash {
        Some(expected) => {
            authority.reveal_grant_hash == Some(expected) && authority.reveal_grant_active
        }
        None => authority.reveal_grant_hash.is_none(),
    };
    if authority.operation != operation
        || authority.operator_id != slot.operator
        || authority.control_grant_hash != slot.control_grant_hash
        || !authority.control_grant_active
        || authority.run_id != run.run_id
        || authority.plan_epoch != run.plan_epoch
        || authority.watch_id != slot.id
        || authority.lease_id != slot.lease
        || authority.lease_epoch != run.plan_epoch
        || !authority.lease_available
        || !reveal_matches
        || authority.time_basis != slot.time_basis
        || authority.validated_at_tick >= authority.valid_until_tick
    {
        return Err(RuntimeError::new(
            "CND-WAT-004",
            "Watch operator, grant, reveal, lease, or time observation is not current and exact",
        ));
    }
    Ok(())
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
        let watched_cord = plan.cords.iter().find(|cord| match admission.subject {
            WatchSubject::Cord(id) => cord.id == id,
            WatchSubject::NodePort {
                node,
                port,
                direction,
            } => {
                let endpoint = if direction == Direction::Output {
                    cord.from
                } else {
                    cord.to
                };
                endpoint.node == node && endpoint.port == port && endpoint.direction == direction
            }
        });
        let clock_domain_bytes = watched_cord
            .and_then(|cord| {
                plan.value_envelopes
                    .iter()
                    .find(|policy| policy.cord == cord.id)
            })
            .map_or(0, |policy| {
                policy
                    .clock_domains
                    .iter()
                    .map(|domain| domain.as_str().len())
                    .sum()
            });
        let origin_bytes = watched_cord
            .and_then(|cord| {
                plan.nodes
                    .iter()
                    .find(|node| node.instance == cord.from.node)
            })
            .map_or(0, |node| {
                node.host.as_str().len()
                    + node.host_observation.as_str().len()
                    + plan
                        .host_observations
                        .iter()
                        .find(|observation| observation.id == node.host_observation)
                        .map_or(0, |observation| observation.time_basis.as_str().len())
            });
        let strings = u64::try_from(
            admission.id.as_str().len()
                + admission.operator.as_str().len()
                + admission.lease.as_str().len()
                + admission.representation.id.as_str().len()
                + clock_domain_bytes
                + origin_bytes
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
    use crate::{RuntimeTimestamp, RuntimeValueEnvelope};

    fn slot() -> HostedWatchSlot {
        HostedWatchSlot {
            id: "watch/test".to_owned(),
            subject: ExactWatchSubject::Cord {
                cord: "cord/test".to_owned(),
            },
            operator: "operator/fixture".to_owned(),
            control_grant_hash: SemanticHash::from_bytes([8; 32]),
            lease: "lease/watch-test".to_owned(),
            reveal_grant_hash: None,
            cord: 0,
            producing_host: "host/producer".to_owned(),
            host_observation: "observation/producer".to_owned(),
            time_basis: "clock/producer".to_owned(),
            clock_domains: vec!["clock/value".to_owned()],
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

    fn run() -> ExactRunIdentity {
        ExactRunIdentity {
            plan_identity: SemanticHash::from_bytes([9; 32]),
            source_semantic_hash: SemanticHash::from_bytes([10; 32]),
            plan_epoch: 3,
            run_id: "run/watch-test".to_owned(),
        }
    }

    fn authority(operation: ExactWatchOperation) -> ExactWatchUseAuthority {
        ExactWatchUseAuthority {
            operation,
            operator_id: "operator/fixture".to_owned(),
            control_grant_hash: SemanticHash::from_bytes([8; 32]),
            control_grant_active: true,
            run_id: "run/watch-test".to_owned(),
            plan_epoch: 3,
            watch_id: "watch/test".to_owned(),
            lease_id: "lease/watch-test".to_owned(),
            lease_epoch: 3,
            lease_available: true,
            reveal_grant_hash: None,
            reveal_grant_active: false,
            time_basis: "clock/producer".to_owned(),
            validated_at_tick: 1,
            valid_until_tick: 20,
        }
    }

    #[test]
    fn fixed_watch_ring_reports_rate_and_retention_gaps_without_revealing_protected_values() {
        let mut watches = HostedWatchRuntime {
            slots: vec![slot()],
        };
        let mut values = HostValueStore::with_limits(32, 4).unwrap();
        let public_handle = values.store(b"hello").unwrap();
        let mut public_envelope = RuntimeValueEnvelope::EMPTY;
        public_envelope.timestamp_count = 1;
        public_envelope.timestamps[0] = RuntimeTimestamp {
            domain_index: 0,
            tick: 123,
            uncertainty_ticks: 4,
        };
        let public = RuntimeValue {
            handle: public_handle,
            accounted_bytes: 5,
            envelope: public_envelope,
        };

        watches.observe(0, public, 0, &values);
        watches.observe(0, public, 1, &values);
        watches.observe(0, public, 2, &values);
        watches.observe(0, public, 4, &values);

        let retained = watches
            .read(
                &run(),
                "watch/test",
                0,
                8,
                &authority(ExactWatchOperation::Read),
            )
            .unwrap();
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
        let protected_batch = watches
            .read(
                &run(),
                "watch/test",
                3,
                1,
                &authority(ExactWatchOperation::Read),
            )
            .unwrap();
        assert_eq!(protected_batch.records.len(), 1);
        assert_eq!(
            protected_batch.records[0].material,
            ExactWatchMaterial::Redacted
        );
        assert_eq!(protected_batch.records[0].content_hash, None);
        assert_eq!(protected_batch.records[0].original_bytes, 6);

        watches
            .detach(
                &run(),
                "watch/test",
                &authority(ExactWatchOperation::Detach),
            )
            .unwrap();
        watches.observe(0, public, 8, &values);
        watches
            .attach(
                &run(),
                "watch/test",
                &authority(ExactWatchOperation::Attach),
            )
            .unwrap();
        watches.observe(0, public, 10, &values);
        let resumed = watches
            .read(
                &run(),
                "watch/test",
                4,
                1,
                &authority(ExactWatchOperation::Read),
            )
            .unwrap();
        assert_eq!(resumed.records[0].source_sequence, 6);
        assert_eq!(resumed.records[0].producing_host, "host/producer");
        assert_eq!(resumed.records[0].time_basis, "clock/producer");
        assert_eq!(resumed.records[0].clock_uncertainty_ticks, 0);
        assert_eq!(
            resumed.records[0].value_timestamps,
            vec![ExactWatchTimestamp {
                clock_domain: "clock/value".to_owned(),
                tick: 123,
                uncertainty_ticks: 4,
            }]
        );
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

        let retained = watches
            .read(
                &run(),
                "watch/test",
                0,
                1,
                &authority(ExactWatchOperation::Read),
            )
            .unwrap();
        assert_eq!(retained.records.len(), 1);
        assert_eq!(
            retained.records[0].material,
            ExactWatchMaterial::Preview(b"hel".to_vec())
        );
        assert_eq!(watches.usage().retained_preview_bytes, 3);
    }

    #[test]
    fn watch_control_rechecks_exact_operator_grant_lease_and_time_for_every_operation() {
        let mut watches = HostedWatchRuntime {
            slots: vec![slot()],
        };
        let exact_run = run();
        let mut invalid = Vec::new();

        let mut wrong_operation = authority(ExactWatchOperation::Attach);
        wrong_operation.operation = ExactWatchOperation::Detach;
        invalid.push(wrong_operation);
        let mut wrong_operator = authority(ExactWatchOperation::Attach);
        wrong_operator.operator_id = "operator/wrong".to_owned();
        invalid.push(wrong_operator);
        let mut revoked = authority(ExactWatchOperation::Attach);
        revoked.control_grant_active = false;
        invalid.push(revoked);
        let mut wrong_grant = authority(ExactWatchOperation::Attach);
        wrong_grant.control_grant_hash = SemanticHash::from_bytes([99; 32]);
        invalid.push(wrong_grant);
        let mut wrong_run = authority(ExactWatchOperation::Attach);
        wrong_run.run_id = "run/wrong".to_owned();
        invalid.push(wrong_run);
        let mut wrong_epoch = authority(ExactWatchOperation::Attach);
        wrong_epoch.plan_epoch += 1;
        invalid.push(wrong_epoch);
        let mut wrong_watch = authority(ExactWatchOperation::Attach);
        wrong_watch.watch_id = "watch/wrong".to_owned();
        invalid.push(wrong_watch);
        let mut missing_lease = authority(ExactWatchOperation::Attach);
        missing_lease.lease_available = false;
        invalid.push(missing_lease);
        let mut wrong_lease = authority(ExactWatchOperation::Attach);
        wrong_lease.lease_id = "lease/wrong".to_owned();
        invalid.push(wrong_lease);
        let mut unexpected_reveal = authority(ExactWatchOperation::Attach);
        unexpected_reveal.reveal_grant_hash = Some(SemanticHash::from_bytes([98; 32]));
        unexpected_reveal.reveal_grant_active = true;
        invalid.push(unexpected_reveal);
        let mut stale = authority(ExactWatchOperation::Attach);
        stale.validated_at_tick = stale.valid_until_tick;
        invalid.push(stale);

        for observation in invalid {
            assert_eq!(
                watches
                    .attach(&exact_run, "watch/test", &observation)
                    .unwrap_err()
                    .code,
                "CND-WAT-004"
            );
        }

        watches
            .attach(
                &exact_run,
                "watch/test",
                &authority(ExactWatchOperation::Attach),
            )
            .unwrap();
        watches
            .read(
                &exact_run,
                "watch/test",
                0,
                1,
                &authority(ExactWatchOperation::Read),
            )
            .unwrap();
        watches
            .detach(
                &exact_run,
                "watch/test",
                &authority(ExactWatchOperation::Detach),
            )
            .unwrap();
    }
}
