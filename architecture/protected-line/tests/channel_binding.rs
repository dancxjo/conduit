#![cfg(feature = "noise-session")]
use conduit_protected_line::*;

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
        negotiation_id: "attempt/1".into(),
        line_session_id: "session/1".into(),
        candidate_binding: "candidate/1".into(),
        transport_binding: "carrier/1".into(),
    }
}
fn pair(binding: &SessionBinding, key: u8, ephemeral: u8) -> (ProtectedSession, ProtectedSession) {
    let limits = SessionLimits {
        maximum_payload_bytes: 64,
        maximum_frames_per_direction: 16,
        maximum_bytes_per_direction: 1024,
    };
    let mut a =
        ProtectedHandshake::new(Role::Initiator, binding, limits, [key; 32], [ephemeral; 32])
            .unwrap();
    let mut b = ProtectedHandshake::new(
        Role::Responder,
        binding,
        limits,
        [key; 32],
        [ephemeral + 1; 32],
    )
    .unwrap();
    let mut frame = [0; 64];
    let n = a.next_message_bytes().unwrap();
    a.write_message(&mut frame[..n]).unwrap();
    b.read_message(&frame[..n]).unwrap();
    let n = b.next_message_bytes().unwrap();
    b.write_message(&mut frame[..n]).unwrap();
    a.read_message(&frame[..n]).unwrap();
    (a.finish().unwrap(), b.finish().unwrap())
}
#[test]
fn both_endpoints_observe_the_same_nonzero_channel_binding() {
    let (mut a, mut b) = pair(&binding(), 3, 5);
    assert_eq!(a.channel_binding(), b.channel_binding());
    assert_ne!(a.channel_binding(), &[0; 32]);
    let before = *a.channel_binding();
    let mut frame = [0; 128];
    let n = a.seal(b"request", &mut frame).unwrap();
    let mut output = [0; 64];
    let n = b.open(&frame[..n], &mut output).unwrap();
    assert_eq!(&output[..n], b"request");
    assert_eq!(a.channel_binding(), &before);
    assert_eq!(b.channel_binding(), &before);
}
#[test]
fn reused_descriptive_binding_does_not_alias_different_handshakes() {
    let binding = binding();
    let (original, _) = pair(&binding, 3, 5);
    let (fresh_ephemeral, _) = pair(&binding, 3, 7);
    let (fresh_secret, _) = pair(&binding, 4, 5);
    assert_ne!(
        original.channel_binding(),
        fresh_ephemeral.channel_binding()
    );
    assert_ne!(original.channel_binding(), fresh_secret.channel_binding());
}
#[test]
fn changed_negotiation_is_bound_into_the_handshake() {
    let mut binding = binding();
    let (original, _) = pair(&binding, 3, 5);
    binding.negotiation_id = "attempt/2".into();
    let (changed, _) = pair(&binding, 3, 5);
    assert_ne!(original.channel_binding(), changed.channel_binding());
}
