use super::*;
use conduit_core::{BootId, HostId, HostProfileId, OfferGeneration, PROTOCOL_VERSION};
use conduit_protected_line::{
    establish_protected_session, CarrierFailure, EndpointBinding, ProtectedCarrier,
    ProtectedFrameCarrier, ProtectedSessionPolicy, RelayEndpointRole, Role, SessionBinding,
    SessionLimits,
};
use conduit_std_host::relay_client::{HostedRelayCarrier, RelayClientDescriptor};
use rcgen::{generate_simple_self_signed, CertifiedKey};
use sha2::{Digest, Sha256};
use std::fs;
use std::net::{Ipv4Addr, SocketAddrV4, TcpListener};
use std::sync::mpsc::{self, Receiver, SyncSender};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

struct ChannelCarrier {
    send: Option<SyncSender<Vec<u8>>>,
    receive: Receiver<Vec<u8>>,
}

impl ProtectedFrameCarrier for ChannelCarrier {
    fn send_frame(&mut self, frame: &[u8]) -> Result<(), CarrierFailure> {
        self.send
            .as_ref()
            .ok_or(CarrierFailure::Lost)?
            .try_send(frame.to_vec())
            .map_err(|error| match error {
                mpsc::TrySendError::Full(_) => CarrierFailure::Pressure,
                mpsc::TrySendError::Disconnected(_) => CarrierFailure::Lost,
            })
    }

    fn receive_frame(
        &mut self,
        output: &mut [u8],
        timeout_millis: u32,
    ) -> Result<usize, CarrierFailure> {
        let frame = self
            .receive
            .recv_timeout(Duration::from_millis(u64::from(timeout_millis)))
            .map_err(|error| match error {
                mpsc::RecvTimeoutError::Timeout => CarrierFailure::TimedOut,
                mpsc::RecvTimeoutError::Disconnected => CarrierFailure::Lost,
            })?;
        if frame.len() > output.len() {
            return Err(CarrierFailure::Pressure);
        }
        output[..frame.len()].copy_from_slice(&frame);
        Ok(frame.len())
    }

    fn close(&mut self) -> Result<(), CarrierFailure> {
        self.send = None;
        Ok(())
    }
}

struct ProtectedMemoryLine {
    inner: ProtectedCarrier<ChannelCarrier>,
}

impl RendezvousLine for ProtectedMemoryLine {
    fn receive(&mut self) -> Result<Vec<u8>, String> {
        self.inner
            .receive()
            .map(<[u8]>::to_vec)
            .map_err(|error| format!("protected receive: {error:?}"))
    }

    fn send(&mut self, bytes: &[u8]) -> Result<(), String> {
        self.inner
            .send(bytes)
            .map_err(|error| format!("protected send: {error:?}"))
    }

    fn close(&mut self) -> Result<(), String> {
        self.inner
            .close()
            .map_err(|error| format!("protected close: {error:?}"))
    }
}

fn binding() -> SessionBinding {
    SessionBinding {
        initiator: EndpointBinding {
            host_id: "host/browser".into(),
            boot_id: "boot/browser".into(),
        },
        responder: EndpointBinding {
            host_id: "host/durable-fixture".into(),
            boot_id: "boot/durable-fixture".into(),
        },
        negotiation_id: "rendezvous/protected-test".into(),
        line_session_id: "line/protected-test".into(),
        candidate_binding: "relay/route/test".into(),
        transport_binding: "relay/memory@1".into(),
    }
}

fn policy() -> ProtectedSessionPolicy {
    ProtectedSessionPolicy {
        traffic: SessionLimits {
            maximum_payload_bytes: 65_519,
            maximum_frames_per_direction: 16,
            maximum_bytes_per_direction: 65_519 * 16,
        },
        maximum_simultaneous_sessions: 1,
        maximum_pending_frames_per_session: 1,
        handshake_work_units: 2,
        handshake_timeout_millis: 1_000,
        idle_timeout_millis: 1_000,
    }
}

fn protected_lines() -> (ProtectedMemoryLine, ProtectedMemoryLine) {
    let (client_send, server_receive) = mpsc::sync_channel(1);
    let (server_send, client_receive) = mpsc::sync_channel(1);
    let mut client_io = ChannelCarrier {
        send: Some(client_send),
        receive: client_receive,
    };
    let mut server_io = ChannelCarrier {
        send: Some(server_send),
        receive: server_receive,
    };
    let server = std::thread::spawn(move || {
        let session = establish_protected_session(
            &mut server_io,
            Role::Responder,
            &binding(),
            policy(),
            [41; 32],
            [43; 32],
        )
        .unwrap();
        ProtectedMemoryLine {
            inner: ProtectedCarrier::new(server_io, session, policy()).unwrap(),
        }
    });
    let session = establish_protected_session(
        &mut client_io,
        Role::Initiator,
        &binding(),
        policy(),
        [41; 32],
        [42; 32],
    )
    .unwrap();
    (
        ProtectedMemoryLine {
            inner: ProtectedCarrier::new(client_io, session, policy()).unwrap(),
        },
        server.join().unwrap(),
    )
}

#[test]
fn ordinary_rendezvous_session_succeeds_above_the_protected_carrier() {
    let (client, server) = protected_lines();
    exercise_ordinary_session(client, server, "conduit-line/protected-relay@1");
}

fn exercise_ordinary_session(
    mut client: impl RendezvousLine,
    mut server: impl RendezvousLine + Send + 'static,
    line_id: &'static str,
) {
    let session_secret = [37; 32];
    let advertisement = HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("host/durable-fixture"),
        boot_id: BootId::from("boot/durable-fixture"),
        offer_generation: OfferGeneration(7),
        profile: HostProfileId::from("profile/durable-fixture"),
        bases: vec![],
        resources: Vec::new(),
        capabilities: Vec::new(),
        planner_capabilities: Vec::new(),
    };
    let truth = crate::durable_host_control::DurableHostTruth {
        target_id: "std/x86_64/computer".into(),
        image_content_digest: format!("sha256:{}", "a".repeat(64)),
        advertisement,
    };
    let server_thread = std::thread::spawn(move || {
        run_session_with_join(
            &mut server,
            Path::new("unused-after-close"),
            &session_secret,
            line_id,
            truth,
            |expected, claim, _secret| {
                Ok(crate::durable_host_control::DurableJoinProof {
                    advertisement: expected.clone(),
                    invitation_id: claim.invitation_id.as_str().into(),
                    body_id: claim.body_id.as_str().into(),
                    nonce: [47; 32],
                    signature: vec![53; 64],
                    observed_at_millis: 1_000,
                })
            },
        )
    });

    client
        .send(
            &serde_json::to_vec(&serde_json::json!({
                "kind":"hello", "protocol":PROTOCOL, "session_secret":session_secret
            }))
            .unwrap(),
        )
        .unwrap();
    let host: serde_json::Value = serde_json::from_slice(&client.receive().unwrap()).unwrap();
    assert_eq!(host["kind"], "host");
    assert_eq!(host["lines"][0], line_id);

    client
        .send(
            &serde_json::to_vec(&serde_json::json!({
                "kind":"invite",
                "protocol":PROTOCOL,
                "session_secret":session_secret,
                "spore_id":"spore/protected",
                "image_id":"image/protected",
                "claim":{
                    "invitation_id":"invitation/protected",
                    "body_id":"body/protected",
                    "nonce":vec![59;32],
                    "expires_at_millis":4_000_000_000_000_u64
                },
                "secret":vec![61;32]
            }))
            .unwrap(),
        )
        .unwrap();
    let join: serde_json::Value = serde_json::from_slice(&client.receive().unwrap()).unwrap();
    assert_eq!(join["kind"], "join");
    assert_eq!(join["host_id"], "host/durable-fixture");

    client
        .send(
            &serde_json::to_vec(&serde_json::json!({"kind":"close", "protocol":PROTOCOL})).unwrap(),
        )
        .unwrap();
    assert_eq!(server_thread.join().unwrap(), Ok(()));
}

#[test]
fn ordinary_rendezvous_session_crosses_the_real_user_operated_relay() {
    let CertifiedKey { cert, signing_key } =
        generate_simple_self_signed(vec!["localhost".into()]).unwrap();
    let certificate_binding: [u8; 32] = Sha256::digest(cert.der().as_ref()).into();
    let directory = std::env::temp_dir().join(format!(
        "conduit-relayed-run-session-{}-{}",
        std::process::id(),
        thread::current().name().unwrap_or("proof")
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
    let now: u64 = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_millis()
        .try_into()
        .unwrap();
    fs::write(
        &slot_path,
        serde_json::json!({
            "schema":"conduit.relay/private-slot@1",
            "route_id":"relay/route/test",
            "negotiation_id":"rendezvous/protected-test",
            "first_endpoint_binding":"host/browser/boot/browser",
            "second_endpoint_binding":"host/durable-fixture/boot/durable-fixture",
            "expires_at_millis":now + 60_000,
            "first_capability":vec![7;32],
            "second_capability":vec![8;32],
            "limits":{
                "maximum_protected_frame_bytes":65_553,
                "maximum_queued_frames_per_direction":2,
                "maximum_queued_bytes_per_direction":131_106,
                "maximum_attachment_attempts_per_slot":4,
                "maximum_idle_millis":5_000,
                "maximum_active_millis":30_000
            }
        })
        .to_string(),
    )
    .unwrap();
    let public_url = format!("wss://localhost:{port}/conduit");
    let server_url = public_url.clone();
    let relay_server = thread::spawn(move || {
        crate::rendezvous_relay::serve(crate::rendezvous_relay::ServeOptions {
            bind: SocketAddrV4::new(Ipv4Addr::UNSPECIFIED, port).to_string(),
            public_url: server_url,
            tls_cert: certificate_path,
            tls_key: private_key_path,
            slot: slot_path,
            accept_timeout_seconds: 5,
            authorize_network: true,
        })
    });
    thread::sleep(Duration::from_millis(100));
    let address = SocketAddrV4::new(Ipv4Addr::LOCALHOST, port).into();
    let server_binding = binding();
    let server_url = public_url.clone();
    let server_endpoint = thread::spawn(move || {
        let mut carrier = HostedRelayCarrier::connect(RelayClientDescriptor {
            address,
            public_url: server_url,
            server_identity: "localhost".into(),
            certificate_binding_sha256: certificate_binding,
            route_id: "relay/route/test".into(),
            role: RelayEndpointRole::Second,
            endpoint_binding: "host/durable-fixture/boot/durable-fixture".into(),
            capability: [8; 32],
            maximum_protected_frame_bytes: 65_553,
            timeout_millis: 2_000,
        })
        .unwrap();
        let session = establish_protected_session(
            &mut carrier,
            Role::Responder,
            &server_binding,
            policy(),
            [41; 32],
            [43; 32],
        )
        .unwrap();
        ProtectedCarrier::new(carrier, session, policy()).unwrap()
    });
    let mut client_carrier = HostedRelayCarrier::connect(RelayClientDescriptor {
        address,
        public_url,
        server_identity: "localhost".into(),
        certificate_binding_sha256: certificate_binding,
        route_id: "relay/route/test".into(),
        role: RelayEndpointRole::First,
        endpoint_binding: "host/browser/boot/browser".into(),
        capability: [7; 32],
        maximum_protected_frame_bytes: 65_553,
        timeout_millis: 2_000,
    })
    .unwrap();
    let client_session = establish_protected_session(
        &mut client_carrier,
        Role::Initiator,
        &binding(),
        policy(),
        [41; 32],
        [42; 32],
    )
    .unwrap();
    let client = ProtectedCarrier::new(client_carrier, client_session, policy()).unwrap();
    exercise_ordinary_session(
        client,
        server_endpoint.join().unwrap(),
        "conduit-line/user-operated-protected-relay@1",
    );
    assert_eq!(relay_server.join().unwrap(), Ok(()));
    fs::remove_dir_all(directory).unwrap();
}
