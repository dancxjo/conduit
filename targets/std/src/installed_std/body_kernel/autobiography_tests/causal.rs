//! Actual retained runtime evidence includes both input and feedback fan-in.
use conduit_kernel::causal_evidence::{
    CausalTraceCompleteness, EvidenceMetadataFact, EvidenceMetadataLookup,
};
use conduit_observatory::{
    CausalExplanationMetadata, CausalExplanationMetadataFact as Fact, CausalExplanationVisibility,
};
pub(super) fn assert_two_episode_ancestry(
    plan: &conduit_body::BodyPlan,
    report: &crate::body_execution::BodyRunReport,
) {
    let record = crate::body_causal_evidence::BodyRunCausalRecord::from_run(plan, report).unwrap();
    let root = report
        .kernel_events
        .iter()
        .rev()
        .find(|event| {
            event.kind == conduit_kernel::KernelEventKind::ValueRouted
                && report.partitions.iter().any(|map| {
                    map.port_identity(
                        event.node,
                        conduit_core::PortDirection::Output,
                        event.port.unwrap(),
                    )
                    .is_some_and(|port| port.port_id.as_str() == "response")
                })
        })
        .unwrap();
    let identity = record
        .retained_evidence()
        .find(|identity| {
            let mut matches = false;
            record.visit(*identity, &mut |fact| {
                if let EvidenceMetadataFact::LocalOrder { sequence } = fact {
                    matches = sequence == root.sequence;
                }
                true
            });
            matches
        })
        .unwrap();
    let trace = conduit_observatory::explain_trace_with_metadata(
        record.graph(),
        &record,
        identity,
        CausalExplanationVisibility::Operator,
    )
    .unwrap();
    assert_eq!(trace.completeness, CausalTraceCompleteness::Complete);
    let facts = trace
        .nodes
        .iter()
        .flat_map(|node| match &node.metadata {
            CausalExplanationMetadata::Visible(facts) => facts.as_slice(),
            _ => panic!("actual ancestor metadata must remain inspectable"),
        })
        .collect::<Vec<_>>();
    for kind in [
        "experience/situation-current",
        "experience/autobiography-recall",
        "experience/integrate-current",
        "experience/project-evidence-flow",
        "experience/context-to-prompt",
        "llm/generate-recoverable-flow",
        "experience/autobiography-retain-recoverable",
        "state/value",
    ] {
        assert!(
            facts.iter().any(
                |fact| matches!(fact, Fact::SemanticSubject {kind: actual,..} if actual == kind)
            ),
            "missing {kind}"
        );
    }
    let sequences = facts
        .iter()
        .filter_map(|fact| {
            if let Fact::LocalOrder { sequence } = fact {
                Some(*sequence)
            } else {
                None
            }
        })
        .collect::<Vec<_>>();
    let frames = report
        .kernel_events
        .iter()
        .filter(|event| {
            event.kind == conduit_kernel::KernelEventKind::ValueConsumed
                && sequences.contains(&event.sequence)
                && report.partitions.iter().any(|map| {
                    map.port_identity(
                        event.node,
                        conduit_core::PortDirection::Input,
                        event.port.unwrap(),
                    )
                    .is_some_and(|port| port.port_id.as_str() == "frame")
                })
        })
        .count();
    assert_eq!(
        frames, 2,
        "final response ancestry must retain both observed episodes"
    );
    assert!(
        trace.nodes.iter().any(|node| trace
            .edges
            .iter()
            .filter(|edge| edge.effect == node.ordinal)
            .count()
            >= 2),
        "fan-in must remain a DAG rather than a single-parent timeline"
    );
    assert!(facts.iter().all(|fact| match fact {
        Fact::Wake(value) => value == plan.wake_id.as_str(),
        Fact::Plan(value) => value == plan.plan_id.as_str(),
        Fact::Play(value) => value == report.play.active_play_id.as_str(),
        _ => true,
    }));
}
