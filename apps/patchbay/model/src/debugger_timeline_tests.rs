use conduit_kernel::debug_observation::{
    DebugEventKind, DebugExecutionIdentity, DebugObservationGap, DebugObservationRecord,
    DebugSubject, DEBUG_OBSERVATION_SCHEMA_VERSION, MAX_DEBUG_VALUE_PREVIEW_BYTES,
};
use conduit_kernel::{CordId, NodeId};

use crate::{
    DebuggerTimeline, DebuggerTimelineBinding, DebuggerTimelineError, DebuggerTimelineMode,
    DebuggerWatchBinding, DebuggerWatchSet, DebuggerWatchSubjectRole, MAX_DEBUGGER_TIMELINE_EVENTS,
};

fn execution(byte: u8) -> DebugExecutionIdentity {
    DebugExecutionIdentity {
        body: [byte; 32],
        plan: [byte + 1; 32],
        play: [byte + 2; 32],
    }
}

fn record(
    execution: DebugExecutionIdentity,
    sequence: u64,
    subject: DebugSubject,
    value: &str,
) -> DebugObservationRecord {
    let mut preview = [0; MAX_DEBUG_VALUE_PREVIEW_BYTES];
    preview[..value.len()].copy_from_slice(value.as_bytes());
    DebugObservationRecord {
        schema_version: DEBUG_OBSERVATION_SCHEMA_VERSION,
        execution,
        sequence,
        host_sequence: sequence,
        host: 1,
        form: 2,
        subject,
        related_subject: None,
        kind: DebugEventKind::ValueSent,
        type_identity: Some(9),
        value_bytes: value.len() as u32,
        preview_len: value.len() as u8,
        preview_truncated: false,
        preview,
        fault_code: None,
        causal_parent_sequence: None,
        invocation_sequence: None,
    }
}

fn timeline(executions: &[DebugExecutionIdentity]) -> DebuggerTimeline {
    DebuggerTimeline::new(
        executions
            .iter()
            .flat_map(|execution| {
                [
                    DebuggerTimelineBinding {
                        execution: *execution,
                        runtime_subject: DebugSubject::Gear(NodeId(1)),
                        visible_subject: format!("gear/{}", execution.body[0]),
                    },
                    DebuggerTimelineBinding {
                        execution: *execution,
                        runtime_subject: DebugSubject::Cord(CordId(1)),
                        visible_subject: format!("cord/{}", execution.body[0]),
                    },
                    DebuggerTimelineBinding {
                        execution: *execution,
                        runtime_subject: DebugSubject::Cord(CordId(2)),
                        visible_subject: format!("branch/{}", execution.body[0]),
                    },
                ]
            })
            .collect(),
    )
    .unwrap()
}

#[test]
fn live_pause_scrub_and_jump_live_share_one_projection() {
    let identity = execution(1);
    let mut timeline = timeline(&[identity]);
    timeline
        .observe(&record(identity, 1, DebugSubject::Cord(CordId(1)), "41"))
        .unwrap();
    timeline
        .observe(&record(identity, 2, DebugSubject::Cord(CordId(1)), "42"))
        .unwrap();
    assert_eq!(timeline.project(None).cursor_sequence, Some(2));
    timeline.pause();
    timeline
        .observe(&record(identity, 3, DebugSubject::Cord(CordId(1)), "43"))
        .unwrap();
    assert_eq!(timeline.project(None).cursor_sequence, Some(2));
    timeline.move_cursor(0).unwrap();
    assert_eq!(
        timeline.project(None).states[0]
            .value
            .as_ref()
            .unwrap()
            .summary,
        "41"
    );
    timeline.next_event().unwrap();
    assert_eq!(
        timeline.project(None).states[0]
            .value
            .as_ref()
            .unwrap()
            .summary,
        "42"
    );
    timeline.jump_live();
    assert_eq!(timeline.mode, DebuggerTimelineMode::Live);
    assert_eq!(timeline.project(None).cursor_sequence, Some(3));
}

#[test]
fn exact_event_graph_filter_and_watch_replay_are_two_way() {
    let identity = execution(4);
    let mut timeline = timeline(&[identity]);
    timeline
        .observe(&record(identity, 1, DebugSubject::Gear(NodeId(1)), "10"))
        .unwrap();
    timeline
        .observe(&record(identity, 2, DebugSubject::Cord(CordId(1)), "20"))
        .unwrap();
    timeline
        .observe(&record(identity, 3, DebugSubject::Cord(CordId(1)), "30"))
        .unwrap();
    let mut watches = DebuggerWatchSet::new(
        identity,
        vec![DebuggerWatchBinding {
            runtime_subject: DebugSubject::Cord(CordId(1)),
            visible_subject: "cord/4".into(),
            role: DebuggerWatchSubjectRole::Cord,
        }],
    )
    .unwrap();
    watches.add("cord/4").unwrap();
    assert_eq!(timeline.select_event(1).unwrap(), "cord/4");
    assert_eq!(timeline.visible_events().count(), 2);
    let projection = timeline.project(Some(&watches));
    assert!(projection.watch_states[0].historical);
    assert_eq!(
        projection.watch_states[0].latest.as_ref().unwrap().sequence,
        2
    );
    timeline.filter_subject(None).unwrap();
    assert_eq!(timeline.visible_events().count(), 3);
}

#[test]
fn overflow_gap_and_replacement_execution_remain_explicit() {
    let first = execution(7);
    let second = execution(8);
    let mut timeline = timeline(&[first, second]);
    for sequence in 0..MAX_DEBUGGER_TIMELINE_EVENTS as u64 + 10 {
        timeline
            .observe(&record(first, sequence, DebugSubject::Cord(CordId(1)), "1"))
            .unwrap();
    }
    timeline.note_gap(DebugObservationGap {
        dropped_records: 5,
        first_retained_sequence: 10,
    });
    timeline
        .observe(&record(second, 1, DebugSubject::Cord(CordId(1)), "99"))
        .unwrap();
    assert_eq!(timeline.events.len(), MAX_DEBUGGER_TIMELINE_EVENTS);
    assert_eq!(timeline.evicted_events, 11);
    let projection = timeline.project(None);
    assert_eq!(projection.execution.unwrap().body, second.body);
    assert_eq!(projection.states.len(), 1);
    assert_eq!(projection.states[0].subject, "cord/8");
    assert!(!projection.exact_reconstruction);
    assert_eq!(timeline.gap.as_ref().unwrap().dropped_records, 5);
    timeline.move_cursor(0).unwrap();
    let prior = timeline.project(None);
    assert_eq!(prior.execution.unwrap().body, first.body);
    assert_eq!(prior.states[0].subject, "cord/7");
}

#[test]
fn stale_unknown_and_nonmonotonic_inputs_refuse() {
    let identity = execution(10);
    let mut timeline = timeline(&[identity]);
    timeline
        .observe(&record(identity, 2, DebugSubject::Cord(CordId(1)), "2"))
        .unwrap();
    assert_eq!(
        timeline.observe(&record(identity, 1, DebugSubject::Cord(CordId(1)), "1")),
        Err(DebuggerTimelineError::NonmonotonicSequence)
    );
    assert_eq!(
        timeline.observe(&record(identity, 3, DebugSubject::Cord(CordId(99)), "3")),
        Err(DebuggerTimelineError::UnknownSubject)
    );
    assert_eq!(
        timeline.move_cursor(99),
        Err(DebuggerTimelineError::InvalidCursor)
    );
}

#[test]
fn causal_trace_follows_exact_observed_parents_and_exposes_missing_history() {
    let identity = execution(12);
    let mut timeline = timeline(&[identity]);
    let mut admit = |sequence, subject, parent| {
        let mut event = record(identity, sequence, subject, "42");
        event.causal_parent_sequence = parent;
        timeline.observe(&event).unwrap();
    };
    admit(0, DebugSubject::Gear(NodeId(1)), None);
    admit(1, DebugSubject::Cord(CordId(1)), Some(0));
    admit(2, DebugSubject::Gear(NodeId(1)), Some(1));
    admit(3, DebugSubject::Cord(CordId(1)), Some(2));
    // A structurally possible sibling is not a descendant of event 1.
    admit(4, DebugSubject::Cord(CordId(2)), Some(0));

    timeline.trace_upstream(3).unwrap();
    let upstream = timeline.trace.as_ref().unwrap();
    assert_eq!(
        upstream
            .steps
            .iter()
            .map(|step| step.sequence)
            .collect::<Vec<_>>(),
        vec![0, 1, 2, 3]
    );
    assert!(!upstream.steps.iter().any(|step| step.sequence == 4));

    timeline.trace_downstream(1).unwrap();
    let downstream = timeline.trace.as_ref().unwrap();
    assert_eq!(
        downstream
            .steps
            .iter()
            .map(|step| step.sequence)
            .collect::<Vec<_>>(),
        vec![1, 2, 3]
    );

    let mut missing = record(identity, 5, DebugSubject::Cord(CordId(1)), "fault");
    missing.causal_parent_sequence = Some(99);
    timeline.observe(&missing).unwrap();
    timeline.trace_upstream(5).unwrap();
    assert_eq!(
        timeline.trace.as_ref().unwrap().missing_parent_sequences,
        vec![99]
    );
}
