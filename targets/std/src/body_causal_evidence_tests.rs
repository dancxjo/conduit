use super::*;
use conduit_body::{Body, BodyPlayIdentity};
use conduit_core::{FailureReason, SignId};

fn clock_observation(
    body_basis: &str,
    host: &str,
    boot: &str,
    local_ticks: u64,
    body_ticks: u64,
) -> BodyEventTimeObservation {
    let local = conduit_core::MonotonicInstant::new(
        local_ticks,
        conduit_core::MonotonicClockIdentity::new(
            host.into(),
            boot.into(),
            "steady".into(),
            conduit_core::TemporalScale::Milliseconds,
            1,
            1,
        )
        .unwrap(),
    )
    .unwrap();
    let correlation = conduit_core::BodyClockCorrelation::new(
        body_basis.into(),
        conduit_core::TemporalScale::Milliseconds,
        1,
        local.clone(),
        body_ticks,
        0,
        100,
        2,
        100,
        conduit_core::ClockProvenance::External {
            provider_id: "provider/test".into(),
            admission_reference: "admitted/test".into(),
            policy_id: "policy/test".into(),
        },
    )
    .unwrap();
    BodyEventTimeObservation::new(local.clone(), Some(correlation.project(&local).unwrap()))
        .unwrap()
}

#[test]
fn causal_send_receive_survives_overlapping_inverted_physical_estimates() {
    let send = EvidenceIdentity {
        sign: 1,
        execution: 1,
        host_session: 1,
    };
    let receive = EvidenceIdentity {
        sign: 2,
        execution: 1,
        host_session: 2,
    };
    let independent = EvidenceIdentity {
        sign: 3,
        execution: 1,
        host_session: 3,
    };
    let mut receive_node = node(
        receive,
        KernelEventKind::ValueConsumed,
        EvidenceOutcome::InfoConsumed,
    );
    receive_node.host_id = "host/receive".into();
    receive_node.boot_id = "boot/receive".into();
    let mut send_node = node(
        send,
        KernelEventKind::ValueRouted,
        EvidenceOutcome::InfoRouted,
    );
    send_node.host_id = "host/send".into();
    send_node.boot_id = "boot/send".into();
    let mut independent_node = node(
        independent,
        KernelEventKind::ValueRouted,
        EvidenceOutcome::InfoRouted,
    );
    independent_node.host_id = "host/independent".into();
    independent_node.boot_id = "boot/independent".into();
    let mut graph = CausalEvidence::default();
    graph
        .record(CausalEdge {
            effect: receive,
            relationship: CausalRelationship::CausedBy,
            cause: send,
        })
        .unwrap();
    let mut record = BodyRunCausalRecord {
        graph,
        terminals: TerminalEvidenceIndex::default(),
        nodes: vec![send_node, receive_node, independent_node],
    };
    let body_id = wake().body_id;
    let sent_at = clock_observation(body_id.as_str(), "host/send", "boot/send", 100, 10_006);
    let received_at = clock_observation(
        body_id.as_str(),
        "host/receive",
        "boot/receive",
        9_000,
        10_000,
    );
    assert_eq!(
        record.attach_time(
            send,
            clock_observation("body/other", "host/send", "boot/send", 100, 10_006)
        ),
        Err(BodyCausalEvidenceRefusal::InvalidClockObservation)
    );
    record.attach_time(send, sent_at.clone()).unwrap();
    record.attach_time(receive, received_at).unwrap();
    record
        .attach_time(
            independent,
            clock_observation(
                body_id.as_str(),
                "host/independent",
                "boot/independent",
                42,
                11_000,
            ),
        )
        .unwrap();
    assert_eq!(
        record.physical_relation(send, receive),
        Ok(Some(conduit_core::BodyTimeRelation::Indeterminate))
    );
    assert_eq!(
        record
            .graph()
            .cause_of(receive, CausalRelationship::CausedBy),
        Ok(send)
    );
    assert_eq!(
        record.physical_relation(send, independent),
        Ok(Some(conduit_core::BodyTimeRelation::Before))
    );
    assert_eq!(
        record
            .graph()
            .cause_of(independent, CausalRelationship::CausedBy),
        Err(CausalEvidenceRefusal::Unknown)
    );
    assert_eq!(
        record.attach_time(send, sent_at.clone()),
        Err(BodyCausalEvidenceRefusal::ConflictingClockObservation)
    );
    let encoded = serde_json::to_string(&sent_at).unwrap();
    let replayed: BodyEventTimeObservation = serde_json::from_str(&encoded).unwrap();
    replayed.validate().unwrap();
    assert_eq!(replayed, sent_at);
    assert_eq!(replayed.capture(), EventTimeCapture::AtEvent);
    assert_eq!(replayed.body().unwrap().generation, 1);
}

fn wake() -> conduit_body::Wake {
    Body::born(
        "source/test".into(),
        "checked/test".into(),
        1,
        SignId::from("sign/born"),
    )
    .unwrap()
    .wake(1, SignId::from("sign/wake"))
    .unwrap()
    .1
}

fn node(
    evidence: EvidenceIdentity,
    kind: KernelEventKind,
    outcome: EvidenceOutcome,
) -> BodyCausalNode {
    BodyCausalNode {
        evidence,
        outcome,
        source_document_id: "source/test".into(),
        source_span: Some(conduit_core::SourceSpan {
            start: 4,
            end: 12,
            line: 2,
            column: 3,
            end_line: 2,
            end_column: 11,
        }),
        body_id: wake().body_id,
        wake_id: wake().wake_id,
        plan_id: "plan/test".into(),
        play_id: "play/test".into(),
        placement_id: "placement/test".into(),
        gear_id: "gear/test".into(),
        kind_id: "kind/test".into(),
        implementation_id: "implementation/test".into(),
        host_id: "host/test".into(),
        boot_id: "boot/test".into(),
        resources: Vec::new(),
        authority: Vec::new(),
        kernel_kind: kind,
        kernel_port: None,
        kernel_sequence: evidence.sign as u32,
        semantic_terminal: false,
        observed_time: None,
    }
}

fn report(terminal: TerminalDisposition) -> BodyRunReport {
    let wake = wake();
    BodyRunReport {
        play: BodyPlayIdentity {
            active_play_id: "play/test".into(),
            body_id: wake.body_id.clone(),
            wake_id: wake.wake_id.clone(),
            plan_id: "plan/test".into(),
            play_sequence: 1,
        },
        wake_at_start: wake,
        terminal,
        failure: None,
        cleanup_failure: None,
        terminal_sign: conduit_core::SignIdentity {
            sign_id: "sign/terminal".into(),
            host_id: "host/test".into(),
            boot_id: "boot/test".into(),
            active_play_id: Some("play/test".into()),
            sequence: 9,
        },
        partitions: Vec::new(),
        requests: Vec::new(),
        kernel_events: Vec::new(),
        clock_observations: Vec::new(),
    }
}

#[test]
fn successful_recovery_metadata_never_implies_a_semantic_terminal() {
    let cause = EvidenceIdentity {
        sign: 1,
        execution: 1,
        host_session: 1,
    };
    let recovered = EvidenceIdentity {
        sign: 2,
        execution: 1,
        host_session: 1,
    };
    let mut graph = CausalEvidence::default();
    graph
        .record(CausalEdge {
            effect: recovered,
            relationship: CausalRelationship::Corrects,
            cause,
        })
        .unwrap();
    let record = BodyRunCausalRecord {
        graph,
        terminals: TerminalEvidenceIndex::default(),
        nodes: vec![
            node(
                cause,
                KernelEventKind::SemanticAbnormal,
                EvidenceOutcome::RealizationUnsatisfied,
            ),
            node(
                recovered,
                KernelEventKind::SemanticAbnormalRecovered,
                EvidenceOutcome::Recovered,
            ),
        ],
    };
    assert_eq!(
        record.terminals().terminal_for([7; 32]),
        Err(CausalEvidenceRefusal::Unknown)
    );
    let mut outcomes = Vec::new();
    assert_eq!(
        record.visit(recovered, &mut |fact| {
            if let EvidenceMetadataFact::Outcome(outcome) = fact {
                outcomes.push(outcome);
            }
            true
        }),
        EvidenceMetadataVisit::Visited
    );
    assert_eq!(outcomes, vec![EvidenceOutcome::Recovered]);
}

#[test]
fn only_failed_unrecovered_semantic_abnormal_accepts_terminal_correlation() {
    let evidence = EvidenceIdentity {
        sign: 1,
        execution: 1,
        host_session: 1,
    };
    let mut record = BodyRunCausalRecord {
        graph: CausalEvidence::default(),
        terminals: TerminalEvidenceIndex::default(),
        nodes: vec![node(
            evidence,
            KernelEventKind::SemanticAbnormal,
            EvidenceOutcome::RealizationUnsatisfied,
        )],
    };
    let completed = report(TerminalDisposition::Completed);
    let failed_wake = wake().fail(SignId::from("sign/terminal")).unwrap();
    assert_eq!(
        record.correlate_unresolved_semantic_abnormal(
            &failed_wake,
            &completed,
            TerminalInfo::new(conduit_core::TerminalCategory::ExecutionFault, [7; 32]),
        ),
        Err(BodyCausalEvidenceRefusal::NotFailed)
    );
    let failed = report(TerminalDisposition::Failed {
        reason: FailureReason::UnknownImplementation,
    });
    assert_eq!(
        record
            .correlate_unresolved_semantic_abnormal(
                &failed_wake,
                &failed,
                TerminalInfo::new(conduit_core::TerminalCategory::ExecutionFault, [7; 32]),
            )
            .unwrap(),
        evidence
    );
    assert_eq!(record.terminals().terminal_for([7; 32]), Ok(evidence));
    let mut outcome = None;
    record.visit(evidence, &mut |fact| {
        if let EvidenceMetadataFact::Outcome(value) = fact {
            outcome = Some(value);
        }
        true
    });
    assert_eq!(outcome, Some(EvidenceOutcome::SemanticTerminal));
}

#[test]
fn lookup_is_exact_and_honors_bounded_visitor_refusal() {
    let evidence = EvidenceIdentity {
        sign: 1,
        execution: 1,
        host_session: 1,
    };
    let record = BodyRunCausalRecord {
        graph: CausalEvidence::default(),
        terminals: TerminalEvidenceIndex::default(),
        nodes: vec![node(
            evidence,
            KernelEventKind::BackCompleted,
            EvidenceOutcome::PlayCompleted,
        )],
    };
    assert_eq!(
        record.visit(
            EvidenceIdentity {
                sign: 99,
                ..evidence
            },
            &mut |_| true
        ),
        EvidenceMetadataVisit::Missing
    );
    assert_eq!(
        record.visit(evidence, &mut |_| false),
        EvidenceMetadataVisit::VisitorRefused
    );
    let mut source = None;
    assert_eq!(
        record.visit(evidence, &mut |fact| {
            if let EvidenceMetadataFact::Source {
                document,
                start,
                end,
                line,
                column,
                end_line,
                end_column,
            } = fact
            {
                source = Some((document, start, end, line, column, end_line, end_column));
            }
            true
        }),
        EvidenceMetadataVisit::Visited
    );
    assert_eq!(
        source,
        Some((
            "source/test",
            Some(4),
            Some(12),
            Some(2),
            Some(3),
            Some(2),
            Some(11),
        ))
    );
}
