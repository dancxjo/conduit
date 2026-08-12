use conduit_body::{
    AdmissionManager, Body, BodyMembership, CandidateInventory, CandidateObservation,
    CandidateRefusal, DiscoveryProofId, MAX_PENDING_ADMISSIONS,
};
use conduit_core::{
    BootId, CheckedFormId, HostAdvertisement, HostId, HostProfileId, LinkBindingId,
    OfferGeneration, SignId, SourceDocumentId, PROTOCOL_VERSION,
};
use ed25519_dalek::SigningKey;

const NOW: u64 = 10_000;
const EXPIRES: u64 = 20_000;

fn advertisement(index: usize) -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from(format!("host/pressure/{index}")),
        boot_id: BootId::from(format!("boot/pressure/{index}")),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from("profile/admission-pressure"),
        resources: Vec::new(),
        capabilities: Vec::new(),
        planner_capabilities: Vec::new(),
    }
}

fn observation(index: usize) -> CandidateObservation {
    CandidateObservation {
        advertisement: advertisement(index),
        friendly_label: "bounded peer".into(),
        observed_binding_id: LinkBindingId::from(format!("line/pressure/{index}")),
        observation_sign_id: SignId::from(format!("sign/pressure/{index}/observed")),
        proof_id: DiscoveryProofId::bind(&format!("proof/pressure/{index}")).unwrap(),
        freshness_sequence: 1,
        encoded_bytes: 512,
    }
}

#[test]
fn ambient_admission_storage_pressure_refuses_before_membership() {
    let body = Body::born(
        SourceDocumentId::from("source/admission-pressure"),
        CheckedFormId::from("checked/admission-pressure"),
        1,
        SignId::from("sign/admission-pressure/body-born"),
    )
    .unwrap();
    let mut candidates = CandidateInventory::new(body.body_id.clone()).unwrap();
    let mut manager = AdmissionManager::new(body.body_id.clone()).unwrap();
    let membership = BodyMembership::new(body.body_id).unwrap();
    let key = SigningKey::from_bytes(&[17; 32]);

    for index in 0..MAX_PENDING_ADMISSIONS {
        let candidate_id = candidates.observe(observation(index)).unwrap();
        manager
            .begin_ambient(
                &mut candidates,
                &candidate_id,
                key.verifying_key().to_bytes(),
                [u8::try_from(index + 1).unwrap(); 32],
                NOW + u64::try_from(index).unwrap(),
                EXPIRES + u64::try_from(index).unwrap(),
                SignId::from(format!("sign/pressure/{index}/requested")),
            )
            .unwrap();
    }

    assert_eq!(candidates.candidates.len(), MAX_PENDING_ADMISSIONS);
    assert_eq!(
        candidates.observe(observation(MAX_PENDING_ADMISSIONS)),
        Err(CandidateRefusal::CandidateCapacityExhausted)
    );
    assert!(membership.parts.is_empty());
}
