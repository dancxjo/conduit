use super::*;
use conduit_core::{BootId, HostId, HostProfileId, OfferGeneration, PROTOCOL_VERSION};
use conduit_protected_line::{
    establish_protected_session, CarrierFailure, EndpointBinding, ProtectedCarrier,
    ProtectedFrameCarrier, ProtectedSessionPolicy, Role, SessionBinding, SessionLimits,
};
use std::sync::mpsc::{self, Receiver, SyncSender};

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
    let (mut client, mut server) = protected_lines();
    let session_secret = [37; 32];
    let advertisement = HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from("host/durable-fixture"),
        boot_id: BootId::from("boot/durable-fixture"),
        offer_generation: OfferGeneration(7),
        profile: HostProfileId::from("profile/durable-fixture"),
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
            "conduit-line/protected-relay@1",
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
    assert_eq!(host["lines"][0], "conduit-line/protected-relay@1");

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
