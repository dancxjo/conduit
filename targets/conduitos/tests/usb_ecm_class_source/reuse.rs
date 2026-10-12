use super::*;
fn received(s: &PreparedProtocolSource, ordinal: u64, wire: &[u8]) -> StructuredInfoValue {
    received_result(s, ordinal, wire, wire.len() as u64, true)
}
fn received_result(
    s: &PreparedProtocolSource,
    ordinal: u64,
    wire: &[u8],
    actual: u64,
    short: bool,
) -> StructuredInfoValue {
    let p = program(s, "cdc-ecm-read-event");
    let ty = case(&p.input_type, "completed");
    eval(
        &p,
        &variant(
            &p.input_type,
            "completed",
            record(
                ty,
                vec![
                    (
                        "ordinal",
                        leaf(field(ty, "ordinal"), &ordinal.to_le_bytes()),
                    ),
                    ("actual", leaf(field(ty, "actual"), &actual.to_le_bytes())),
                    ("short", leaf(field(ty, "short"), &[u8::from(short)])),
                    ("wire", leaf(field(ty, "wire"), wire)),
                ],
            ),
        ),
    )
}
#[test]
fn sixteen_actual_source_cycles_keep_bytes_ordinals_and_terminal_fence_exact() {
    let s = source();
    let mut state = phase(&start(&s, &[]), 6);
    for ordinal in 0..16 {
        let tx = frame(if ordinal == 0 { 64 } else { 1514 });
        let rx = frame(1514 - ordinal as usize);
        state = step(&s, &state, &transmit(&s, ordinal, &tx));
        let req = eval(&program(&s, "cdc-ecm-write-request"), &state);
        assert_eq!(
            member(&req, "wire"),
            &leaf(field(req.value_type(), "wire"), &tx)
        );
        state = step(&s, &state, &written(&s, tx.len() as u64));
        if number(member(&state, "phase")) == 8 {
            state = step(&s, &state, &written(&s, 0));
        }
        state = step(&s, &state, &queue(&s, "acknowledged"));
        state = step(&s, &state, &queue(&s, "pressure"));
        state = step(&s, &state, &received(&s, ordinal, &rx));
        assert_eq!(number(member(&state, "phase")), 11);
        let req = eval(&program(&s, "cdc-ecm-queue-request"), &state);
        let StructuredInfoValueShape::Variant { tag, payload } = req.shape() else {
            panic!()
        };
        assert_eq!(tag, "deliver-receive");
        assert_eq!(
            member(payload, "wire"),
            &leaf(field(payload.value_type(), "wire"), &rx)
        );
        state = step(&s, &state, &queue(&s, "accepted"));
        assert_eq!(number(member(&state, "cycles")), ordinal + 1);
    }
    assert_eq!(tag(&eval(&program(&s, "cdc-ecm-route"), &state)), "stopped");
}
#[test]
fn read_wrong_ordinal_and_malformed_frame_terminate_before_network_delivery() {
    let s = source();
    let state = phase(&start(&s, &[]), 10);
    for event in [received(&s, 1, &frame(14)), received(&s, 0, &[0; 13])] {
        let stopped = step(&s, &state, &event);
        assert_eq!(tag(member(&stopped, "outcome")), "malformed");
        assert_eq!(
            tag(&eval(&program(&s, "cdc-ecm-route"), &stopped)),
            "stopped"
        );
    }
}

#[test]
fn handshake_ack_does_not_block_the_next_queued_transmit_on_an_unrequested_receive() {
    let s = source();
    let mut state = phase(&start(&s, &[]), 6);
    for ordinal in 0..2 {
        state = step(&s, &state, &transmit(&s, ordinal, &frame(54)));
        state = step(&s, &state, &written(&s, 54));
        state = step(&s, &state, &queue(&s, "acknowledged"));
        assert_eq!(number(member(&state, "phase")), 6);
        assert_eq!(number(member(&state, "transmitted")), ordinal + 1);
        assert_eq!(number(member(&state, "cycles")), 0);
    }
    state = step(&s, &state, &queue(&s, "pressure"));
    assert_eq!(number(member(&state, "phase")), 10);
    state = step(&s, &state, &received(&s, 0, &frame(60)));
    state = step(&s, &state, &queue(&s, "accepted"));
    assert_eq!(number(member(&state, "transmitted")), 2);
    assert_eq!(number(member(&state, "cycles")), 1);
}

#[test]
fn full_mtu_read_event_fits_admitted_bound_and_preserves_metadata_and_wire() {
    let s = source();
    let wire = frame(1514);
    let event = received(&s, 15, &wire);
    let encoded = event.canonical_bytes().unwrap();
    assert!(
        encoded.len() <= 4096,
        "read event requires {} bytes",
        encoded.len()
    );
    let StructuredInfoValueShape::Variant { tag, payload } = event.shape() else {
        panic!("read event")
    };
    assert_eq!(tag, "read");
    assert_eq!(number(member(payload, "ordinal")), 15);
    assert_eq!(number(member(payload, "actual")), 1514);
    assert_eq!(
        member(payload, "short"),
        &leaf(field(payload.value_type(), "short"), &[1])
    );
    let StructuredInfoValueShape::Variant {
        payload: request, ..
    } = member(payload, "gate").shape()
    else {
        panic!("receive gate")
    };
    let StructuredInfoValueShape::Variant {
        payload: delivered, ..
    } = request.shape()
    else {
        panic!("receive request")
    };
    assert_eq!(
        member(delivered, "wire"),
        &leaf(field(delivered.value_type(), "wire"), &wire)
    );
    assert_eq!(
        StructuredInfoValue::from_canonical_bytes(&encoded).unwrap(),
        event
    );
}

#[test]
fn transmit_budget_is_independent_of_receive_and_refuses_seventeenth_usb_write() {
    let s = source();
    let mut state = phase(&start(&s, &[]), 6);
    for ordinal in 0..16 {
        state = step(&s, &state, &transmit(&s, ordinal, &frame(1514)));
        assert_eq!(tag(&eval(&program(&s, "cdc-ecm-route"), &state)), "write");
        state = step(&s, &state, &written(&s, 1514));
        state = step(&s, &state, &queue(&s, "acknowledged"));
        assert_eq!(number(member(&state, "transmitted")), ordinal + 1);
        assert_eq!(number(member(&state, "cycles")), 0);
        assert_eq!(tag(member(&state, "transmit")), "pressure");
    }
    let stopped = step(&s, &state, &transmit(&s, 16, &frame(1514)));
    assert_eq!(
        tag(&eval(&program(&s, "cdc-ecm-route"), &stopped)),
        "stopped"
    );
    assert_eq!(number(member(&stopped, "transmitted")), 16);
    assert_eq!(number(member(&stopped, "cycles")), 0);
    assert_eq!(tag(member(&stopped, "outcome")), "malformed");
    state = step(&s, &state, &queue(&s, "pressure"));
    state = step(&s, &state, &received(&s, 0, &frame(1514)));
    state = step(&s, &state, &queue(&s, "accepted"));
    assert_eq!(number(member(&state, "transmitted")), 16);
    assert_eq!(number(member(&state, "cycles")), 1);
    assert_eq!(tag(member(&state, "receive")), "take-transmit");
}

#[test]
fn receive_sentinel_refuses_oversized_prefix_and_missing_frame_termination_before_delivery() {
    let s = source();
    let state = phase(&start(&s, &[]), 10);
    let request = eval(&program(&s, "cdc-ecm-read-request"), &state);
    assert_eq!(number(member(&request, "length")), 1515);
    let mtu = frame(1514);
    let oversized = frame(1515);
    for (wire, actual, short) in [
        (mtu.as_slice(), 1514, false),
        (oversized.as_slice(), 1515, false),
        (oversized.as_slice(), 1515, true),
        (mtu.as_slice(), 1513, true),
        (&[][..], 0, true),
        (&[0; 13][..], 13, true),
    ] {
        let event = received_result(&s, 0, wire, actual, short);
        let stopped = step(&s, &state, &event);
        assert_eq!(tag(member(&stopped, "outcome")), "malformed");
        assert_eq!(
            tag(&eval(&program(&s, "cdc-ecm-route"), &stopped)),
            "stopped"
        );
        assert_eq!(number(member(&stopped, "cycles")), 0);
    }
}

#[test]
fn short_terminated_mtu_and_packet_aligned_frames_preserve_separate_ordinals() {
    let s = source();
    let mut state = phase(&start(&s, &[]), 10);
    for (ordinal, length) in [(0, 1514), (1, 64)] {
        let wire = frame(length);
        let event = received_result(&s, ordinal, &wire, length as u64, true);
        state = step(&s, &state, &event);
        assert_eq!(number(member(&state, "phase")), 11);
        let request = eval(&program(&s, "cdc-ecm-queue-request"), &state);
        assert_eq!(tag(&request), "deliver-receive");
        state = step(&s, &state, &queue(&s, "accepted"));
        assert_eq!(number(member(&state, "cycles")), ordinal + 1);
        state = step(&s, &state, &queue(&s, "pressure"));
        assert_eq!(number(member(&state, "phase")), 10);
    }
}
