//! Trusted continuation and custody tests; ordinary-Plot authority is separate.
use super::super::service_workset_tests::{fixture, SOURCE};
use super::*;
use std::fs;
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
    let workspace_probe = crate::allocation_probe::begin();
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
    let workspace = workspace_probe.finish_statistics();
    println!("Trusted workset revision current-thread allocation diagnostic: {workspace:?}");
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

#[test]
fn revision_preflight_preserves_selected_external_fore_bounds() {
    use conduit_core::{ConnectionTrack, PortDirection};
    use conduit_planner::{ConnectionQueueLimits, ForeBoundaryKey};
    use std::collections::BTreeMap;
    let (root, mut owner) = fixture();
    let source = crate::plot_source::parse(
        "plot external-message (\n >> message: Text\n) {\n show: face/text(\"External message\")\n message >> show\n}.",
    ).unwrap();
    let plot = source.expand_entry_for_authoring().unwrap();
    let resident = ResidentPlot::new(
        plot.expanded.source_document_id.clone(),
        plot.expanded.checked_plot_id.clone(),
    );
    let key = ForeBoundaryKey {
        direction: PortDirection::Input,
        front_port_id: "message".into(),
        track: ConnectionTrack::Payload,
    };
    let boundaries = BTreeMap::from([(
        resident.clone(),
        BTreeMap::from([(
            key.clone(),
            ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: 64,
            },
        )]),
    )]);
    let expected = owner.session.evidence().clone();
    let before = fs::read(root.join("body/biography.json")).unwrap();
    let mut invalid = boundaries.clone();
    invalid
        .get_mut(&resident)
        .unwrap()
        .get_mut(&key)
        .unwrap()
        .item_capacity = 0;
    assert!(owner
        .commit_admitted_workset_revision_with_boundaries(
            &root,
            &expected,
            &[(&source, &plot)],
            &invalid,
            false,
        )
        .is_err());
    assert_eq!(owner.session.evidence(), &expected);
    assert_eq!(fs::read(root.join("body/biography.json")).unwrap(), before);
    assert!(!root.join("body/owner-transaction.json").exists());
    let mut foreign = boundaries.clone();
    foreign.insert(expected.body.workset.plots()[0].clone(), BTreeMap::new());
    assert!(owner
        .commit_admitted_workset_revision_with_boundaries(
            &root,
            &expected,
            &[(&source, &plot)],
            &foreign,
            false,
        )
        .is_err());
    assert_eq!(owner.session.evidence(), &expected);
    owner
        .commit_admitted_workset_revision_with_boundaries(
            &root,
            &expected,
            &[(&source, &plot)],
            &boundaries,
            false,
        )
        .unwrap();
    assert!(owner.session.realization().is_none());
    assert!(!owner.host.is_playing());
    assert_eq!(
        state::load(&root).unwrap().unwrap(),
        *owner.session.evidence()
    );
    let partitions = owner
        .prepare_workset_partitions(&[(&source, &plot)], &boundaries)
        .unwrap();
    let ports: Vec<_> = partitions[0]
        .plan
        .fragments
        .iter()
        .flat_map(|fragment| &fragment.fore_ports)
        .filter(|port| {
            port.front_port_id == key.front_port_id
                && port.direction == key.direction
                && port.track == key.track
        })
        .collect();
    assert_eq!(ports.len(), 1);
    assert_eq!(ports[0].item_capacity, 1);
    assert_eq!(ports[0].byte_capacity, 64);
    fs::remove_dir_all(root).unwrap();
}

#[test]
fn exact_retained_workset_revalidation_preserves_biography_and_checks_basis() {
    let (root, mut owner) = fixture();
    let before = owner.session.evidence().clone();
    let (a, pa) = checked("alpha");
    let (b, pb) = checked("beta");
    let sources = [(&b, &pb), (&a, &pa)];
    let boundaries =
        super::super::continuing::boundaries_for_plots(sources.iter().map(|(_, plot)| *plot), None)
            .unwrap();
    let bytes = fs::read(root.join("body/biography.json")).unwrap();
    owner
        .commit_admitted_workset_revision_with_boundaries(
            &root,
            &before,
            &sources,
            &boundaries,
            true,
        )
        .unwrap();
    assert_eq!(owner.session.evidence(), &before);
    assert_eq!(fs::read(root.join("body/biography.json")).unwrap(), bytes);
    assert!(owner.session.realization().is_none());
    assert!(owner
        .commit_admitted_workset_revision_with_boundaries(
            &root,
            &before,
            &sources,
            &boundaries,
            false
        )
        .is_err());
    let mut stale = before.clone();
    stale.body.workload_revision += 1;
    assert!(owner
        .commit_admitted_workset_revision_with_boundaries(
            &root,
            &stale,
            &sources,
            &boundaries,
            true
        )
        .is_err());
    fs::remove_dir_all(root).unwrap();
}
