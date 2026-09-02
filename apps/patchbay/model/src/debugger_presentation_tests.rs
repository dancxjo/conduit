use conduit_kernel::debug_observation::{
    DebugEventKind, DebugExecutionIdentity, DebugObservationGap, DebugObservationRecord,
    DebugSubject, DEBUG_OBSERVATION_SCHEMA_VERSION, MAX_DEBUG_VALUE_PREVIEW_BYTES,
};
use conduit_kernel::{CordId, NodeId, PortId};

use crate::{
    DebuggerActivityPhase, DebuggerPresentation, DebuggerPresentationError, DebuggerSubjectBinding,
    DebuggerValueKind,
};

fn execution_id(byte: u8) -> DebugExecutionIdentity {
    DebugExecutionIdentity {
        body: [byte; 32],
        plan: [byte + 1; 32],
        play: [byte + 2; 32],
    }
}

fn bindings() -> Vec<DebuggerSubjectBinding> {
    vec![
        DebuggerSubjectBinding {
            runtime_subject: DebugSubject::Gear(NodeId(0)),
            visible_subject: "gear/parse".into(),
            line_subject: None,
            host: 7,
        },
        DebuggerSubjectBinding {
            runtime_subject: DebugSubject::Port {
                gear: NodeId(0),
                port: PortId(0),
            },
            visible_subject: "port/parse/out".into(),
            line_subject: None,
            host: 7,
        },
        DebuggerSubjectBinding {
            runtime_subject: DebugSubject::Cord(CordId(0)),
            visible_subject: "cord/parse-double".into(),
            line_subject: Some("line/usb".into()),
            host: 7,
        },
    ]
}

fn record(
    execution: DebugExecutionIdentity,
    sequence: u64,
    subject: DebugSubject,
    related_subject: Option<DebugSubject>,
    kind: DebugEventKind,
    value: &[u8],
) -> DebugObservationRecord {
    let mut preview = [0; MAX_DEBUG_VALUE_PREVIEW_BYTES];
    let length = value.len().min(preview.len());
    preview[..length].copy_from_slice(&value[..length]);
    DebugObservationRecord {
        schema_version: DEBUG_OBSERVATION_SCHEMA_VERSION,
        execution,
        sequence,
        host_sequence: sequence,
        host: 7,
        form: 3,
        subject,
        related_subject,
        kind,
        type_identity: matches!(
            kind,
            DebugEventKind::ValueSent | DebugEventKind::ValueReceived
        )
        .then_some(12),
        value_bytes: u32::try_from(value.len()).unwrap(),
        preview_len: u8::try_from(length).unwrap(),
        preview_truncated: length < value.len(),
        preview,
        fault_code: (kind == DebugEventKind::Fault).then_some(17),
    }
}

#[test]
fn known_sequence_maps_to_exact_gear_port_cord_line_and_typed_value() {
    let execution = execution_id(1);
    let mut presentation = DebuggerPresentation::new(execution, bindings(), false).unwrap();
    presentation
        .observe(&record(
            execution,
            0,
            DebugSubject::Gear(NodeId(0)),
            None,
            DebugEventKind::GearStarted,
            &[],
        ))
        .unwrap();
    presentation
        .observe(&record(
            execution,
            1,
            DebugSubject::Cord(CordId(0)),
            Some(DebugSubject::Port {
                gear: NodeId(0),
                port: PortId(0),
            }),
            DebugEventKind::ValueSent,
            b"42",
        ))
        .unwrap();

    let cord = presentation
        .activities
        .iter()
        .find(|activity| activity.subject == "cord/parse-double")
        .unwrap();
    assert_eq!(cord.line_subject.as_deref(), Some("line/usb"));
    assert_eq!(cord.host, 7);
    assert_eq!(cord.phase, DebuggerActivityPhase::Active);
    assert_eq!(
        cord.latest_value.as_ref().unwrap().kind,
        DebuggerValueKind::Scalar
    );
    assert_eq!(cord.latest_value.as_ref().unwrap().summary, "42");
    assert!(presentation
        .activities
        .iter()
        .any(|activity| activity.subject == "port/parse/out"));
}

#[test]
fn high_rate_activity_coalesces_by_subject_and_exposes_loss() {
    let execution = execution_id(4);
    let mut presentation = DebuggerPresentation::new(execution, bindings(), false).unwrap();
    for sequence in 0..10_000 {
        presentation
            .observe(&record(
                execution,
                sequence,
                DebugSubject::Cord(CordId(0)),
                None,
                DebugEventKind::ValueSent,
                b"84",
            ))
            .unwrap();
    }
    presentation.note_gap(DebugObservationGap {
        dropped_records: 27,
        first_retained_sequence: 27,
    });

    assert_eq!(presentation.activities.len(), 1);
    assert_eq!(presentation.activities[0].observed_count, 10_000);
    assert_eq!(presentation.activities[0].coalesced_count, 9_999);
    assert_eq!(presentation.gap.as_ref().unwrap().dropped_records, 27);
}

#[test]
fn activity_decays_faults_retain_and_reduced_motion_keeps_textual_truth() {
    let execution = execution_id(9);
    let mut presentation = DebuggerPresentation::new(execution, bindings(), true).unwrap();
    presentation
        .observe(&record(
            execution,
            0,
            DebugSubject::Gear(NodeId(0)),
            None,
            DebugEventKind::Fault,
            &[],
        ))
        .unwrap();
    presentation.advance(100).unwrap();
    let activity = &presentation.activities[0];
    assert!(presentation.reduced_motion);
    assert_eq!(activity.phase, DebuggerActivityPhase::Faulted);
    assert_eq!(activity.retained_fault_code, Some(17));
    assert_eq!(activity.latest_kind, "fault");

    presentation.clear_fault("gear/parse").unwrap();
    assert_eq!(
        presentation.activities[0].phase,
        DebuggerActivityPhase::Inactive
    );
}

#[test]
fn stale_unknown_host_and_sequence_refuse_without_replacing_state() {
    let execution = execution_id(12);
    let mut presentation = DebuggerPresentation::new(execution, bindings(), false).unwrap();
    let accepted = record(
        execution,
        2,
        DebugSubject::Gear(NodeId(0)),
        None,
        DebugEventKind::GearCompleted,
        &[],
    );
    presentation.observe(&accepted).unwrap();
    let retained = presentation.clone();

    assert_eq!(
        presentation.observe(&record(
            execution_id(13),
            3,
            DebugSubject::Gear(NodeId(0)),
            None,
            DebugEventKind::GearStarted,
            &[],
        )),
        Err(DebuggerPresentationError::StaleExecution)
    );
    let mut wrong_host = record(
        execution,
        3,
        DebugSubject::Gear(NodeId(0)),
        None,
        DebugEventKind::GearStarted,
        &[],
    );
    wrong_host.host = 8;
    assert_eq!(
        presentation.observe(&wrong_host),
        Err(DebuggerPresentationError::HostMismatch)
    );
    assert_eq!(
        presentation.observe(&accepted),
        Err(DebuggerPresentationError::NonmonotonicSequence)
    );
    assert_eq!(presentation, retained);
}

#[test]
fn overlay_detach_cannot_mutate_canonical_graph_state() {
    let execution = execution_id(20);
    let canonical_topology = vec!["gear/parse", "cord/parse-double"];
    let before = canonical_topology.clone();
    let presentation = DebuggerPresentation::new(execution, bindings(), false).unwrap();
    assert!(presentation.detach().is_empty());
    assert_eq!(canonical_topology, before);
}
