//! Quantity admission and refusal through the real browser execution envelope.

use super::*;
use std::collections::BTreeMap;

fn fragment(minimum: i64, maximum: i64) -> PlanFragment {
    let source = format!(
        r#"form quantity-test {{
 input: scalar/literal(value = -1)
 map: math/map-quantity(source-minimum = {minimum}, source-maximum = {maximum}, target-minimum = 0, target-maximum = 100, target-granularity = 1, unit = "%", range-policy = "refuse", quantization = "exact")
 input.value > map.in
}}"#
    );
    let (_, catalog) = crate::installed_browser::catalogs().unwrap();
    let form = conduit_form::parse(&source, &catalog).unwrap();
    let hosts = [crate::installed_browser::advertisement(
        "quantity-browser".into(),
        "quantity-boot".into(),
    )];
    let placements = conduit_planner::default_placements(&form, &hosts).unwrap();
    conduit_planner::plan_with_options(
        &form,
        &hosts,
        &placements,
        &crate::installed_browser::local_bases(),
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: 8,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .unwrap()
    .fragments
    .remove(0)
}

#[test]
fn browser_quantity_range_and_inexact_refusals_cross_admitted_kernel_requests() {
    for (minimum, maximum, detail) in [(0, 1_000_000, 3), (-1_000_000, 0, 4)] {
        let fragment = fragment(minimum, maximum);
        let lowered = lower_plan_fragment(&fragment).unwrap();
        let mut scheduler = prepare_scheduler(&fragment, &lowered).unwrap();
        let capacity = scheduler.values().allocation_capacities();
        let result = drive(&mut scheduler, &fragment);
        assert!(matches!(result, Err(ref error) if error == &format!("OperationFailed({detail})")));
        assert_eq!(scheduler.values().allocation_capacities(), capacity);
        assert!(scheduler
            .signs()
            .events()
            .any(|event| event.kind == conduit_kernel::KernelEventKind::HostOperationCompleted));
    }
}

#[test]
fn browser_quantity_realization_preserves_canonical_value_and_refuses_identity_drift() {
    let fragment = fragment(-2, 0);
    let placement = fragment
        .placements
        .iter()
        .find(|gear| gear.kind_id.as_str() == conduit_semantic_catalog::QUANTITY_MAP_KIND)
        .unwrap();
    assert_eq!(placement.host_operations[0].maximum_input_bytes, 8);
    assert_eq!(placement.host_operations[0].maximum_output_bytes, 9);
    let value = crate::installed_browser::transform_quantity(
        crate::installed_browser::prepare_quantity_mapping(placement).unwrap(),
        &conduit_core::Scalar::from_raw_microunits(-1).encode(),
    )
    .unwrap()
    .unwrap();
    assert_eq!(
        conduit_core::Quantity::decode(&value).unwrap(),
        conduit_core::Quantity::new(50, conduit_core::QuantityUnit::Percent)
    );
    let installation = factory(&placement.implementation_id).unwrap();
    let mut altered = placement.clone();
    altered.artifact_id = "wrong/browser-quantity".into();
    let mut values = HostedValueStore::new(4, 9, 36).unwrap();
    assert!((installation.prepare)(&altered, &mut values).is_err());
}
