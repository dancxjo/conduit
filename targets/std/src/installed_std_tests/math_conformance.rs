use super::{host, installed_std, BTreeMap, BaseImplementationId, PlanningOptions, RecordingTimer};
use conduit_core::{ArtifactId, ObservationKind, TerminalDisposition, SCALAR_ENCODED_LEN};
use conduit_form::parse;
use conduit_planner::{default_placements, plan_with_options};

const FORM: &str = r#"form math_control {
 source: conduit-test/scalar-literal
 deadband: math/deadband(radius = 0)
 scale: math/scale(gain = 1000000)
 clamp: math/clamp(minimum = -1, maximum = -1)
 sink: conduit-test/logic-sink
 source.value > deadband.in
 deadband.out > scale.in
 scale.out > clamp.in
 clamp.out > sink.in
}
"#;

fn plan(source: &str) -> (super::StdHost, conduit_core::Plan) {
    let host = host("typed-math-host");
    let form = parse(source, &installed_std::test_catalog()).expect("typed math Form parses");
    let hosts = [host.advertisement().clone()];
    let placements = default_placements(&form, &hosts).expect("math placements resolve");
    let plan = plan_with_options(
        &form,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: SCALAR_ENCODED_LEN as u32,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .expect("math Form plans with capacity-one cords");
    (host, plan)
}

#[test]
fn deadband_scale_and_clamp_execute_together_through_the_production_kernel() {
    let (mut host, plan) = plan(FORM);
    let fragment = &plan.fragments[0];
    assert_eq!(fragment.placements.len(), 5);
    assert_eq!(fragment.connections.len(), 4);
    assert!(fragment.connections.iter().all(|cord| {
        cord.item_capacity == 1 && cord.byte_capacity == SCALAR_ENCODED_LEN as u32
    }));
    for (kind, implementation) in [
        (
            conduit_semantic_catalog::MATH_DEADBAND_KIND,
            conduit_std_offers::MATH_DEADBAND_IMPLEMENTATION,
        ),
        (
            conduit_semantic_catalog::MATH_SCALE_KIND,
            conduit_std_offers::MATH_SCALE_IMPLEMENTATION,
        ),
        (
            conduit_semantic_catalog::MATH_CLAMP_KIND,
            conduit_std_offers::MATH_CLAMP_IMPLEMENTATION,
        ),
    ] {
        let placement = fragment
            .placements
            .iter()
            .find(|placement| placement.kind_id.as_str() == kind)
            .expect("math placement exists");
        assert_eq!(placement.implementation_id.as_str(), implementation);
        assert_eq!(placement.host_operations.len(), 1);
    }

    let mut output = Vec::with_capacity(1_024);
    let mut timer = RecordingTimer { waits: Vec::new() };
    let report = host
        .run_fragment_to(fragment.clone(), &mut output, &mut timer)
        .expect("combined math Form executes through the installed kernel");
    assert!(timer.waits.is_empty());
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

#[test]
fn invalid_clamp_and_mutated_math_implementation_refuse_before_play() {
    let invalid = FORM
        .replace("minimum = -1", "minimum = 2")
        .replace("maximum = -1", "maximum = 1");
    let (mut host, invalid_plan) = plan(&invalid);
    let mut output = Vec::new();
    let mut timer = RecordingTimer { waits: Vec::new() };
    assert!(host
        .run_fragment_to(invalid_plan.fragments[0].clone(), &mut output, &mut timer)
        .is_err());
    assert!(timer.waits.is_empty());

    let (mut host, plan) = plan(FORM);
    let mut fragment = plan.fragments[0].clone();
    fragment
        .placements
        .iter_mut()
        .find(|placement| placement.kind_id.as_str() == conduit_semantic_catalog::MATH_SCALE_KIND)
        .expect("scale placement exists")
        .artifact_id = ArtifactId::from("mutated/math-scale");
    assert!(host
        .run_fragment_to(fragment, &mut output, &mut timer)
        .is_err());
}

#[test]
fn quantity_range_and_quantization_refusals_reach_the_production_kernel() {
    for (minimum, maximum, detail) in [(0, 1_000_000, 3), (-1_000_000, 0, 4)] {
        let source = format!(
            r#"form quantity_refusal {{
 source: conduit-test/scalar-literal
 map: math/map-quantity(source-minimum = {minimum}, source-maximum = {maximum}, target-minimum = 0, target-maximum = 100, target-granularity = 1, unit = "%", range-policy = "refuse", quantization = "exact")
 source.value > map.in
}}
"#
        );
        let (mut host, plan) = plan(&source);
        let mapping = plan.fragments[0]
            .placements
            .iter()
            .find(|placement| {
                placement.kind_id.as_str() == conduit_semantic_catalog::QUANTITY_MAP_KIND
            })
            .unwrap();
        assert_eq!(
            mapping.implementation_id.as_str(),
            conduit_std_offers::QUANTITY_MAP_IMPLEMENTATION
        );
        assert_eq!(mapping.host_operations[0].maximum_input_bytes, 8);
        assert_eq!(mapping.host_operations[0].maximum_output_bytes, 9);
        let mut output = Vec::new();
        let mut timer = RecordingTimer { waits: Vec::new() };
        let report = host
            .run_fragment_to(plan.fragments[0].clone(), &mut output, &mut timer)
            .unwrap();
        let expected = if detail == 3 {
            "math/map-quantity:out-of-range"
        } else {
            "math/map-quantity:inexact"
        };
        let failure = report.observations.iter().find(|sign| matches!(
            &sign.kind, ObservationKind::Failure { message: Some(message), .. } if message == expected
        )).expect("exact mapping failure is retained in a Sign");
        assert_eq!(failure.plan_id.as_ref(), Some(&plan.plan_id));
        assert_eq!(failure.placement_id.as_ref(), Some(&mapping.placement_id));
        assert!(failure.active_play_id.is_some());
        assert!(matches!(
            report.observations.last().unwrap().kind,
            ObservationKind::PlanTerminal {
                disposition: TerminalDisposition::Failed { .. }
            }
        ));
        let kernel = report.kernel.unwrap();
        assert_eq!(kernel.post_play_start_allocations, 0);
        assert_eq!(
            kernel.value_allocation_capacity_before,
            kernel.value_allocation_capacity_after
        );
        assert!(timer.waits.is_empty());
    }
}
