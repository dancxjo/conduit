use super::protocol::Output;
use super::session::{Role, Session};

const SOURCE: &str = r#"form hello-across {
    message: text/literal("hello across one planned Cord")
    show: presentation/text
    message > show
}"#;

fn prepare_pair() -> ((Session, Output), (Session, Output)) {
    prepare_pair_for(SOURCE)
}

fn prepare_pair_for(source: &str) -> ((Session, Output), (Session, Output)) {
    let interaction = crate::source_interaction::admit_source(source.as_bytes(), 7).unwrap();
    let source_plan =
        super::plan::prepare("browser/a", "boot/a", "browser/b", "boot/b", source).unwrap();
    let sink_plan = super::plan::accept(source_plan.plan.clone(), "browser/b", "boot/b").unwrap();
    (
        Session::prepare(Role::Source, source_plan, 9, interaction.clone()).unwrap(),
        Session::prepare(Role::Sink, sink_plan, 9, interaction).unwrap(),
    )
}

fn prepare_pair_for_alternate_line(source: &str) -> ((Session, Output), (Session, Output)) {
    let interaction = crate::source_interaction::admit_source(source.as_bytes(), 7).unwrap();
    let source_plan = super::plan::prepare_with_alternate_line(
        "browser/a",
        "boot/a",
        "browser/b",
        "boot/b",
        source,
    )
    .unwrap();
    let sink_plan = super::plan::accept(source_plan.plan.clone(), "browser/b", "boot/b").unwrap();
    (
        Session::prepare(Role::Source, source_plan, 9, interaction.clone()).unwrap(),
        Session::prepare(Role::Sink, sink_plan, 9, interaction).unwrap(),
    )
}

fn prepare_firefly_pair() -> ((Session, Output), (Session, Output)) {
    let source = include_str!("../../../../../../forms/firefly-line-follower/main.conduit");
    let interaction = crate::source_interaction::admit_source(source.as_bytes(), 7).unwrap();
    let source_plan = super::plan::prepare(
        "browser/firefly-a",
        "boot/firefly-a",
        "browser/firefly-b",
        "boot/firefly-b",
        source,
    )
    .unwrap();
    let sink_plan = super::plan::accept(
        source_plan.plan.clone(),
        "browser/firefly-b",
        "boot/firefly-b",
    )
    .unwrap();
    (
        Session::prepare(Role::Source, source_plan, 9, interaction.clone()).unwrap(),
        Session::prepare(Role::Sink, sink_plan, 9, interaction).unwrap(),
    )
}

fn complete_firefly_timers(sender: &mut Session, mut output: Output) -> Output {
    while let Output::Timer { timer, .. } = output {
        output = sender
            .complete_timer(&timer.active_play_id, timer.request_sequence)
            .unwrap();
    }
    output
}

#[test]
fn two_browser_hosts_exchange_pulses_and_converge_over_one_planned_line() {
    let ((mut sender, mut output), (mut follower, waiting)) = prepare_firefly_pair();
    assert!(matches!(waiting, Output::Waiting { .. }));
    let mut periods = Vec::new();
    for sequence in 0..4 {
        output = complete_firefly_timers(&mut sender, output);
        let Output::Line {
            frame,
            plan_projection: Some(projection),
            ..
        } = output
        else {
            panic!("Firefly source did not offer pulse {sequence}")
        };
        assert_eq!(frame.phase, "value");
        assert_eq!(frame.sequence, sequence);
        assert_eq!(frame.value_kind, conduit_time::PULSE_OBSERVATION_VALUE_KIND);
        assert_eq!(
            conduit_time::decode_pulse_observation(&frame.payload).unwrap(),
            conduit_time::PulseObservation {
                sequence: sequence as u32,
                period_ms: 240,
            }
        );
        assert_eq!(projection.hosts.len(), 2);
        assert_eq!(projection.cord.maximum_in_flight_items, 1);
        assert_eq!(projection.cord.maximum_payload_bytes, 4096);
        let Output::Manifestation {
            manifestation,
            accepted_frame,
            ..
        } = follower.ingest(*frame).unwrap()
        else {
            panic!("Firefly follower did not manifest phase update {sequence}")
        };
        assert_eq!(manifestation.host_id, "browser/firefly-b");
        assert_eq!(
            manifestation.presentation_kind,
            conduit_semantic_catalog::RHYTHM_PRESENTATION_KIND
        );
        let text = manifestation.text.unwrap();
        let period = text
            .split("period ")
            .nth(1)
            .and_then(|suffix| suffix.split(" ms").next())
            .unwrap()
            .parse::<u16>()
            .unwrap();
        periods.push(period);
        assert!(matches!(
            sender.ingest(*accepted_frame).unwrap(),
            Output::Waiting { .. }
        ));
        let delivered = match follower.complete_manifestation().unwrap() {
            Output::Line { frame, .. } => frame,
            _ => panic!("Firefly follower did not acknowledge manifestation"),
        };
        output = sender.ingest(*delivered).unwrap();
    }
    assert_eq!(periods, [270, 262, 262, 262]);
    let Output::Line { frame: close, .. } = output else {
        panic!("Firefly source did not close its pulse Cord")
    };
    assert_eq!(close.phase, "close");
    let Output::Line {
        frame: terminal,
        receipt: Some(sink_receipt),
        ..
    } = follower.ingest(*close).unwrap()
    else {
        panic!("Firefly follower did not complete")
    };
    assert_eq!(sink_receipt.transferred_values, 4);
    assert_eq!(sink_receipt.disposition, "completed");
    let Output::Receipt { receipt, .. } = sender.ingest(*terminal).unwrap() else {
        panic!("Firefly sender did not retain terminal truth")
    };
    assert_eq!(receipt.transferred_values, 4);
    assert_eq!(receipt.disposition, "completed");
}

#[test]
fn firefly_stale_peer_input_refuses_without_admission() {
    let ((mut sender, output), (mut follower, _)) = prepare_firefly_pair();
    let Output::Line {
        frame: mut stale, ..
    } = complete_firefly_timers(&mut sender, output)
    else {
        panic!("Firefly source did not offer its first pulse")
    };
    stale.sequence = 1;
    assert_eq!(
        follower.ingest(*stale).unwrap_err(),
        "multi-Host Line frame does not match the exact planned identity"
    );
}

#[test]
fn firefly_line_pressure_keeps_one_pulse_in_flight() {
    let ((mut sender, output), (mut follower, _)) = prepare_firefly_pair();
    let Output::Line { frame, .. } = complete_firefly_timers(&mut sender, output) else {
        panic!("Firefly source did not offer its first bounded pulse")
    };
    let Output::Manifestation { accepted_frame, .. } = follower.ingest(*frame).unwrap() else {
        panic!("Firefly follower did not accept its one admitted in-flight pulse")
    };
    assert!(matches!(
        sender.ingest((*accepted_frame).clone()).unwrap(),
        Output::Waiting { .. }
    ));
    assert_eq!(
        sender.ingest(*accepted_frame).unwrap_err(),
        "multi-Host Line frame arrived in the wrong exact lifecycle phase"
    );
}

#[test]
fn firefly_line_loss_terminals_remain_distinct() {
    use super::session::TransportTermination;
    for (termination, disposition) in [
        (TransportTermination::Unavailable, "transport-unavailable"),
        (TransportTermination::Disconnected, "disconnected"),
        (TransportTermination::TimedOut, "timed-out"),
    ] {
        let ((mut sender, output), _) = prepare_firefly_pair();
        assert!(matches!(
            complete_firefly_timers(&mut sender, output),
            Output::Line { .. }
        ));
        let Output::Receipt { receipt, .. } = sender.terminate_transport(termination, 73).unwrap()
        else {
            panic!("Firefly Line loss did not retain terminal truth")
        };
        assert_eq!(receipt.disposition, disposition);
        assert_eq!(receipt.deliveries[0].state, disposition);
        assert_eq!(receipt.deliveries[0].failure_code, Some(73));
    }
}

#[test]
fn unchanged_form_executes_two_exact_fragments_over_one_planned_line() {
    let ((mut source, source_output), (mut sink, sink_output)) = prepare_pair();
    assert!(matches!(sink_output, Output::Waiting { .. }));
    let (value, source_projection) = match source_output {
        Output::Line {
            frame,
            plan_projection: Some(projection),
            receipt: None,
            ..
        } => (frame, projection),
        _ => panic!("source did not offer its exact Line value"),
    };
    assert_eq!(value.phase, "value");
    assert_eq!(value.sequence, 0);
    assert_eq!(value.payload, b"hello across one planned Cord");
    assert_eq!(source_projection.hosts.len(), 2);
    assert!(source_projection.cord.crosses_host);
    assert_eq!(source_projection.cord.maximum_in_flight_items, 1);

    let (accepted, sink_projection, manifestation) = match sink.ingest(*value).unwrap() {
        Output::Manifestation {
            accepted_frame,
            plan_projection,
            manifestation,
            ..
        } => (accepted_frame, plan_projection, manifestation),
        _ => panic!("sink did not request its planned presentation"),
    };
    assert_eq!(source_projection.plan_id, sink_projection.plan_id);
    assert_eq!(
        manifestation.text.as_deref(),
        Some("hello across one planned Cord")
    );
    assert_eq!(manifestation.host_id, "browser/b");
    assert!(matches!(
        source.ingest(*accepted).unwrap(),
        Output::Waiting { .. }
    ));

    let delivered = match sink.complete_manifestation().unwrap() {
        Output::Line { frame, .. } => frame,
        _ => panic!("sink did not acknowledge exact delivery"),
    };
    let close = match source.ingest(*delivered).unwrap() {
        Output::Line { frame, .. } => frame,
        _ => panic!("source did not close its exact remote Cord"),
    };
    let (terminal, sink_receipt) = match sink.ingest(*close).unwrap() {
        Output::Line {
            frame,
            receipt: Some(receipt),
            ..
        } => (frame, receipt),
        _ => panic!("sink did not reach terminal truth"),
    };
    assert_eq!(sink_receipt.disposition, "completed");
    assert_eq!(sink_receipt.transferred_values, 1);
    match source.ingest(*terminal).unwrap() {
        Output::Receipt { receipt, .. } => {
            assert_eq!(receipt.disposition, "completed");
            assert_eq!(receipt.transferred_values, 1);
            assert_ne!(receipt.fragment_id, sink_receipt.fragment_id);
            assert_ne!(receipt.active_play_id, sink_receipt.active_play_id);
            assert_eq!(receipt.deliveries.len(), 1);
            assert_eq!(receipt.deliveries[0].state, "remote-accepted");
            assert!(receipt.deliveries[0].remote_receipt_hex.is_some());
        }
        _ => panic!("source did not retain terminal truth"),
    }
}

#[test]
fn exact_plan_admission_refuses_a_stale_sink_boot_before_play() {
    let exact = super::plan::prepare("browser/a", "boot/a", "browser/b", "boot/b", SOURCE).unwrap();
    let error = super::plan::accept(exact.plan, "browser/b", "boot/stale")
        .err()
        .expect("the exact Plan must not move to a stale Boot");
    assert_eq!(
        error,
        "received multi-Host Plan does not name this exact sink Host and Boot"
    );
}

#[test]
fn wrong_boot_frame_refuses_before_remote_admission_and_cancel_is_distinct() {
    let ((mut source, source_output), (mut sink, _)) = prepare_pair();
    let mut value = match source_output {
        Output::Line { frame, .. } => frame,
        _ => panic!("source did not offer a value"),
    };
    value.sink_boot_id.push_str("/stale");
    assert_eq!(
        sink.ingest(*value).unwrap_err(),
        "multi-Host Line frame does not match the exact planned identity"
    );
    match source.cancel().unwrap() {
        Output::Receipt { receipt, .. } => {
            assert_eq!(receipt.disposition, "cancelled");
            assert_eq!(receipt.transferred_values, 0);
            assert_eq!(receipt.deliveries.len(), 1);
            assert_eq!(receipt.deliveries[0].state, "framed-queued");
            assert!(receipt.deliveries[0].remote_receipt_hex.is_none());
        }
        _ => panic!("cancellation did not retain a distinct receipt"),
    }
}

#[test]
fn oversized_line_value_refuses_before_remote_admission() {
    let ((_, source_output), (mut sink, _)) = prepare_pair();
    let mut value = match source_output {
        Output::Line { frame, .. } => frame,
        _ => panic!("source did not offer a value"),
    };
    value.payload = vec![0; crate::installed_browser::MAXIMUM_BROWSER_VALUE_BYTES + 1];
    assert!(sink.ingest(*value).is_err());
}

#[test]
fn ordinary_transform_can_run_after_the_remote_cord() {
    let source = r#"form too-large {
        message: text/literal("hello")
        upper: text/upper
        show: presentation/text
        message > upper > show
    }"#;
    let plan = super::plan::prepare("browser/a", "boot/a", "browser/b", "boot/b", source).unwrap();
    assert_eq!(plan.plan.fragments.len(), 2);
    assert_eq!(
        plan.plan
            .fragments
            .iter()
            .map(|fragment| fragment.placements.len())
            .sum::<usize>(),
        3
    );
}

#[test]
fn desk_telegraph_patchbay_separates_record_queue_line_delivery_transcript_and_signs() {
    let source = include_str!("../../../../../../forms/desk-telegraph/main.conduit");
    let prepared =
        super::plan::prepare("browser/a", "boot/a", "browser/b", "boot/b", source).unwrap();
    assert_eq!(prepared.plan.fragments.len(), 2);
    let source_fragment = prepared
        .plan
        .fragments
        .iter()
        .find(|fragment| fragment.host_id.as_str() == "browser/a")
        .unwrap();
    let sink_fragment = prepared
        .plan
        .fragments
        .iter()
        .find(|fragment| fragment.host_id.as_str() == "browser/b")
        .unwrap();
    assert!(source_fragment
        .placements
        .iter()
        .any(|placement| { placement.kind_id.as_str() == conduit_net::TYPED_RECORD_FRAME_KIND }));
    assert!(sink_fragment
        .placements
        .iter()
        .any(|placement| { placement.kind_id.as_str() == conduit_net::TYPED_RECORD_DEFRAME_KIND }));
    let selected = source_fragment
        .connections
        .iter()
        .find_map(|connection| connection.selected_line.as_ref())
        .unwrap();
    assert_eq!(selected.binding.limits.maximum_in_flight_items, 1);
    assert_eq!(
        selected.contract.ordering,
        conduit_core::LineOrdering::Ordered
    );

    let ((mut source_session, source_output), (mut sink_session, sink_output)) =
        prepare_pair_for(source);
    assert!(matches!(sink_output, Output::Waiting { .. }));
    let (offered, projection) = match source_output {
        Output::Line {
            frame,
            plan_projection: Some(projection),
            ..
        } => (frame, projection),
        _ => panic!("Desk Telegraph did not offer its framed value"),
    };
    let kinds = projection
        .hosts
        .iter()
        .flat_map(|host| host.gears.iter())
        .map(|gear| gear.kind_id.as_str())
        .collect::<Vec<_>>();
    for kind in [
        conduit_net::TEXT_TO_TYPED_RECORD_KIND,
        conduit_net::TYPED_RECORD_FRAME_KIND,
        conduit_net::RECORD_SINGLETON_STREAM_KIND,
        conduit_net::ORDERED_RECORD_QUEUE_KIND,
        conduit_net::RECORD_EXACTLY_ONE_KIND,
        conduit_net::TYPED_RECORD_DEFRAME_KIND,
        conduit_net::TYPED_RECORD_TO_TEXT_KIND,
    ] {
        assert!(kinds.contains(&kind), "Patchbay omitted {kind}");
    }
    assert_eq!(
        projection.cord.value_kind,
        conduit_net::framed_typed_record_type()
            .profile()
            .unwrap()
            .value_kind()
            .as_str()
    );
    assert_eq!(projection.cord.line_id, "tour/browser-memory-line");
    assert_eq!(projection.cord.maximum_in_flight_items, 1);
    assert_ne!(offered.payload, b"CALLING");
    let framed = conduit_core::StructuredInfoValue::from_canonical_bytes(&offered.payload).unwrap();
    assert_eq!(
        framed.value_type(),
        &conduit_net::framed_typed_record_type()
    );
    let accepted = match sink_session.ingest(*offered).unwrap() {
        Output::Manifestation {
            accepted_frame,
            manifestation,
            ..
        } => {
            assert_eq!(manifestation.text.as_deref(), Some("CALLING"));
            accepted_frame
        }
        _ => panic!("remote Desk Telegraph did not present reconstructed text"),
    };
    assert!(matches!(
        source_session.ingest(*accepted).unwrap(),
        Output::Waiting { .. }
    ));
    let delivered = match sink_session.complete_manifestation().unwrap() {
        Output::Line { frame, .. } => frame,
        _ => panic!("Desk Telegraph sink did not acknowledge delivery"),
    };
    let close = match source_session.ingest(*delivered).unwrap() {
        Output::Line { frame, .. } => frame,
        _ => panic!("Desk Telegraph source did not close its Line"),
    };
    let (terminal, sink_receipt) = match sink_session.ingest(*close).unwrap() {
        Output::Line {
            frame,
            receipt: Some(receipt),
            ..
        } => (frame, receipt),
        _ => panic!("Desk Telegraph sink did not retain terminal evidence"),
    };
    let sink_transcript = sink_receipt.transcript.as_ref().unwrap();
    assert_eq!(sink_transcript.retention_gap, 0);
    assert_eq!(sink_transcript.entries.len(), 2);
    assert_eq!(sink_transcript.entries[0].event, "received-record");
    assert_eq!(sink_transcript.entries[1].event, "completed");
    let source_receipt = match source_session.ingest(*terminal).unwrap() {
        Output::Receipt { receipt, .. } => receipt,
        _ => panic!("Desk Telegraph source did not retain terminal evidence"),
    };
    assert_eq!(source_receipt.deliveries[0].state, "remote-accepted");
    assert!(source_receipt.deliveries[0].remote_receipt_hex.is_some());
    assert!(!source_receipt.terminal_sign_id.is_empty());
    let source_transcript = source_receipt.transcript.as_ref().unwrap();
    assert_eq!(source_transcript.retention_gap, 0);
    assert_eq!(source_transcript.entries.len(), 2);
    assert_eq!(source_transcript.entries[0].event, "sent-record");
    assert_eq!(source_transcript.entries[1].event, "completed");
}

#[test]
fn unchanged_desk_telegraph_executes_over_a_second_compatible_line_implementation() {
    let source = include_str!("../../../../../../forms/desk-telegraph/main.conduit");
    let ((mut source_session, source_output), (mut sink_session, sink_output)) =
        prepare_pair_for_alternate_line(source);
    assert!(matches!(sink_output, Output::Waiting { .. }));
    let offered = match source_output {
        Output::Line {
            frame,
            plan_projection: Some(projection),
            ..
        } => {
            assert_eq!(
                projection.cord.base_implementation_id,
                super::plan::ALTERNATE_MEMORY_BASE
            );
            frame
        }
        _ => panic!("alternate Line did not offer the Desk Telegraph frame"),
    };
    let accepted = match sink_session.ingest(*offered).unwrap() {
        Output::Manifestation {
            accepted_frame,
            manifestation,
            ..
        } => {
            assert_eq!(manifestation.text.as_deref(), Some("CALLING"));
            accepted_frame
        }
        _ => panic!("alternate Line did not reconstruct Desk Telegraph text"),
    };
    assert!(matches!(
        source_session.ingest(*accepted).unwrap(),
        Output::Waiting { .. }
    ));
}

#[test]
fn renderer_loss_cannot_be_promoted_from_queued_to_remote_accepted() {
    let source = include_str!("../../../../../../forms/desk-telegraph/main.conduit");
    let ((mut source_session, source_output), (mut sink_session, _)) = prepare_pair_for(source);
    let offered = match source_output {
        Output::Line { frame, .. } => frame,
        _ => panic!("Desk Telegraph did not offer its framed value"),
    };
    assert!(matches!(
        sink_session.ingest(*offered).unwrap(),
        Output::Manifestation { .. }
    ));
    let Output::Receipt {
        receipt: sink_cancelled,
        ..
    } = sink_session.cancel().unwrap()
    else {
        panic!("renderer loss did not cancel the sink Play")
    };
    assert_eq!(sink_cancelled.disposition, "cancelled");
    let sink_transcript = sink_cancelled.transcript.as_ref().unwrap();
    assert_eq!(sink_transcript.entries[0].event, "received-record");
    assert_eq!(sink_transcript.entries[1].event, "cancelled");

    let Output::Receipt {
        receipt: source_cancelled,
        ..
    } = source_session.cancel().unwrap()
    else {
        panic!("sender cancellation did not retain delivery truth")
    };
    assert_eq!(source_cancelled.deliveries.len(), 1);
    assert_eq!(source_cancelled.deliveries[0].state, "framed-queued");
    assert!(source_cancelled.deliveries[0].remote_receipt_hex.is_none());
    let source_transcript = source_cancelled.transcript.as_ref().unwrap();
    assert_eq!(source_transcript.entries[0].event, "sent-record");
    assert_eq!(source_transcript.entries[1].event, "cancelled");
}

#[test]
fn unavailable_disconnect_and_timeout_remain_distinct_line_terminal_truth() {
    use super::session::TransportTermination;

    let source = include_str!("../../../../../../forms/desk-telegraph/main.conduit");
    for (termination, expected) in [
        (TransportTermination::Unavailable, "transport-unavailable"),
        (TransportTermination::Disconnected, "disconnected"),
        (TransportTermination::TimedOut, "timed-out"),
    ] {
        let ((mut source_session, source_output), _) = prepare_pair_for(source);
        assert!(matches!(source_output, Output::Line { .. }));
        let Output::Receipt { receipt, .. } =
            source_session.terminate_transport(termination, 71).unwrap()
        else {
            panic!("Line termination did not retain a receipt")
        };
        assert_eq!(receipt.disposition, expected);
        assert_eq!(receipt.deliveries.len(), 1);
        assert_eq!(receipt.deliveries[0].state, expected);
        assert_eq!(receipt.deliveries[0].failure_code, Some(71));
        assert!(receipt.deliveries[0].remote_receipt_hex.is_none());
        let transcript = receipt.transcript.as_ref().unwrap();
        assert_eq!(transcript.entries[0].event, "sent-record");
        assert_eq!(transcript.entries[1].event, expected);
    }
}

#[test]
fn partial_send_cancelled_before_receipt_remains_explicitly_undelivered() {
    let source = include_str!("../../../../../../forms/desk-telegraph/main.conduit");
    let ((mut source_session, source_output), _) = prepare_pair_for(source);
    let frame_bytes = match source_output {
        Output::Line { frame, .. } => frame.payload.len(),
        _ => panic!("Desk Telegraph did not offer its framed value"),
    };
    assert!(frame_bytes > 1);

    assert!(matches!(
        source_session
            .observe_partial_send(frame_bytes - 1)
            .unwrap(),
        Output::Waiting {
            phase: "partially-sent-awaiting-delivery",
            ..
        }
    ));
    let Output::Receipt { receipt, .. } = source_session.cancel().unwrap() else {
        panic!("partial undelivered send did not retain cancellation evidence")
    };
    assert_eq!(receipt.disposition, "cancelled");
    assert_eq!(receipt.transferred_values, 0);
    assert_eq!(receipt.deliveries.len(), 1);
    assert_eq!(receipt.deliveries[0].state, "partially-sent");
    assert_eq!(
        receipt.deliveries[0].sent_bytes,
        Some(u32::try_from(frame_bytes - 1).unwrap())
    );
    assert!(receipt.deliveries[0].remote_receipt_hex.is_none());
    let transcript = receipt.transcript.as_ref().unwrap();
    assert_eq!(transcript.entries[0].event, "sent-record");
    assert_eq!(transcript.entries[1].event, "cancelled");
}
