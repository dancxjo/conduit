//! Pure bounded DNS parser; this is not a radio proof.
#[path = "../src/bench_mdns.rs"]
mod mdns;
fn payload() -> Vec<u8> {
    let mut bytes = vec![0x12, 0x34, 3, 4, 5, 6, 7, 8];
    bytes.extend_from_slice(b"conduit-climate-b0a732286178.local");
    bytes
}
fn answer(query: &mdns::Query<'_>) -> Vec<u8> {
    let mut out = [0; 512];
    let n = query.packet(&mut out);
    let mut bytes = out[..n].to_vec();
    bytes[2] = 0x84;
    bytes[7] = 1;
    bytes.extend_from_slice(&[0xc0, 12, 0, 1, 0, 1, 0, 0, 0, 10, 0, 4, 192, 168, 4, 2]);
    bytes
}
#[test]
fn exact_correlated_answer_and_fail_closed_extents() {
    let bytes = payload();
    let query = mdns::Query::parse(&bytes, [0xb0, 0xa7, 0x32, 0x28, 0x61, 0x78]).unwrap();
    let packet = answer(&query);
    assert!(query
        .answer(&packet, [192, 168, 4, 2], 5353, [192, 168, 4, 2])
        .is_ok());
    for length in 0..packet.len() {
        assert!(query
            .answer(&packet[..length], [192, 168, 4, 2], 5353, [192, 168, 4, 2])
            .is_err());
    }
    for (at, value) in [
        (0, 0),
        (2, 0x86),
        (5, 2),
        (7, 0),
        (packet.len() - 1, 3),
        (packet.len() - 7, 0),
    ] {
        let mut invalid = packet.clone();
        invalid[at] = value;
        assert!(
            query
                .answer(&invalid, [192, 168, 4, 2], 5353, [192, 168, 4, 2])
                .is_err(),
            "mutation {at}"
        );
    }
    assert!(query
        .answer(&packet, [192, 168, 4, 3], 5353, [192, 168, 4, 2])
        .is_err());
    assert!(query
        .answer(&packet, [192, 168, 4, 2], 53, [192, 168, 4, 2])
        .is_err());
    let mut trailing = packet.clone();
    trailing.push(0);
    assert!(query
        .answer(&trailing, [192, 168, 4, 2], 5353, [192, 168, 4, 2])
        .is_err());
    let mut pointer = packet.clone();
    let at = pointer.len() - 16;
    pointer[at] = 0xc0;
    pointer[at + 1] = at as u8;
    assert!(query
        .answer(&pointer, [192, 168, 4, 2], 5353, [192, 168, 4, 2])
        .is_err());
    for cut in 0..bytes.len() {
        assert!(mdns::Query::parse(&bytes[..cut], [0xb0, 0xa7, 0x32, 0x28, 0x61, 0x78]).is_err());
    }
    assert!(mdns::Query::parse(&bytes, [0xb0, 0xa7, 0x32, 0x28, 0x61, 0x79]).is_err());
}

#[path = "../../../../esp32/proof/tm1637/src/native_mdns_wire.rs"]
mod esp_wire;
#[test]
fn actual_esp_legacy_encoder_matches_observer_without_radio_claim() {
    let bytes = payload();
    let query = mdns::Query::parse(&bytes, [0xb0, 0xa7, 0x32, 0x28, 0x61, 0x78]).unwrap();
    let mut request = [0; 512];
    let length = query.packet(&mut request);
    let name = esp_wire::Name::new("conduit-climate-b0a732286178").unwrap();
    let interest = esp_wire::inspect(&request[..length], &name, [192, 168, 4, 2]).unwrap();
    let mut response = [0; 512];
    let length = esp_wire::answer(
        &name,
        [192, 168, 4, 2],
        Some((&request[..length], interest)),
        &mut response,
    )
    .unwrap();
    query
        .answer(
            &response[..length],
            [192, 168, 4, 2],
            5353,
            [192, 168, 4, 2],
        )
        .unwrap();
}
