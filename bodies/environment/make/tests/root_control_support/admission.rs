//! Production responder with deterministic CPU entropy, ordinary Body verifier.
use super::*;
use conduit_body::*;
#[path = "../../../../../targets/esp32/proof/tm1637/src/native_admission.rs"]
pub(super) mod responder;
#[test]
fn root_proof_enters_ordinary_admission_without_creating_a_plan_or_play() {
    assert_eq!(responder::MAXIMUM_ADVERTISEMENT_BYTES, 8192);
    assert!(responder::NativeAdmission::new([0; 32]).is_err());
    let signer = responder::NativeAdmission::new([7; 32]).unwrap();
    let body = Body::born(
        "source/admission".into(),
        "checked/admission".into(),
        1,
        "sign/body-born".into(),
    )
    .unwrap();
    let mut candidates = CandidateInventory::new(body.body_id.clone()).unwrap();
    let ad = fixtures::host();
    let candidate = candidates
        .observe(CandidateObservation {
            advertisement: ad.clone(),
            friendly_label: "CPU fixture".into(),
            observed_binding_id: "line/cpu-fixture".into(),
            observation_sign_id: "sign/observed".into(),
            proof_id: DiscoveryProofId::bind("fixture/discovery").unwrap(),
            freshness_sequence: 1,
            encoded_bytes: 512,
        })
        .unwrap();
    let mut membership = BodyMembership::new(body.body_id.clone()).unwrap();
    let mut manager = AdmissionManager::new(body.body_id).unwrap();
    let challenge = manager
        .begin_ambient(
            &mut candidates,
            &candidate,
            signer.verifying_key(),
            [9; 32],
            100,
            200,
            "sign/requested".into(),
        )
        .unwrap();
    let borrowed = PicoAdmissionChallenge {
        protocol: PICO_ADMISSION_PROTOCOL,
        admission_id: challenge.admission_id.as_str(),
        body_id: challenge.body_id.as_str(),
        host_id: ad.host_id.as_str(),
        boot_id: ad.boot_id.as_str(),
        offer_generation: ad.offer_generation.0,
        nonce: challenge.nonce,
        issued_at_millis: 100,
        expires_at_millis: 200,
    };
    for mode in 0..6 {
        let mut wrong = PicoAdmissionChallenge { ..borrowed };
        match mode {
            0 => wrong.protocol += 1,
            1 => wrong.host_id = "foreign/host",
            2 => wrong.boot_id = "stale/boot",
            3 => wrong.offer_generation += 1,
            4 => wrong.nonce = [0; 32],
            _ => wrong.expires_at_millis = wrong.issued_at_millis,
        }
        let (result, memory) = allocation_probe::observe(|| {
            signer.sign(
                &wrong,
                ad.host_id.as_str(),
                ad.boot_id.as_str(),
                ad.offer_generation.0,
            )
        });
        assert!(result.is_err());
        assert_eq!((memory.allocations, memory.reallocations), (0, 0));
    }
    let (signature, memory) = allocation_probe::observe(|| {
        signer.sign(
            &borrowed,
            ad.host_id.as_str(),
            ad.boot_id.as_str(),
            ad.offer_generation.0,
        )
    });
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
    let signature = signature.unwrap();
    let mut proof = AmbientAdmissionProof {
        admission_id: challenge.admission_id.clone(),
        body_id: challenge.body_id.clone(),
        host_id: ad.host_id.clone(),
        boot_id: ad.boot_id.clone(),
        nonce: challenge.nonce,
        signature,
    };
    let signs = || AdmissionSigns {
        part_admitted: "sign/admitted".into(),
        host_attached: "sign/present".into(),
        candidate_admitted: "sign/candidate".into(),
    };
    let original = membership.clone();
    proof.signature[0] ^= 1;
    assert!(manager
        .complete_ambient(&mut candidates, &mut membership, &proof, 101, signs())
        .is_err());
    assert_eq!(membership, original);
    proof.signature[0] ^= 1;
    let credential = manager
        .complete_ambient(&mut candidates, &mut membership, &proof, 101, signs())
        .unwrap();
    assert_eq!(credential.host_id, ad.host_id);
    assert_eq!(credential.boot_id, ad.boot_id);
    assert!(membership.parts[0].is_present());
    assert_eq!(
        manager.complete_ambient(&mut candidates, &mut membership, &proof, 102, signs()),
        Err(AdmissionRefusal::Replay)
    );
    // The responder has no Body owner, Plan, Play or native capability handle.
}
