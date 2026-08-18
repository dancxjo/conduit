use conduit_body::{
    AdmissionManager, AdmissionRefusal, AdmissionSigns, AmbientAdmissionProof, Body,
    BodyMembership, CandidateInventory, CandidateObservation, DiscoveryProofId,
};
use conduit_browser_sim::{BrowserSimConfig, BrowserSimPage};
use conduit_core::{
    BootId, CheckedFormId, HostId, LinkBindingId, OfferGeneration, SignId, SourceDocumentId,
};
use conduit_form::parse_with_startup;
use conduit_signal::signal_profile_catalog;
use ed25519_dalek::{Signer, SigningKey};

fn signs(label: &str) -> AdmissionSigns {
    AdmissionSigns {
        part_admitted: SignId::from(format!("sign/{label}/part")),
        host_attached: SignId::from(format!("sign/{label}/host")),
        candidate_admitted: SignId::from(format!("sign/{label}/candidate")),
    }
}

#[test]
fn hostile_reachable_browser_cannot_mutate_form_plan_or_membership_without_exact_proof() {
    let configs = (0..3).map(|index| BrowserSimConfig {
        host_id: HostId::from(format!("browser/hostile-{index}")),
        boot_id: BootId::from(format!("browser-boot/hostile-{index}")),
        offer_generation: OfferGeneration(1),
    });
    let page = BrowserSimPage::with_hosts(configs);
    let advertisements = page.advertisements();
    let checked = parse_with_startup(
        "form signal-demo {\n pulse: flow/pulse(count = 4, period-ms = 1, initial = false)\n show: presentation/show\n pulse > show\n}\n", &conduit_signal::signal_startup_catalog(), &signal_profile_catalog())
    .unwrap();
    let plan = page
        .plan_pair(
            &checked,
            &advertisements[0].host_id,
            &advertisements[1].host_id,
        )
        .unwrap();
    let retained_plan = plan.clone();
    let retained_checked = checked.clone();
    let body = Body::born(
        SourceDocumentId::from("source/hostile-body"),
        CheckedFormId::from("checked/hostile-body"),
        1,
        SignId::from("sign/hostile-body-born"),
    )
    .unwrap();
    let mut membership = BodyMembership::new(body.body_id.clone()).unwrap();
    let mut candidates = CandidateInventory::new(body.body_id.clone()).unwrap();
    let advertisement = advertisements[2].clone();
    let candidate = candidates
        .observe(CandidateObservation {
            advertisement,
            // Deliberately collides with the trusted local presentation label.
            friendly_label: "This computer".into(),
            observed_binding_id: LinkBindingId::from("line/untrusted-browser-origin"),
            observation_sign_id: SignId::from("sign/hostile-observed"),
            proof_id: DiscoveryProofId::bind("proof/reachability-only").unwrap(),
            freshness_sequence: 1,
            encoded_bytes: 512,
        })
        .unwrap();
    assert!(membership.parts.is_empty());
    assert_eq!(plan, retained_plan);
    assert_eq!(checked, retained_checked);

    let legitimate_key = SigningKey::from_bytes(&[7; 32]);
    let hostile_key = SigningKey::from_bytes(&[8; 32]);
    let mut manager = AdmissionManager::new(body.body_id).unwrap();
    let challenge = manager
        .begin_ambient(
            &mut candidates,
            &candidate,
            legitimate_key.verifying_key().to_bytes(),
            [9; 32],
            1_000,
            2_000,
            SignId::from("sign/hostile-requested"),
        )
        .unwrap();
    let proof_from = |key: &SigningKey| AmbientAdmissionProof {
        admission_id: challenge.admission_id.clone(),
        body_id: challenge.body_id.clone(),
        host_id: challenge.host_id.clone(),
        boot_id: challenge.boot_id.clone(),
        nonce: challenge.nonce,
        signature: key.sign(&challenge.signing_transcript()).to_bytes(),
    };
    assert_eq!(
        manager.complete_ambient(
            &mut candidates,
            &mut membership,
            &proof_from(&hostile_key),
            1_001,
            signs("invalid"),
        ),
        Err(AdmissionRefusal::InvalidProof)
    );
    assert!(membership.parts.is_empty());
    assert_eq!(plan, retained_plan);
    assert_eq!(checked, retained_checked);

    let valid = proof_from(&legitimate_key);
    manager
        .complete_ambient(
            &mut candidates,
            &mut membership,
            &valid,
            1_002,
            signs("valid"),
        )
        .unwrap();
    let retained_membership = membership.clone();
    assert!(matches!(
        manager.complete_ambient(
            &mut candidates,
            &mut membership,
            &valid,
            1_003,
            signs("replay"),
        ),
        Err(AdmissionRefusal::UnknownAdmission | AdmissionRefusal::Replay)
    ));
    assert_eq!(membership, retained_membership);
    assert_eq!(plan, retained_plan);
    assert_eq!(checked, retained_checked);
}
