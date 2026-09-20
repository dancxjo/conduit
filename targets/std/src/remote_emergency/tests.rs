use super::*;
use conduit_body::{AuthenticatedHostObservation, BodyMembershipRevision, MembershipProofId};
use conduit_core::{OfferGeneration, SignId};
use conduit_protected_line::{EndpointBinding, ProtectedHandshake, Role, SessionLimits};

const PEER_HOST: &str = "host/remote-emergency-peer";
const PEER_BOOT: &str = "boot/remote-emergency-peer";
const LOCAL_HOST: &str = "host/remote-emergency-local";
const LOCAL_BOOT: &str = "boot/remote-emergency-local";

struct Fixture {
    body_id: BodyId,
    membership: BodyMembership,
    credential: MembershipCredential,
    sender: ProtectedSession,
    receiver: ProtectedSession,
    adapter: RemoteEmergencyAdapter,
}

fn binding() -> SessionBinding {
    SessionBinding {
        initiator: EndpointBinding {
            host_id: PEER_HOST.into(),
            boot_id: PEER_BOOT.into(),
        },
        responder: EndpointBinding {
            host_id: LOCAL_HOST.into(),
            boot_id: LOCAL_BOOT.into(),
        },
        negotiation_id: "negotiation/remote-emergency".into(),
        line_session_id: "line/remote-emergency".into(),
        candidate_binding: "candidate/remote-emergency".into(),
        transport_binding: "transport/remote-emergency".into(),
    }
}

fn sessions() -> (ProtectedSession, ProtectedSession) {
    let limits = SessionLimits {
        maximum_payload_bytes: MAX_REMOTE_EMERGENCY_PAYLOAD_BYTES as u32,
        maximum_frames_per_direction: 16,
        maximum_bytes_per_direction: 8_192,
    };
    let mut initiator =
        ProtectedHandshake::new(Role::Initiator, &binding(), limits, [7; 32], [1; 32]).unwrap();
    let mut responder =
        ProtectedHandshake::new(Role::Responder, &binding(), limits, [7; 32], [2; 32]).unwrap();
    let mut first = vec![0; initiator.next_message_bytes().unwrap()];
    initiator.write_message(&mut first).unwrap();
    responder.read_message(&first).unwrap();
    let mut second = vec![0; responder.next_message_bytes().unwrap()];
    responder.write_message(&mut second).unwrap();
    initiator.read_message(&second).unwrap();
    (initiator.finish().unwrap(), responder.finish().unwrap())
}

fn policy() -> EmergencyPolicy {
    EmergencyPolicy {
        allow_keyboard_rescue: false,
        allow_physical: false,
        allow_acoustic: false,
        allow_remote: true,
        attempt_graceful_lull: true,
        revoke_local_execution: true,
        isolate_carriers: true,
        terminal_action: conduit_body::EmergencyMachineAction::Halt,
    }
}

fn fixture() -> Fixture {
    let body_id: BodyId =
        serde_json::from_value(serde_json::json!("body/remote-emergency")).unwrap();
    let part_id = PartId::bind(&body_id, "part/remote-emergency-peer", 1).unwrap();
    let mut membership = BodyMembership::new(body_id.clone()).unwrap();
    membership
        .admit(
            &body_id,
            BodyMembershipRevision(0),
            part_id.clone(),
            MembershipProofId::bind("proof/remote-emergency-admission").unwrap(),
            SignId::from("sign/remote-emergency-admission"),
        )
        .unwrap();
    membership
        .observe_present(
            &body_id,
            membership.revision,
            &part_id,
            AuthenticatedHostObservation {
                host_id: HostId::from(PEER_HOST),
                boot_id: BootId::from(PEER_BOOT),
                offer_generation: OfferGeneration(1),
                proof_id: MembershipProofId::bind("proof/remote-emergency-presence").unwrap(),
                sequence: 1,
            },
            SignId::from("sign/remote-emergency-presence"),
        )
        .unwrap();
    let credential: MembershipCredential = serde_json::from_value(serde_json::json!({
        "credential_id": "credential/remote-emergency-peer",
        "body_id": body_id.as_str(),
        "part_id": part_id.as_str(),
        "host_id": PEER_HOST,
        "boot_id": PEER_BOOT,
        "issued_at_millis": 1,
    }))
    .unwrap();
    let (sender, receiver) = sessions();
    let adapter = RemoteEmergencyAdapter::admit(
        body_id.clone(),
        HostId::from(LOCAL_HOST),
        BootId::from(LOCAL_BOOT),
        &membership,
        &credential,
        &receiver,
        policy(),
    )
    .unwrap();
    Fixture {
        body_id,
        membership,
        credential,
        sender,
        receiver,
        adapter,
    }
}

fn protected_request(fixture: &mut Fixture, freshness: u64, request_id: &str) -> Vec<u8> {
    let mut plaintext = [0; MAX_REMOTE_EMERGENCY_PAYLOAD_BYTES];
    let length = encode_remote_emergency_request(
        &fixture.body_id,
        fixture.credential.credential_id.as_str(),
        freshness,
        request_id,
        &mut plaintext,
    )
    .unwrap();
    let mut encrypted = vec![0; MAX_REMOTE_EMERGENCY_PAYLOAD_BYTES + 32];
    let length = fixture
        .sender
        .seal(&plaintext[..length], &mut encrypted)
        .unwrap();
    encrypted.truncate(length);
    encrypted
}

#[test]
fn current_member_over_exact_protected_session_reduces_local_authority() {
    let mut fixture = fixture();
    let frame = protected_request(&mut fixture, 1, "request/remote-emergency");
    let receipt = fixture
        .adapter
        .receive_authenticated(&fixture.membership, &mut fixture.receiver, &frame)
        .unwrap();
    assert_eq!(
        receipt.local_outcome.trigger,
        EmergencyTriggerClass::AuthenticatedRemoteEmergency
    );
    assert_eq!(
        receipt.local_outcome.local_execution_revocation,
        conduit_body::EmergencyStepOutcome::Requested
    );
    assert_eq!(receipt.authenticated_peer_host_id, HostId::from(PEER_HOST));
    assert_eq!(receipt.line_session_id, "line/remote-emergency");
    assert_eq!(
        receipt.propagation_summary(),
        RemotePropagationSummary::NotRequested
    );
}

#[test]
fn transport_and_application_replay_are_independently_refused() {
    let mut transport = fixture();
    let frame = protected_request(&mut transport, 1, "request/one");
    transport
        .adapter
        .receive_authenticated(&transport.membership, &mut transport.receiver, &frame)
        .unwrap();
    assert_eq!(
        transport.adapter.receive_authenticated(
            &transport.membership,
            &mut transport.receiver,
            &frame
        ),
        Err(RemoteEmergencyRefusal::Transport(
            ProtectedLineError::Replay
        ))
    );

    let mut application = fixture();
    let first = protected_request(&mut application, 4, "request/four");
    application
        .adapter
        .receive_authenticated(&application.membership, &mut application.receiver, &first)
        .unwrap();
    let fresh_transport_frame = protected_request(&mut application, 4, "request/four-again");
    assert_eq!(
        application.adapter.receive_authenticated(
            &application.membership,
            &mut application.receiver,
            &fresh_transport_frame
        ),
        Err(RemoteEmergencyRefusal::StaleOrReplayed)
    );
}

#[test]
fn phrase_knowledge_and_stale_membership_never_authorize() {
    let mut plaintext = [0; MAX_REMOTE_EMERGENCY_PAYLOAD_BYTES];
    let fixture = fixture();
    let length = encode_remote_emergency_request(
        &fixture.body_id,
        fixture.credential.credential_id.as_str(),
        1,
        "request/no-phrase-authority",
        &mut plaintext,
    )
    .unwrap();
    assert!(!plaintext[..length]
        .windows(b"emergency stop".len())
        .any(|bytes| bytes == b"emergency stop"));

    let mut stale = fixture;
    stale
        .membership
        .observe_offline(
            &stale.body_id,
            stale.membership.revision,
            &stale.credential.part_id,
            &stale.credential.boot_id,
            SignId::from("sign/remote-emergency-detached"),
        )
        .unwrap();
    let frame = protected_request(&mut stale, 1, "request/stale-member");
    assert_eq!(
        stale
            .adapter
            .receive_authenticated(&stale.membership, &mut stale.receiver, &frame),
        Err(RemoteEmergencyRefusal::StaleMembership)
    );
}

#[test]
fn propagation_is_finite_evidence_after_independent_local_action() {
    let mut fixture = fixture();
    let frame = protected_request(&mut fixture, 1, "request/propagation");
    let mut receipt = fixture
        .adapter
        .receive_authenticated(&fixture.membership, &mut fixture.receiver, &frame)
        .unwrap();
    receipt
        .record_propagation(
            HostId::from("host/propagation/one"),
            RemotePropagationStatus::Delivered,
        )
        .unwrap();
    receipt
        .record_propagation(
            HostId::from("host/propagation/two"),
            RemotePropagationStatus::Unreachable,
        )
        .unwrap();
    assert_eq!(
        receipt.propagation_summary(),
        RemotePropagationSummary::PartialFailure
    );
    assert_eq!(
        receipt.local_outcome.local_execution_revocation,
        conduit_body::EmergencyStepOutcome::Requested
    );
    assert_eq!(
        receipt.record_propagation(
            HostId::from("host/propagation/two"),
            RemotePropagationStatus::Delivered
        ),
        Err(RemoteEmergencyRefusal::DuplicatePropagationTarget)
    );
    for index in 2..MAX_REMOTE_PROPAGATION_TARGETS {
        receipt
            .record_propagation(
                HostId::from(format!("host/propagation/{index}")),
                RemotePropagationStatus::Refused,
            )
            .unwrap();
    }
    assert_eq!(
        receipt.record_propagation(
            HostId::from("host/propagation/overflow"),
            RemotePropagationStatus::Delivered
        ),
        Err(RemoteEmergencyRefusal::PropagationCapacity)
    );
}
