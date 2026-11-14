use super::*;
use crate::{BodyId, MembershipCredentialId, PartId, SpawnInvitationSecret};
use conduit_core::{BootId, HostId, HostProfileId, OfferGeneration};
fn fixture() -> (
    MembershipCredential,
    HostAdvertisement,
    FederationPeer,
    SpawnInvitationSecret,
) {
    let credential = MembershipCredential {
        credential_id: MembershipCredentialId::bound("credential/old".into()),
        body_id: BodyId::bound("body/house".into()),
        part_id: PartId::bound("part/mcu".into()),
        host_id: HostId::from("host/mcu"),
        boot_id: BootId::from("boot/old"),
        issued_at_millis: 1,
    };
    let ad = HostAdvertisement {
        declared_name: None,
        protocol_version: PROTOCOL_VERSION,
        host_id: credential.host_id.clone(),
        boot_id: "boot/current".into(),
        offer_generation: OfferGeneration(2),
        profile: HostProfileId::from("profile/current"),
        bases: alloc::vec![],
        resources: alloc::vec![],
        capabilities: alloc::vec![],
        planner_capabilities: alloc::vec![],
    };
    let key = SpawnInvitationSecret::from_csprng_bytes([7; 32]).unwrap();
    let peer = FederationPeer {
        host_id: "host/owner".into(),
        boot_id: "boot/owner".into(),
        offer_generation: OfferGeneration(1),
        verifying_key: key.verifying_key(),
    };
    (credential, ad, peer, key)
}
#[test]
fn return_preserves_relationship_and_cannot_be_verified_as_spawn() {
    let (prior, ad, owner, key) = fixture();
    let mut current = prior.clone();
    current.boot_id = ad.boot_id.clone();
    current.credential_id = MembershipCredentialId::bound("credential/current".into());
    let ack = MemberReturnAcknowledgement::new(current, ad.clone());
    let signed =
        attest_member_return_acknowledgement(&ack, owner.clone(), [9; 32], |t| key.sign(t))
            .unwrap();
    let decoded = signed.verify_return(&owner, &[9; 32], &prior, &ad).unwrap();
    assert_eq!(decoded.credential.part_id, prior.part_id);
    assert_eq!(decoded.credential.boot_id, ad.boot_id);
    assert!(signed
        .verify(&owner, &[9; 32], &prior.body_id, &ad)
        .is_err());
    let mut foreign = prior.clone();
    foreign.part_id = PartId::bound("part/other".into());
    assert!(signed
        .verify_return(&owner, &[9; 32], &foreign, &ad)
        .is_err());
    let mut foreign = ad.clone();
    foreign.boot_id = "boot/other".into();
    assert!(signed
        .verify_return(&owner, &[9; 32], &prior, &foreign)
        .is_err());
    let mut foreign = owner.clone();
    foreign.boot_id = "boot/stale-owner".into();
    assert!(signed
        .verify_return(&foreign, &[9; 32], &prior, &ad)
        .is_err());
    assert!(signed.verify_return(&owner, &[8; 32], &prior, &ad).is_err());
    let mut changed = signed;
    changed.receipt_json.push(' ');
    assert!(changed
        .verify_return(&owner, &[9; 32], &prior, &ad)
        .is_err());
}
#[test]
fn wrong_outcome_schema_boot_or_offers_cannot_be_attested() {
    let (mut credential, ad, owner, key) = fixture();
    credential.boot_id = ad.boot_id.clone();
    let ack = MemberReturnAcknowledgement::new(credential.clone(), ad.clone());
    for field in ["kind", "schema", "boot", "offer"] {
        let mut changed = ack.clone();
        match field {
            "kind" => changed.kind = "Refused".into(),
            "schema" => changed.schema = crate::SPAWN_ADMISSION_RECEIPT_SCHEMA.into(),
            "boot" => changed.credential.boot_id = "boot/old".into(),
            _ => changed.advertisement.offer_generation = OfferGeneration(0),
        };
        assert!(
            attest_member_return_acknowledgement(&changed, owner.clone(), [9; 32], |t| key.sign(t))
                .is_err()
        );
    }
}
