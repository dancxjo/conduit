//! Bounded hosted projection of exact executor observations.

use conduit_core::{
    CanonicalDescriptor, CanonicalValue, FieldDisposition, FlowEventKind, Id, MapField,
    SchedulerDecisionReason, SemanticHash, StepOutcomeKind, StopPolicy, TerminalClass,
};
use serde::Serialize;

use crate::scheduler::RuntimePlan;
use crate::{SchedulerEvent, SchedulerEventKind, SchedulerSubject};

/// One stable, typed observation from an exact-plan executor run.
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct ExactEvidenceRecord {
    pub schema: &'static str,
    pub schema_version: u16,
    pub plan_identity: String,
    pub plan_epoch: u64,
    pub run_id: String,
    pub sequence: u64,
    pub tick: u64,
    pub subject_kind: &'static str,
    pub subject_id: String,
    pub node_id: Option<String>,
    pub semantic_contract_id: Option<String>,
    pub semantic_contract_descriptor_hash: Option<String>,
    pub cord_id: Option<String>,
    pub from_port: Option<String>,
    pub to_port: Option<String>,
    pub implementation_id: Option<String>,
    pub implementation_identity: Option<String>,
    pub artifact_id: Option<String>,
    pub host_id: Option<String>,
    pub host_observation_id: Option<String>,
    pub pressure: Option<&'static str>,
    pub event_kind: &'static str,
    pub event_detail: Option<&'static str>,
    pub terminal_cause: Option<&'static str>,
    pub occupancy_items: u16,
    pub occupancy_bytes: u64,
    pub scheduling_latency_ticks: u64,
    pub processing_latency_ticks: u64,
}

/// Computes the exact, domain-separated identity of one cursor-bounded
/// evidence commit batch.
///
/// `ExactEvidenceRecord` has one current pre-release serializer. Its complete
/// ordered byte form is nested inside Conduit's canonical descriptor encoding,
/// alongside the exact half-open cursor range. A retry therefore produces the
/// same digest, while reordering, mutation, truncation, or range drift changes
/// the identity checked by the runtime before scheduler reclamation.
pub fn exact_evidence_batch_digest(
    start_cursor: u64,
    end_cursor: u64,
    records: &[ExactEvidenceRecord],
) -> Result<SemanticHash, serde_json::Error> {
    let records = serde_json::to_vec(records)?;
    let fields = [
        MapField {
            name: Id("start-cursor"),
            value: CanonicalValue::Integer(i128::from(start_cursor)),
            disposition: FieldDisposition::Semantic,
        },
        MapField {
            name: Id("end-cursor"),
            value: CanonicalValue::Integer(i128::from(end_cursor)),
            disposition: FieldDisposition::Semantic,
        },
        MapField {
            name: Id("records"),
            value: CanonicalValue::Bytes(&records),
            disposition: FieldDisposition::Semantic,
        },
    ];
    Ok(CanonicalDescriptor {
        kind: Id("conduit/exact-evidence-batch"),
        schema_version: 0,
        body: CanonicalValue::Map(&fields),
    }
    .semantic_hash()
    .expect("the exact-evidence batch descriptor uses static valid identifiers"))
}

pub(crate) fn project_runtime_exact_evidence(
    plan: &RuntimePlan,
    plan_identity: &str,
    plan_epoch: u64,
    run_id: &str,
    observations: &[SchedulerEvent],
) -> Vec<ExactEvidenceRecord> {
    observations
        .iter()
        .filter(|observation| retained(observation.kind))
        .map(|observation| {
            let mut record = ExactEvidenceRecord {
                schema: "conduit.exact-execution-evidence",
                schema_version: 0,
                plan_identity: plan_identity.to_owned(),
                plan_epoch,
                run_id: run_id.to_owned(),
                sequence: observation.sequence,
                tick: observation.tick,
                subject_kind: "run",
                subject_id: run_id.to_owned(),
                node_id: None,
                semantic_contract_id: None,
                semantic_contract_descriptor_hash: None,
                cord_id: None,
                from_port: None,
                to_port: None,
                implementation_id: None,
                implementation_identity: None,
                artifact_id: None,
                host_id: None,
                host_observation_id: None,
                pressure: None,
                event_kind: event_kind(observation.kind),
                event_detail: event_detail(observation.kind),
                terminal_cause: terminal_cause(observation.kind),
                occupancy_items: observation.occupancy_items,
                occupancy_bytes: observation.occupancy_bytes,
                scheduling_latency_ticks: observation.scheduling_latency_ticks,
                processing_latency_ticks: observation.processing_latency_ticks,
            };
            match observation.subject {
                SchedulerSubject::Run => {}
                SchedulerSubject::Node(index) => {
                    record.subject_kind = "node";
                    if let Some(node) = plan.nodes.get(usize::from(index)) {
                        record.subject_id = node.instance.clone();
                        record.node_id = Some(node.instance.clone());
                        record.semantic_contract_id = Some(node.contract_id.clone());
                        record.semantic_contract_descriptor_hash =
                            Some(node.contract_hash.to_string());
                        record.implementation_id = Some(node.implementation_id.clone());
                        record.implementation_identity = Some(node.implementation_hash.to_string());
                        record.artifact_id = Some(node.artifact_id.clone());
                        record.host_id = Some(node.host_id.clone());
                        record.host_observation_id = Some(node.host_observation_id.clone());
                    } else {
                        record.subject_id = format!("invalid-node-index/{index}");
                    }
                }
                SchedulerSubject::Cord(index) => {
                    record.subject_kind = "cord";
                    if let Some(cord) = plan.cords.get(usize::from(index)) {
                        record.subject_id = cord.id.clone();
                        record.cord_id = Some(cord.id.clone());
                        let from = &plan.nodes[cord.from_node].instance;
                        let to = &plan.nodes[cord.to_node].instance;
                        record.from_port = Some(format!("{from}.{}", cord.from_port));
                        record.to_port = Some(format!("{to}.{}", cord.to_port));
                        record.pressure = Some(cord.flow.pressure_name());
                    } else {
                        record.subject_id = format!("invalid-cord-index/{index}");
                    }
                }
            }
            record
        })
        .collect()
}

const fn retained(kind: SchedulerEventKind) -> bool {
    matches!(
        kind,
        SchedulerEventKind::AllocationPrepared
            | SchedulerEventKind::NodePrepared
            | SchedulerEventKind::RunStarted
            | SchedulerEventKind::Cord(_)
            | SchedulerEventKind::ValueAccepted
            | SchedulerEventKind::ValueConsumed
            | SchedulerEventKind::DerivationCommitted
            | SchedulerEventKind::CancellationRequested { .. }
            | SchedulerEventKind::Terminal(_)
    )
}

const fn event_kind(value: SchedulerEventKind) -> &'static str {
    match value {
        SchedulerEventKind::AllocationPrepared => "allocation",
        SchedulerEventKind::NodePrepared => "node-prepared",
        SchedulerEventKind::RunStarted => "run-started",
        SchedulerEventKind::Decision { .. } => "decision",
        SchedulerEventKind::NodeOutcome { .. } => "node-outcome",
        SchedulerEventKind::Cord(_) => "cord-flow",
        SchedulerEventKind::ValueAccepted => "value-accepted",
        SchedulerEventKind::ValueConsumed => "value-consumed",
        SchedulerEventKind::DerivationCommitted => "derivation-committed",
        SchedulerEventKind::NodeWoken { .. } => "node-woken",
        SchedulerEventKind::CancellationRequested { .. } => "cancellation-requested",
        SchedulerEventKind::Terminal(_) => "terminal",
    }
}

const fn event_detail(value: SchedulerEventKind) -> Option<&'static str> {
    match value {
        SchedulerEventKind::Decision { reason } | SchedulerEventKind::NodeWoken { reason } => {
            Some(decision_reason(reason))
        }
        SchedulerEventKind::NodeOutcome { outcome } => Some(step_outcome(outcome)),
        SchedulerEventKind::Cord(kind) => Some(flow_event(kind)),
        SchedulerEventKind::CancellationRequested { stop } => Some(stop_policy(stop)),
        SchedulerEventKind::Terminal(class) => Some(terminal_class(class)),
        SchedulerEventKind::AllocationPrepared
        | SchedulerEventKind::NodePrepared
        | SchedulerEventKind::RunStarted
        | SchedulerEventKind::ValueAccepted
        | SchedulerEventKind::ValueConsumed
        | SchedulerEventKind::DerivationCommitted => None,
    }
}

const fn terminal_cause(value: SchedulerEventKind) -> Option<&'static str> {
    match value {
        SchedulerEventKind::Terminal(class) => Some(terminal_class(class)),
        _ => None,
    }
}

const fn decision_reason(value: SchedulerDecisionReason) -> &'static str {
    match value {
        SchedulerDecisionReason::Initial => "initial",
        SchedulerDecisionReason::Progress => "progress",
        SchedulerDecisionReason::FairYield => "fair-yield",
        SchedulerDecisionReason::InputReady => "input-ready",
        SchedulerDecisionReason::OutputReady => "output-ready",
        SchedulerDecisionReason::TimerReady => "timer-ready",
        SchedulerDecisionReason::HostOperationReady => "host-operation-ready",
        SchedulerDecisionReason::Cancellation => "cancellation",
        SchedulerDecisionReason::TerminalPropagation => "terminal-propagation",
    }
}

const fn step_outcome(value: StepOutcomeKind) -> &'static str {
    match value {
        StepOutcomeKind::Progress => "progress",
        StepOutcomeKind::Pending => "pending",
        StepOutcomeKind::Yielded => "yielded",
        StepOutcomeKind::Completed => "completed",
        StepOutcomeKind::Failed => "failed",
    }
}

const fn flow_event(value: FlowEventKind) -> &'static str {
    match value {
        FlowEventKind::PressureEntered => "pressure-entered",
        FlowEventKind::PressureCleared => "pressure-cleared",
        FlowEventKind::ValueRejected => "value-rejected",
        FlowEventKind::ValueCoalesced { .. } => "value-coalesced",
        FlowEventKind::ValueSampledOut => "value-sampled-out",
        FlowEventKind::ValueDroppedDisposable => "value-dropped-disposable",
        FlowEventKind::ConsumerReady => "consumer-ready",
        FlowEventKind::ProducerReady => "producer-ready",
        FlowEventKind::Disconnected => "disconnected",
        FlowEventKind::Failed => "failed",
        FlowEventKind::Cancelled { .. } => "cancelled",
        FlowEventKind::DrainStarted { .. } => "drain-started",
        FlowEventKind::ValuesDiscardedOnAbort { .. } => "values-discarded-on-abort",
        FlowEventKind::Completed => "completed",
    }
}

const fn stop_policy(value: StopPolicy) -> &'static str {
    match value {
        StopPolicy::Abort => "abort",
        StopPolicy::Drain => "drain",
    }
}

const fn terminal_class(value: TerminalClass) -> &'static str {
    match value {
        TerminalClass::Succeeded => "succeeded",
        TerminalClass::Cancelled => "cancelled",
        TerminalClass::Failed => "failed",
        TerminalClass::Disconnected => "disconnected",
    }
}

#[cfg(test)]
mod tests {
    use super::{ExactEvidenceRecord, exact_evidence_batch_digest};

    fn record(sequence: u64, event_kind: &'static str) -> ExactEvidenceRecord {
        ExactEvidenceRecord {
            schema: "conduit.exact-execution-evidence",
            schema_version: 0,
            plan_identity: "sha256:fixture-plan".to_owned(),
            plan_epoch: 7,
            run_id: "run/fixture".to_owned(),
            sequence,
            tick: sequence,
            subject_kind: "run",
            subject_id: "run/fixture".to_owned(),
            node_id: None,
            semantic_contract_id: None,
            semantic_contract_descriptor_hash: None,
            cord_id: None,
            from_port: None,
            to_port: None,
            implementation_id: None,
            implementation_identity: None,
            artifact_id: None,
            host_id: None,
            host_observation_id: None,
            pressure: None,
            event_kind,
            event_detail: None,
            terminal_cause: None,
            occupancy_items: 0,
            occupancy_bytes: 0,
            scheduling_latency_ticks: 0,
            processing_latency_ticks: 0,
        }
    }

    #[test]
    fn exact_batch_digest_binds_order_content_and_cursor_range() {
        let records = [record(4, "run-started"), record(5, "value-accepted")];
        let digest = exact_evidence_batch_digest(4, 6, &records).unwrap();
        assert_eq!(exact_evidence_batch_digest(4, 6, &records).unwrap(), digest);

        let reversed = [records[1].clone(), records[0].clone()];
        assert_ne!(
            exact_evidence_batch_digest(4, 6, &reversed).unwrap(),
            digest
        );
        assert_ne!(exact_evidence_batch_digest(3, 6, &records).unwrap(), digest);
        assert_ne!(exact_evidence_batch_digest(4, 7, &records).unwrap(), digest);

        let mutated = [record(4, "run-started"), record(5, "value-consumed")];
        assert_ne!(exact_evidence_batch_digest(4, 6, &mutated).unwrap(), digest);
    }
}
