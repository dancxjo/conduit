use super::{host, installed_std, BTreeMap, ConnectionBase, PlanningOptions, RecordingTimer};
use conduit_core::{ObservationKind, TerminalDisposition};
use conduit_form::parse;
use conduit_planner::{default_placements, plan_with_options};
use conduit_presentation::MAX_PRESENTATION_COMPOSITION_BYTES;

const FORM: &str = r#"form gear_face_presentation {
 icon: presentation/icon(icon = "presentation", accessibility-name = "Patchbay")
 frame: presentation/frame(role = "panel", accessibility-name = "Gear Face")
 badge: presentation/badge(state = "warning", accessibility-name = "Cord pressure")
 sink: conduit-test/presentation-sink
 icon.presented > frame.content
 frame.presented > badge.content
 badge.presented > sink.in
}
"#;

#[test]
fn ordinary_form_executes_the_canonical_composition_family() {
    let mut host = host("presentation-composition-host");
    let form = parse(FORM, &installed_std::test_catalog()).expect("presentation Form parses");
    let hosts = [host.advertisement().clone()];
    let placements = default_placements(&form, &hosts).expect("placements resolve");
    let plan = plan_with_options(
        &form,
        &hosts,
        &placements,
        &[ConnectionBase::Local],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: MAX_PRESENTATION_COMPOSITION_BYTES as u32,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .expect("presentation Form plans");
    let fragment = &plan.fragments[0];
    assert_eq!(fragment.placements.len(), 4);
    assert_eq!(fragment.connections.len(), 3);
    let identity = (fragment.checked_form_id.clone(), fragment.plan_id.clone());
    let mut output = Vec::new();
    let mut timer = RecordingTimer { waits: Vec::new() };
    let report = host
        .run_fragment_to(fragment.clone(), &mut output, &mut timer)
        .expect("presentation Form executes through production kernel");
    assert_eq!(
        identity,
        (fragment.checked_form_id.clone(), fragment.plan_id.clone()),
        "presentation metadata does not rewrite checked Form or Plan identity"
    );
    assert!(matches!(
        report.observations.last().map(|item| &item.kind),
        Some(ObservationKind::PlanTerminal {
            disposition: TerminalDisposition::Completed
        })
    ));
    let kernel = report.kernel.expect("kernel report exists");
    assert_eq!(
        kernel.value_allocation_capacity_before,
        kernel.value_allocation_capacity_after
    );
    assert_eq!(kernel.post_play_start_allocations, 0);

    let mut unsupported = fragment.clone();
    unsupported.placements[0].execution_profile_id =
        conduit_core::ExecutionProfileId::from("renderer/private-profile@1");
    let error = host
        .run_fragment_to(unsupported, &mut Vec::new(), &mut timer)
        .expect_err("unsupported presentation profile refuses before Play");
    assert!(
        error.contains("InvalidFragment"),
        "unexpected refusal: {error}"
    );
}
