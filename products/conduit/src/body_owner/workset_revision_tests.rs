//! Trusted continuation and custody tests; ordinary-Plot authority is separate.
use super::*;
use std::fs;
const SOURCE: &str = "plot alpha {\n show: face/text(\"Alpha\")\n \"first output\" >> show\n}\nplot beta {\n show: face/text(\"Beta\")\n \"second output\" >> show\n}.";

fn fixture() -> (std::path::PathBuf, Owner) {
    let root = std::env::temp_dir().join(super::super::super::super::fresh_identity(
        "service-workset",
        "resume",
    ));
    fs::create_dir_all(&root).unwrap();
    let installation = super::super::super::super::Installation {
        schema: super::super::super::super::INSTALL_SCHEMA.into(),
        host_id: "host/workset-service".into(),
        release_source_identity: "source/test".into(),
        release_bundle_sha256: super::super::super::super::digest(b"bundle"),
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
    let host = conduit_std_host::StdHost::new_with_config(conduit_std_host::StdHostConfig {
        host_id: installation.host_id.into(),
        boot_id: "boot/workset-service".into(),
        offer_generation: conduit_core::OfferGeneration(1),
    });
    let source = crate::plot_source::parse(SOURCE).unwrap();
    let a = source.expand_named_for_authoring("alpha").unwrap();
    let b = source.expand_named_for_authoring("beta").unwrap();
    let resident = |plot: &conduit_plot::ExpandedAuthoringPlot| {
        ResidentPlot::new(
            plot.expanded.source_document_id.clone(),
            plot.expanded.checked_plot_id.clone(),
        )
    };
    let mut owner = Owner::open(host, resident(&a), None, "Two checked Plots").unwrap();
    let ad = owner.host.advertisement();
    owner
        .session
        .admit_plot(0, resident(&b), &ad.host_id, &ad.boot_id)
        .unwrap();
    let sources = state::WorksetSources::checked(
        owner.session.evidence(),
        vec![
            (SOURCE.into(), "alpha".into()),
            (SOURCE.into(), "beta".into()),
        ],
    )
    .unwrap();
    state::retain_session_with_workset_sources(&root, &mut owner.session, &sources, None, None)
        .unwrap();
    (root, owner)
}

fn checked(
    entry: &str,
) -> (
    crate::plot_source::CanonicalSource,
    conduit_plot::ExpandedAuthoringPlot,
) {
    let source = crate::plot_source::parse(SOURCE).unwrap();
    let plot = source.expand_named_for_authoring(entry).unwrap();
    (source, plot)
}

#[test]
fn trusted_continuation_adds_and_removes_complete_meaning_without_waking() {
    let (root, mut owner) = fixture();
    let original = owner.session.evidence().clone();
    let (a, pa) = checked("alpha");
    let (b, pb) = checked("beta");
    owner
        .commit_admitted_workset_revision(&root, &original, &[(&a, &pa)])
        .unwrap();
    assert_eq!(owner.session.evidence().body_id, original.body_id);
    assert_eq!(owner.session.evidence().membership, original.membership);
    assert_eq!(owner.session.evidence().body.workset.len(), 1);
    assert!(owner.session.realization().is_none());
    assert!(!owner.host.is_playing());
    assert_eq!(
        super::super::super::retained_source::checked(&root)
            .unwrap()
            .unwrap()
            .expanded
            .name,
        "alpha"
    );
    let reduced = owner.session.evidence().clone();
    assert!(owner
        .commit_admitted_workset_revision(&root, &original, &[(&a, &pa), (&b, &pb)])
        .is_err());
    assert_eq!(owner.session.evidence(), &reduced);
    owner
        .commit_admitted_workset_revision(&root, &reduced, &[(&b, &pb), (&a, &pa)])
        .unwrap();
    assert_eq!(owner.session.evidence().body.workset, original.body.workset);
    assert_eq!(
        owner.session.evidence().body.workload_revision,
        original.body.workload_revision + 2
    );
    assert_eq!(
        state::load(&root).unwrap().unwrap(),
        *owner.session.evidence()
    );
    assert!(owner.session.realization().is_none());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn forged_or_unavailable_replacement_refuses_before_mutating_retained_state() {
    let (root, mut owner) = fixture();
    let before = owner.session.evidence().clone();
    let bytes = fs::read(root.join("body/biography.json")).unwrap();
    let (a, pa) = checked("alpha");
    let mut forged = pa.clone();
    forged.expanded.name = "beta".into();
    assert!(owner
        .commit_admitted_workset_revision(&root, &before, &[(&a, &forged)])
        .is_err());
    let unavailable =
        crate::plot_source::parse(include_str!("../../../../plots/todo/live.conduit")).unwrap();
    let plot = unavailable.expand_entry_for_authoring().unwrap();
    assert!(owner
        .commit_admitted_workset_revision(&root, &before, &[(&unavailable, &plot)])
        .is_err());
    assert_eq!(owner.session.evidence(), &before);
    assert_eq!(fs::read(root.join("body/biography.json")).unwrap(), bytes);
    assert!(!root.join("body/owner-transaction.json").exists());
    assert!(owner.require_workset_recovered().is_ok());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn partial_publication_fences_old_owner_until_normal_restart_recovery() {
    let (root, mut owner) = fixture();
    let expected = owner.session.evidence().clone();
    let (a, pa) = checked("alpha");
    let path = root.join("body/workset-sources.json");
    fs::remove_file(&path).unwrap();
    fs::create_dir(&path).unwrap();
    assert!(owner
        .commit_admitted_workset_revision(&root, &expected, &[(&a, &pa)])
        .is_err());
    assert!(root.join("body/owner-transaction.json").is_file());
    assert!(owner.require_workset_recovered().is_err());
    assert!(owner.persist(&root).is_err());
    assert!(owner.start_service_run(&root, 5_000).is_err());
    assert!(owner.local_face_snapshot().is_err());
    assert!(owner.session.realization().is_none());
    let host_id = owner.host.advertisement().host_id.clone();
    drop(owner);
    fs::remove_dir(&path).unwrap();
    let host = conduit_std_host::StdHost::new_with_config(conduit_std_host::StdHostConfig {
        host_id,
        boot_id: "boot/recovered-workset-owner".into(),
        offer_generation: conduit_core::OfferGeneration(1),
    });
    let resumed = super::super::super::resume_service(host, &root).unwrap();
    assert!(resumed.require_workset_recovered().is_ok());
    assert_eq!(resumed.session.evidence().body_id, expected.body_id);
    assert_eq!(resumed.session.evidence().body.workset.len(), 1);
    assert_eq!(resumed.session.evidence().body.state, BodyState::Lulled);
    assert!(!root.join("body/owner-transaction.json").exists());
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn stale_retained_writer_is_fenced_without_overwriting_the_newer_workset() {
    let (root, mut owner) = fixture();
    let expected = owner.session.evidence().clone();
    let (a, pa) = checked("alpha");
    let alpha = ResidentPlot::new(
        pa.expanded.source_document_id.clone(),
        pa.expanded.checked_plot_id.clone(),
    );
    let removed = expected
        .body
        .workset
        .plots()
        .iter()
        .find(|plot| **plot != alpha)
        .unwrap()
        .clone();
    let ad = owner.host.advertisement();
    let mut committed = owner.session.clone();
    committed
        .remove_plot(
            expected.body.workload_revision,
            &removed,
            &ad.host_id,
            &ad.boot_id,
        )
        .unwrap();
    let retained =
        state::WorksetSources::checked(committed.evidence(), vec![(SOURCE.into(), "alpha".into())])
            .unwrap();
    state::retain_workset_revision(&root, &mut committed, &expected, &retained, None, None)
        .unwrap();
    let bytes = fs::read(root.join("body/biography.json")).unwrap();
    assert!(owner
        .commit_admitted_workset_revision(&root, &expected, &[(&a, &pa)])
        .is_err());
    assert!(owner.require_workset_recovered().is_err());
    assert!(owner.start_service_run(&root, 5_000).is_err());
    assert_eq!(fs::read(root.join("body/biography.json")).unwrap(), bytes);
    assert_eq!(state::load(&root).unwrap().unwrap(), *committed.evidence());
    fs::remove_dir_all(root).unwrap();
}
