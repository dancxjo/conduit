use conduit_protected_line::{
    BindingMismatch, EndpointBinding, ProtectedHandshake, ProtectedLineError, ProtectedSession,
    Role, SessionBinding, SessionDisposition, SessionLimits,
};

fn binding() -> SessionBinding {
    SessionBinding {
        initiator: EndpointBinding {
            host_id: "host/a".into(),
            boot_id: "boot/a".into(),
        },
        responder: EndpointBinding {
            host_id: "host/b".into(),
            boot_id: "boot/b".into(),
        },
        negotiation_id: "negotiation/1".into(),
        line_session_id: "line/1".into(),
        candidate_binding: "relay/route/7".into(),
        transport_binding: "relay/websocket@1".into(),
    }
}

fn limits() -> SessionLimits {
    SessionLimits {
        maximum_payload_bytes: 64,
        maximum_frames_per_direction: 8,
        maximum_bytes_per_direction: 128,
    }
}

fn sessions() -> (ProtectedSession, ProtectedSession) {
    let psk = [7; 32];
    let mut a =
        ProtectedHandshake::new(Role::Initiator, &binding(), limits(), psk, [1; 32]).unwrap();
    let mut b =
        ProtectedHandshake::new(Role::Responder, &binding(), limits(), psk, [2; 32]).unwrap();
    let mut first = vec![0; a.next_message_bytes().unwrap()];
    a.write_message(&mut first).unwrap();
    b.read_message(&first).unwrap();
    let mut second = vec![0; b.next_message_bytes().unwrap()];
    b.write_message(&mut second).unwrap();
    a.read_message(&second).unwrap();
    (a.finish().unwrap(), b.finish().unwrap())
}

#[test]
fn established_noise_profile_protects_both_directions_and_retires_keys() {
    let (mut a, mut b) = sessions();
    let mut frame = [0; 128];
    let length = a.seal(b"secret", &mut frame).unwrap();
    assert!(!frame[..length].windows(6).any(|part| part == b"secret"));
    let mut output = [0; 64];
    assert_eq!(b.open(&frame[..length], &mut output).unwrap(), 6);
    assert_eq!(&output[..6], b"secret");
    let length = b.seal(b"reply", &mut frame).unwrap();
    assert_eq!(a.open(&frame[..length], &mut output).unwrap(), 5);
    assert_eq!(&output[..5], b"reply");
    a.close();
    assert_eq!(a.seal(b"late", &mut frame), Err(ProtectedLineError::Closed));
    let evidence = a.evidence();
    assert_eq!(evidence.binding, &binding());
    assert_eq!(evidence.role, Role::Initiator);
    assert_eq!(evidence.disposition, SessionDisposition::Closed);
    b.cancel();
    assert_eq!(b.evidence().disposition, SessionDisposition::Cancelled);
}

#[test]
fn mutation_replay_reordering_direction_and_bounds_refuse_distinctly() {
    let (mut a, mut b) = sessions();
    let mut first = [0; 128];
    let first_len = a.seal(b"one", &mut first).unwrap();
    let mut mutated = first;
    mutated[first_len - 1] ^= 1;
    assert_eq!(
        b.open(&mutated[..first_len], &mut [0; 64]),
        Err(ProtectedLineError::AuthenticationFailed)
    );
    assert_eq!(
        b.evidence().disposition,
        SessionDisposition::Failed(ProtectedLineError::AuthenticationFailed)
    );
    assert_eq!(
        b.open(&first[..first_len], &mut [0; 64]),
        Err(ProtectedLineError::Closed)
    );

    let (mut replay_sender, mut replay_receiver) = sessions();
    let first_len = replay_sender.seal(b"one", &mut first).unwrap();
    assert_eq!(
        replay_receiver
            .open(&first[..first_len], &mut [0; 64])
            .unwrap(),
        3
    );
    assert_eq!(
        replay_receiver.open(&first[..first_len], &mut [0; 64]),
        Err(ProtectedLineError::Replay)
    );

    let (mut reordered_sender, mut reordered_receiver) = sessions();
    let mut second = [0; 128];
    let second_len = reordered_sender.seal(b"two", &mut second).unwrap();
    second[6..14].copy_from_slice(&2_u64.to_le_bytes());
    assert_eq!(
        reordered_receiver.open(&second[..second_len], &mut [0; 64]),
        Err(ProtectedLineError::Reordered)
    );

    let (mut direction_sender, _) = sessions();
    let direction_len = direction_sender.seal(b"wrong way", &mut second).unwrap();
    assert_eq!(
        direction_sender.open(&second[..direction_len], &mut [0; 64]),
        Err(ProtectedLineError::WrongDirection)
    );
    assert_eq!(
        a.seal(&[0; 65], &mut first),
        Err(ProtectedLineError::FrameTooLarge)
    );
}

#[test]
fn wrong_secret_or_bound_identity_cannot_complete_the_handshake() {
    let mut a =
        ProtectedHandshake::new(Role::Initiator, &binding(), limits(), [7; 32], [1; 32]).unwrap();
    let mut stale = binding();
    stale.responder.boot_id = "boot/stale".into();
    assert_eq!(
        binding().compare(&stale),
        Err(ProtectedLineError::BindingMismatch(
            BindingMismatch::ResponderBoot
        ))
    );
    let mut b =
        ProtectedHandshake::new(Role::Responder, &stale, limits(), [7; 32], [2; 32]).unwrap();
    let mut first = vec![0; a.next_message_bytes().unwrap()];
    a.write_message(&mut first).unwrap();
    assert_eq!(
        b.read_message(&first),
        Err(ProtectedLineError::AuthenticationFailed)
    );

    let mut wrong_secret =
        ProtectedHandshake::new(Role::Responder, &binding(), limits(), [8; 32], [2; 32]).unwrap();
    assert_eq!(
        wrong_secret.read_message(&first),
        Err(ProtectedLineError::AuthenticationFailed)
    );
}

#[test]
fn every_bound_identity_mismatch_is_classified_before_handshake() {
    let expected = binding();
    let mut candidate = expected.clone();
    candidate.initiator.host_id = "host/wrong".into();
    assert_eq!(
        expected.compare(&candidate),
        Err(ProtectedLineError::BindingMismatch(
            BindingMismatch::InitiatorHost
        ))
    );
    candidate = expected.clone();
    candidate.initiator.boot_id = "boot/wrong".into();
    assert_eq!(
        expected.compare(&candidate),
        Err(ProtectedLineError::BindingMismatch(
            BindingMismatch::InitiatorBoot
        ))
    );
    candidate = expected.clone();
    candidate.responder.host_id = "host/wrong".into();
    assert_eq!(
        expected.compare(&candidate),
        Err(ProtectedLineError::BindingMismatch(
            BindingMismatch::ResponderHost
        ))
    );
    candidate = expected.clone();
    candidate.responder.boot_id = "boot/wrong".into();
    assert_eq!(
        expected.compare(&candidate),
        Err(ProtectedLineError::BindingMismatch(
            BindingMismatch::ResponderBoot
        ))
    );
    for (candidate, mismatch) in [
        (
            SessionBinding {
                negotiation_id: "negotiation/wrong".into(),
                ..expected.clone()
            },
            BindingMismatch::Negotiation,
        ),
        (
            SessionBinding {
                line_session_id: "line/wrong".into(),
                ..expected.clone()
            },
            BindingMismatch::LineSession,
        ),
        (
            SessionBinding {
                candidate_binding: "relay/wrong".into(),
                ..expected.clone()
            },
            BindingMismatch::Candidate,
        ),
        (
            SessionBinding {
                transport_binding: "transport/wrong".into(),
                ..expected.clone()
            },
            BindingMismatch::Transport,
        ),
    ] {
        assert_eq!(
            expected.compare(&candidate),
            Err(ProtectedLineError::BindingMismatch(mismatch))
        );
    }
}

#[test]
fn fixed_buffers_cross_one_hundred_thousand_frames_without_lifetime_growth() {
    let stress_limits = SessionLimits {
        maximum_payload_bytes: 8,
        maximum_frames_per_direction: 100_000,
        maximum_bytes_per_direction: 800_000,
    };
    let psk = [9; 32];
    let mut initiator =
        ProtectedHandshake::new(Role::Initiator, &binding(), stress_limits, psk, [3; 32]).unwrap();
    let mut responder =
        ProtectedHandshake::new(Role::Responder, &binding(), stress_limits, psk, [4; 32]).unwrap();
    let mut handshake = [0; 64];
    let first = initiator.next_message_bytes().unwrap();
    initiator.write_message(&mut handshake[..first]).unwrap();
    responder.read_message(&handshake[..first]).unwrap();
    let second = responder.next_message_bytes().unwrap();
    responder.write_message(&mut handshake[..second]).unwrap();
    initiator.read_message(&handshake[..second]).unwrap();
    let (mut sender, mut receiver) = (initiator.finish().unwrap(), responder.finish().unwrap());

    let mut protected = [0; 42];
    let mut plaintext = [0; 8];
    for sequence in 0_u64..100_000 {
        let value = sequence.to_le_bytes();
        let length = sender.seal(&value, &mut protected).unwrap();
        assert_eq!(
            receiver.open(&protected[..length], &mut plaintext).unwrap(),
            8
        );
        assert_eq!(plaintext, value);
    }
    assert_eq!(sender.maximum_frame_bytes(), protected.len());
    assert_eq!(sender.evidence().sent_frames, 100_000);
    assert_eq!(receiver.evidence().received_frames, 100_000);
    assert_eq!(
        sender.seal(&[0; 8], &mut protected),
        Err(ProtectedLineError::FrameLimitExhausted)
    );
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(DIGITS[(byte >> 4) as usize] as char);
        output.push(DIGITS[(byte & 0x0f) as usize] as char);
    }
    output
}

#[test]
fn deterministic_profile_vector() {
    let vector: serde_json::Value =
        serde_json::from_str(include_str!("../vectors/noise-nnpsk0-v1.json")).unwrap();
    let psk = [7; 32];
    let mut a =
        ProtectedHandshake::new(Role::Initiator, &binding(), limits(), psk, [1; 32]).unwrap();
    let mut b =
        ProtectedHandshake::new(Role::Responder, &binding(), limits(), psk, [2; 32]).unwrap();
    let mut first = vec![0; a.next_message_bytes().unwrap()];
    a.write_message(&mut first).unwrap();
    b.read_message(&first).unwrap();
    let mut second = vec![0; b.next_message_bytes().unwrap()];
    b.write_message(&mut second).unwrap();
    a.read_message(&second).unwrap();
    let (mut a, mut b) = (a.finish().unwrap(), b.finish().unwrap());
    let mut protected = [0; 128];
    let length = a.seal(b"secret", &mut protected).unwrap();
    assert_eq!(
        (hex(&first), hex(&second), hex(&protected[..length])),
        (
            vector["first_handshake_message_hex"]
                .as_str()
                .unwrap()
                .into(),
            vector["second_handshake_message_hex"]
                .as_str()
                .unwrap()
                .into(),
            vector["first_protected_frame_hex"].as_str().unwrap().into(),
        )
    );
    assert_eq!(b.open(&protected[..length], &mut [0; 64]).unwrap(), 6);
}
