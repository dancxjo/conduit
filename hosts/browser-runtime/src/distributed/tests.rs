use super::*;

fn trigger(sink: &mut DistributedSink) {
    let binding = sink.binding.clone();
    sink.session
        .admit_inbound(binding.hello_frame())
        .expect("peer hello");
    sink.session
        .admit_outbound(binding.frame(SessionMessage::Ready))
        .expect("local ready");
    sink.session
        .admit_inbound(binding.frame(SessionMessage::Ready))
        .expect("peer ready");
    sink.clear_output();
    assert!(sink.session.is_active());
}

fn offered(binding: &SessionBinding, sequence: u64) -> Vec<u8> {
    let payload = conduit_signal::encode_signal(&conduit_signal::Signal {
        sequence,
        level: sequence % 2 == 1,
    });
    let mut frame = [0_u8; FRAME_CAPACITY];
    let length = encode_session_frame_into(
        binding.frame(SessionMessage::Offered {
            sequence,
            payload: &payload.encoded,
        }),
        &mut frame,
        SIGNAL_ENCODED_LEN,
        DISTRIBUTED_MAXIMUM_FRAME_BYTES,
    )
    .unwrap();
    frame[..length].to_vec()
}

#[test]
fn browser_reconstructs_the_exact_sink_fragment_and_session() {
    let sink =
        DistributedSink::prepare(None, PlanKind::StdBrowser, None, None).expect("sink prepares");
    assert_eq!(sink.fragment.placements.len(), 1);
    assert_eq!(sink.lowered.remote_endpoints.len(), 1);
    assert_eq!(sink.binding.plan_id, sink.fragment.plan_id);
    assert_eq!(sink.binding.sink_fragment_id, sink.fragment.fragment_id);
    assert_eq!(sink.capacity_seal(), sink.seal);
}

#[test]
fn browser_reconstructs_a_dynamically_identified_native_source() {
    let host = HostId::from("patchbay-native/process-7");
    let boot = BootId::from("patchbay-native/boot-9");
    let sink = DistributedSink::prepare(
        None,
        PlanKind::StdBrowser,
        Some((host.clone(), boot.clone())),
        None,
    )
    .expect("sink prepares");
    assert_eq!(sink.binding.source.host_id, host);
    assert_eq!(sink.binding.source.boot_id, boot);
    let exact = exact_distributed_signal_plan_for(
        sink.binding.source.host_id.clone(),
        sink.binding.source.boot_id.clone(),
    )
    .expect("same exact plan");
    assert_eq!(sink.binding.plan_id, exact.plan.plan_id);
}

#[test]
fn launched_browser_boot_is_plan_truth_and_wrong_session_refuses() {
    let source = (
        HostId::from("product/std/7"),
        BootId::from("product/std/7/boot"),
    );
    let browser = (
        HostId::from("product/browser/7"),
        BootId::from("product/browser/7/boot"),
    );
    let mut sink = DistributedSink::prepare(
        None,
        PlanKind::StdBrowser,
        Some(source),
        Some(browser.clone()),
    )
    .expect("fresh browser sink prepares");
    assert_eq!(sink.binding.sink.host_id, browser.0);
    assert_eq!(sink.binding.sink.boot_id, browser.1);
    let mut wrong = sink.binding.clone();
    wrong.plan_id = "stale-browser-session".into();
    assert_eq!(
        sink.session.admit_inbound(wrong.hello_frame()),
        Err(conduit_wire::WireError::PlanMismatch)
    );
}

#[test]
fn stale_completion_from_replaced_browser_boot_refuses_current_truth() {
    let source = (
        HostId::from("product/std/7"),
        BootId::from("product/std/7/boot"),
    );
    let mut old = DistributedSink::prepare(
        None,
        PlanKind::StdBrowser,
        Some(source.clone()),
        Some((
            HostId::from("product/browser/old"),
            BootId::from("product/browser/old/boot"),
        )),
    )
    .expect("old browser sink prepares");
    trigger(&mut old);
    let old_binding = old.binding.clone();
    old.ingest(&offered(&old_binding, 0))
        .expect("old value admits");
    old.advance().expect("old value delivered");
    old.hold_first_value = false;
    old.advance().expect("old presentation prepares");
    let old_completion = old.expected_completion[..old.expected_completion_len].to_vec();

    let mut current = DistributedSink::prepare(
        None,
        PlanKind::StdBrowser,
        Some(source),
        Some((
            HostId::from("product/browser/current"),
            BootId::from("product/browser/current/boot"),
        )),
    )
    .expect("replacement browser sink prepares");
    trigger(&mut current);
    let current_binding = current.binding.clone();
    current
        .ingest(&offered(&current_binding, 0))
        .expect("current value admits");
    current.advance().expect("current value delivered");
    current.hold_first_value = false;
    current.advance().expect("current presentation prepares");

    assert_eq!(
        current.complete_presentation(&old_completion),
        Err(ERROR_PRESENTATION)
    );
}

#[test]
fn triple_browser_reconstructs_its_fragment_from_the_same_three_host_plan() {
    let sink =
        DistributedSink::prepare(None, PlanKind::Triple, None, None).expect("triple sink prepares");
    let exact = conduit_signal_conformance::triple::exact_plan().expect("triple plan resolves");
    assert_eq!(sink.fragment.host_id, exact.browser_advertisement.host_id);
    assert_eq!(sink.binding.plan_id, exact.plan.plan_id);
    assert_eq!(
        sink.binding.attachment.link_binding_id,
        exact.browser_line.binding.binding_id
    );
    assert_eq!(
        sink.binding.attachment.base_instance_id,
        exact.browser_line.binding.base_instance_id
    );
    assert_eq!(sink.lowered.remote_endpoints.len(), 1);
    assert_eq!(sink.capacity_seal(), sink.seal);
}

#[test]
fn sign_exhaustion_is_structured_before_remote_admission_changes_sequence() {
    let mut sink = DistributedSink::prepare(Some(1), PlanKind::StdBrowser, None, None)
        .expect("small sign sink prepares");
    trigger(&mut sink);
    let binding = sink.binding.clone();
    sink.ingest(&offered(&binding, 0)).expect("first admits");
    sink.advance().expect("first delivered");
    sink.advance().expect("first held for pressure");
    assert_eq!(sink.ingest(&offered(&binding, 1)), Err(ERROR_SIGN));
    assert_eq!(sink.output_kind, OUTPUT_SESSION);
    assert_eq!(sink.output[5], 8, "structured Failed frame");
    assert_eq!(sink.advance(), Err(ERROR_SIGN));
    assert_eq!(sink.output[5], 9, "sign terminal frame");
    assert_eq!(
        sink.scheduler.values().used_items(),
        1,
        "sign exhaustion preserves rather than silently releases the admitted value"
    );
    assert_eq!(sink.capacity_seal(), sink.seal);
}

#[test]
fn browser_sink_failure_and_cancellation_emit_structured_terminal_frames() {
    let mut failed =
        DistributedSink::prepare(None, PlanKind::Triple, None, None).expect("sink prepares");
    trigger(&mut failed);
    let binding = failed.binding.clone();
    failed.ingest(&offered(&binding, 0)).expect("value admits");
    failed.advance().expect("value delivered");
    failed.hold_first_value = false;
    failed.advance().expect("presentation prepared");
    let mut completion = failed.expected_completion[..failed.expected_completion_len].to_vec();
    completion.push(0);
    assert_eq!(
        failed.complete_presentation(&completion),
        Err(ERROR_PRESENTATION)
    );
    assert_eq!(failed.output[5], 8, "sink failure frame");
    assert_eq!(failed.advance(), Err(ERROR_PRESENTATION));
    assert_eq!(failed.output[5], 9, "failed terminal frame");
    assert_eq!(failed.scheduler.values().used_items(), 0);

    let mut cancelled =
        DistributedSink::prepare(None, PlanKind::Triple, None, None).expect("sink prepares");
    trigger(&mut cancelled);
    assert_eq!(cancelled.cancel(), Err(ERROR_CANCELLED));
    assert_eq!(cancelled.output[5], 7, "cancelled frame");
    assert_eq!(cancelled.advance(), Err(ERROR_CANCELLED));
    assert_eq!(cancelled.output[5], 9, "cancelled terminal frame");
    assert_eq!(cancelled.scheduler.values().used_items(), 0);
}

#[test]
fn triple_browser_rejects_a_malformed_live_frame_without_admission() {
    let mut sink =
        DistributedSink::prepare(None, PlanKind::Triple, None, None).expect("sink prepares");
    trigger(&mut sink);
    assert_eq!(sink.ingest(&[0_u8; 8]), Err(ERROR_SESSION));
    assert_eq!(sink.session.next_sequence(), 0);
    assert_eq!(sink.scheduler.values().used_items(), 0);
}

#[test]
fn wrong_remote_completion_identity_fails_closed() {
    let mut sink =
        DistributedSink::prepare(None, PlanKind::StdBrowser, None, None).expect("sink prepares");
    trigger(&mut sink);
    let binding = sink.binding.clone();
    sink.ingest(&offered(&binding, 0)).expect("value admits");
    sink.advance().expect("value delivered");
    sink.hold_first_value = false;
    sink.advance().expect("presentation prepared");
    let mut completion = sink.expected_completion[..sink.expected_completion_len].to_vec();
    let identity = binding.attachment.link_binding_id.as_str().as_bytes();
    let offset = completion
        .windows(identity.len())
        .position(|window| window == identity)
        .expect("completion carries the exact link identity");
    completion[offset] ^= 1;
    completion.push(1);
    assert_eq!(
        sink.complete_presentation(&completion),
        Err(ERROR_PRESENTATION)
    );
    assert_eq!(
        sink.output[5], 8,
        "identity failure is a structured Failed frame"
    );
}
