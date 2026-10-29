use super::*;
use crate::{
    AuthenticatedHostObservation, Body, BodyBiographyEvidence, BodyLifecycleSession,
    BodyMembership, CandidateObservation, DiscoveryProofId, MembershipProofId, PartId,
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

fn detached_observer() -> (
    BodyLifecycleSession,
    AdmissionManager,
    MembershipCredential,
    SigningKey,
) {
    let mut s = session();
    let (mut m, mut i, p, k) = pending(&s);
    let c = s
        .admit_ambient_host(&mut m, &mut i, &p, 110, &host(), &boot())
        .unwrap();
    s.observe_host_lost(&c.host_id, &c.boot_id, &host(), &boot())
        .unwrap();
    (s, m, c, k)
}
fn start_local(s: &mut BodyLifecycleSession) {
    let plans = s
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
    s.propose(plans, &host(), &boot()).unwrap();
    let current = s.realization.as_ref().unwrap();
    let play = crate::BodyPlayIdentity::bind(&current.plan, 1);
    let sign = |n| conduit_core::bind_sign(&host(), &boot(), Some(&play.active_play_id), n).sign_id;
    let wake = current
        .wake
        .body_plan_ready(&current.plan, sign(0))
        .unwrap()
        .body_play_started(&current.plan, &play, sign(1))
        .unwrap();
    s.started(&host(), &boot(), play, wake).unwrap();
}
fn return_proof(challenge: &crate::PartReturnChallenge, key: &SigningKey) -> PartReturnProof {
    PartReturnProof {
        admission_id: challenge.admission_id.clone(),
        body_id: challenge.body_id.clone(),
        part_id: challenge.part_id.clone(),
        host_id: challenge.host_id.clone(),
        boot_id: challenge.boot_id.clone(),
        nonce: challenge.nonce,
        signature: key.sign(&challenge.signing_transcript()).to_bytes(),
    }
}
#[test]
fn observer_return_preserves_started_play_and_is_consumed_once() {
    let (mut s, mut m, c, key) = detached_observer();
    start_local(&mut s);
    let realized = s.realization.clone();
    let body = s.evidence.body.clone();
    let foreground = s.foreground.clone();
    let fresh = advertisement("boot/browser/fresh");
    let challenge = s
        .issue_observer_return(
            &mut m,
            &c.part_id,
            &fresh,
            [19; 32],
            120,
            200,
            &host(),
            &boot(),
        )
        .unwrap();
    let proof = return_proof(&challenge, &key);
    let returned = s
        .admit_returning_observer(&mut m, &fresh, &proof, 130, &host(), &boot())
        .unwrap();
    assert_eq!(returned.part_id, c.part_id);
    assert_eq!(returned.boot_id, fresh.boot_id);
    assert_eq!(s.realization, realized);
    assert_eq!(s.evidence.body, body);
    assert_eq!(s.foreground, foreground);
    s.evidence.validate().unwrap();
    let before = s.evidence.clone();
    let manager = m.clone();
    assert!(s
        .admit_returning_observer(&mut m, &fresh, &proof, 131, &host(), &boot())
        .is_err());
    assert_eq!(s.evidence, before);
    assert_eq!(m, manager);
}
#[test]
fn observer_return_refusals_are_atomic_during_play() {
    let (mut s, mut m, c, key) = detached_observer();
    start_local(&mut s);
    let fresh = advertisement("boot/browser/fresh");
    let challenge = s
        .issue_observer_return(
            &mut m,
            &c.part_id,
            &fresh,
            [19; 32],
            120,
            200,
            &host(),
            &boot(),
        )
        .unwrap();
    let valid = return_proof(&challenge, &key);
    for case in 0..5 {
        let mut proof = valid.clone();
        let mut ad = fresh.clone();
        let mut now = 130;
        let mut authority = boot();
        match case {
            0 => proof.signature[0] ^= 1,
            1 => ad.boot_id = "boot/stale".into(),
            2 => ad.host_id = "host/wrong".into(),
            3 => now = 201,
            _ => authority = "boot/stale".into(),
        }
        let before = s.evidence.clone();
        let manager = m.clone();
        let realized = s.realization.clone();
        assert!(s
            .admit_returning_observer(&mut m, &ad, &proof, now, &host(), &authority)
            .is_err());
        assert_eq!(s.evidence, before);
        assert_eq!(m, manager);
        assert_eq!(s.realization, realized);
    }
    // A capability-bearing return must stay on the ordinary lifecycle path.
    // A resource-bearing advertisement is rejected by the observer path.
    let mut executable = fresh.clone();
    executable.resources.push(conduit_core::ResourceOffer {
        pool_id: "pool/test".into(),
        class_id: "class/test".into(),
        capacity_units: 1,
        compute: None,
        content: None,
    });
    let before = s.evidence.clone();
    let manager = m.clone();
    assert!(s
        .admit_returning_observer(&mut m, &executable, &valid, 130, &host(), &boot())
        .is_err());
    assert!(s
        .issue_observer_return(
            &mut m,
            &c.part_id,
            &executable,
            [20; 32],
            130,
            200,
            &host(),
            &boot()
        )
        .is_err());
    assert_eq!(s.evidence, before);
    assert_eq!(m, manager);
}

#[test]
fn pending_return_lookup_and_expiry_are_exact_without_resetting_attempts() {
    let (s, mut m, c, key) = detached_observer();
    let fresh = advertisement("boot/browser/fresh");
    assert_eq!(
        m.pending_return_challenge(&s.evidence.membership, &c.part_id, &fresh, 120)
            .unwrap(),
        None
    );
    let challenge = s
        .issue_observer_return(
            &mut m,
            &c.part_id,
            &fresh,
            [19; 32],
            120,
            200,
            &host(),
            &boot(),
        )
        .unwrap();
    let retained = m.clone();
    assert_eq!(
        m.pending_return_challenge(&s.evidence.membership, &c.part_id, &fresh, 150)
            .unwrap(),
        Some(challenge.clone())
    );
    assert_eq!(m, retained);
    for change in 0..3 {
        let mut ad = fresh.clone();
        match change {
            0 => ad.boot_id = "boot/wrong".into(),
            1 => ad.host_id = "host/wrong".into(),
            _ => ad.offer_generation = OfferGeneration(2),
        };
        assert!(m
            .pending_return_challenge(&s.evidence.membership, &c.part_id, &ad, 150)
            .is_err());
        assert_eq!(m, retained);
    }
    assert!(m
        .retire_expired_return(&challenge.admission_id, 200)
        .is_err());
    assert_eq!(m, retained);
    assert!(m
        .pending_return_challenge(&s.evidence.membership, &c.part_id, &fresh, 201)
        .is_err());
    assert_eq!(m, retained);
    m.retire_expired_return(&challenge.admission_id, 201)
        .unwrap();
    let replacement = s
        .issue_observer_return(
            &mut m,
            &c.part_id,
            &fresh,
            [20; 32],
            202,
            300,
            &host(),
            &boot(),
        )
        .unwrap();
    assert_ne!(challenge.admission_id, replacement.admission_id);
    assert_ne!(challenge.nonce, replacement.nonce);
    // Canonical manager counts bad proofs. Lookup and expiry cannot reset a cap.
    let mut bad = return_proof(&replacement, &key);
    bad.signature[0] ^= 1;
    let mut membership = s.evidence.membership.clone();
    for _ in 0..crate::admission::MAX_ADMISSION_ATTEMPTS {
        assert!(m
            .complete_return(&mut membership, &fresh, &bad, 220, "sign/attempt".into())
            .is_err());
    }
    let exhausted = m.clone();
    assert!(m
        .pending_return_challenge(&membership, &c.part_id, &fresh, 230)
        .is_err());
    assert!(m
        .retire_expired_return(&replacement.admission_id, 301)
        .is_err());
    assert_eq!(m, exhausted);
}
