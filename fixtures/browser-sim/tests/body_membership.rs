use conduit_body::{
    AdmissionManager, AdmissionSigns, AmbientAdmissionProof, Body, BodyMembership,
    CandidateInventory, CandidateObservation, DiscoveryProofId, SpawnAdmissionProof,
    SpawnInvitationSecret,
};
use conduit_browser_sim::{BrowserSimConfig, BrowserSimPage};
use conduit_core::{
    BootId, CheckedFormId, HostAdvertisement, HostId, LinkBindingId, OfferGeneration, SignId,
    SourceDocumentId,
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

fn pair_form() -> conduit_form::CheckedForm {
    parse_with_startup(
        "form signal-demo {\n pulse: flow/pulse(count = 4, period-ms = 1, initial = false)\n show: presentation/show\n pulse > show\n}\n", &conduit_signal::signal_startup_catalog(), &signal_profile_catalog())
    .unwrap()
}

fn observe(
    candidates: &mut CandidateInventory,
    advertisement: HostAdvertisement,
    sequence: u64,
) -> conduit_body::CandidateId {
    candidates
        .observe(CandidateObservation {
            advertisement,
            friendly_label: "browser-controlled label".into(),
            observed_binding_id: LinkBindingId::from(format!("line/browser/{sequence}")),
            observation_sign_id: SignId::from(format!("sign/browser/{sequence}/observed")),
            proof_id: DiscoveryProofId::bind(&format!("proof/browser/{sequence}")).unwrap(),
            freshness_sequence: sequence,
            encoded_bytes: 512,
        })
        .unwrap()
}

#[test]
fn three_independent_browser_hosts_require_admission_and_can_plan_without_plan_mutation() {
    let configs = (0..3)
        .map(|index| BrowserSimConfig {
            host_id: HostId::from(format!("browser/tab-{index}")),
            boot_id: BootId::from(format!("browser-boot/tab-{index}")),
            offer_generation: OfferGeneration(1),
        })
        .collect::<Vec<_>>();
    let mut page = BrowserSimPage::with_hosts(configs);
    let advertisements = page.advertisements();
    assert_eq!(advertisements.len(), 3);
    assert!(advertisements
        .iter()
        .enumerate()
        .all(|(index, host)| advertisements
            .iter()
            .skip(index + 1)
            .all(|other| host.host_id != other.host_id && host.boot_id != other.boot_id)));

    let body = Body::born(
        SourceDocumentId::from("source/browser-body"),
        CheckedFormId::from("checked/browser-body"),
        1,
        SignId::from("sign/browser-body-born"),
    )
    .unwrap();
    let mut membership = BodyMembership::new(body.body_id.clone()).unwrap();
    let mut candidates = CandidateInventory::new(body.body_id.clone()).unwrap();
    let mut manager = AdmissionManager::new(body.body_id).unwrap();

    for (index, advertisement) in advertisements[..2].iter().enumerate() {
        let key = SigningKey::from_bytes(&[(index + 1) as u8; 32]);
        let candidate = observe(&mut candidates, advertisement.clone(), index as u64 + 1);
        assert!(membership.parts.is_empty() || membership.parts.len() == index);
        let challenge = manager
            .begin_ambient(
                &mut candidates,
                &candidate,
                key.verifying_key().to_bytes(),
                [(index + 11) as u8; 32],
                1_000 + index as u64,
                2_000 + index as u64,
                SignId::from(format!("sign/browser/{index}/requested")),
            )
            .unwrap();
        let proof = AmbientAdmissionProof {
            admission_id: challenge.admission_id.clone(),
            body_id: challenge.body_id.clone(),
            host_id: challenge.host_id.clone(),
            boot_id: challenge.boot_id.clone(),
            nonce: challenge.nonce,
            signature: key.sign(&challenge.signing_transcript()).to_bytes(),
        };
        manager
            .complete_ambient(
                &mut candidates,
                &mut membership,
                &proof,
                1_100 + index as u64,
                signs(&format!("ambient-{index}")),
            )
            .unwrap();
    }

    let plan = page
        .plan_pair(
            &pair_form(),
            &advertisements[0].host_id,
            &advertisements[1].host_id,
        )
        .unwrap();
    let active_plan_id = plan.plan_id.clone();

    let invitation = manager
        .issue_spawn_invitation(
            SpawnInvitationSecret::from_csprng_bytes([31; 32]).unwrap(),
            [32; 32],
            1_200,
            2_200,
        )
        .unwrap();
    let spawned = &advertisements[2];
    let spawn_proof = SpawnAdmissionProof {
        invitation_id: invitation.invitation_id.clone(),
        body_id: invitation.body_id.clone(),
        host_id: spawned.host_id.clone(),
        boot_id: spawned.boot_id.clone(),
        nonce: invitation.nonce,
        signature: invitation.secret.sign(&invitation.signing_transcript(
            &spawned.host_id,
            &spawned.boot_id,
            spawned.offer_generation,
        )),
    };
    manager
        .complete_spawn(
            &mut membership,
            spawned,
            &spawn_proof,
            1_201,
            signs("spawned"),
        )
        .unwrap();
    assert_eq!(membership.parts.len(), 3);
    assert_eq!(plan.plan_id, active_plan_id);

    let report = page.run_plan(plan).unwrap();
    assert_eq!(report.receipts.len(), 4);

    let first_part = membership.parts[0].part_id.clone();
    let first_boot = membership.parts[0]
        .current
        .as_ref()
        .unwrap()
        .boot_id
        .clone();
    membership
        .observe_offline(
            &membership.body_id.clone(),
            membership.revision,
            &first_part,
            &first_boot,
            SignId::from("sign/browser/tab-closed"),
        )
        .unwrap();
    assert!(!membership.parts[0].is_present());
    assert!(membership.parts[1].is_present());
    assert!(membership.parts[2].is_present());
}
