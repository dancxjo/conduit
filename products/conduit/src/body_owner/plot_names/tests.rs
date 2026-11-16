//! Checked Source -> canonical Library Face -> shared native scene (CPU proof).
use super::super::service_workset_tests;
use super::*;
use conduit_face::{BodyFace, FaceContext, FaceFocus, FaceNames, FaceRole};
use conduit_std_host::{StdHost, StdHostConfig};

fn resumed(root: &std::path::Path) -> Owner {
    crate::durable_host::owner::resume_service(
        StdHost::new_with_config(StdHostConfig {
            host_id: "host/workset-service".into(),
            boot_id: crate::durable_host::fresh_identity("boot/names-resume", "test").into(),
            offer_generation: conduit_core::OfferGeneration(1),
        }),
        root,
    )
    .unwrap()
}
fn names(owner: &Owner) -> Vec<String> {
    owner
        .local_face_snapshot()
        .unwrap()
        .subjects
        .into_iter()
        .filter(|s| s.role == FaceRole::Plot)
        .map(|s| s.name)
        .collect()
}
#[test]
fn complete_workset_names_survive_restart_and_render_in_native_library() {
    let (root, original) = service_workset_tests::fixture();
    let body = original.session.evidence().body_id.clone();
    drop(original);
    let owner = resumed(&root);
    assert_eq!(owner.session.evidence().body_id, body);
    let projected_names = names(&owner);
    assert_eq!(projected_names.len(), 2);
    assert!(projected_names.iter().any(|n| n == "alpha"));
    assert!(projected_names.iter().any(|n| n == "beta"));
    let labels = owner.face_plot_names();
    let face = BodyFace::project_with_names(
        &owner.session.evidence().body,
        None,
        1,
        FaceContext::Library,
        FaceFocus::Body,
        vec![],
        FaceNames {
            body_name: Some("Two checked Plots"),
            resident_plots: &labels,
        },
    )
    .unwrap()
    .face;
    let mut scene =
        conduitos::native_face_scene::NativeFaceScene::prepare(face, 1024, 600).unwrap();
    let mut seen = [false; 2];
    for _ in 0..scene.pages() {
        let frame = scene.frame().unwrap();
        for (found, name) in seen.iter_mut().zip(["alpha", "beta"]) {
            *found |= frame
                .scene
                .commands()
                .iter()
                .any(|c| c.payload().contains(name));
        }
        scene.turn_page(true);
    }
    assert!(seen.into_iter().all(|found| found));
    drop(owner);
    assert_eq!(names(&resumed(&root)), projected_names);
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn replacement_retires_removed_names_only_after_successful_commit() {
    let (root, original) = service_workset_tests::fixture();
    drop(original);
    let mut owner = resumed(&root);
    let expected = owner.session.evidence().clone();
    let source = crate::plot_source::parse(service_workset_tests::SOURCE).unwrap();
    let alpha = source.expand_named_for_authoring("alpha").unwrap();
    let beta = source.expand_named_for_authoring("beta").unwrap();
    let before = names(&owner);
    let mut forged = alpha.clone();
    forged.expanded.name = "forged friendly name".into();
    assert!(owner
        .commit_admitted_workset_revision(&root, &expected, &[(&source, &forged)])
        .is_err());
    assert_eq!(names(&owner), before);
    owner
        .commit_admitted_workset_revision(&root, &expected, &[(&source, &beta)])
        .unwrap();
    assert_eq!(names(&owner), ["beta"]);
    assert!(owner.set_foreground_plot_name(&alpha).is_err());
    assert_eq!(names(&owner), ["beta"]);
    drop(owner);
    assert_eq!(names(&resumed(&root)), ["beta"]);
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn label_table_refuses_foreign_duplicate_and_unbounded_labels() {
    let p = ResidentPlot::new("source/one".into(), "checked/one".into());
    let q = ResidentPlot::new("source/other".into(), "checked/other".into());
    let workset = BodyWorkset::from_plots(vec![p.clone()]).unwrap();
    let label = (&p.source_document_id, &p.checked_plot_id, "One");
    assert!(PlotNames::prepare(&workset, [label, label].into_iter(), None).is_err());
    assert!(PlotNames::prepare(
        &workset,
        [(&q.source_document_id, &q.checked_plot_id, "Other")].into_iter(),
        None
    )
    .is_err());
    let long = "x".repeat(conduit_plot::MAX_CHECK_LABEL_BYTES + 1);
    assert!(PlotNames::prepare(
        &workset,
        [(&p.source_document_id, &p.checked_plot_id, long.as_str())].into_iter(),
        None
    )
    .is_err());
}

fn image_host() -> StdHost {
    use conduit_host_make::*;
    let source = include_str!("../../../../../targets/std/profiles/linux-computer.host.conduit")
        .replace("\"clock/monotonic\"", "\"timer/monotonic\"")
        .replace(
            "  limits =",
            r#"
  capability = {kind: "time/tick", pinned_implementation: "std/kernel-time-tick@2"}
  need = {id: "timer/main", class: "conduit.resource/timer-slot@1", slots: 8, bytes: 512}
  limits ="#,
        );
    let catalog = conduit_workspace_make::catalog();
    let packages = conduit_workspace_make::package_set();
    let mut configuration = parse_host_configuration_conduit(&source).unwrap();
    configuration.limits.heap_arena_bytes = 1024 * 1024;
    let checked = check_host_configuration(configuration, &catalog, &packages).unwrap();
    let (image, _) = build_default_host_image(
        checked.profile().clone(),
        &catalog,
        &packages,
        &BuildInputs {
            source_identity: "fixture/plot-label-custody".into(),
            toolchain_available: true,
        },
    )
    .unwrap();
    StdHost::from_selected_timer_image(
        StdHostConfig {
            host_id: "host/labels".into(),
            boot_id: "boot/labels".into(),
            offer_generation: conduit_core::OfferGeneration(1),
        },
        &image,
        &catalog,
    )
    .unwrap()
}
fn available(owner: &ImageRootPreparationOwner) -> usize {
    let (mut lo, mut hi): (usize, usize) = (1, 1024 * 1024);
    while lo < hi {
        let mid = lo + (hi - lo).div_ceil(2);
        // SAFETY: allocation-free callback, immediately retired workspace only.
        let probe = unsafe { owner.prepare_value(mid, 0, || Ok(())) };
        if probe.is_ok() {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    lo
}
#[test]
fn image_labels_charge_exact_original_counter_refuse_pressure_and_refund() {
    let host = image_host();
    let root = host.image_root_preparation_owner().unwrap();
    let p = ResidentPlot::new("source/one".into(), "checked/one".into());
    let workset = BodyWorkset::from_plots(vec![p.clone()]).unwrap();
    let label = (&p.source_document_id, &p.checked_plot_id, "Room sampling");
    let before = available(&root);
    let allocation = crate::allocation_probe::begin();
    let names = PlotNames::prepare(&workset, [label].into_iter(), Some(&root)).unwrap();
    let allocation = allocation.finish_statistics();
    let charged = before - available(&root);
    println!("one-label IMAGE charge {charged} bytes; factory allocation {allocation:?}");
    assert!(charged >= size_of::<Vec<Entry>>() + size_of::<Entry>() + 10 + 11 + 13);
    root.verify_prepared(match &names {
        PlotNames::Image(value) => value,
        _ => panic!("IMAGE custody required"),
    })
    .unwrap();
    // Consume the remaining actual original counter, rather than creating a
    // second budget or granting capacity to make the replacement fit.
    let free = available(&root);
    let pressure = unsafe { root.prepare_value(free, free, || Ok(())) }.unwrap();
    assert!(PlotNames::prepare(&workset, [label].into_iter(), Some(&root)).is_err());
    assert_eq!(names.entries()[0].1, "Room sampling");
    drop(pressure);
    drop(names);
    assert_eq!(available(&root), before);
}
