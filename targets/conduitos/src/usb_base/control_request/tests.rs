use super::{ControlRequestRefusal as Refusal, ControlTransferRequest as Request};

#[test]
fn input_geometry_checks_the_full_length_without_truncation() {
    for length in 0..=u16::MAX {
        let mut setup = [0; 8];
        setup[0] = 128;
        setup[6..].copy_from_slice(&length.to_le_bytes());
        match Request::new(setup, &[], 256) {
            Ok(request) => {
                assert!(length <= 256);
                assert_eq!(request.length(), length);
                assert!(request.output().is_empty());
            }
            Err(reason) => {
                assert!(length > 256);
                assert_eq!(reason, Refusal::DataEnvelope);
            }
        }
    }
}

#[test]
fn output_payload_must_match_the_exact_setup_length() {
    let payload = [0xa5; 257];
    for length in 0..=256_u16 {
        let mut setup = [0; 8];
        setup[6..].copy_from_slice(&length.to_le_bytes());
        let request = Request::new(setup, &payload[..usize::from(length)], 256).unwrap();
        assert_eq!(request.output(), &payload[..usize::from(length)]);
        assert!(!request.input());
        assert!(matches!(
            Request::new(setup, &payload[..usize::from(length) + 1], 256),
            Err(Refusal::OutputLengthMismatch)
        ));
        if length > 0 {
            assert!(matches!(
                Request::new(setup, &payload[..usize::from(length) - 1], 256),
                Err(Refusal::OutputLengthMismatch)
            ));
        }
        setup[0] = 128;
        assert!(matches!(
            Request::new(setup, &[1], 256),
            Err(Refusal::UnexpectedOutputData)
        ));
    }
}

#[test]
fn class_protocol_octets_are_preserved_without_probe_or_opcode_policy() {
    for request in 0..=u8::MAX {
        let setup = [0xc0, request, 0xff, 0xfe, 0xfd, 0xfc, 8, 0];
        assert_eq!(Request::new(setup, &[], 8).unwrap().setup(), &setup);
    }
}
