use super::*;
use crate::{
    AuthenticatedHostObservation, Body, BodyBiographyEvidence, BodyMembership,
    CandidateObservation, DiscoveryProofId, MembershipProofId, PartId,
};
use alloc::vec;
use conduit_core::{HostProfileId, OfferGeneration, PROTOCOL_VERSION};
use ed25519_dalek::{Signer, SigningKey};
fn host() -> HostId {
    "host/owner".into()
}
fn boot() -> BootId {
    "boot/owner".into()
}
fn session() -> BodyLifecycleSession {
    let body = Body::born(
        "source/test".into(),
        "checked/test".into(),
        1,
        "sign/birth".into(),
    )
    .unwrap();
    let mut membership = BodyMembership::new(body.body_id.clone()).unwrap();
    let mut evidence =
        BodyBiographyEvidence::born(body.clone(), membership.clone(), "Same Body".into()).unwrap();
    let part = PartId::bind(&body.body_id, "owner", 1).unwrap();
    let proof = MembershipProofId::bind("proof/owner").unwrap();
    let admitted = membership
        .admit(
            &body.body_id,
            membership.revision,
            part.clone(),
            proof.clone(),
            "sign/admit".into(),
        )
        .unwrap();
    let present = membership
        .observe_present(
            &body.body_id,
            membership.revision,
            &part,
            AuthenticatedHostObservation {
                host_id: host(),
                boot_id: boot(),
                offer_generation: OfferGeneration(1),
                proof_id: proof,
                sequence: 1,
            },
            "sign/present".into(),
        )
        .unwrap();
    evidence
        .append_membership_events(membership, &[(admitted, 2), (present, 3)])
        .unwrap();
    BodyLifecycleSession::open(evidence).unwrap()
}
fn advertisement(boot: &str) -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: "host/browser".into(),
        boot_id: boot.into(),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("profile/test"),
        bases: vec![],
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    }
}
fn pending(
    session: &BodyLifecycleSession,
) -> (
    AdmissionManager,
    CandidateInventory,
    AmbientAdmissionProof,
    SigningKey,
) {
    let key = SigningKey::from_bytes(&[7; 32]);
    let mut inventory = CandidateInventory::new(session.evidence.body_id.clone()).unwrap();
    let candidate = inventory
        .observe(CandidateObservation {
            advertisement: advertisement("boot/browser/1"),
            friendly_label: "Browser".into(),
            observed_binding_id: "line/test".into(),
            observation_sign_id: "sign/observe".into(),
            proof_id: DiscoveryProofId::bind("proof/test").unwrap(),
            freshness_sequence: 1,
            encoded_bytes: 512,
        })
        .unwrap();
    let mut manager = AdmissionManager::new(session.evidence.body_id.clone()).unwrap();
    let challenge = manager
        .begin_ambient(
            &mut inventory,
            &candidate,
            key.verifying_key().to_bytes(),
            [9; 32],
            100,
            200,
            "sign/request".into(),
        )
        .unwrap();
    let signature = key.sign(&challenge.signing_transcript()).to_bytes();
    let proof = AmbientAdmissionProof {
        admission_id: challenge.admission_id,
        body_id: challenge.body_id,
        host_id: challenge.host_id,
        boot_id: challenge.boot_id,
        nonce: challenge.nonce,
        signature,
    };
    (manager, inventory, proof, key)
}
#[test]
fn admission_loss_and_fresh_boot_return_preserve_body_and_workset() {
    let mut session = session();
    let body = session.evidence.body.clone();
    let foreground = session.foreground.clone();
    let (mut manager, mut inventory, proof, key) = pending(&session);
    let credential = session
        .admit_ambient_host(&mut manager, &mut inventory, &proof, 110, &host(), &boot())
        .unwrap();
    assert_eq!(session.evidence.body, body);
    assert_eq!(session.foreground, foreground);
    assert!(session.realization.is_none());
    assert_eq!(session.evidence.membership.parts.len(), 2);
    session
        .observe_host_lost(&credential.host_id, &credential.boot_id, &host(), &boot())
        .unwrap();
    let fresh = advertisement("boot/browser/2");
    let challenge = manager
        .begin_return(
            &session.evidence.membership,
            &credential.part_id,
            &fresh,
            [3; 32],
            120,
            200,
        )
        .unwrap();
    let signature = key.sign(&challenge.signing_transcript()).to_bytes();
    let proof = PartReturnProof {
        admission_id: challenge.admission_id,
        body_id: challenge.body_id,
        part_id: challenge.part_id,
        host_id: challenge.host_id,
        boot_id: challenge.boot_id,
        nonce: challenge.nonce,
        signature,
    };
    let returned = session
        .admit_returning_host(&mut manager, &fresh, &proof, 130, &host(), &boot())
        .unwrap();
    assert_eq!(returned.part_id, credential.part_id);
    assert_eq!(returned.boot_id, fresh.boot_id);
    assert_eq!(session.evidence.body, body);
    assert_eq!(session.foreground, foreground);
    session.evidence.validate().unwrap();
}
#[test]
fn invalid_signature_and_wrong_authority_do_not_mutate_any_state() {
    let mut session = session();
    let (mut manager, mut inventory, mut proof, _) = pending(&session);
    let before = session.evidence.clone();
    let manager_before = manager.clone();
    let inventory_before = inventory.clone();
    assert_eq!(
        session.admit_ambient_host(
            &mut manager,
            &mut inventory,
            &proof,
            110,
            &host(),
            &"boot/stale".into()
        ),
        Err(Error::StaleHost)
    );
    proof.signature[0] ^= 1;
    assert!(session
        .admit_ambient_host(&mut manager, &mut inventory, &proof, 110, &host(), &boot())
        .is_err());
    assert_eq!(session.evidence, before);
    assert_eq!(manager, manager_before);
    assert_eq!(inventory, inventory_before);
}
#[test]
fn pending_realization_refuses_browser_admission_without_resetting_it() {
    let mut session = session();
    let (mut manager, mut inventory, proof, _) = pending(&session);
    let plans = session
        .evidence
        .body
        .workset
        .plots()
        .iter()
        .map(|plot| crate::BodyPlotPlan {
            plot: plot.clone(),
            plan: conduit_core::seal_plan(
                conduit_core::PlotIdentity {
                    source_document_id: plot.source_document_id.clone(),
                    checked_plot_id: plot.checked_plot_id.clone(),
                    expanded_plot_id: "expanded/test".into(),
                },
                vec![],
            ),
        })
        .collect();
    session.propose(plans, &host(), &boot()).unwrap();
    let before = session.realization.clone();
    assert_eq!(
        session.admit_ambient_host(&mut manager, &mut inventory, &proof, 110, &host(), &boot()),
        Err(Error::NotLulled)
    );
    assert_eq!(session.realization, before);
}
