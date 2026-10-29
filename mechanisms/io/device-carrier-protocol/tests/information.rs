use conduit_device_carrier_protocol::*;
fn wire(kind: u8, id: u32, payload: &[u8]) -> Vec<u8> {
    let mut b = header(kind, id, payload.len()).unwrap().to_vec();
    b.extend_from_slice(payload);
    b
}
#[test]
fn status_response_survives_every_split_and_duplicate_completion_is_refused() {
    let payload = br#"{"role":"conduit.environment-wifi-carrier@1"}"#;
    let expected = wire(131, 1, payload);
    for split in 0..=expected.len() {
        let mut session = InformationSession::default();
        let request = session.begin(3).unwrap();
        assert_eq!(request.id(), 1);
        assert_eq!(request.command(), 3);
        let mut decoder = InformationDecoder::<524>::new(request).unwrap();
        decoder.accept(&session, &expected[..split]).unwrap();
        decoder.accept(&session, &expected[split..]).unwrap();
        assert!(decoder.complete());
        assert_eq!(decoder.finish(&mut session).unwrap().payload, payload);
        assert_eq!(decoder.finish(&mut session).unwrap_err(), Refusal::Stale);
    }
}
#[test]
fn response_identity_direction_capacity_and_pipeline_are_fail_closed() {
    for (kind, id, payload, expected) in [
        (131, 2, &b"{}"[..], Refusal::Stale),
        (3, 1, &b"{}"[..], Refusal::Kind),
        (131, 1, &b""[..], Refusal::Capacity),
    ] {
        let mut session = InformationSession::default();
        let mut decoder = InformationDecoder::<524>::new(session.begin(3).unwrap()).unwrap();
        assert_eq!(
            decoder.accept(&session, &wire(kind, id, payload)),
            Err(expected)
        );
        assert_eq!(decoder.finish(&mut session).unwrap_err(), Refusal::Stale);
    }
    let mut session = InformationSession::default();
    let mut decoder = InformationDecoder::<524>::new(session.begin(3).unwrap()).unwrap();
    assert_eq!(
        decoder.accept(&session, &header(131, 1, 513).unwrap()),
        Err(Refusal::Capacity)
    );
    let mut session = InformationSession::default();
    let mut decoder = InformationDecoder::<524>::new(session.begin(3).unwrap()).unwrap();
    let mut pipeline = wire(131, 1, b"{}");
    pipeline.extend_from_slice(&wire(131, 1, b"{}"));
    assert_eq!(decoder.accept(&session, &pipeline), Err(Refusal::Pipeline));
}
#[test]
fn old_request_ids_and_old_local_session_handles_never_become_current() {
    let mut session = InformationSession::default();
    let old = session.begin(3).unwrap();
    let old_id = old.id();
    let mut old_decoder = InformationDecoder::<524>::new(old).unwrap();
    assert!(matches!(session.begin(3), Err(Refusal::Pipeline)));
    session.advance_session().unwrap();
    assert_eq!(
        old_decoder.accept(&session, &wire(131, old_id, b"{}")),
        Err(Refusal::Stale)
    );
    let new = session.begin(3).unwrap();
    assert!(new.id() > old_id);
    let mut decoder = InformationDecoder::<524>::new(new).unwrap();
    assert_eq!(
        decoder.accept(&session, &wire(131, old_id, b"{}")),
        Err(Refusal::Stale)
    );
}
#[test]
fn sensor_status_and_error_have_their_actual_distinct_bounds() {
    let mut session = InformationSession::default();
    let mut decoder = InformationDecoder::<1548>::new(session.begin(6).unwrap()).unwrap();
    let payload = vec![b'x'; 1536];
    decoder.accept(&session, &wire(131, 1, &payload)).unwrap();
    assert_eq!(decoder.finish(&mut session).unwrap().payload, payload);
    let request = session.begin(7).unwrap();
    assert!(matches!(
        InformationDecoder::<524>::new(request),
        Err(Refusal::Capacity)
    ));
    session.advance_session().unwrap();
    let request = session.begin(3).unwrap();
    let id = request.id();
    let mut decoder = InformationDecoder::<524>::new(request).unwrap();
    decoder
        .accept(&session, &wire(130, id, b"refused"))
        .unwrap();
    let response = decoder.finish(&mut session).unwrap();
    assert_eq!(response.kind, 130);
    assert_eq!(response.payload, b"refused");
    assert!(matches!(session.begin(5), Err(Refusal::Kind)));
}
#[test]
fn partial_response_cannot_finish_or_open_another_request() {
    let mut session = InformationSession::default();
    let mut decoder = InformationDecoder::<524>::new(session.begin(3).unwrap()).unwrap();
    let bytes = wire(131, 1, b"{}");
    decoder.accept(&session, &bytes[..HEADER + 1]).unwrap();
    assert_eq!(
        decoder.finish(&mut session).unwrap_err(),
        Refusal::Incomplete
    );
    assert!(matches!(session.begin(3), Err(Refusal::Pipeline)));
    decoder.accept(&session, &bytes[HEADER + 1..]).unwrap();
    assert_eq!(decoder.finish(&mut session).unwrap().payload, b"{}");
    assert!(session.begin(3).is_ok());
}
