use super::*;
use crate::durable_host as installed;
use conduit_body::{
    MembershipCredential, PortableSpawnAdmissionRequest, SpawnInvitationSecret,
    SPAWN_ADMISSION_REQUEST_SCHEMA,
};
use std::path::{Path, PathBuf};

pub(super) fn owner_fixture() -> (Owner, PathBuf) {
    owner_fixture_with_host(host("boot/member-leave-owner"))
}
pub(super) fn owner_fixture_with_host(host: StdHost) -> (Owner, PathBuf) {
    owner_fixture_with_plot(host, &source())
}
pub(super) fn owner_fixture_with_plot(
    host: StdHost,
    plot: &conduit_plot::ExpandedAuthoringPlot,
) -> (Owner, PathBuf) {
    let root = std::env::temp_dir().join(installed::fresh_identity("owner-member-leave", "test"));
    std::fs::create_dir_all(&root).unwrap();
    installed::write_json_atomic(
        &root.join("installation.json"),
        &installed::Installation {
            schema: installed::INSTALL_SCHEMA.into(),
            host_id: host.advertisement().host_id.as_str().into(),
            release_source_identity: "source/test".into(),
            release_bundle_sha256: installed::digest(b"test"),
            product_executable: "fixture-unused".into(),
            body_state: None,
            joined_body_state: None,
            selected_speech: None,
            selected_model: None,
            selected_host_configuration: None,
            selected_todo_checkpoint: None,
        },
    )
    .unwrap();
    let mut owner = Owner::open(host, resident(plot), None, "Shared").unwrap();
    owner.persist(&root).unwrap();
    (owner, root)
}
fn admit(owner: &mut Owner, root: &Path, name: &str) -> MembershipCredential {
    admit_with_key(owner, root, name).0
}
pub(super) fn admit_with_key(
    owner: &mut Owner,
    root: &Path,
    name: &str,
) -> (
    MembershipCredential,
    SpawnInvitationSecret,
    conduit_core::HostAdvertisement,
) {
    let guest = StdHost::new_with_config(StdHostConfig {
        host_id: name.into(),
        boot_id: format!("boot/{name}/1").into(),
        offer_generation: OfferGeneration(1),
    });
    admit_advertisement(owner, root, name, guest.advertisement().clone())
}

pub(super) fn admit_advertisement(
    owner: &mut Owner,
    root: &Path,
    name: &str,
    advertisement: conduit_core::HostAdvertisement,
) -> (
    MembershipCredential,
    SpawnInvitationSecret,
    conduit_core::HostAdvertisement,
) {
    let invitation = owner.issue_invitation(root, 60, None).unwrap();
    let key = SpawnInvitationSecret::from_csprng_bytes(invitation.secret).unwrap();
    let request = PortableSpawnAdmissionRequest {
        schema: SPAWN_ADMISSION_REQUEST_SCHEMA.into(),
        invitation_id: invitation.claim.invitation_id.clone(),
        body_id: invitation.claim.body_id.clone(),
        host_advertisement: advertisement.clone(),
        nonce: invitation.claim.nonce,
        signature: key
            .sign(&invitation.claim.signing_transcript(
                &advertisement.host_id,
                &advertisement.boot_id,
                advertisement.offer_generation,
            ))
            .to_vec(),
        membership_admitted: false,
        plan_created: false,
        play_created: false,
    };
    (
        owner.admit_invited(root, request, name).unwrap().credential,
        key,
        advertisement,
    )
}
#[test]
fn member_leave_is_durable_exact_and_preserves_membership_keys_and_sibling() {
    let (mut owner, root) = owner_fixture();
    let credential = admit(&mut owner, &root, "host/leaving");
    let sibling = admit(&mut owner, &root, "host/sibling");
    let before = owner.session.evidence().clone();
    let manager = owner.admissions.clone();
    let result = owner.member_leave(&root, &credential).unwrap();
    let part = result
        .membership
        .parts
        .iter()
        .find(|p| p.part_id == credential.part_id)
        .unwrap();
    assert_eq!(part.state, conduit_body::MembershipState::Admitted);
    assert!(part.current.is_none());
    assert_eq!(owner.admissions, manager);
    assert_eq!(result.body, before.body);
    assert_eq!(
        result
            .membership
            .parts
            .iter()
            .find(|p| p.part_id == sibling.part_id),
        before
            .membership
            .parts
            .iter()
            .find(|p| p.part_id == sibling.part_id)
    );
    assert_eq!(state::load(&root).unwrap().unwrap(), result);
    assert_eq!(
        state::admissions(&root, &credential.body_id).unwrap(),
        manager
    );
    assert!(owner.member_leave(&root, &credential).is_err());
    assert_eq!(owner.session.evidence(), &result);
    // The unchanged original key remains eligible for an ordinary return
    // challenge; no replacement Part or invitation is required.
    let guest = StdHost::new_with_config(StdHostConfig {
        host_id: credential.host_id.clone(),
        boot_id: "boot/leaving/2".into(),
        offer_generation: OfferGeneration(1),
    });
    let mut admissions = owner.admissions.clone().unwrap();
    assert!(admissions
        .begin_return(
            &result.membership,
            &credential.part_id,
            guest.advertisement(),
            [41; 32],
            100,
            200
        )
        .is_ok());
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn member_leave_refuses_stale_scope_and_failed_persistence_without_withdrawing() {
    let (mut owner, root) = owner_fixture();
    let credential = admit(&mut owner, &root, "host/leaving");
    let before = owner.session.evidence().clone();
    let manager = owner.admissions.clone();
    for change in 0..3 {
        let mut wrong = credential.clone();
        match change {
            0 => {
                wrong.credential_id =
                    serde_json::from_value(serde_json::json!("credential/other")).unwrap()
            }
            1 => wrong.boot_id = "boot/other".into(),
            _ => wrong.host_id = owner.host.advertisement().host_id.clone(),
        }
        assert!(owner.member_leave(&root, &wrong).is_err());
        assert_eq!(owner.session.evidence(), &before);
    }
    let blocked = root.join("not-a-directory");
    std::fs::write(&blocked, b"refuse owner persistence").unwrap();
    assert!(owner.member_leave(&blocked, &credential).is_err());
    assert_eq!(owner.session.evidence(), &before);
    assert_eq!(owner.admissions, manager);
    assert_eq!(state::load(&root).unwrap().unwrap(), before);
    std::fs::remove_dir_all(root).unwrap();
}
