use conduit_body::{
    AdmissionManager, AdmissionRefusal, AdmissionSigns, AmbientAdmissionProof, Body,
    BodyMembership, CandidateInventory, CandidateObservation, CandidateState, DiscoveryProofId,
};
use conduit_core::{
    BootId, CheckedFormId, HostId, LinkBindingId, OfferGeneration, SignId, SourceDocumentId,
};
use conduit_form::parse;
use conduit_pico_sim::{PicoSim, PicoSimConfig};
use conduit_signal::signal_profile_catalog;
use ed25519_dalek::{Signer, SigningKey};

fn pico() -> PicoSim {
    PicoSim::new(PicoSimConfig {
        host_id: HostId::from("pico/provisioned-1"),
        boot_id: BootId::from("pico-boot/1"),
        offer_generation: OfferGeneration(7),
    })
}

fn form() -> conduit_form::CheckedForm {
    parse(
        "form 0\n\nsignal-demo {\n pulse: flow/pulse\n show: presentation/show\n pulse.count = 2\n pulse.period-ms = 1\n pulse.initial = false\n pulse > show\n}\n",
        &signal_profile_catalog(),
    )
    .unwrap()
}

fn signs() -> AdmissionSigns {
    AdmissionSigns {
        part_admitted: SignId::from("sign/pico/part-admitted"),
        host_attached: SignId::from("sign/pico/host-attached"),
        candidate_admitted: SignId::from("sign/pico/candidate-admitted"),
    }
}

#[test]
fn provisioned_pico_is_inert_until_proof_then_eligible_for_an_ordinary_plan() {
    let pico = pico();
    let advertisement = pico.advertisement().clone();
    assert_eq!(advertisement.capabilities.len(), 2);
    let body = Body::born(
        SourceDocumentId::from("source/pico-body"),
        CheckedFormId::from("checked/pico-body"),
        1,
        SignId::from("sign/pico-body-born"),
    )
    .unwrap();
    let mut candidates = CandidateInventory::new(body.body_id.clone()).unwrap();
    let candidate = candidates
        .observe(CandidateObservation {
            advertisement: advertisement.clone(),
            friendly_label: "USB Pico W".into(),
            observed_binding_id: LinkBindingId::from("line/usb-cdc/provisioned-pico"),
            observation_sign_id: SignId::from("sign/pico-observed"),
            proof_id: DiscoveryProofId::bind("proof/usb-cdc-observation").unwrap(),
            freshness_sequence: 1,
            encoded_bytes: 768,
        })
        .unwrap();
    let mut membership = BodyMembership::new(body.body_id.clone()).unwrap();
    assert!(membership.parts.is_empty());
    assert_eq!(candidates.candidates[0].state, CandidateState::Discovered);

    let key = SigningKey::from_bytes(&[41; 32]);
    let mut manager = AdmissionManager::new(body.body_id).unwrap();
    let challenge = manager
        .begin_ambient(
            &mut candidates,
            &candidate,
            key.verifying_key().to_bytes(),
            [42; 32],
            1_000,
            2_000,
            SignId::from("sign/pico-admission-requested"),
        )
        .unwrap();
    assert!(membership.parts.is_empty());

    let proof = AmbientAdmissionProof {
        admission_id: challenge.admission_id.clone(),
        body_id: challenge.body_id.clone(),
        host_id: challenge.host_id.clone(),
        boot_id: challenge.boot_id.clone(),
        nonce: challenge.nonce,
        signature: key.sign(&challenge.signing_transcript()).to_bytes(),
    };
    manager
        .complete_ambient(&mut candidates, &mut membership, &proof, 1_001, signs())
        .unwrap();
    assert_eq!(membership.parts.len(), 1);
    let current = membership.parts[0].current.as_ref().unwrap();
    assert_eq!(current.host_id, advertisement.host_id);
    assert_eq!(current.boot_id, advertisement.boot_id);
    assert_eq!(current.offer_generation, advertisement.offer_generation);

    let plan = pico.plan_local(&form()).unwrap();
    assert!(plan.fragments.iter().all(|fragment| {
        fragment.host_id == advertisement.host_id && fragment.boot_id == advertisement.boot_id
    }));
}

#[test]
fn stale_pico_boot_and_offer_disconnect_leave_no_half_member() {
    for stale_boot in [false, true] {
        let pico = pico();
        let body = Body::born(
            SourceDocumentId::from("source/pico-negative"),
            CheckedFormId::from("checked/pico-negative"),
            u64::from(stale_boot) + 1,
            SignId::from(format!("sign/pico-negative-born/{stale_boot}")),
        )
        .unwrap();
        let mut candidates = CandidateInventory::new(body.body_id.clone()).unwrap();
        let candidate = candidates
            .observe(CandidateObservation {
                advertisement: pico.advertisement().clone(),
                friendly_label: "untrusted".into(),
                observed_binding_id: LinkBindingId::from("line/pico-negative"),
                observation_sign_id: SignId::from("sign/pico-negative-observed"),
                proof_id: DiscoveryProofId::bind("proof/pico-negative").unwrap(),
                freshness_sequence: 1,
                encoded_bytes: 512,
            })
            .unwrap();
        let key = SigningKey::from_bytes(&[51; 32]);
        let mut manager = AdmissionManager::new(body.body_id.clone()).unwrap();
        let challenge = manager
            .begin_ambient(
                &mut candidates,
                &candidate,
                key.verifying_key().to_bytes(),
                [52; 32],
                1_000,
                2_000,
                SignId::from("sign/pico-negative-requested"),
            )
            .unwrap();
        if stale_boot {
            candidates.candidates[0].observation.advertisement.boot_id =
                BootId::from("pico-boot/replugged");
        } else {
            candidates.candidates[0]
                .observation
                .advertisement
                .offer_generation = OfferGeneration(8);
        }
        let proof = AmbientAdmissionProof {
            admission_id: challenge.admission_id.clone(),
            body_id: challenge.body_id.clone(),
            host_id: challenge.host_id.clone(),
            boot_id: challenge.boot_id.clone(),
            nonce: challenge.nonce,
            signature: key.sign(&challenge.signing_transcript()).to_bytes(),
        };
        let mut membership = BodyMembership::new(body.body_id).unwrap();
        assert!(matches!(
            manager.complete_ambient(&mut candidates, &mut membership, &proof, 1_001, signs()),
            Err(AdmissionRefusal::StaleBoot | AdmissionRefusal::StaleOfferGeneration)
        ));
        assert!(membership.parts.is_empty());
        manager
            .disconnect_ambient(
                &mut candidates,
                &challenge.admission_id,
                SignId::from("sign/pico-disconnected"),
            )
            .unwrap();
        assert!(membership.parts.is_empty());
    }
}
