use conduit_kernel::debug_observation::{
    DebugEventKind, DebugExecutionIdentity, DebugObservationGap, DebugObservationRecord,
    DebugSubject, DEBUG_OBSERVATION_SCHEMA_VERSION, MAX_DEBUG_VALUE_PREVIEW_BYTES,
};
use conduit_kernel::{CordId, NodeId, PortId};

use crate::{
    DebuggerValueKind, DebuggerWatchBinding, DebuggerWatchError, DebuggerWatchLifecycle,
    DebuggerWatchSet, DebuggerWatchSubjectRole, MAX_DEBUGGER_WATCHES, MAX_WATCH_HISTORY_RECORDS,
};

fn execution(byte: u8) -> DebugExecutionIdentity {
    DebugExecutionIdentity {
        body: [byte; 32],
        plan: [byte + 1; 32],
        play: [byte + 2; 32],
    }
}

fn binding(index: u16, role: DebuggerWatchSubjectRole) -> DebuggerWatchBinding {
    let runtime_subject = match role {
        DebuggerWatchSubjectRole::Gear => DebugSubject::Gear(NodeId(index)),
        DebuggerWatchSubjectRole::Port => DebugSubject::Port {
            gear: NodeId(index),
            port: PortId(0),
        },
        DebuggerWatchSubjectRole::Cord => DebugSubject::Cord(CordId(index)),
    };
    DebuggerWatchBinding {
        runtime_subject,
        visible_subject: format!("subject/{index}"),
        role,
    }
}

fn record(
    execution: DebugExecutionIdentity,
    sequence: u64,
    subject: DebugSubject,
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
        host: 1,
        form: 2,
        subject,
        related_subject: None,
        kind,
        type_identity: matches!(kind, DebugEventKind::ValueSent).then_some(9),
        value_bytes: u32::try_from(value.len()).unwrap(),
        preview_len: u8::try_from(length).unwrap(),
        preview_truncated: length < value.len(),
        preview,
        fault_code: (kind == DebugEventKind::Fault).then_some(77),
        causal_parent_sequence: None,
        invocation_sequence: None,
    }
}

#[test]
fn numeric_text_and_fault_watches_retain_exact_bounded_history() {
    let identity = execution(1);
    let bindings = vec![
        binding(0, DebuggerWatchSubjectRole::Cord),
        binding(1, DebuggerWatchSubjectRole::Port),
        binding(2, DebuggerWatchSubjectRole::Gear),
    ];
    let mut watches = DebuggerWatchSet::new(identity, bindings.clone()).unwrap();
    for subject in ["subject/0", "subject/1", "subject/2"] {
        watches.add(subject).unwrap();
    }
    watches
        .observe(&record(
            identity,
            1,
            bindings[0].runtime_subject,
            DebugEventKind::ValueSent,
            b"42",
        ))
        .unwrap();
    watches
        .observe(&record(
            identity,
            2,
            bindings[1].runtime_subject,
            DebugEventKind::ValueSent,
            b"hello watch",
        ))
        .unwrap();
    watches
        .observe(&record(
            identity,
            3,
            bindings[2].runtime_subject,
            DebugEventKind::Fault,
            &[],
        ))
        .unwrap();

    let numeric = &watches.watches[0];
    assert_eq!(
        numeric
            .latest
            .as_ref()
            .unwrap()
            .value
            .as_ref()
            .unwrap()
            .kind,
        DebuggerValueKind::Scalar
    );
    assert_eq!(
        watches.watches[1]
            .latest
            .as_ref()
            .unwrap()
            .value
            .as_ref()
            .unwrap()
            .summary,
        "\"hello watch\""
    );
    assert_eq!(
        watches.watches[2].latest.as_ref().unwrap().fault_code,
        Some(77)
    );
    assert!(watches.watches.iter().all(|watch| {
        watch.execution == identity.into() && watch.lifecycle == DebuggerWatchLifecycle::Current
    }));
}

#[test]
fn long_text_and_opaque_values_degrade_to_explicit_bounded_previews() {
    let identity = execution(3);
    let bindings = vec![
        binding(0, DebuggerWatchSubjectRole::Port),
        binding(1, DebuggerWatchSubjectRole::Cord),
    ];
    let mut watches = DebuggerWatchSet::new(identity, bindings.clone()).unwrap();
    watches.add("subject/0").unwrap();
    watches.add("subject/1").unwrap();
    watches
        .observe(&record(
            identity,
            1,
            bindings[0].runtime_subject,
            DebugEventKind::ValueSent,
            &[b'x'; MAX_DEBUG_VALUE_PREVIEW_BYTES + 40],
        ))
        .unwrap();
    watches
        .observe(&record(
            identity,
            2,
            bindings[1].runtime_subject,
            DebugEventKind::ValueSent,
            &[0xff, 0xfe, 0xfd],
        ))
        .unwrap();
    let text = watches.watches[0]
        .latest
        .as_ref()
        .unwrap()
        .value
        .as_ref()
        .unwrap();
    assert_eq!(text.kind, DebuggerValueKind::Text);
    assert!(text.truncated);
    assert_eq!(
        text.total_bytes as usize,
        MAX_DEBUG_VALUE_PREVIEW_BYTES + 40
    );
    assert!(text.summary.len() <= crate::MAX_DEBUGGER_SUMMARY_BYTES);
    let bytes = watches.watches[1]
        .latest
        .as_ref()
        .unwrap()
        .value
        .as_ref()
        .unwrap();
    assert_eq!(bytes.kind, DebuggerValueKind::Bytes);
    assert_eq!(bytes.summary, "3 B");
}

#[test]
fn history_overflow_and_telemetry_gap_are_explicit_and_finite() {
    let identity = execution(4);
    let binding = binding(0, DebuggerWatchSubjectRole::Cord);
    let mut watches = DebuggerWatchSet::new(identity, vec![binding.clone()]).unwrap();
    watches.add("subject/0").unwrap();
    for sequence in 0..10_000 {
        watches
            .observe(&record(
                identity,
                sequence,
                binding.runtime_subject,
                DebugEventKind::ValueSent,
                sequence.to_string().as_bytes(),
            ))
            .unwrap();
    }
    watches.note_gap(DebugObservationGap {
        dropped_records: 9_000,
        first_retained_sequence: 9_000,
    });
    let watch = &watches.watches[0];
    assert_eq!(watch.update_count, 10_000);
    assert_eq!(watch.history.len(), MAX_WATCH_HISTORY_RECORDS);
    assert_eq!(
        watch.evicted_history,
        10_000 - MAX_WATCH_HISTORY_RECORDS as u64
    );
    assert_eq!(watch.history.first().unwrap().sequence, 9_968);
    assert_eq!(watch.latest.as_ref().unwrap().sequence, 9_999);
    assert_eq!(watch.telemetry_gap.as_ref().unwrap().dropped_records, 9_000);
    assert_eq!(watch.rate.as_ref().unwrap().sequence_span, 31);
}

#[test]
fn watch_limit_lifecycle_and_removal_never_mutate_execution_or_topology() {
    let identity = execution(7);
    let bindings = (0..=MAX_DEBUGGER_WATCHES)
        .map(|index| binding(index as u16, DebuggerWatchSubjectRole::Gear))
        .collect::<Vec<_>>();
    let mut watches = DebuggerWatchSet::new(identity, bindings).unwrap();
    for index in 0..MAX_DEBUGGER_WATCHES {
        watches.add(&format!("subject/{index}")).unwrap();
    }
    assert_eq!(
        watches.add(&format!("subject/{MAX_DEBUGGER_WATCHES}")),
        Err(DebuggerWatchError::WatchLimit)
    );
    let topology = vec!["gear/a", "port/a/out", "cord/a-b", "gear/b"];
    let before = topology.clone();
    watches.subject_disappeared("subject/1").unwrap();
    assert_eq!(
        watches.watches[1].lifecycle,
        DebuggerWatchLifecycle::Missing
    );
    watches.replace_execution(execution(8));
    assert!(watches
        .watches
        .iter()
        .all(|watch| watch.lifecycle == DebuggerWatchLifecycle::StaleExecution));
    assert_eq!(
        watches.observe(&record(
            identity,
            1,
            DebugSubject::Gear(NodeId(0)),
            DebugEventKind::GearStarted,
            &[],
        )),
        Err(DebuggerWatchError::StaleExecution)
    );
    watches.remove("subject/0").unwrap();
    watches.clear_history("subject/2").unwrap();
    assert_eq!(topology, before);
}

#[test]
fn friendly_label_cannot_create_or_retarget_a_watch() {
    let identity = execution(10);
    let mut watches =
        DebuggerWatchSet::new(identity, vec![binding(0, DebuggerWatchSubjectRole::Port)]).unwrap();
    assert_eq!(
        watches.add("friendly output"),
        Err(DebuggerWatchError::IneligibleSubject)
    );
    watches.add("subject/0").unwrap();
    assert_eq!(
        watches.add("subject/0"),
        Err(DebuggerWatchError::DuplicateWatch)
    );
}
