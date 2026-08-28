use super::{host, installed_std, BTreeMap, BaseImplementationId, PlanningOptions, RecordingTimer};
use conduit_core::{ObservationKind, TerminalDisposition};
use conduit_form::parse;
use conduit_planner::{default_placements, plan_with_options};
use conduit_presentation::MAX_LAYOUT_FRAME_BYTES;

const FORM: &str = r#"form patchbay_shell {
 viewport: layout/viewport(width = 960, height = 540, children = 3, child-width = 120, child-height = 80)
 inset: layout/inset(inset = 12)
 row: layout/row(gap = 8)
 column: layout/column(gap = 6)
 stack: layout/stack
 face: layout/align(horizontal = "center", vertical = "end")
 sink: conduit-test/layout-sink
 viewport.placements > inset.frame
 inset.placements > row.frame
 row.placements > column.frame
 column.placements > stack.frame
 stack.placements > face.frame
 face.placements > sink.in
}
"#;

#[test]
fn representative_shell_and_face_execute_as_one_ordinary_form() {
    let mut host = host("layout-host");
    let form = parse(FORM, &installed_std::test_catalog()).expect("layout Form parses");
    let hosts = [host.advertisement().clone()];
    let placements = default_placements(&form, &hosts).expect("layout placements resolve");
    let plan = plan_with_options(
        &form,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: MAX_LAYOUT_FRAME_BYTES as u32,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .expect("layout Form plans");
    let fragment = &plan.fragments[0];
    assert_eq!(fragment.placements.len(), 7);
    assert_eq!(fragment.connections.len(), 6);
    let identity = (fragment.checked_form_id.clone(), fragment.plan_id.clone());
    let mut output = Vec::new();
    let mut timer = RecordingTimer { waits: Vec::new() };
    let report = host
        .run_fragment_to(fragment.clone(), &mut output, &mut timer)
        .expect("layout Form executes through production kernel");
    assert_eq!(
        identity,
        (fragment.checked_form_id.clone(), fragment.plan_id.clone()),
        "layout state is not semantic identity"
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
}
