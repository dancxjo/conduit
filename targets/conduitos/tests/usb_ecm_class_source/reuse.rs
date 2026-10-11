use super::*;
fn received(s: &PreparedProtocolSource, ordinal: u64, wire: &[u8]) -> StructuredInfoValue {
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
                    (
                        "actual",
                        leaf(field(ty, "actual"), &(wire.len() as u64).to_le_bytes()),
                    ),
                    (
                        "short",
                        leaf(field(ty, "short"), &[u8::from(wire.len() < 1514)]),
                    ),
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
        let tx = frame(if ordinal == 0 { 64 } else { 14 });
        let rx = frame(1514 - ordinal as usize);
        state = step(&s, &state, &transmit(&s, ordinal, &tx));
        let req = eval(&program(&s, "cdc-ecm-write-request"), &state);
        assert_eq!(
            member(&req, "wire"),
            leaf(field(req.value_type(), "wire"), &tx)
        );
        state = step(&s, &state, &written(&s, tx.len() as u64));
        if number(member(&state, "phase")) == 8 {
            state = step(&s, &state, &written(&s, 0));
        }
        state = step(&s, &state, &queue(&s, "acknowledged"));
        state = step(&s, &state, &received(&s, ordinal, &rx));
        assert_eq!(number(member(&state, "phase")), 11);
        let req = eval(&program(&s, "cdc-ecm-queue-request"), &state);
        let StructuredInfoValueShape::Variant { tag, payload } = req.shape() else {
            panic!()
        };
        assert_eq!(tag, "deliver-receive");
        assert_eq!(
            member(payload, "wire"),
            leaf(field(payload.value_type(), "wire"), &rx)
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
