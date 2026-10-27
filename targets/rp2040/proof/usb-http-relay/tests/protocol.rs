use conduit_pico_usb_http_relay::*;
fn frame(kind: u8, id: u32, data: &[u8]) -> Vec<u8> {
    let mut bytes = header(kind, id, data.len()).unwrap().to_vec();
    bytes.extend_from_slice(data);
    bytes
}
#[test]
fn bench_ap_transitions_require_selected_unicast_peer_and_separate_stop() {
    for command in [TEST_AP_START, TEST_AP_STOP] {
        let payload = if command == TEST_AP_START {
            &[2, 3, 4, 5, 6, 7][..]
        } else {
            &[]
        };
        let mut decoder = Decoder::new();
        decoder.accept(&frame(command, 1, payload)).unwrap();
        assert_eq!(decoder.frame().unwrap().kind, command);
        decoder.reset();
        assert_eq!(
            decoder.accept(&frame(command, 2, b"SSID")),
            Err(Refusal::Capacity)
        );
    }
    assert_eq!(test_peer_mac(&[2, 3, 4, 5, 6, 7]), Ok([2, 3, 4, 5, 6, 7]));
    for invalid in [
        &[0; 6][..],
        &[255; 6],
        &[1, 2, 3, 4, 5, 6],
        &[2; 5],
        &[2; 7],
    ] {
        assert_eq!(test_peer_mac(invalid), Err(Refusal::Peer));
    }
}
#[test]
fn every_fragment_boundary_preserves_exact_request_bytes_without_heap_in_decoder() {
    let payload = (0..MAX_REQUEST)
        .map(|i| (i % 256) as u8)
        .collect::<Vec<_>>();
    let wire = frame(REQUEST, 1, &payload);
    for width in [1, 2, 3, 11, 12, 13, 63, 64, 65, 127, 512, 8192, 8204] {
        let mut decoder = Decoder::new();
        for part in wire.chunks(width) {
            decoder.accept(part).unwrap();
        }
        let decoded = decoder.frame().unwrap();
        assert_eq!(decoded.id, 1);
        assert_eq!(decoded.kind, REQUEST);
        assert_eq!(decoded.payload, payload);
        decoder.reset();
        assert!(!decoder.complete());
        assert!(!decoder.has_partial());
    }
    for split in 0..=wire.len() {
        let mut d = Decoder::new();
        d.accept(&wire[..split]).unwrap();
        d.accept(&wire[split..]).unwrap();
        assert_eq!(d.frame().unwrap().payload, payload);
    }
    assert!(core::mem::size_of::<Decoder>() < MAX_REQUEST + 64);
}
#[test]
fn malformed_overflow_truncation_and_pipeline_refuse_before_any_request_is_admitted() {
    let wire = frame(STATUS, 1, &[]);
    for end in 0..wire.len() {
        let mut d = Decoder::new();
        d.accept(&wire[..end]).unwrap();
        assert_eq!(d.frame().err(), Some(Refusal::Incomplete));
    }
    for offset in [0, 1, 2, 3, 4, 5, 10, 11] {
        let mut bytes = wire.clone();
        bytes[offset] = 255;
        let mut d = Decoder::new();
        assert!(d.accept(&bytes).is_err());
        d.reset();
        d.accept(&wire).unwrap();
        assert!(d.complete());
    }
    let mut d = Decoder::new();
    d.accept(&wire).unwrap();
    assert_eq!(d.accept(&wire), Err(Refusal::Pipeline));
    assert_eq!(header(REQUEST, 1, MAX_REQUEST + 1), Err(Refusal::Capacity));
    let mut session = Session::default();
    assert_eq!(session.admit(0), Err(Refusal::Stale));
    session.admit(1).unwrap();
    assert_eq!(session.admit(1), Err(Refusal::Stale));
    session.admit(u32::MAX).unwrap();
    assert_eq!(session.admit(2), Err(Refusal::Stale));
    session.reset();
    session.admit(1).unwrap();
}
#[test]
fn ssid_and_password_are_exact_bounded_borrowed_bytes_and_never_implicit_defaults() {
    let mut data = vec![4, 8];
    data.extend_from_slice(b"ESP!");
    data.extend_from_slice(b"password");
    let c = credentials(&data).unwrap();
    assert_eq!(c.ssid, "ESP!");
    assert_eq!(c.password, b"password");
    let mut open = vec![32, 0];
    open.extend_from_slice(&[b's'; 32]);
    assert_eq!(credentials(&open).unwrap().password, []);
    for bytes in [
        &[][..],
        &[0, 0],
        &[33, 0],
        &[1, 7, b's'],
        &[1, 64, b's'],
        &[1, 0, 0],
        &[1, 0, 255],
    ] {
        assert!(credentials(bytes).is_err());
    }
    data.push(0);
    assert!(credentials(&data).is_err());
}
#[test]
fn fixed_peer_admission_rejects_other_networks_routers_and_special_addresses() {
    for last in 2..=254 {
        assert!(peer_network([192, 168, 4, last], 24, None));
        assert!(peer_network(
            [192, 168, 4, last],
            24,
            Some([192, 168, 4, 1])
        ));
    }
    for last in [0, 1, 255] {
        assert!(!peer_network(
            [192, 168, 4, last],
            24,
            Some([192, 168, 4, 1])
        ));
    }
    for (ip, prefix, gateway) in [
        ([192, 168, 1, 2], 24, Some([192, 168, 4, 1])),
        ([192, 168, 4, 2], 16, Some([192, 168, 4, 1])),
        ([192, 168, 4, 2], 25, None),
        ([192, 168, 4, 2], 24, Some([192, 168, 4, 2])),
    ] {
        assert!(!peer_network(ip, prefix, gateway));
    }
}

#[test]
fn bootsel_control_requires_explicit_kind_exact_current_build_and_fresh_sequence() {
    assert_eq!(BUILD_ID.len(), 64);
    assert!(BUILD_ID
        .bytes()
        .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()));
    assert!(bootsel_matches(BUILD_ID.as_bytes()));
    let mut wrong = *BUILD_ID.as_bytes().first_chunk::<64>().unwrap();
    wrong[0] = if wrong[0] == b'0' { b'1' } else { b'0' };
    assert!(!bootsel_matches(&wrong));
    assert!(!bootsel_matches(&BUILD_ID.as_bytes()[..63]));
    for (kind, payload) in [
        (BOOTSEL_QUERY, &b""[..]),
        (BOOTSEL_REQUEST, BUILD_ID.as_bytes()),
    ] {
        let mut d = Decoder::new();
        d.accept(&header(kind, 1, payload.len()).unwrap()).unwrap();
        d.accept(payload).unwrap();
        assert_eq!(d.frame().unwrap().kind, kind);
    }
    let mut session = Session::default();
    session.admit(1).unwrap();
    assert!(session.admit(1).is_err());
    session.admit(2).unwrap();
}

#[test]
fn sensor_status_is_an_explicit_empty_control_request_independent_of_join() {
    for kind in [SENSOR_STATUS, SENSOR_DIAGNOSTIC] {
        let mut decoder = Decoder::new();
        decoder.accept(&header(kind, 1, 0).unwrap()).unwrap();
        assert_eq!(decoder.frame().unwrap().kind, kind);
        let mut invalid = Decoder::new();
        assert!(invalid.accept(&header(kind, 2, 1).unwrap()).is_err());
    }
}

#[test]
fn mdns_observation_is_explicit_bounded_and_uses_fresh_usb_sequence() {
    for length in [9, 56] {
        let mut decoder = Decoder::new();
        decoder
            .accept(&header(MDNS_QUERY, 1, length).unwrap())
            .unwrap();
        decoder.accept(&vec![0; length]).unwrap();
        assert_eq!(decoder.frame().unwrap().kind, MDNS_QUERY);
    }
    for length in [0, 8, 57, MAX_REQUEST] {
        let mut decoder = Decoder::new();
        assert!(decoder
            .accept(&header(MDNS_QUERY, 1, length).unwrap())
            .is_err());
    }
    let mut session = Session::default();
    session.admit(1).unwrap();
    assert!(session.admit(1).is_err());
}
