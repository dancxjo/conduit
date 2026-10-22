//! Protocol boundary and finite credential ownership; no radio or authority fixture.
use crate::network_setup::*;
use crate::tests::allocation_probe;
fn limits() -> NetworkSetupLimits {
    NetworkSetupLimits {
        minimum_credential_bytes: 8,
        maximum_credential_bytes: 63,
    }
}
fn input() -> NetworkSetupInput {
    NetworkSetupInput::from_form(b"ssid=Test+LAN&password=sample%26value%2B", limits()).unwrap()
}
#[test]
fn page_submission_crosses_the_existing_private_wire_only_after_response_acknowledgement() {
    let mut mailbox = NetworkSetupMailbox::new(2);
    let mut output = [0; conduit_net::MAXIMUM_JOIN_INPUT_BYTES as usize];
    let ((), allocation) = allocation_probe::observe(|| {
        let ticket = mailbox.submit(input()).unwrap();
        assert!(!mailbox.has_acknowledged());
        assert_eq!(
            mailbox.take_acknowledged(&mut output),
            Err(NetworkSetupRefusal::NotAcknowledged)
        );
        assert!(matches!(
            mailbox.submit(input()),
            Err(NetworkSetupRefusal::Busy)
        ));
        mailbox.acknowledge(ticket).unwrap();
        assert!(mailbox.has_acknowledged());
        assert_eq!(
            mailbox.acknowledge(ticket),
            Err(NetworkSetupRefusal::AlreadyAcknowledged)
        );
        assert!(matches!(
            mailbox.take_acknowledged(&mut [0; 1]),
            Err(NetworkSetupRefusal::Wire(_))
        ));
        let length = mailbox.take_acknowledged(&mut output).unwrap();
        assert!(!mailbox.has_acknowledged());
        let request = conduit_net::decode_network_join_request(&output[..length]).unwrap();
        assert_eq!(request.ssid, b"Test LAN");
        assert_eq!(request.credential, b"sample&value+");
        assert_eq!(
            mailbox.take_acknowledged(&mut output),
            Err(NetworkSetupRefusal::NoSubmission)
        );
        let next = mailbox.submit(input()).unwrap();
        assert_eq!(
            mailbox.acknowledge(ticket),
            Err(NetworkSetupRefusal::StaleSubmission)
        );
        assert_eq!(
            mailbox.cancel(ticket),
            Err(NetworkSetupRefusal::StaleSubmission)
        );
        mailbox.cancel(next).unwrap();
        assert_eq!(
            mailbox.take_acknowledged(&mut output),
            Err(NetworkSetupRefusal::NoSubmission)
        );
        assert!(matches!(
            mailbox.submit(input()),
            Err(NetworkSetupRefusal::Exhausted)
        ));
    });
    assert_eq!((allocation.allocations, allocation.reallocations), (0, 0));
}
#[test]
fn malformed_duplicate_unknown_and_oversized_fields_are_refused_before_submission() {
    use NetworkSetupRefusal as E;
    for (body, expected) in [
        (&b"ssid=x&password=12345678&ssid=y"[..], E::DuplicateField),
        (
            &b"ssid=x&password=12345678&password=87654321"[..],
            E::DuplicateField,
        ),
        (
            &b"ssid=x&password=12345678&redirect=outside"[..],
            E::UnknownField,
        ),
        (&b"ssid=x&password=12345678%"[..], E::MalformedForm),
        (&b"ssid=%zz&password=12345678"[..], E::MalformedForm),
        (&b"ssid=%ff&password=12345678"[..], E::InvalidSsid),
        (&b"ssid=&password=12345678"[..], E::InvalidSsid),
        (&b"ssid=x"[..], E::MissingField),
        (&b"ssid=x&password=short"[..], E::InvalidCredential),
        (&b"ssid=x&password="[..], E::InvalidCredential),
    ] {
        assert_eq!(
            NetworkSetupInput::from_form(body, limits()).err(),
            Some(expected)
        );
    }
    let large = [b'x'; 512];
    assert_eq!(
        NetworkSetupInput::from_form(&large, limits()).err(),
        Some(E::TooLarge)
    );
    let long_ssid = b"ssid=123456789012345678901234567890123&password=12345678";
    assert_eq!(
        NetworkSetupInput::from_form(long_ssid, limits()).err(),
        Some(E::TooLarge)
    );
    let bad = NetworkSetupLimits {
        minimum_credential_bytes: 0,
        maximum_credential_bytes: 128,
    };
    assert_eq!(
        NetworkSetupInput::from_form(b"", bad).err(),
        Some(E::InvalidLimits)
    );
}

#[test]
fn fully_percent_encoded_maximum_fields_use_byte_bounds_without_truncation() {
    let mut body = [0; 300];
    body[..5].copy_from_slice(b"ssid=");
    for octet in body[5..101].chunks_exact_mut(3) {
        octet.copy_from_slice(b"%41");
    }
    body[101..111].copy_from_slice(b"&password=");
    for octet in body[111..].chunks_exact_mut(3) {
        octet.copy_from_slice(b"%42");
    }
    let input = NetworkSetupInput::from_form(&body, limits()).unwrap();
    assert_eq!(input.request().ssid, &[b'A'; 32]);
    assert_eq!(input.request().credential, &[b'B'; 63]);
    let mut zero = NetworkSetupMailbox::new(0);
    assert!(matches!(
        zero.submit(input),
        Err(NetworkSetupRefusal::Exhausted)
    ));
}
