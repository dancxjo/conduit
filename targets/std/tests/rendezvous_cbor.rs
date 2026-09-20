use conduit_body::{decode_running_host_rendezvous_cbor, RendezvousLineFamily};

fn fixture() -> Vec<u8> {
    include_str!("../../../architecture/body/schemas/running-host-rendezvous-v1.hex")
        .trim()
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|digits| {
            let digit = |value| match value {
                b'0'..=b'9' => value - b'0',
                b'a'..=b'f' => value - b'a' + 10,
                _ => panic!("fixture is lowercase hexadecimal"),
            };
            (digit(digits[0]) << 4) | digit(digits[1])
        })
        .collect()
}

#[test]
fn std_host_consumes_the_checked_target_neutral_descriptor() {
    let fixture = fixture();
    let descriptor = decode_running_host_rendezvous_cbor(&fixture, 1_700_000_000_000).unwrap();
    let families: Vec<_> = descriptor
        .candidates()
        .map(|candidate| candidate.line_family)
        .collect();
    assert_eq!(
        families,
        vec![
            RendezvousLineFamily::AuthenticatedTlsStream,
            RendezvousLineFamily::AuthenticatedConduitLine,
        ]
    );
    assert_eq!(descriptor.copy_session_secret_for_attempt(), [0x55; 32]);
}
