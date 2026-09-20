use super::*;
use conduit_core::PROTOCOL_VERSION;
use conduit_protected_line::{
    establish_protected_session, EndpointBinding, ProtectedCarrier, ProtectedSessionPolicy, Role,
    SessionBinding, SessionLimits,
};
use conduit_std_host::relay_client::{HostedRelayCarrier, RelayClientDescriptor};
use conduit_wire::{
    decode_session_frame, encode_session_frame_into, SessionFrame, SessionIdentity, SessionMessage,
};
use rcgen::{generate_simple_self_signed, CertifiedKey};
use sha2::{Digest, Sha256};
use std::net::{Ipv4Addr, SocketAddrV4, TcpListener};
use std::thread;

fn options(bind: &str, authorize_network: bool) -> ServeOptions {
    ServeOptions {
        bind: bind.into(),
        public_url: "wss://relay.example/conduit".into(),
        tls_cert: "unused-cert.pem".into(),
        tls_key: "unused-key.pem".into(),
        slot: "unused-slot.json".into(),
        accept_timeout_seconds: 1,
        authorize_network,
    }
}

#[test]
fn network_listening_requires_explicit_non_loopback_authority() {
    assert_eq!(
        serve(options("192.0.2.10:7443", false)),
        Err("relay network listening requires --authorize-network".into())
    );
    assert_eq!(
        serve(options("127.0.0.1:7443", true)),
        Err("relay --bind must be one explicit non-loopback socket".into())
    );
    assert_eq!(
        serve(options("192.0.2.10:0", true)),
        Err("relay --bind must be one explicit non-loopback socket".into())
    );
}

#[test]
fn capability_ingress_is_exact_and_erases_the_source() {
    let mut exact = vec![7; 32];
    assert_eq!(take_capability(&mut exact), Ok([7; 32]));
    assert_eq!(exact, vec![0; 32]);

    let mut short = vec![9; 31];
    assert_eq!(
        take_capability(&mut short),
        Err("relay capability must contain exactly 32 bytes".into())
    );
    assert_eq!(short, vec![0; 31]);
}

#[test]
fn explicit_close_control_is_versioned_and_exact() {
    let control: Control =
        serde_json::from_slice(br#"{"schema":"conduit.relay/control@1","kind":"close"}"#).unwrap();
    assert_eq!(control.schema, CONTROL_SCHEMA);
    assert!(matches!(control.kind, ControlKind::Close));
    assert!(serde_json::from_slice::<Control>(
        br#"{"schema":"conduit.relay/control@1","kind":"close","retry":true}"#,
    )
    .is_err());
}

#[test]
fn remote_execution_session_frames_cross_two_outbound_protected_relay_clients() {
    let CertifiedKey { cert, signing_key } =
        generate_simple_self_signed(vec!["localhost".into()]).unwrap();
    let certificate_binding: [u8; 32] = Sha256::digest(cert.der().as_ref()).into();
    let directory = std::env::temp_dir().join(format!(
        "conduit-relay-service-{}-{}",
        std::process::id(),
        thread::current().name().unwrap_or("native-clients")
    ));
    fs::create_dir_all(&directory).unwrap();
    let certificate_path = directory.join("certificate.pem");
    let private_key_path = directory.join("private-key.pem");
    let slot_path = directory.join("slot.json");
    fs::write(&certificate_path, cert.pem()).unwrap();
    fs::write(&private_key_path, signing_key.serialize_pem()).unwrap();
    let reservation = TcpListener::bind((Ipv4Addr::UNSPECIFIED, 0)).unwrap();
    let port = reservation.local_addr().unwrap().port();
    drop(reservation);
    let expires_at_millis = now_millis().unwrap() + 60_000;
    fs::write(
        &slot_path,
        serde_json::json!({
            "schema": SLOT_SCHEMA,
            "route_id": "route/native-proof",
            "negotiation_id": "negotiation/native-proof",
            "first_endpoint_binding": "host/first/boot/one",
            "second_endpoint_binding": "host/second/boot/two",
            "expires_at_millis": expires_at_millis,
            "first_capability": vec![7; 32],
            "second_capability": vec![8; 32],
            "limits": {
                "maximum_protected_frame_bytes": 1024,
                "maximum_queued_frames_per_direction": 2,
                "maximum_queued_bytes_per_direction": 2048,
                "maximum_attachment_attempts_per_slot": 4,
                "maximum_idle_millis": 5000,
                "maximum_active_millis": 30000
            }
        })
        .to_string(),
    )
    .unwrap();
    let bind = SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, port).to_string();
    let public_url = format!("wss://localhost:{port}/conduit");
    let server_url = public_url.clone();
    let server = thread::spawn(move || {
        serve(ServeOptions {
            bind,
            public_url: server_url,
            tls_cert: certificate_path,
            tls_key: private_key_path,
            slot: slot_path,
            accept_timeout_seconds: 5,
            authorize_network: true,
        })
    });
    thread::sleep(Duration::from_millis(100));

    let binding = SessionBinding {
        initiator: EndpointBinding {
            host_id: "host/first".into(),
            boot_id: "boot/one".into(),
        },
        responder: EndpointBinding {
            host_id: "host/second".into(),
            boot_id: "boot/two".into(),
        },
        negotiation_id: "negotiation/native-proof".into(),
        line_session_id: "line/native-proof".into(),
        candidate_binding: "route/native-proof".into(),
        transport_binding: format!("relay/wss/{}", hex(&certificate_binding)),
    };
    let policy = ProtectedSessionPolicy {
        traffic: SessionLimits {
            maximum_payload_bytes: 256,
            maximum_frames_per_direction: 8,
            maximum_bytes_per_direction: 2048,
        },
        maximum_simultaneous_sessions: 1,
        maximum_pending_frames_per_session: 1,
        handshake_work_units: 2,
        handshake_timeout_millis: 2_000,
        idle_timeout_millis: 2_000,
    };
    let address = SocketAddrV4::new(Ipv4Addr::LOCALHOST, port).into();
    let first_url = public_url.clone();
    let first_binding = binding.clone();
    let first = thread::spawn(move || {
        let mut carrier = HostedRelayCarrier::connect(RelayClientDescriptor {
            address,
            public_url: first_url,
            server_identity: "localhost".into(),
            certificate_binding_sha256: certificate_binding,
            route_id: "route/native-proof".into(),
            role: RelayEndpointRole::First,
            endpoint_binding: "host/first/boot/one".into(),
            capability: [7; 32],
            maximum_protected_frame_bytes: 1024,
            timeout_millis: 2_000,
        })
        .unwrap();
        let session = establish_protected_session(
            &mut carrier,
            Role::Initiator,
            &first_binding,
            policy,
            [3; 32],
            [4; 32],
        )
        .unwrap();
        let mut line = ProtectedCarrier::new(carrier, session, policy).unwrap();
        let mut frame = [0_u8; 256];
        let length = encode_session_frame_into(
            remote_frame(SessionMessage::Offered {
                sequence: 0,
                payload: b"remote-value",
            }),
            &mut frame,
            32,
            256,
        )
        .unwrap();
        line.send(&frame[..length]).unwrap();
        let response = line.receive().unwrap();
        let response = decode_session_frame(response, 32, 256).unwrap();
        assert_eq!(response.identity.plan_id, "plan/relay-proof");
        assert_eq!(response.message, SessionMessage::Accepted { sequence: 0 });
        line.close().unwrap();
    });
    let mut second_carrier = HostedRelayCarrier::connect(RelayClientDescriptor {
        address,
        public_url,
        server_identity: "localhost".into(),
        certificate_binding_sha256: certificate_binding,
        route_id: "route/native-proof".into(),
        role: RelayEndpointRole::Second,
        endpoint_binding: "host/second/boot/two".into(),
        capability: [8; 32],
        maximum_protected_frame_bytes: 1024,
        timeout_millis: 2_000,
    })
    .unwrap();
    let second_session = establish_protected_session(
        &mut second_carrier,
        Role::Responder,
        &binding,
        policy,
        [3; 32],
        [5; 32],
    )
    .unwrap();
    let mut second_line = ProtectedCarrier::new(second_carrier, second_session, policy).unwrap();
    let offered = second_line.receive().unwrap();
    let offered = decode_session_frame(offered, 32, 256).unwrap();
    assert_eq!(offered.identity.plan_id, "plan/relay-proof");
    assert_eq!(offered.identity.source_active_play_id, "play/source");
    assert_eq!(offered.identity.sink_active_play_id, "play/sink");
    assert_eq!(offered.identity.connection_id, "connection/relay-proof");
    assert_eq!(
        offered.message,
        SessionMessage::Offered {
            sequence: 0,
            payload: b"remote-value"
        }
    );
    let mut response = [0_u8; 256];
    let response_length = encode_session_frame_into(
        remote_frame(SessionMessage::Accepted { sequence: 0 }),
        &mut response,
        32,
        256,
    )
    .unwrap();
    second_line.send(&response[..response_length]).unwrap();
    first.join().unwrap();
    second_line.close().unwrap();
    assert_eq!(server.join().unwrap(), Ok(()));
    fs::remove_dir_all(directory).unwrap();
}

fn remote_frame(message: SessionMessage<'_>) -> SessionFrame<'_> {
    SessionFrame {
        identity: SessionIdentity {
            protocol_version: PROTOCOL_VERSION,
            plan_id: "plan/relay-proof",
            source_fragment_id: "fragment/source",
            sink_fragment_id: "fragment/sink",
            source_active_play_id: "play/source",
            sink_active_play_id: "play/sink",
            connection_id: "connection/relay-proof",
            source_host_id: "host/first",
            source_boot_id: "boot/one",
            sink_host_id: "host/second",
            sink_boot_id: "boot/two",
            value_kind: "text/plain",
            limits: conduit_wire::SessionLimits {
                maximum_in_flight_items: 1,
                maximum_payload_bytes: 32,
                maximum_buffered_bytes: 32,
            },
        },
        message,
    }
}
