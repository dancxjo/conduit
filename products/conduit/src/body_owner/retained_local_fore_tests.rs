//! Real retained IMAGE/File custody plus canonical signed member acceptance.
use super::*;
use conduit_body::{PortableSpawnAdmissionRequest, SPAWN_ADMISSION_REQUEST_SCHEMA};
use conduit_core as c;
fn admit(
    owner: &mut Owner,
    root: &std::path::Path,
    name: &str,
) -> conduit_body::PortableAdmissionReceipt {
    let invitation = owner.issue_invitation(root, 60, None).unwrap();
    let mut ad = conduit_std_host::StdHost::new_with_config(conduit_std_host::StdHostConfig {
        host_id: name.into(),
        boot_id: format!("{name}/boot").into(),
        offer_generation: c::OfferGeneration(1),
    })
    .advertisement()
    .clone();
    ad.profile = "fixture/target-config".into();
    let key = conduit_body::SpawnInvitationSecret::from_csprng_bytes(invitation.secret).unwrap();
    let request = PortableSpawnAdmissionRequest {
        schema: SPAWN_ADMISSION_REQUEST_SCHEMA.into(),
        invitation_id: invitation.claim.invitation_id.clone(),
        body_id: invitation.claim.body_id.clone(),
        host_advertisement: ad.clone(),
        nonce: invitation.claim.nonce,
        signature: key
            .sign(&invitation.claim.signing_transcript(
                &ad.host_id,
                &ad.boot_id,
                ad.offer_generation,
            ))
            .to_vec(),
        membership_admitted: false,
        plan_created: false,
        play_created: false,
    };
    owner.admit_invited(root, request, name).unwrap()
}
#[test]
#[ignore = "requires explicit digest-pinned product Source worker artifact"]
fn retained_fore_restart_refreshes_after_committed_authentication_and_fences_changed_source() {
    let (host, custody, biography, path) =
        crate::durable_host::source_revision::real_owner_custody();
    let mut owner = Owner::resume(host, biography).unwrap();
    let root = path.with_extension("state");
    std::fs::create_dir(&root).unwrap();
    let installation = super::super::super::super::Installation {
        schema: super::super::super::super::INSTALL_SCHEMA.into(),
        host_id: owner.host.advertisement().host_id.as_str().into(),
        release_source_identity: "source/retained-source-test".into(),
        release_bundle_sha256: super::super::super::super::digest(b"fixture-bundle"),
        product_executable: "fixture-unused".into(),
        body_state: None,
        joined_body_state: None,
        selected_speech: None,
        selected_model: None,
        selected_host_configuration: None,
        selected_todo_checkpoint: None,
    };
    super::super::super::super::write_json_atomic(&root.join("installation.json"), &installation)
        .unwrap();

    let before = owner.session.evidence().clone();
    owner.restore_selected_file_planning(custody).unwrap();
    assert_eq!(
        owner.session.evidence(),
        &before,
        "planning recovery changed Body biography"
    );
    assert!(owner.require_retained_local_fore().is_err());
    let other = admit(&mut owner, &root, "member/other");
    assert!(
        other.membership_admitted,
        "missing target must not falsify committed membership"
    );
    assert!(owner.local_fore_adapters.is_none());
    let target = admit(&mut owner, &root, "member/target");
    assert!(target.membership_admitted);
    owner.require_retained_local_fore().unwrap();
    assert!(owner.local_fore_adapters.is_some());
    assert!(
        owner.session.realization().is_none(),
        "refresh must not Plan or Wake"
    );
    let source = crate::plot_source::parse(include_str!("../../../../plots/hello/main.conduit"))
        .unwrap()
        .expand_entry_for_authoring()
        .unwrap();
    let ad = owner.host.advertisement();
    owner
        .session
        .admit_plot(
            0,
            conduit_body::ResidentPlot::new(
                source.expanded.source_document_id,
                source.expanded.checked_plot_id,
            ),
            &ad.host_id,
            &ad.boot_id,
        )
        .unwrap();
    owner.refresh_retained_local_fore();
    assert!(
        owner.local_fore_adapters.is_none(),
        "changed Source must invalidate old binding"
    );
    assert!(owner.require_retained_local_fore().is_err());
    owner.selected_file_workset = None;
    owner.refresh_retained_local_fore();
    assert!(owner.local_fore_adapters.is_none());
    std::fs::remove_dir_all(root).unwrap();
    std::fs::remove_file(path).unwrap();
}
