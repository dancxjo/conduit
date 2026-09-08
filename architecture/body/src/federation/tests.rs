use super::*;
extern crate std;
use crate::{MembershipProofId, MembershipState, PartId};
use alloc::{format, string::ToString};
use conduit_core::{
    ActivePlayId, AuthorityContractId, AuthorityGrant, AuthorityGrantId, BaseCapabilityAuthority,
    BaseCapabilityScope, BaseInstanceId, CapabilityEnvelopeId, CapabilityId,
    CapabilityIssueRequest, HostOperationContractId, ImplementationId, KindId, PlanId,
    ResourceGenerationId, ResourcePoolId,
};
use ed25519_dalek::{Signer, SigningKey};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::thread;

#[derive(Serialize, Deserialize)]
struct WireOperation {
    frame: FederatedOperationFrame,
    signature: Vec<u8>,
}

fn peer(name: &str, key: &SigningKey, generation: u64) -> FederationPeer {
    FederationPeer {
        host_id: HostId::from(format!("host/{name}")),
        boot_id: BootId::from(format!("boot/{name}/{generation}")),
        offer_generation: OfferGeneration(generation),
        verifying_key: key.verifying_key().to_bytes(),
    }
}

fn challenge(
    initiator: FederationPeer,
    responder: FederationPeer,
    suffix: &str,
) -> FederationChallenge {
    FederationChallenge {
        session_id: LinkBindingId::from(format!("session/{suffix}")),
        line_id: LineId::from(format!("line/{suffix}")),
        line_security: conduit_core::LineSecurity::PlaintextNetwork,
        initiator,
        responder,
        nonce: [7; 32],
        expires_at_millis: 1_000,
        maximum_frames: 4,
    }
}

fn establish(
    challenge: FederationChallenge,
    initiator: &SigningKey,
    responder: &SigningKey,
) -> FederatedReceivingSession {
    let transcript = challenge_transcript(&challenge);
    FederatedReceivingSession::establish(
        challenge,
        initiator.sign(&transcript).to_bytes(),
        responder.sign(&transcript).to_bytes(),
        10,
    )
    .unwrap()
}

fn member(peer: &FederationPeer) -> PartMembership {
    PartMembership {
        part_id: PartId::bound(format!("part/{}", peer.host_id.as_str())),
        state: MembershipState::Admitted,
        current: Some(AuthenticatedHostObservation {
            host_id: peer.host_id.clone(),
            boot_id: peer.boot_id.clone(),
            offer_generation: peer.offer_generation,
            proof_id: MembershipProofId::bind("federation-test").unwrap(),
            sequence: 1,
        }),
    }
}

fn scope(receiver: &FederationPeer, base: &str) -> BaseCapabilityScope {
    BaseCapabilityScope {
        host_id: receiver.host_id.clone(),
        boot_id: receiver.boot_id.clone(),
        base_instance_id: BaseInstanceId::from(base.to_string()),
        base_provider_generation: 3,
        plan_id: PlanId::from("plan/remote"),
        active_play_id: ActivePlayId::from("play/remote"),
        authority_grant_id: AuthorityGrantId::from("grant/remote-effect"),
        authority_contract_id: AuthorityContractId::from("authority/remote-effect@1"),
        capability_id: CapabilityId::from("remote/effect"),
        implementation_id: ImplementationId::from("base/remote-effect@1"),
        operation_contract_id: HostOperationContractId::from("host/remote-effect@1"),
        subject_kind: KindId::from("effect/exact"),
        resource_pool_id: ResourcePoolId::from("resource/exact"),
        resource_generation_id: ResourceGenerationId("resource/generation/3".into()),
        envelope_id: CapabilityEnvelopeId::from("envelope/exact"),
        maximum_parameter_bytes: 8,
        maximum_result_bytes: 8,
        maximum_work_units: 1,
        maximum_in_flight: 1,
        maximum_operations: 4,
    }
}

fn capability(
    receiver: &FederationPeer,
    base: &str,
    key: u8,
) -> (
    BaseCapabilityTable,
    BaseCapabilityHandle,
    BaseOperationClaim,
) {
    let scope = scope(receiver, base);
    let authority = BaseCapabilityAuthority {
        grant: AuthorityGrant {
            grant_id: scope.authority_grant_id.clone(),
            contract_id: scope.authority_contract_id.clone(),
            host_operation_contract_id: scope.operation_contract_id.clone(),
            subject_kind: scope.subject_kind.clone(),
            host_id: scope.host_id.clone(),
            boot_id: scope.boot_id.clone(),
            capability_id: scope.capability_id.clone(),
        },
        base_instance_id: scope.base_instance_id.clone(),
        base_provider_generation: scope.base_provider_generation,
        resource_pool_id: scope.resource_pool_id.clone(),
        resource_generation_id: scope.resource_generation_id.clone(),
        operation_contract_id: scope.operation_contract_id.clone(),
        envelope_id: scope.envelope_id.clone(),
        maximum_parameter_bytes: 8,
        maximum_result_bytes: 8,
        maximum_work_units: 1,
        maximum_in_flight: 1,
        maximum_operations: 4,
    };
    let mut table = BaseCapabilityTable::new(
        scope.host_id.clone(),
        scope.boot_id.clone(),
        scope.base_instance_id.clone(),
        scope.base_provider_generation,
        [key; 32],
        1,
    )
    .unwrap();
    let handle = table
        .issue(CapabilityIssueRequest {
            scope: scope.clone(),
            authority,
        })
        .unwrap();
    let claim = BaseOperationClaim {
        host_id: scope.host_id,
        boot_id: scope.boot_id,
        base_instance_id: scope.base_instance_id,
        base_provider_generation: scope.base_provider_generation,
        plan_id: scope.plan_id,
        active_play_id: scope.active_play_id,
        implementation_id: scope.implementation_id,
        operation_contract_id: scope.operation_contract_id,
        subject_kind: scope.subject_kind,
        resource_pool_id: scope.resource_pool_id,
        resource_generation_id: scope.resource_generation_id,
        envelope_id: scope.envelope_id,
        parameter_bytes: 8,
        work_units: 1,
    };
    (table, handle, claim)
}

fn frame(
    challenge: &FederationChallenge,
    sequence: u32,
    claim: BaseOperationClaim,
) -> FederatedOperationFrame {
    FederatedOperationFrame {
        session_id: challenge.session_id.clone(),
        line_id: challenge.line_id.clone(),
        line_security: challenge.line_security,
        sender_host_id: challenge.initiator.host_id.clone(),
        sender_boot_id: challenge.initiator.boot_id.clone(),
        receiver_host_id: challenge.responder.host_id.clone(),
        receiver_boot_id: challenge.responder.boot_id.clone(),
        sequence,
        claim,
    }
}

#[test]
fn authentication_membership_and_effect_authority_remain_independent() {
    let a_key = SigningKey::from_bytes(&[1; 32]);
    let b_key = SigningKey::from_bytes(&[2; 32]);
    let a = peer("a", &a_key, 1);
    let b = peer("b", &b_key, 1);
    let challenge = challenge(a.clone(), b.clone(), "a-b");
    let mut session = establish(challenge.clone(), &a_key, &b_key);
    let (mut table, handle, claim) = capability(&b, "base/b/effect", 8);
    let first = frame(&challenge, 0, claim.clone());
    let signature = a_key.sign(&operation_transcript(&first)).to_bytes();

    assert_eq!(
        session.authorize(&first, signature, 20, &[], &mut table, &handle),
        Err(FederationRefusal::PeerNotCurrentMember)
    );
    let membership = [member(&a)];
    let lease = session
        .authorize(&first, signature, 20, &membership, &mut table, &handle)
        .unwrap();
    table.complete(lease, 1).unwrap();
    assert_eq!(
        session.authorize(&first, signature, 20, &membership, &mut table, &handle),
        Err(FederationRefusal::Replay)
    );

    let exact_claim = claim.clone();
    let mut broadened = frame(&challenge, 1, claim);
    broadened.claim.subject_kind = KindId::from("effect/sibling");
    let signature = a_key.sign(&operation_transcript(&broadened)).to_bytes();
    assert_eq!(
        session.authorize(&broadened, signature, 20, &membership, &mut table, &handle),
        Err(FederationRefusal::CapabilityRefused)
    );
    let mut wrong_operation = frame(&challenge, 1, exact_claim.clone());
    wrong_operation.claim.operation_contract_id =
        HostOperationContractId::from("host/sibling-effect@1");
    let wrong_operation_signature = a_key
        .sign(&operation_transcript(&wrong_operation))
        .to_bytes();
    assert_eq!(
        session.authorize(
            &wrong_operation,
            wrong_operation_signature,
            20,
            &membership,
            &mut table,
            &handle,
        ),
        Err(FederationRefusal::CapabilityRefused)
    );
    let mut stale_play = frame(&challenge, 1, exact_claim);
    stale_play.claim.active_play_id = ActivePlayId::from("play/stale");
    let stale_play_signature = a_key.sign(&operation_transcript(&stale_play)).to_bytes();
    assert_eq!(
        session.authorize(
            &stale_play,
            stale_play_signature,
            20,
            &membership,
            &mut table,
            &handle,
        ),
        Err(FederationRefusal::CapabilityRefused)
    );
    assert!(session.inspection(&membership).membership_current);
    session.revoke_credential();
    assert_eq!(
        session.authorize(&broadened, signature, 20, &membership, &mut table, &handle,),
        Err(FederationRefusal::CredentialRevoked)
    );
}

#[test]
fn three_peer_compromise_is_non_transitive_and_boot_revocation_fence_reuse() {
    let a_key = SigningKey::from_bytes(&[1; 32]);
    let b_key = SigningKey::from_bytes(&[2; 32]);
    let c_key = SigningKey::from_bytes(&[3; 32]);
    let a = peer("a", &a_key, 1);
    let b = peer("b", &b_key, 1);
    let c = peer("c", &c_key, 1);
    let bc = challenge(b.clone(), c.clone(), "b-c");
    let mut c_receiver = establish(bc.clone(), &b_key, &c_key);
    let (mut c_table, c_handle, c_claim) = capability(&c, "base/c/effect", 9);
    let members = [member(&b)];

    let mut redirected = frame(&bc, 0, c_claim.clone());
    redirected.sender_host_id = a.host_id.clone();
    redirected.sender_boot_id = a.boot_id.clone();
    let a_signature = a_key.sign(&operation_transcript(&redirected)).to_bytes();
    assert_eq!(
        c_receiver.authorize(
            &redirected,
            a_signature,
            20,
            &members,
            &mut c_table,
            &c_handle,
        ),
        Err(FederationRefusal::Redirected)
    );

    let mut wrong_resource = frame(&bc, 0, c_claim.clone());
    wrong_resource.claim.resource_pool_id = ResourcePoolId::from("resource/a-b-only");
    let b_signature = b_key
        .sign(&operation_transcript(&wrong_resource))
        .to_bytes();
    assert_eq!(
        c_receiver.authorize(
            &wrong_resource,
            b_signature,
            20,
            &members,
            &mut c_table,
            &c_handle,
        ),
        Err(FederationRefusal::CapabilityRefused)
    );

    c_table.revoke(&c_handle).unwrap();
    let valid = frame(&bc, 0, c_claim.clone());
    let signature = b_key.sign(&operation_transcript(&valid)).to_bytes();
    assert_eq!(
        c_receiver.authorize(&valid, signature, 20, &members, &mut c_table, &c_handle),
        Err(FederationRefusal::CapabilityRefused)
    );

    let mut stale_boot = valid;
    stale_boot.sender_boot_id = BootId::from("boot/b/replaced");
    let signature = b_key.sign(&operation_transcript(&stale_boot)).to_bytes();
    assert_eq!(
        c_receiver.authorize(
            &stale_boot,
            signature,
            20,
            &members,
            &mut c_table,
            &c_handle,
        ),
        Err(FederationRefusal::StaleBoot)
    );
}

#[test]
fn loopback_transport_delivers_only_an_authenticated_authorized_frame() {
    let a_key = SigningKey::from_bytes(&[11; 32]);
    let b_key = SigningKey::from_bytes(&[12; 32]);
    let a = peer("loopback-a", &a_key, 1);
    let b = peer("loopback-b", &b_key, 1);
    let challenge = challenge(a.clone(), b.clone(), "loopback-a-b");
    let frame_challenge = challenge.clone();
    let transcript = challenge_transcript(&challenge);
    let a_session_signature = a_key.sign(&transcript).to_bytes();
    let b_session_signature = b_key.sign(&transcript).to_bytes();
    let (mut table, handle, claim) = capability(&b, "base/b/loopback", 14);
    let membership = [member(&a)];
    let listener = TcpListener::bind((std::net::Ipv4Addr::LOCALHOST, 0)).unwrap();
    let address = listener.local_addr().unwrap();

    let receiver = thread::spawn(move || {
        let mut session = FederatedReceivingSession::establish(
            challenge,
            a_session_signature,
            b_session_signature,
            10,
        )
        .unwrap();
        let (mut stream, _) = listener.accept().unwrap();
        let mut encoded = Vec::new();
        stream.read_to_end(&mut encoded).unwrap();
        let wire: WireOperation = serde_json::from_slice(&encoded).unwrap();
        let signature: [u8; 64] = wire.signature.try_into().unwrap();
        let lease = session
            .authorize(&wire.frame, signature, 20, &membership, &mut table, &handle)
            .unwrap();
        table.complete(lease, 1).unwrap();
        session.inspection(&membership)
    });

    let frame = frame(&frame_challenge, 0, claim);
    let wire = WireOperation {
        signature: a_key
            .sign(&operation_transcript(&frame))
            .to_bytes()
            .to_vec(),
        frame,
    };
    let mut stream = TcpStream::connect(address).unwrap();
    stream
        .write_all(&serde_json::to_vec(&wire).unwrap())
        .unwrap();
    stream.shutdown(Shutdown::Write).unwrap();
    let inspection = receiver.join().unwrap();
    assert_eq!(inspection.authenticated_peer_host, a.host_id);
    assert!(inspection.membership_current);
    assert_eq!(inspection.next_sequence, 1);
}
