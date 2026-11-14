//! Semantic availability and exact current selection, not execution custody.
use super::*;
use crate::durable_host::local_fore_policy::{
    LocalForeAdapterIntent, LocalForePortIntent, SelectedLocalForeSet,
};
use conduit_body::ResidentPlot;
use conduit_core::{ConnectionTrack, PortDirection};
fn fixture() -> (Body, ExpandedAuthoringPlot) {
    let source = crate::plot_source::parse(include_str!(
        "../../../../../make/body/tests/source_check_call/check.conduit"
    ))
    .unwrap();
    let plot = source.expand_entry_for_authoring().unwrap();
    let body = Body::born(
        plot.expanded.source_document_id.clone(),
        plot.expanded.checked_plot_id.clone(),
        1,
        "fixture/birth".into(),
    )
    .unwrap();
    (body, plot)
}
fn face(body: &Body, wake: Option<&conduit_body::Wake>) -> Face {
    conduit_face::BodyFace::project(
        body,
        wake,
        1,
        conduit_face::FaceContext::Overview,
        conduit_face::FaceFocus::Body,
        vec![],
    )
    .unwrap()
    .face
}
fn ad(name: &str) -> conduit_core::HostAdvertisement {
    conduit_std_host::StdHost::new_with_config(conduit_std_host::StdHostConfig {
        host_id: name.into(),
        boot_id: format!("{name}/boot").into(),
        offer_generation: conduit_core::OfferGeneration(1),
    })
    .advertisement()
    .clone()
}
fn selected(
    root: &conduit_core::HostAdvertisement,
    target: &conduit_core::HostAdvertisement,
    plot: &ExpandedAuthoringPlot,
) -> SelectedLocalForeSet {
    let input = &plot
        .expanded
        .gears
        .iter()
        .find(|g| g.gear_id == plot.input_bindings[0].gear_id)
        .unwrap()
        .inputs[0];
    SelectedLocalForeSet::prepare(
        root,
        std::slice::from_ref(plot),
        std::slice::from_ref(target),
        &[LocalForeAdapterIntent {
            entry: plot.expanded.name.clone(),
            host_id: target.host_id.clone(),
            configuration_id: target.profile.as_str().into(),
            ports: vec![LocalForePortIntent {
                front_port_id: plot.input_bindings[0].front_port_id.clone(),
                direction: PortDirection::Input,
                track: ConnectionTrack::Payload,
                value_kind: input.value_kind.clone(),
                item_capacity: 1,
                byte_capacity: 16384,
            }],
        }],
    )
    .unwrap()
}
#[test]
fn generic_wake_disables_missing_input_but_preserves_body_and_other_available_actions() {
    let (body, plot) = fixture();
    let before = body.clone();
    let mut requirements = WakeIngressRequirements::default();
    requirements.observe(&body, &plot).unwrap();
    let original = face(&body, None);
    let revised = project(original.clone(), requirements.refusal(&body, |_| false)).unwrap();
    let wake = revised
        .actions
        .iter()
        .find(|a| a.intent == "conduit.intent/wake@1")
        .unwrap();
    assert!(
        matches!(&wake.availability,FaceActionAvailability::Unavailable {reason_code,..} if reason_code=="service-input-adapter-required")
    );
    assert_ne!(original.identity, revised.identity);
    revised.validate().unwrap();
    assert_eq!(body, before);
    for action in original
        .actions
        .iter()
        .filter(|a| a.intent != "conduit.intent/wake@1")
    {
        assert_eq!(
            revised
                .actions
                .iter()
                .find(|a| a.identity == action.identity),
            Some(action)
        );
    }
}
#[test]
fn exact_selected_remote_partition_input_preserves_available_wake_but_stale_boot_refuses() {
    let (body, plot) = fixture();
    let root = ad("root");
    let target = ad("moon");
    let selected = selected(&root, &target, &plot);
    let mut requirements = WakeIngressRequirements::default();
    requirements.observe(&body, &plot).unwrap();
    let current = [&root, &target];
    let refusal = requirements.refusal(&body, |p| {
        selected.contains_current_boundaries(&root, p, current.into_iter())
    });
    assert_eq!(refusal, None);
    let original = face(&body, None);
    assert_eq!(project(original.clone(), refusal).unwrap(), original);
    let mut stale = target.clone();
    stale.boot_id = "moon/next-boot".into();
    assert_eq!(
        requirements.refusal(&body, |p| selected.contains_current_boundaries(
            &root,
            p,
            [&root, &stale].into_iter()
        )),
        Some("service-input-adapter-required")
    );
    let mut other_root = root.clone();
    other_root.offer_generation.0 += 1;
    let resident = ResidentPlot::new(
        plot.expanded.source_document_id.clone(),
        plot.expanded.checked_plot_id.clone(),
    );
    assert!(!selected.contains_current_boundaries(
        &other_root,
        &resident,
        [&other_root, &target].into_iter()
    ));
}
#[test]
fn source_change_requires_recheck_and_awake_lifecycle_is_unchanged() {
    let (body, plot) = fixture();
    let mut requirements = WakeIngressRequirements::default();
    requirements.observe(&body, &plot).unwrap();
    let mut changed = body.clone();
    changed.workload_revision += 1;
    assert_eq!(
        requirements.refusal(&changed, |_| true),
        Some("owner-wake-input-readiness-unchecked")
    );
    let (awake, wake) = body.wake(2, "fixture/wake".into()).unwrap();
    let face = face(&awake, Some(&wake));
    assert_eq!(requirements.refusal(&awake, |_| false), None);
    assert_eq!(project(face.clone(), None).unwrap(), face);
    assert!(
        face.actions
            .iter()
            .any(|a| a.intent == "conduit.intent/lull@1" && a.availability.is_available())
    );
}
