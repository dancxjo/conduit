use super::*;
use crate::durable_host::owner::Owner;
use conduit_body::{
    RendezvousAuthentication, RendezvousCandidate, RendezvousLineFamily, ResidentPlot,
    SpawnInvitationSecret, ROUTED_INVITATION_SCHEMA, SPAWN_ADMISSION_REQUEST_SCHEMA,
};
use conduit_core::{BootId, HostId, OfferGeneration};
use conduit_presentation::{
    OwnerFaceSnapshotRequest, OwnerFaceSnapshotResponse, PresentationRole,
    OWNER_FACE_REQUEST_SCHEMA,
};
use conduit_std_host::{StdHost, StdHostConfig};
use sha2::{Digest, Sha256};
use std::fs;

fn host(id: &str, boot: &str) -> StdHost {
    StdHost::new_with_config(StdHostConfig {
        host_id: HostId::from(id),
        boot_id: BootId::from(boot),
        offer_generation: OfferGeneration(1),
    })
}

#[test]
fn service_resumes_one_body_and_serializes_invitation_admission_on_current_boot() {
    let root = std::env::temp_dir().join(format!(
        "conduit-service-body-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    fs::create_dir_all(&root).unwrap();
    fs::write(
        root.join("installation.json"),
        serde_json::to_vec(&serde_json::json!({
            "schema":"conduit.install/durable-host@1",
            "host_id":"host/service-body-test",
            "release_source_identity":"source/test",
            "release_bundle_sha256":format!("sha256:{:x}", Sha256::digest(b"test bundle")),
            "product_executable":"fixture-unused",
            "body_state":null,
            "joined_body_state":null
        }))
        .unwrap(),
    )
    .unwrap();
    let checked = crate::plot_source::parse(
        "plot hello {\n show: presentation/text\n \"Hello.\" >> show\n}.",
    )
    .unwrap()
    .expand_entry_for_authoring()
    .unwrap();
    let resident = ResidentPlot::new(
        checked.expanded.source_document_id,
        checked.expanded.checked_plot_id,
    );
    let mut first = Owner::open(
        host("host/service-body-test", "boot/first"),
        resident,
        None,
        "One Body",
    )
    .unwrap();
    first.persist(&root).unwrap();
    let body_id = first.truth()["biography"]["body_id"]
        .as_str()
        .unwrap()
        .to_owned();
    drop(first);

    // A leftover marker alone cannot send invitation authority to a
    // second disk writer or claim a current service Boot.
    fs::write(root.join("runtime.json"), b"stale marker").unwrap();
    assert!(crate::durable_host::issue_body_invitation(&root, 60).is_err());
    assert!(!root.join("body/admission.json").exists());
    fs::remove_file(root.join("runtime.json")).unwrap();

    let mut runtime = DurableHostRuntime::new(
        "std/x86_64/computer".into(),
        format!("sha256:{:x}", Sha256::digest(b"host image")),
        host("host/service-body-test", "boot/service"),
    )
    .with_owned_body(&root)
    .unwrap();
    let truth = runtime.owned_body_truth().unwrap();
    assert_eq!(truth["biography"]["body_id"], body_id);
    assert_eq!(truth["host"]["boot_id"], "boot/service");
    assert_eq!(truth["remote_carrier_availability"], "unobserved");
    assert_eq!(
        truth["biography"]["membership"]["parts"][0]["current"]["boot_id"],
        "boot/service"
    );
    let (local_face, local_host) = runtime.owned_body_local_face().unwrap();
    local_face.validate().unwrap();
    assert_eq!(local_face.basis.body_id.as_ref().unwrap().as_str(), body_id);
    assert_eq!(local_host.boot_id.as_str(), "boot/service");
    let token = [9_u8; 32];
    assert!(matches!(
        super::super::handle(
            Request::BodyLocalFace { protocol: PROTOCOL, token: vec![8; 32] },
            &token,
            &mut runtime
        ),
        Response::Refused { code, .. } if code == "unauthorized"
    ));
    assert!(matches!(
        super::super::handle(
            Request::BodyLocalFace { protocol: PROTOCOL, token: token.to_vec() },
            &token,
            &mut runtime
        ),
        Response::BodyLocalFace { presentation, advertisement, .. }
            if *presentation == local_face && advertisement.boot_id == local_host.boot_id
    ));
    assert!(matches!(
        super::super::handle(
            Request::BodyInspect {
                protocol: PROTOCOL,
                token: vec![8; 32]
            },
            &token,
            &mut runtime
        ),
        Response::Refused { code, .. } if code == "unauthorized"
    ));
    let invitation = match super::super::handle(
        Request::BodyInvite {
            protocol: PROTOCOL,
            token: token.to_vec(),
            ttl_seconds: 60,
            candidates: Some(vec![RendezvousCandidate {
                candidate_id: "candidate/test-route".into(),
                line_family: RendezvousLineFamily::AuthenticatedTlsStream,
                reachability: "wss://localhost:8443/owner".into(),
                authentication: RendezvousAuthentication {
                    server_identity: "localhost".into(),
                    transport_binding_sha256: [7; 32],
                },
                expires_at_millis: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap()
                    .as_millis() as u64
                    + 30_000,
                maximum_attempts: 1,
                attempt_timeout_millis: 5_000,
            }]),
        },
        &token,
        &mut runtime,
    ) {
        Response::BodyInvitation { invitation, .. } => *invitation,
        _ => panic!("unexpected invitation response"),
    };
    assert_eq!(invitation.schema, ROUTED_INVITATION_SCHEMA);
    assert_eq!(invitation.rendezvous.as_ref().unwrap().candidates.len(), 1);
    let guest = host("host/guest", "boot/guest").advertisement().clone();
    let secret = SpawnInvitationSecret::from_csprng_bytes(invitation.secret).unwrap();
    let request = PortableSpawnAdmissionRequest {
        schema: SPAWN_ADMISSION_REQUEST_SCHEMA.into(),
        invitation_id: invitation.claim.invitation_id.clone(),
        body_id: invitation.claim.body_id.clone(),
        host_advertisement: guest.clone(),
        nonce: invitation.claim.nonce,
        signature: secret
            .sign(&invitation.claim.signing_transcript(
                &guest.host_id,
                &guest.boot_id,
                guest.offer_generation,
            ))
            .to_vec(),
        membership_admitted: false,
        plan_created: false,
        play_created: false,
    };
    assert!(matches!(
        super::super::handle(
            Request::BodyAdmit {
                protocol: PROTOCOL,
                token: vec![8; 32],
                request: Box::new(request.clone())
            },
            &token,
            &mut runtime
        ),
        Response::Refused { code, .. } if code == "unauthorized"
    ));
    let receipt = match super::super::handle(
        Request::BodyAdmit {
            protocol: PROTOCOL,
            token: token.to_vec(),
            request: Box::new(request.clone()),
        },
        &token,
        &mut runtime,
    ) {
        Response::BodyAdmitted { receipt, .. } => *receipt,
        _ => panic!("unexpected admission response"),
    };
    receipt.validate_against(&request).unwrap();
    let face_request = OwnerFaceSnapshotRequest {
        schema: OWNER_FACE_REQUEST_SCHEMA.into(),
        credential_id: receipt.credential.credential_id.as_str().into(),
        body_id: receipt.credential.body_id.clone(),
        part_id: receipt.credential.part_id.clone(),
        host_id: receipt.credential.host_id.clone(),
        boot_id: receipt.credential.boot_id.clone(),
        last_seen_revision: None,
        last_seen_identity: None,
    };
    let face = match super::super::handle(
        Request::BodyFace {
            protocol: PROTOCOL,
            token: token.to_vec(),
            request: face_request.clone(),
        },
        &token,
        &mut runtime,
    ) {
        Response::BodyFace { response, .. } => match *response {
            OwnerFaceSnapshotResponse::Snapshot {
                presentation,
                interactions_admitted: false,
                ..
            } => *presentation,
            other => panic!("unexpected Face response: {other:?}"),
        },
        _ => panic!("unexpected Face control response"),
    };
    face.validate().unwrap();
    assert_eq!(
        face.basis.body_id.as_ref(),
        Some(&receipt.credential.body_id)
    );
    assert!(face
        .subjects
        .iter()
        .any(|subject| subject.role == PresentationRole::Plot));
    let unchanged = runtime
        .owned_body_face(&OwnerFaceSnapshotRequest {
            last_seen_revision: Some(face.revision),
            last_seen_identity: Some(face.identity.clone()),
            ..face_request.clone()
        })
        .unwrap();
    assert!(matches!(
        unchanged,
        OwnerFaceSnapshotResponse::Unchanged { .. }
    ));
    let mut wrong_boot = face_request;
    wrong_boot.boot_id = BootId::from("boot/stale");
    assert!(matches!(
        runtime.owned_body_face(&wrong_boot),
        Err(code) if code == "owner-face-credential-not-admitted"
    ));
    assert_eq!(
        runtime.owned_body_truth().unwrap()["biography"]["membership"]["parts"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert!(matches!(
        super::super::handle(
            Request::BodyAdmit {
                protocol: PROTOCOL,
                token: token.to_vec(),
                request: Box::new(request)
            },
            &token,
            &mut runtime
        ),
        Response::Refused { .. }
    ));
    let retained: serde_json::Value =
        serde_json::from_slice(&fs::read(root.join("body/biography.json")).unwrap()).unwrap();
    assert_eq!(retained["body_id"], body_id);
    assert_eq!(retained["membership"]["parts"].as_array().unwrap().len(), 2);
    fs::remove_dir_all(root).unwrap();
}
