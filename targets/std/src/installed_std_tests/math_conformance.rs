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
        let rendered = String::from_utf8(output).unwrap();
        assert!(rendered.contains(" failed reason="));
        assert!(!rendered.contains(" complete\n"));
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

fn run_presented_quantity(source: &str, entry: &str) -> (conduit_core::Plan, crate::StdRunReport) {
    let mut catalog = installed_std::test_catalog();
    let value_type = conduit_semantic_catalog::wrapped_quantity_type();
    let contract =
        conduit_semantic_catalog::structured_presentation_contract("Quantity", &value_type);
    catalog
        .insert(conduit_form::KindDefinition {
            kind_id: contract.kind_id,
            kind_contract_revision: contract.kind_contract_revision,
            inputs: contract.inputs,
            outputs: contract.outputs,
            configuration: Vec::new(),
        })
        .unwrap();
    let syntax = conduit_form::parse_syntax_document(source);
    let mut startup = catalog.startup_catalog().unwrap();
    startup
        .insert_value_kind_alias(
            "Scalar",
            conduit_core::kind_id(conduit_core::SCALAR_INFO_ID),
        )
        .unwrap();
    startup
        .insert_value_kind_alias(
            "Quantity",
            conduit_core::kind_id(conduit_core::QUANTITY_INFO_ID),
        )
        .unwrap();
    let checked = conduit_form::check_syntax_document(&syntax, &startup).unwrap();
    let form = conduit_form::expand_canonical_form(&checked, entry, &catalog).unwrap();
    let limits = form
        .connections
        .iter()
        .map(|cord| {
            let bytes = if cord.value_kind.as_str() == conduit_core::SCALAR_INFO_ID {
                8
            } else if cord.value_kind.as_str() == conduit_core::QUANTITY_INFO_ID {
                9
            } else {
                conduit_semantic_catalog::QUANTITY_INFO_MAXIMUM_BYTES as u32
            };
            (
                (
                    cord.source_gear_id.clone(),
                    cord.source_port_id.clone(),
                    cord.sink_gear_id.clone(),
                    cord.sink_port_id.clone(),
                ),
                conduit_planner::ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity: bytes,
                },
            )
        })
        .collect();
    let mut advertisement = host("quantity-output-host").advertisement().clone();
    advertisement
        .capabilities
        .push(conduit_std_offers::structured_presentation_std_offer(
            "Quantity",
            &value_type,
        ));
    advertisement
        .capabilities
        .sort_by(|a, b| a.capability_id.cmp(&b.capability_id));
    let hosts = [advertisement.clone()];
    let placements = conduit_planner::default_expanded_placements(&form, &hosts).unwrap();
    let plan = conduit_planner::plan_expanded_canonical_with_connection_limits(
        &form,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: conduit_semantic_catalog::QUANTITY_INFO_MAXIMUM_BYTES as u32,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &limits,
    )
    .unwrap();
    let mut output = Vec::new();
    let mut timer = RecordingTimer { waits: Vec::new() };
    let report = installed_std::run_fragment(
        installed_std::InstalledRunHost {
            advertisement: &advertisement,
            playback: None,
            midi_input: None,
            midi_output: None,
            keyboard: None,
            local_model: None,
            vector_search: None,
            calendar: None,
        },
        &plan.fragments[0],
        0,
        &mut 0,
        &mut output,
        &mut timer,
        &crate::RunControl::default(),
    )
    .unwrap();
    assert!(timer.waits.is_empty());
    (plan, report)
}

#[test]
fn quantity_mapping_completes_one_admitted_kernel_request() {
    let source = r#"form quantity_success {
 source: conduit-test/scalar-literal
 map: math/map-quantity(source-minimum = -2, source-maximum = 0, target-maximum = 100, unit = "%")
 wrap: structured-info/wrap-quantity
 show: presentation/structured-info
 source.value > map.in
 map.out > wrap.in
 wrap.out > show.input
}
"#;
    let (plan, report) = run_presented_quantity(source, "quantity_success");
    assert_quantity_presented(
        &plan,
        &report,
        conduit_core::Quantity::new(50, conduit_core::QuantityUnit::Percent),
    );
    assert_eq!(
        report
            .kernel
            .as_ref()
            .unwrap()
            .kernel_sign
            .iter()
            .filter(|event| event.kind == conduit_kernel::KernelEventKind::HostOperationCompleted)
            .count(),
        3
    );
}

fn assert_quantity_presented(
    plan: &conduit_core::Plan,
    report: &crate::StdRunReport,
    quantity: conduit_core::Quantity,
) {
    let value_type = conduit_semantic_catalog::wrapped_quantity_type();
    let presented = report
        .observations
        .iter()
        .find(|sign| matches!(sign.kind, ObservationKind::ValuePresented { .. }))
        .unwrap();
    let ObservationKind::ValuePresented { value } = &presented.kind else {
        unreachable!()
    };
    let expected =
        conduit_core::StructuredInfoValue::leaf(value_type.clone(), quantity.encode().to_vec())
            .unwrap();
    assert_eq!(
        value.value_kind,
        *value_type.profile().unwrap().value_kind()
    );
    assert_eq!(value.encoded, expected.canonical_bytes().unwrap());
    assert_eq!(presented.plan_id.as_ref(), Some(&plan.plan_id));
    assert!(presented.active_play_id.is_some());
    assert!(presented.placement_id.is_some());
    assert!(presented.connection_id.is_some());
    assert!(matches!(
        report.observations.last().unwrap().kind,
        ObservationKind::PlanTerminal {
            disposition: TerminalDisposition::Completed
        }
    ));
    let kernel = report.kernel.as_ref().unwrap();
    let identity = kernel.identity.sign_identity(&presented.sign_id).unwrap();
    assert_eq!(identity.presentation_id, presented.presentation_id);
    assert!(kernel
        .identity
        .request(identity.node.unwrap(), identity.request.unwrap())
        .is_some());
    assert_eq!(
        presented.active_play_id.as_ref(),
        Some(&kernel.active_play_id)
    );
    assert_eq!(kernel.post_play_start_allocations, 0);
    assert_eq!(
        kernel.value_allocation_capacity_before,
        kernel.value_allocation_capacity_after
    );
}

#[test]
fn quantity_refusals_do_not_fabricate_a_connected_presentation() {
    for (minimum, maximum, expected) in [(0, 1_000_000, "out-of-range"), (-1_000_000, 0, "inexact")]
    {
        let source = format!(
            r#"form quantity_refusal {{
 source: conduit-test/scalar-literal
 map: math/map-quantity(source-minimum = {minimum}, source-maximum = {maximum}, target-maximum = 100, unit = "%", range-policy = "refuse", quantization = "exact")
 wrap: structured-info/wrap-quantity
 show: presentation/structured-info
 source.value > map.in
 map.out > wrap.in
 wrap.out > show.input
}}"#
        );
        let (plan, report) = run_presented_quantity(&source, "quantity_refusal");
        assert!(!report
            .observations
            .iter()
            .any(|sign| matches!(sign.kind, ObservationKind::ValuePresented { .. })));
        let failure = report.observations.iter().find(|sign| matches!(&sign.kind,
            ObservationKind::Failure { message: Some(message), .. } if message == &format!("math/map-quantity:{expected}"))).unwrap();
        assert_eq!(failure.plan_id.as_ref(), Some(&plan.plan_id));
        assert!(failure.active_play_id.is_some());
        assert!(failure.placement_id.is_some());
        assert!(matches!(
            report.observations.last().unwrap().kind,
            ObservationKind::PlanTerminal {
                disposition: TerminalDisposition::Failed { .. }
            }
        ));
        let kernel = report.kernel.unwrap();
        let identity = kernel.identity.sign_identity(&failure.sign_id).unwrap();
        let request = kernel
            .identity
            .request(identity.node.unwrap(), identity.request.unwrap())
            .unwrap();
        assert_eq!(
            request.contract_id.as_str(),
            conduit_std_offers::QUANTITY_MAP_HOST_OPERATION
        );
        assert_eq!(
            failure.active_play_id.as_ref(),
            Some(&kernel.active_play_id)
        );
        assert_eq!(kernel.post_play_start_allocations, 0);
        assert!(kernel.presentation_ids.is_empty());
    }
}

#[test]
fn authored_quantity_forms_execute_and_present_through_the_production_kernel() {
    use conduit_core::{Quantity, QuantityUnit};
    for (authored, name, output, expected) in [
        (
            include_str!("../../../../forms/quantity-range-map/main.conduit"),
            "quantity-range-map",
            "quantity",
            Quantity::new(10010, QuantityUnit::Hertz),
        ),
        (
            include_str!("../../../../forms/normalized-light-intensity/main.conduit"),
            "normalized-light-intensity",
            "intensity",
            Quantity::new(50, QuantityUnit::Percent),
        ),
    ] {
        let source = format!(
            r#"{authored}
form quantity_composition {{
 source: conduit-test/scalar-literal
 input: math/scale(gain = -500000000000)
 map: {name}
 wrap: structured-info/wrap-quantity
 show: presentation/structured-info
 source.value > input.in
 input.out > map.control
 map.{output} > wrap.in
 wrap.out > show.input
}}"#
        );
        let (plan, report) = run_presented_quantity(&source, "quantity_composition");
        assert_quantity_presented(&plan, &report, expected);
    }
}
