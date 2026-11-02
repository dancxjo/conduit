use super::*;

#[test]
fn running_owner_commits_expired_invitation_retirement_and_rolls_back_on_persistence_refusal() {
    use crate::durable_host as installed;
    use conduit_body::{AdmissionManager, SpawnInvitationSecret, MAX_SPAWN_INVITATIONS};
    let root = std::env::temp_dir().join(installed::fresh_identity(
        "owner-invitation-retirement",
        "test",
    ));
    std::fs::create_dir_all(&root).unwrap();
    let installation = installed::Installation {
        schema: installed::INSTALL_SCHEMA.into(),
        host_id: "host/owner-test".into(),
        release_source_identity: "source/test".into(),
        release_bundle_sha256: installed::digest(b"bundle/test"),
        product_executable: "fixture-unused".into(),
        body_state: None,
        joined_body_state: None,
        selected_speech: None,
        selected_model: None,
        selected_host_configuration: None,
        selected_todo_checkpoint: None,
    };
    installed::write_json_atomic(&root.join("installation.json"), &installation).unwrap();
    let plot = source();
    let mut owner = Owner::open(
        host("boot/retirement"),
        resident(&plot),
        None,
        "Retirement fixture",
    )
    .unwrap();
    let mut manager = AdmissionManager::new(owner.session.evidence().body_id.clone()).unwrap();
    for index in 0..MAX_SPAWN_INVITATIONS {
        manager
            .issue_spawn_invitation(
                SpawnInvitationSecret::from_csprng_bytes([31; 32]).unwrap(),
                [index as u8 + 1; 32],
                1,
                2,
            )
            .unwrap();
    }
    owner.admissions = Some(manager.clone());
    owner.persist(&root).unwrap();
    let blocked = root.join("not-a-directory");
    std::fs::write(&blocked, b"persistence refusal fixture").unwrap();
    assert!(owner.issue_invitation(&blocked, 60, None).is_err());
    assert_eq!(owner.admissions.as_ref(), Some(&manager));
    let invitation = owner.issue_invitation(&root, 60, None).unwrap();
    let committed = owner.admissions.as_ref().unwrap();
    assert_eq!(committed.storage_usage().invitations.len, 1);
    assert_eq!(committed.receipts, manager.receipts);
    let retained = state::load(&root).unwrap();
    let mut reopened = Owner::open(
        host("boot/retirement-reopened"),
        resident(&plot),
        retained,
        "ignored",
    )
    .unwrap();
    reopened.restore_execution(&root).unwrap();
    assert_eq!(reopened.admissions.as_ref(), Some(committed));
    let current = serde_json::to_value(reopened.admissions.as_ref().unwrap()).unwrap();
    assert_eq!(
        current["invitations"][0]["invitation_id"],
        invitation.claim.invitation_id.as_str()
    );
    std::fs::remove_dir_all(root).unwrap();
}
