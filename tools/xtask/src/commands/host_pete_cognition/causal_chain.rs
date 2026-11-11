//! Inspect actual final-response ancestry; topology alone is not memory evidence.
use conduit_body::BodyPlan;
use conduit_core::PortDirection;
use conduit_kernel::{
    causal_evidence::{
        CausalTraceCompleteness, EvidenceIdentity, EvidenceMetadataFact, EvidenceMetadataLookup,
        EvidenceMetadataVisit,
    },
    KernelEventKind,
};
use conduit_observatory::{
    CausalExplanationMetadata, CausalExplanationMetadataFact as Fact, CausalExplanationVisibility,
};
use conduit_std_host::{body_causal_evidence::BodyRunCausalRecord, body_execution::BodyRunReport};
use serde::Serialize;
use serde_json::{json, Value};
use std::{
    collections::{BTreeMap, BTreeSet},
    error::Error,
    fs,
    path::Path,
};
const MAX_BYTES: usize = 4 * 1024 * 1024;
const RETAIN: &str = "experience/autobiography-retain-recoverable";
const RECALL: &str = "experience/autobiography-recall";
const STATE: &str = "state/value";
const SITUATION: &str = "experience/situation-current";
#[derive(Clone, Serialize)]
pub(super) struct SerializeSummary {
    pub schema: &'static str,
    pub root_sequence: Option<u32>,
    pub retained_explanation_complete: bool,
    pub one_slot_queues: bool,
    pub exact_execution_identity: bool,
    pub all_topology_stages_present: bool,
    pub two_episode_temporal_feedback_chain: bool,
    /// This export has no native observation/result payload custody. Receipt
    /// content comparisons and renderer acceptance remain separate obligations.
    pub full_cognition_verified: bool,
    pub nodes: usize,
    pub edges: usize,
    pub missing_branches: Vec<String>,
}
#[derive(Clone)]
struct Event {
    sequence: u32,
    kind: String,
    placement: String,
    gear: String,
    port: Option<String>,
    event: KernelEventKind,
}
pub(super) fn export(
    output: &Path,
    plan: &BodyPlan,
    report: &BodyRunReport,
) -> Result<SerializeSummary, Box<dyn Error>> {
    let (summary, document) = match build(plan, report) {
        Ok(value) => value,
        Err(error) => {
            let summary = SerializeSummary {
                schema: "conduit.pete/final-response-causal-summary@1",
                root_sequence: None,
                retained_explanation_complete: false,
                one_slot_queues: false,
                exact_execution_identity: false,
                all_topology_stages_present: false,
                two_episode_temporal_feedback_chain: false,
                full_cognition_verified: false,
                nodes: 0,
                edges: 0,
                missing_branches: vec![error],
            };
            let document = json!({"summary":summary,"refusal":"actual retained DAG could not establish final-response ancestry"});
            (summary, document)
        }
    };
    let bytes = serde_json::to_vec_pretty(&document)?;
    if bytes.len() > MAX_BYTES {
        return Err("final-response causal export exceeds its admitted 4 MiB extent".into());
    }
    fs::write(output.join("causal-final-response.json"), bytes)?;
    Ok(summary)
}
fn build(plan: &BodyPlan, report: &BodyRunReport) -> Result<(SerializeSummary, Value), String> {
    let record = BodyRunCausalRecord::from_run(plan, report)
        .map_err(|error| format!("Body causal admission: {error:?}"))?;
    let events = events(plan, report)?;
    let root = events
        .iter()
        .rev()
        .find(|event| {
            event.kind == RETAIN
                && event.event == KernelEventKind::ValueRouted
                && event.port.as_deref() == Some("response")
        })
        .ok_or("no actual retained response routing event")?;
    let mut identities = BTreeMap::<u32, EvidenceIdentity>::new();
    for identity in record.retained_evidence() {
        let mut sequence = None;
        let status = record.visit(identity, &mut |fact| {
            if let EvidenceMetadataFact::LocalOrder { sequence: value } = fact {
                sequence = Some(value);
            }
            true
        });
        if status != EvidenceMetadataVisit::Visited {
            return Err("causal metadata is unavailable".into());
        }
        let sequence = sequence.ok_or("causal node has no exact kernel sequence")?;
        if identities.insert(sequence, identity).is_some() {
            return Err("duplicate causal kernel sequence".into());
        }
    }
    let identity = *identities
        .get(&root.sequence)
        .ok_or("final routing event is outside retained causal evidence")?;
    let explanation = conduit_observatory::explain_trace_with_metadata(
        record.graph(),
        &record,
        identity,
        CausalExplanationVisibility::Operator,
    )
    .map_err(|error| format!("final response explanation: {error:?}"))?;
    let mut trace_sequences = BTreeSet::new();
    let mut exact = report.play.validate_for(plan)
        && report.wake_at_start.body_id == plan.body_id
        && report.wake_at_start.wake_id == plan.wake_id;
    let mut nodes = Vec::with_capacity(explanation.nodes.len());
    for node in &explanation.nodes {
        let CausalExplanationMetadata::Visible(facts) = &node.metadata else {
            exact = false;
            nodes.push(
                json!({"ordinal":node.ordinal,"metadata_visibility":format!("{:?}",node.metadata)}),
            );
            continue;
        };
        let sequence = facts.iter().find_map(|fact| {
            if let Fact::LocalOrder { sequence } = fact {
                Some(*sequence)
            } else {
                None
            }
        });
        let event =
            sequence.and_then(|sequence| events.iter().find(|event| event.sequence == sequence));
        if let Some(sequence) = sequence {
            trace_sequences.insert(sequence);
        }
        exact &= event.is_some() && facts.iter().any(|fact| matches!(fact, Fact::Wake(value) if value == plan.wake_id.as_str()))
            && facts.iter().any(|fact| matches!(fact, Fact::Plan(value) if value == plan.plan_id.as_str()))
            && facts.iter().any(|fact| matches!(fact, Fact::Play(value) if value == report.play.active_play_id.as_str()))
            && facts.iter().any(|fact| matches!(fact, Fact::Host(value) if value == report.terminal_sign.host_id.as_str()))
            && facts.iter().any(|fact| matches!(fact, Fact::Boot(value) if value == report.terminal_sign.boot_id.as_str()));
        if let Some(event) = event {
            exact &= facts.iter().any(|fact| matches!(fact, Fact::Placement(value) if value == &event.placement))
                && facts.iter().any(|fact| matches!(fact, Fact::SemanticSubject {gear,kind} if gear == &event.gear && kind == &event.kind));
        }
        nodes.push(json!({"ordinal":node.ordinal,"evidence":node.evidence.map(evidence_id),
            "terminal":node.terminal,"outcome":node.outcome.map(|value|format!("{value:?}")),
            "kernel_event":event.map(|event|json!({"sequence":event.sequence,"kind":format!("{:?}",event.event),"port":event.port})),
            "metadata":facts.iter().map(metadata).collect::<Vec<_>>()}));
    }
    let retained = events
        .iter()
        .filter(|event| trace_sequences.contains(&event.sequence))
        .collect::<Vec<_>>();
    let stages = [
        SITUATION,
        RECALL,
        "experience/integrate-current",
        "experience/project-evidence-flow",
        "experience/context-to-prompt",
        "llm/generate-recoverable-flow",
        RETAIN,
        STATE,
    ];
    let mut missing = stages
        .iter()
        .filter(|kind| !retained.iter().any(|event| event.kind == **kind))
        .map(|kind| format!("missing actual ancestor stage {kind}"))
        .collect::<Vec<_>>();
    let all_stages = missing.is_empty();
    let one_slot = plan
        .plots
        .iter()
        .flat_map(|plot| &plot.plan.fragments)
        .flat_map(|fragment| &fragment.connections)
        .all(|cord| cord.item_capacity == 1)
        && plan
            .plots
            .iter()
            .flat_map(|plot| &plot.plan.fragments)
            .flat_map(|fragment| &fragment.fore_ports)
            .all(|port| port.item_capacity == 1);
    if !one_slot {
        missing.push(
            "queues exceed one slot; latest-route lookup cannot establish FIFO ancestry".into(),
        );
    }
    let complete = explanation.completeness == CausalTraceCompleteness::Complete;
    if !complete {
        missing.push("retained causal explanation reports incomplete history".into());
    }
    if !exact {
        missing.push("some ancestor lacks exact runtime identity/metadata correlation".into());
    }
    let feedback = feedback(&record, &identities, &retained, root.sequence, &mut missing);
    let summary = SerializeSummary {
        schema: "conduit.pete/final-response-causal-summary@1",
        root_sequence: Some(root.sequence),
        retained_explanation_complete: complete,
        one_slot_queues: one_slot,
        exact_execution_identity: exact,
        all_topology_stages_present: all_stages,
        two_episode_temporal_feedback_chain: feedback && one_slot && exact && complete,
        full_cognition_verified: false,
        nodes: nodes.len(),
        edges: explanation.edges.len(),
        missing_branches: missing,
    };
    let document = json!({"schema":"conduit.pete/final-response-causal-dag@1","summary":summary,
        "scope":"actual runtime ancestry; native content/provenance comparisons and renderer acceptance are separate",
        "body":plan.body_id.as_str(),"wake":plan.wake_id.as_str(),"plan":plan.plan_id.as_str(),
        "play":report.play.active_play_id.as_str(),"host":report.terminal_sign.host_id.as_str(),"boot":report.terminal_sign.boot_id.as_str(),
        "root_evidence":evidence_id(identity),"nodes":nodes,
        "edges":explanation.edges.iter().map(|edge|json!({"effect":edge.effect,"cause":edge.cause,"relationship":format!("{:?}",edge.relationship)})).collect::<Vec<_>>()});
    Ok((summary, document))
}
fn events(plan: &BodyPlan, report: &BodyRunReport) -> Result<Vec<Event>, String> {
    report
        .kernel_events
        .iter()
        .filter(|event| {
            matches!(
                event.kind,
                KernelEventKind::ValueRouted | KernelEventKind::ValueConsumed
            )
        })
        .map(|event| {
            let (map, placement) = report
                .partitions
                .iter()
                .find_map(|map| {
                    map.placement_for_node(event.node)
                        .map(|placement| (map, placement))
                })
                .ok_or("kernel event has no exact partition placement")?;
            let fragment = plan
                .plots
                .iter()
                .flat_map(|plot| &plot.plan.fragments)
                .find(|fragment| {
                    fragment.plan_id == map.plan_id && fragment.fragment_id == map.fragment_id
                })
                .ok_or("partition does not belong to sealed Body Plan")?;
            let gear = fragment
                .placements
                .iter()
                .find(|gear| &gear.placement_id == placement)
                .ok_or("event placement missing from exact fragment")?;
            let direction = if event.kind == KernelEventKind::ValueRouted {
                PortDirection::Output
            } else {
                PortDirection::Input
            };
            let port = event
                .port
                .map(|port| {
                    map.port_identity(event.node, direction, port)
                        .map(|identity| identity.port_id.as_str().to_owned())
                        .ok_or("event port differs from exact lowered direction")
                })
                .transpose()?;
            Ok(Event {
                sequence: event.sequence,
                kind: gear.kind_id.as_str().into(),
                placement: placement.as_str().into(),
                gear: gear.gear_id.as_str().into(),
                port,
                event: event.kind,
            })
        })
        .collect()
}
fn feedback(
    record: &BodyRunCausalRecord,
    identities: &BTreeMap<u32, EvidenceIdentity>,
    events: &[&Event],
    root: u32,
    missing: &mut Vec<String>,
) -> bool {
    let selected = |kind: &str, port: &str, event| {
        events
            .iter()
            .copied()
            .filter(move |value| {
                value.kind == kind && value.port.as_deref() == Some(port) && value.event == event
            })
            .collect::<Vec<_>>()
    };
    let frames = selected(SITUATION, "frame", KernelEventKind::ValueConsumed);
    let next = selected(RETAIN, "next", KernelEventKind::ValueRouted);
    let state_in = selected(STATE, "next", KernelEventKind::ValueConsumed);
    let current = selected(STATE, "current", KernelEventKind::ValueRouted);
    let memory = selected(RECALL, "memory", KernelEventKind::ValueConsumed);
    let model = selected(
        "llm/generate-recoverable-flow",
        "result",
        KernelEventKind::ValueRouted,
    );
    if frames.len() < 2 || next.len() < 2 || memory.len() < 2 || model.len() < 2 {
        missing.push(format!("two-episode ancestry missing: frames={} retained-next={} recall-memory={} model-results={}",frames.len(),next.len(),memory.len(),model.len()));
        return false;
    }
    let ancestor = |effect: u32, cause: u32| {
        identities
            .get(&effect)
            .zip(identities.get(&cause))
            .is_some_and(|(effect, cause)| {
                record
                    .graph()
                    .trace(*effect)
                    .is_ok_and(|trace| trace.edges().any(|edge| edge.cause == *cause))
            })
    };
    let first = next[0].sequence;
    let second = next[1].sequence;
    let chain = state_in
        .iter()
        .filter(|event| first < event.sequence && event.sequence < second)
        .any(|state_in| {
            ancestor(state_in.sequence, first)
                && current
                    .iter()
                    .filter(|event| state_in.sequence < event.sequence && event.sequence < second)
                    .any(|current| {
                        ancestor(current.sequence, state_in.sequence)
                            && memory
                                .iter()
                                .filter(|event| {
                                    current.sequence < event.sequence && event.sequence < second
                                })
                                .any(|memory| {
                                    ancestor(memory.sequence, current.sequence)
                                        && ancestor(root, memory.sequence)
                                })
                    })
        })
        && ancestor(first, model[0].sequence)
        && ancestor(root, model[1].sequence)
        && ancestor(first, frames[0].sequence)
        && ancestor(root, frames[1].sequence);
    if !chain {
        missing.push("no exact ordered first-retention -> State.next/current -> later recall.memory -> final-response ancestry".into());
    }
    chain
}
fn evidence_id(value: EvidenceIdentity) -> Value {
    json!({"sign":value.sign,"execution":value.execution,"host_session":value.host_session})
}
fn metadata(fact: &Fact) -> Value {
    match fact {
        Fact::SemanticSubject { gear, kind } => {
            json!({"fact":"semantic_subject","gear":gear,"kind":kind})
        }
        Fact::Source {
            document,
            start,
            end,
            line,
            column,
            end_line,
            end_column,
        } => {
            json!({"fact":"source","document":document,"start":start,"end":end,"line":line,"column":column,"end_line":end_line,"end_column":end_column})
        }
        Fact::Wake(value) => json!({"fact":"wake","value":value}),
        Fact::Plan(value) => json!({"fact":"plan","value":value}),
        Fact::Play(value) => json!({"fact":"play","value":value}),
        Fact::Placement(value) => json!({"fact":"placement","value":value}),
        Fact::Implementation(value) => json!({"fact":"implementation","value":value}),
        Fact::Host(value) => json!({"fact":"host","value":value}),
        Fact::Boot(value) => json!({"fact":"boot","value":value}),
        Fact::LocalOrder { sequence } => json!({"fact":"local_order","sequence":sequence}),
        Fact::Resource { pool, generation } => {
            json!({"fact":"resource","pool":pool,"generation":generation})
        }
        Fact::Authority { grant, contract } => {
            json!({"fact":"authority","grant":grant,"contract":contract})
        }
        Fact::ClockObservation {
            capture,
            local_ticks,
            local_scale,
            local_basis,
            body,
        } => json!({"fact":"clock_observation",
            "capture":format!("{capture:?}"),"local_ticks":local_ticks,"local_scale":format!("{local_scale:?}"),"local_basis":local_basis,
            "body":body.as_ref().map(|body|json!({"basis":body.basis,"generation":body.generation,"earliest_ticks":body.earliest_ticks,
                "center_ticks":body.center_ticks,"latest_ticks":body.latest_ticks,"scale":format!("{:?}",body.scale),
                "correlation_age_ticks":body.correlation_age_ticks,"correlation_age_scale":format!("{:?}",body.correlation_age_scale),
                "source":format!("{:?}",body.source)}))}),
    }
}
