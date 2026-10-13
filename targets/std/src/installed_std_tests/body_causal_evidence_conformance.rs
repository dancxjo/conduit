use super::{host, installed_std, RecordingTimer};
use crate::{
    body_causal_evidence::BodyRunCausalRecord,
    body_execution::{BodyRunReport, BodyRunRequest},
};
use conduit_body::{Body, BodyPlan, BodyPlotPlan, ResidentPlot};
use conduit_core::{
    BaseImplementationId, SignId, TerminalCategory, TerminalDisposition, TerminalInfo,
};
use conduit_kernel::{
    causal_evidence::{
        CausalRelationship, EvidenceMetadataFact, EvidenceMetadataLookup, EvidenceMetadataVisit,
        EvidenceOutcome,
    },
    KernelEventKind,
};
use conduit_observatory::{
    explain_terminal_with_metadata, CausalExplanationMetadata, CausalExplanationMetadataFact,
    CausalExplanationVisibility,
};
use conduit_planner::{default_placements, plan_with_options, PlanningOptions};
use conduit_plot::parse;
use std::collections::BTreeMap;

const UNRECOVERED: &str = "plot terminal_body {\n cancel: conduit-test/cancellation-source\n tone: audio/tone\n cancel.request >> tone~\n}.\n";
const RECOVERED: &str = "plot recovered_body {\n cancel: conduit-test/cancellation-source\n tone: audio/tone\n recovery: conduit-test/tone-terminal-recovery\n cancel.request >> tone~\n tone.audio! >> recovery.terminal\n}.\n";

#[test]
fn real_body_failure_correlates_typed_terminal_with_exact_execution_evidence() {
    let (body_plan, report) = run_body(UNRECOVERED);
    assert!(matches!(
        report.terminal,
        TerminalDisposition::Failed { .. }
    ));
    assert!(report
        .kernel_events
        .iter()
        .any(|event| event.kind == KernelEventKind::SemanticAbnormal));
    let failed_wake = report
        .wake_at_start
        .fail(report.terminal_sign.sign_id.clone())
        .unwrap();
    let mut evidence = BodyRunCausalRecord::from_run(&body_plan, &report).unwrap();
    let root = evidence
        .correlate_unresolved_semantic_abnormal(
            &failed_wake,
            &report,
            TerminalInfo::new(TerminalCategory::Cancelled, [71; 32]),
        )
        .unwrap();
    assert_eq!(evidence.terminals().terminal_for([71; 32]), Ok(root));
    let facts = facts(&evidence, root);
    assert!(facts.contains(&EvidenceMetadataFact::Outcome(
        EvidenceOutcome::SemanticTerminal
    )));
    assert_exact_execution_facts(&facts, &body_plan, &report);

    let operator = explain_terminal_with_metadata(
        evidence.terminals(),
        evidence.graph(),
        &evidence,
        [71; 32],
        CausalExplanationVisibility::Operator,
    )
    .unwrap();
    let terminal = operator
        .nodes
        .iter()
        .find(|node| node.terminal)
        .expect("the correlated semantic terminal remains the explanation root");
    assert_eq!(terminal.evidence, Some(root));
    assert_eq!(terminal.outcome, Some(EvidenceOutcome::SemanticTerminal));
    let consumed = operator
        .nodes
        .iter()
        .find(|node| node.outcome == Some(EvidenceOutcome::InfoConsumed))
        .expect("the semantic terminal retains the exact consumed control input");
    assert!(operator.edges.iter().any(|edge| {
        edge.effect == terminal.ordinal
            && edge.cause == consumed.ordinal
            && edge.relationship == CausalRelationship::TerminatedBecause
    }));
    let routed = operator
        .nodes
        .iter()
        .find(|node| node.outcome == Some(EvidenceOutcome::InfoRouted))
        .expect("the consumed control input retains its exact planned Cord source");
    assert!(operator.edges.iter().any(|edge| {
        edge.effect == consumed.ordinal
            && edge.cause == routed.ordinal
            && edge.relationship == CausalRelationship::DerivedFrom
    }));
    let CausalExplanationMetadata::Visible(facts) = &terminal.metadata else {
        panic!("an authorized operator receives exact retained execution facts");
    };
    assert!(facts.iter().any(|fact| matches!(
        fact,
        CausalExplanationMetadataFact::Source { document, .. }
            if body_plan.plots.iter().any(|plot| plot.plan.source_document_id.as_str() == document)
    )));
    assert!(facts.iter().any(|fact| matches!(
        fact,
        CausalExplanationMetadataFact::Plan(value) if value == body_plan.plan_id.as_str()
    )));
    assert!(facts.iter().any(|fact| matches!(
        fact,
        CausalExplanationMetadataFact::Play(value)
            if value == report.play.active_play_id.as_str()
    )));
    assert!(facts
        .iter()
        .any(|fact| matches!(fact, CausalExplanationMetadataFact::Implementation(_))));
    assert!(facts.iter().any(|fact| matches!(
        fact,
        CausalExplanationMetadataFact::Host(value)
            if value == report.terminal_sign.host_id.as_str()
    )));
    assert!(facts.iter().any(|fact| matches!(
        fact,
        CausalExplanationMetadataFact::Boot(value)
            if value == report.terminal_sign.boot_id.as_str()
    )));

    let public = explain_terminal_with_metadata(
        evidence.terminals(),
        evidence.graph(),
        &evidence,
        [71; 32],
        CausalExplanationVisibility::Public,
    )
    .unwrap();
    assert_eq!(public.edges, operator.edges);
    assert_eq!(public.completeness, operator.completeness);
    assert!(public.nodes.iter().all(|node| node.evidence.is_none()));
    assert_eq!(public.nodes[0].outcome, operator.nodes[0].outcome);
    assert_eq!(
        public.nodes[0].metadata,
        CausalExplanationMetadata::Redacted
    );
}

#[test]
fn real_body_recovery_retains_exact_execution_evidence_without_a_semantic_terminal() {
    let (body_plan, report) = run_body(RECOVERED);
    assert_eq!(report.terminal, TerminalDisposition::Completed);
    assert!(report
        .kernel_events
        .iter()
        .any(|event| event.kind == KernelEventKind::SemanticAbnormalRecovered));
    let evidence = BodyRunCausalRecord::from_run(&body_plan, &report).unwrap();
    let mut recovered = None;
    for identity in evidence.retained_evidence() {
        let facts = facts(&evidence, identity);
        assert!(!facts.contains(&EvidenceMetadataFact::Outcome(
            EvidenceOutcome::SemanticTerminal
        )));
        assert_exact_execution_facts(&facts, &body_plan, &report);
        if facts.contains(&EvidenceMetadataFact::Outcome(EvidenceOutcome::Recovered)) {
            recovered = Some(identity);
        }
    }
    let trace = evidence.graph().trace(recovered.unwrap()).unwrap();
    assert!(trace
        .edges()
        .any(|edge| edge.relationship == CausalRelationship::Corrects));
}

fn run_body(source: &str) -> (BodyPlan, BodyRunReport) {
    run_body_with_timer(source, &mut RecordingTimer { waits: Vec::new() })
}

fn run_body_with_timer<T: crate::TimerAdapter>(
    source: &str,
    timer: &mut T,
) -> (BodyPlan, BodyRunReport) {
    let mut std_host = host("body-causal-terminal-host");
    let checked = parse(source, &installed_std::test_catalog()).unwrap();
    let hosts = [std_host.advertisement().clone()];
    let placements = default_placements(&checked, &hosts).unwrap();
    let plan = plan_with_options(
        &checked,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: conduit_semantic_catalog::AUDIO_TONE_PCM_BLOCK_BYTES,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .unwrap();
    let resident = ResidentPlot::new(
        plan.source_document_id.clone(),
        plan.checked_plot_id.clone(),
    );
    let wake = Body::born(
        resident.source_document_id.clone(),
        resident.checked_plot_id.clone(),
        1,
        SignId::from("sign/body-causal-born"),
    )
    .unwrap()
    .wake(1, SignId::from("sign/body-causal-wake"))
    .unwrap()
    .1;
    let body_plan = BodyPlan::seal(
        &wake,
        vec![BodyPlotPlan {
            plot: resident,
            plan,
        }],
    )
    .unwrap();
    let report = std_host
        .run_body_plan_to(
            BodyRunRequest {
                wake: &wake,
                plan: &body_plan,
                control: &crate::RunControl::default(),
                keyboard: None,
            },
            &mut Vec::with_capacity(2_048),
            timer,
        )
        .unwrap();
    (body_plan, report)
}

#[test]
fn production_timer_observes_retained_events_on_exact_host_boot_and_basis() {
    let (plan, report) = run_body_with_timer(UNRECOVERED, &mut crate::ThreadTimer);
    assert!(!report.clock_observations.is_empty());
    for observed in &report.clock_observations {
        assert!(report
            .kernel_events
            .iter()
            .any(|event| event.sequence == observed.sequence));
        let local = observed.time.local();
        assert_eq!(local.clock().host_id(), &report.terminal_sign.host_id);
        assert_eq!(local.clock().boot_id(), &report.terminal_sign.boot_id);
        assert_eq!(local.clock().basis_id(), "std/thread-timer/process-epoch");
        assert_eq!(observed.time.body(), None);
    }
    let evidence = BodyRunCausalRecord::from_run(&plan, &report).unwrap();
    assert!(evidence.graph().edges().all(|edge| {
        evidence.time_of(edge.effect).is_some() && evidence.time_of(edge.cause).is_some()
    }));

    let (_, unsupported) = run_body(UNRECOVERED);
    assert!(unsupported.clock_observations.is_empty());
}

fn facts(
    evidence: &BodyRunCausalRecord,
    identity: conduit_kernel::causal_evidence::EvidenceIdentity,
) -> Vec<EvidenceMetadataFact<'_>> {
    let mut facts = Vec::new();
    assert_eq!(
        evidence.visit(identity, &mut |fact| {
            facts.push(fact);
            true
        }),
        EvidenceMetadataVisit::Visited
    );
    facts
}

fn assert_exact_execution_facts(
    facts: &[EvidenceMetadataFact<'_>],
    plan: &BodyPlan,
    report: &BodyRunReport,
) {
    assert!(facts.contains(&EvidenceMetadataFact::Plan(plan.plan_id.as_str())));
    assert!(facts.contains(&EvidenceMetadataFact::Play(
        report.play.active_play_id.as_str()
    )));
    assert!(facts.contains(&EvidenceMetadataFact::Host(
        report.terminal_sign.host_id.as_str()
    )));
    assert!(facts.contains(&EvidenceMetadataFact::Boot(
        report.terminal_sign.boot_id.as_str()
    )));
    assert!(facts.iter().any(|fact| matches!(
        fact,
        EvidenceMetadataFact::Placement(_) | EvidenceMetadataFact::Implementation(_)
    )));
}
