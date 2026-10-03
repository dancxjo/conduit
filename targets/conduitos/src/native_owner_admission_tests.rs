use super::*;
use alloc::{boxed::Box, string::ToString, vec, vec::Vec};
use conduit_body::{
    AdmissionManager, AdmissionSigns, Body, BodyMembership, PortableSpawnAdmissionRequest,
    SPAWN_ADMISSION_RECEIPT_SCHEMA, SPAWN_ADMISSION_REQUEST_SCHEMA, SpawnInvitationSecret,
};
use conduit_body::{RendezvousAuthentication, RendezvousCandidate};
use conduit_core::{OfferGeneration, PROTOCOL_VERSION, SignId};
use conduit_presentation::{
    Face, FaceContext, FaceFocus, OWNER_FACE_RESPONSE_SCHEMA, OwnerFaceSnapshotResponse,
};
use sha2::{Digest, Sha256};

const NOW: u64 = 10_000;

struct ScriptedLine {
    response: Vec<u8>,
    sent: Vec<u8>,
}

impl BinaryWebSocketIo for ScriptedLine {
    fn send_binary(&mut self, payload: &[u8]) -> Result<(), WebSocketError> {
        self.sent.extend_from_slice(payload);
        Ok(())
    }

    fn receive_binary(&mut self, output: &mut [u8]) -> Result<usize, WebSocketError> {
        if self.response.len() > output.len() {
            return Err(WebSocketError::ResponseTooLarge);
        }
        output[..self.response.len()].copy_from_slice(&self.response);
        Ok(self.response.len())
    }
}

fn admitted_exchange() -> (RoutedAdmissionRequest, PortableAdmissionReceipt) {
    let body = Body::born(
        "source/native-route".into(),
        "checked/native-route".into(),
        1,
        SignId::from("sign/native-route-born"),
    )
    .unwrap();
    let mut manager = AdmissionManager::new(body.body_id.clone()).unwrap();
    let invitation = manager
        .issue_spawn_invitation(
            SpawnInvitationSecret::from_csprng_bytes([11; 32]).unwrap(),
            [12; 32],
            NOW,
            NOW + 1_000,
        )
        .unwrap();
    let advertisement = HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: "host/native".into(),
        boot_id: "boot/native".into(),
        offer_generation: OfferGeneration(1),
        profile: "profile/native".into(),
        bases: vec![],
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    };
    let signature = invitation.secret.sign(&invitation.signing_transcript(
        &advertisement.host_id,
        &advertisement.boot_id,
        advertisement.offer_generation,
    ));
    let request = PortableSpawnAdmissionRequest {
        schema: SPAWN_ADMISSION_REQUEST_SCHEMA.into(),
        invitation_id: invitation.invitation_id.clone(),
        body_id: invitation.body_id.clone(),
        host_advertisement: advertisement.clone(),
        nonce: invitation.nonce,
        signature: signature.to_vec(),
        membership_admitted: false,
        plan_created: false,
        play_created: false,
    };
    let mut membership = BodyMembership::new(body.body_id).unwrap();
    let credential = manager
        .complete_spawn(
            &mut membership,
            &advertisement,
            &request.admission_proof().unwrap(),
            NOW + 1,
            AdmissionSigns {
                part_admitted: "sign/native-part".into(),
                host_attached: "sign/native-host".into(),
                candidate_admitted: "sign/native-candidate".into(),
            },
        )
        .unwrap();
    let receipt = PortableAdmissionReceipt {
        schema: SPAWN_ADMISSION_RECEIPT_SCHEMA.into(),
        credential,
        host_advertisement: advertisement,
        membership_admitted: true,
        current_offers_available: false,
        plan_created: false,
        play_created: false,
    };
    (
        RoutedAdmissionRequest {
            schema: ROUTED_ADMISSION_REQUEST_SCHEMA.into(),
            invitation_id: request.invitation_id.as_str().into(),
            request,
        },
        receipt,
    )
}

#[test]
fn exact_owner_receipt_correlates_with_the_single_sent_request() {
    let (request, receipt) = admitted_exchange();
    let response = RoutedAdmissionResponse::Admitted {
        schema: ROUTED_ADMISSION_RESPONSE_SCHEMA.into(),
        receipt: Box::new(receipt.clone()),
    };
    let mut line = ScriptedLine {
        response: serde_json::to_vec(&response).unwrap(),
        sent: Vec::new(),
    };
    let result = exchange_document(&mut line, &request, &request.request.host_advertisement)
        .expect("authenticated owner receipt is exact");
    assert_eq!(result.credential, receipt.credential);
    assert_eq!(line.sent, serde_json::to_vec(&request).unwrap());
}

fn owner_face(source: &str) -> conduit_presentation::Presentation {
    let body = Body::born(
        source.into(),
        "checked/native-route".into(),
        1,
        SignId::from("sign/native-route-born"),
    )
    .unwrap();
    Face::project(
        &body,
        None,
        7,
        FaceContext::Overview,
        FaceFocus::Body,
        vec![],
    )
    .unwrap()
    .presentation
}

#[test]
fn exact_owner_face_follows_receipt_on_the_authenticated_line() {
    let (_, receipt) = admitted_exchange();
    let face = owner_face("source/native-route");
    assert_eq!(
        face.basis.body_id.as_ref(),
        Some(&receipt.credential.body_id)
    );
    let mut line = ScriptedLine {
        response: serde_json::to_vec(&OwnerFaceSnapshotResponse::Snapshot {
            schema: OWNER_FACE_RESPONSE_SCHEMA.into(),
            presentation: Box::new(face.clone()),
            interactions_admitted: false,
        })
        .unwrap(),
        sent: Vec::new(),
    };
    let accepted = exchange_face(&mut line, &receipt).unwrap();
    assert_eq!(accepted.presentation(), &face);
    let request: OwnerFaceSnapshotRequest = serde_json::from_slice(&line.sent).unwrap();
    assert!(request.has_exact_basis());
    assert_eq!(
        request.credential_id,
        receipt.credential.credential_id.as_str()
    );
    assert_eq!(request.body_id, receipt.credential.body_id);
    assert_eq!(request.host_id, receipt.credential.host_id);
    assert_eq!(request.boot_id, receipt.credential.boot_id);
    assert!(request.last_seen_revision.is_none());
}

#[test]
fn foreign_body_or_unadmitted_interaction_route_cannot_become_shared_face() {
    let (_, receipt) = admitted_exchange();
    let mut line = ScriptedLine {
        response: serde_json::to_vec(&OwnerFaceSnapshotResponse::Snapshot {
            schema: OWNER_FACE_RESPONSE_SCHEMA.into(),
            presentation: Box::new(owner_face("source/other-route")),
            interactions_admitted: false,
        })
        .unwrap(),
        sent: Vec::new(),
    };
    assert!(matches!(
        exchange_face(&mut line, &receipt),
        Err(NativeOwnerFaceExchangeRefusal::Face(
            GuestFaceRefusal::BodyBasis
        ))
    ));
    line.response = serde_json::to_vec(&OwnerFaceSnapshotResponse::Snapshot {
        schema: OWNER_FACE_RESPONSE_SCHEMA.into(),
        presentation: Box::new(owner_face("source/native-route")),
        interactions_admitted: true,
    })
    .unwrap();
    assert!(matches!(
        exchange_face(&mut line, &receipt),
        Err(NativeOwnerFaceExchangeRefusal::Face(
            GuestFaceRefusal::InteractionRoute
        ))
    ));
}

#[test]
fn owner_face_refusal_and_frame_pressure_leave_admission_receipt_independent() {
    let (_, receipt) = admitted_exchange();
    let mut line = ScriptedLine {
        response: serde_json::to_vec(&OwnerFaceSnapshotResponse::Refused {
            schema: OWNER_FACE_RESPONSE_SCHEMA.into(),
            code: "face-frame-pressure".into(),
        })
        .unwrap(),
        sent: Vec::new(),
    };
    assert!(matches!(
        exchange_face(&mut line, &receipt),
        Err(NativeOwnerFaceExchangeRefusal::Face(GuestFaceRefusal::OwnerRefused(code)))
            if code == "face-frame-pressure"
    ));
    line.response = vec![0; MAX_OWNER_FACE_SNAPSHOT_FRAME_BYTES + 1];
    assert!(matches!(
        exchange_face(&mut line, &receipt),
        Err(NativeOwnerFaceExchangeRefusal::Receive(
            WebSocketError::ResponseTooLarge
        ))
    ));
    assert!(receipt.membership_admitted);
}

#[test]
fn stale_host_and_oversized_request_refuse_before_transmission() {
    let (mut request, receipt) = admitted_exchange();
    let response = RoutedAdmissionResponse::Admitted {
        schema: ROUTED_ADMISSION_RESPONSE_SCHEMA.into(),
        receipt: Box::new(receipt),
    };
    let mut line = ScriptedLine {
        response: serde_json::to_vec(&response).unwrap(),
        sent: Vec::new(),
    };
    let mut changed = request.request.host_advertisement.clone();
    changed.boot_id = "boot/replaced".into();
    assert!(matches!(
        exchange_document(&mut line, &request, &changed),
        Err(NativeOwnerAdmissionRefusal::RequestBasis)
    ));
    assert!(line.sent.is_empty());
    request.request.host_advertisement.profile = "x".repeat(MAXIMUM_BINARY_MESSAGE_BYTES).into();
    let current = request.request.host_advertisement.clone();
    assert!(matches!(
        exchange_document(&mut line, &request, &current),
        Err(NativeOwnerAdmissionRefusal::RequestPressure)
    ));
    assert!(line.sent.is_empty());
}

#[test]
fn owner_refusal_and_wrong_receipt_never_become_membership() {
    let (request, mut receipt) = admitted_exchange();
    let current = request.request.host_advertisement.clone();
    let mut line = ScriptedLine {
        response: serde_json::to_vec(&RoutedAdmissionResponse::Refused {
            schema: ROUTED_ADMISSION_RESPONSE_SCHEMA.into(),
            code: "invitation-expired".to_string(),
        })
        .unwrap(),
        sent: Vec::new(),
    };
    assert!(matches!(
        exchange_document(&mut line, &request, &current),
        Err(NativeOwnerAdmissionRefusal::OwnerRefused(code))
            if code == "invitation-expired"
    ));
    receipt.host_advertisement.boot_id = "boot/other".into();
    line.response = serde_json::to_vec(&RoutedAdmissionResponse::Admitted {
        schema: ROUTED_ADMISSION_RESPONSE_SCHEMA.into(),
        receipt: Box::new(receipt),
    })
    .unwrap();
    assert!(matches!(
        exchange_document(&mut line, &request, &current),
        Err(NativeOwnerAdmissionRefusal::Receipt(
            AdmissionDocumentRefusal::ReceiptBasis
        ))
    ));
}

#[test]
fn route_candidate_must_match_the_invitation_endpoint_and_certificate() {
    let (request, _) = admitted_exchange();
    let certificate_der = vec![1, 2, 3, 4];
    let route = SpawnRendezvousDescriptor {
        protocol: RENDEZVOUS_DESCRIPTOR_PROTOCOL,
        body_id: request.request.body_id.as_str().into(),
        invitation_id: request.invitation_id.clone(),
        candidates: vec![RendezvousCandidate {
            candidate_id: "candidate/owner".into(),
            line_family: RendezvousLineFamily::AuthenticatedTlsStream,
            reachability: "wss://owner.example:8443/conduit".into(),
            authentication: RendezvousAuthentication {
                server_identity: "owner.example".into(),
                transport_binding_sha256: Sha256::digest(&certificate_der).into(),
            },
            expires_at_millis: NOW + 1_000,
            maximum_attempts: 1,
            attempt_timeout_millis: 100,
        }],
    };
    let certificate = RouteCertificate {
        candidate_id: "candidate/owner".into(),
        certificate_der,
    };
    let endpoint = VirtioTcpEndpoint {
        guest_address: [10, 0, 2, 15],
        prefix_length: 24,
        gateway: [10, 0, 2, 2],
        remote_address: [10, 0, 2, 2],
        remote_port: 8443,
        local_port: 48111,
    };
    assert!(matches!(
        validate_candidate(&route, &certificate, &request, endpoint, 50),
        Ok(_)
    ));
    let mut wrong = route.clone();
    wrong.body_id = "body/other".into();
    assert!(matches!(
        validate_candidate(&wrong, &certificate, &request, endpoint, 50),
        Err(NativeOwnerAdmissionRefusal::RouteIdentity)
    ));
    let mut wrong_certificate = certificate.clone();
    wrong_certificate.certificate_der[0] ^= 1;
    assert!(matches!(
        validate_candidate(&route, &wrong_certificate, &request, endpoint, 50),
        Err(NativeOwnerAdmissionRefusal::CertificateBinding)
    ));
    let wrong_port = VirtioTcpEndpoint {
        remote_port: 8444,
        ..endpoint
    };
    assert!(matches!(
        validate_candidate(&route, &certificate, &request, wrong_port, 50),
        Err(NativeOwnerAdmissionRefusal::EndpointBinding)
    ));
    assert!(matches!(
        validate_candidate(&route, &certificate, &request, endpoint, 0),
        Err(NativeOwnerAdmissionRefusal::EndpointBinding)
    ));

    let mut numeric = route.clone();
    numeric.candidates[0].reachability = "wss://10.0.2.100:8443/conduit".into();
    let numeric_endpoint = VirtioTcpEndpoint {
        remote_address: [10, 0, 2, 100],
        ..endpoint
    };
    assert!(matches!(
        validate_candidate(&numeric, &certificate, &request, numeric_endpoint, 50),
        Ok(_)
    ));
    assert!(matches!(
        validate_candidate(&numeric, &certificate, &request, endpoint, 50),
        Err(NativeOwnerAdmissionRefusal::EndpointBinding)
    ));

    // Poll pressure is an independent work cap, not an elapsed millisecond.
    numeric.candidates[0].attempt_timeout_millis = 1;
    assert!(matches!(
        validate_candidate(&numeric, &certificate, &request, numeric_endpoint, 1_000),
        Ok(_)
    ));
    assert!(matches!(
        validate_candidate(&numeric, &certificate, &request, numeric_endpoint, 50),
        Ok(_)
    ));
    numeric.candidates[0].attempt_timeout_millis = 0;
    assert!(matches!(
        validate_candidate(&numeric, &certificate, &request, numeric_endpoint, 1_000),
        Err(NativeOwnerAdmissionRefusal::EndpointBinding)
    ));
}
