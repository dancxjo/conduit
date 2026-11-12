use super::*;
#[test]
fn owned_invitation_key_matches_native_bootstrap_and_rejects_incoming_substitution() {
    use conduit_body::{
        MemberBootstrap, MemberBootstrapMaterial, PicoSpawnProvision, SpawnInvitationSecret,
    };
    let body = conduit_body::Body::born(
        "source/key-test".into(),
        "checked/key-test".into(),
        1,
        conduit_core::SignId::from("sign/key-test"),
    )
    .unwrap();
    let mut manager = conduit_body::AdmissionManager::new(body.body_id.clone()).unwrap();
    let invitation = super::super::invitation::issue_from_manager(&mut manager, 300, None).unwrap();
    let policy: AdmissionMemberKey =
        serde_json::from_str(r#"{"source":"from-owned-invitation"}"#).unwrap();
    let owner = conduit_body::FederationPeer {
        host_id: "host/owner-key-test".into(),
        boot_id: "boot/current-key-test".into(),
        offer_generation: conduit_core::OfferGeneration(1),
        verifying_key: SpawnInvitationSecret::from_csprng_bytes([7; 32])
            .unwrap()
            .verifying_key(),
    };
    let image = format!("image:sha256:{}", "3".repeat(64));
    let configuration = format!("sha256:{}", "4".repeat(64));
    let host = "rp2040/flash/0102030405060708";
    let bootstrap = MemberBootstrap {
        schema: conduit_body::MEMBER_BOOTSTRAP_SCHEMA.into(),
        body_id: body.body_id,
        host_id: host.into(),
        image_id: image.clone(),
        configuration_id: configuration.clone(),
        owner: owner.clone(),
        admission: MemberBootstrapMaterial::Spawn {
            provision: PicoSpawnProvision {
                protocol: conduit_body::PICO_SPAWN_PROTOCOL,
                spore_id: "spore/key-test",
                image_id: &image,
                invitation_id: invitation.claim.invitation_id.as_str(),
                body_id: invitation.claim.body_id.as_str(),
                nonce: invitation.claim.nonce,
                expires_at_millis: invitation.claim.expires_at_millis,
                secret: invitation.secret,
            },
        },
    };
    assert!(bootstrap.matches_native(host, &image, &configuration));
    // This is the exact secret accessor used by both actual native BSPs and
    // the typed private Seed composer, not an incoming advertised key.
    let seed_key = SpawnInvitationSecret::from_csprng_bytes(*bootstrap.admission.secret())
        .unwrap()
        .verifying_key();
    assert_eq!(policy.resolve(&invitation).unwrap(), seed_key);
    let selected = UsbAdmissionSelection {
        carrier: Default::default(),
        host_id: host.into(),
        member_key: policy,
        image_id: image.clone(),
        configuration_id: configuration.clone(),
        port: "/dev/test-key".into(),
        basis: "/private/basis".into(),
        invitation_output: "/private/invitation".into(),
        receipt_output: "/private/receipt".into(),
        return_receipt_output: "/private/return".into(),
        return_basis_output: "/private/return-basis".into(),
        wait_milliseconds: 1000,
    };
    let basis = conduit_std_host::pico_spawn::PicoSpawnAdmissionBasis {
        spore_id: "spore/key-test".into(),
        image_id: image.clone(),
        invitation: invitation.claim.clone(),
        expected_host: host.into(),
        expected_profile: configuration.into(),
        member_verifying_key: seed_key,
        owner: owner.clone(),
    };
    assert_eq!(
        selected.verify_basis(&basis, &owner, &invitation).unwrap(),
        seed_key
    );
    let mut wrong = basis.clone();
    wrong.member_verifying_key[0] ^= 1;
    assert!(selected.verify_basis(&wrong, &owner, &invitation).is_err());
    let mut wrong = basis.clone();
    wrong.image_id = "image/other".into();
    assert!(selected.verify_basis(&wrong, &owner, &invitation).is_err());
    let mut wrong = basis.clone();
    wrong.expected_host = "host/other".into();
    assert!(selected.verify_basis(&wrong, &owner, &invitation).is_err());
    let pinned = AdmissionMemberKey::Pinned {
        verifying_key: seed_key,
    };
    assert_eq!(pinned.resolve(&invitation).unwrap(), seed_key);
    assert!(!AdmissionMemberKey::Pinned {
        verifying_key: [0; 32]
    }
    .valid());
}
