//! Exact normalized control through the reusable typed measurement boundary.

use super::engine::{
    complete_host_effect, complete_host_effect_with_output, drive, prepare, BrowserHostEffect,
    DriveStatus,
};
use conduit_core::{Quantity, QuantityUnit, StructuredInfoValueShape, TemporalScale};
use conduit_form::{KindDefinition, KindSignature};
use conduit_planner::{default_expanded_placements, plan_expanded_canonical_with_options};
use std::collections::BTreeMap;

const NORMALIZED_MEASUREMENT: &str =
    include_str!("../../../../../forms/normalized-control-measurement/main.conduit");

fn fragment() -> conduit_core::PlanFragment {
    let (mut startup, mut catalog) = crate::installed_browser::catalogs().unwrap();
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
    let sink = crate::installed_browser::test_measurement_observation_sink::offer();
    startup
        .insert(KindSignature {
            kind: sink.kind_id.as_str().into(),
            startup_parameters: Vec::new(),
        })
        .unwrap();
    catalog
        .insert(KindDefinition {
            kind_id: sink.kind_id.clone(),
            kind_contract_revision: sink.kind_contract_revision.clone(),
            inputs: sink.inputs.clone(),
            outputs: Vec::new(),
            configuration: Vec::new(),
        })
        .unwrap();
    let source = format!(
        "{NORMALIZED_MEASUREMENT}\n\nform normalized-control-measurement-proof {{\n    input: scalar/literal(value = 250000)\n    observe: normalized-control-measurement\n    output: {}\n    input.value > observe.control\n    observe.measurement > output.measurement\n}}\n",
        crate::installed_browser::test_measurement_observation_sink::KIND
    );
    let syntax = conduit_form::parse_syntax_document(&source);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = conduit_form::check_syntax_document(&syntax, &startup).unwrap();
    let expanded = conduit_form::expand_canonical_form(
        &checked,
        "normalized-control-measurement-proof",
        &catalog,
    )
    .unwrap();
    let mut host = crate::installed_browser::advertisement(
        "browser/measurement-controller".into(),
        "boot/measurement-controller".into(),
    );
    host.capabilities.push(sink);
    host.capabilities
        .sort_by(|left, right| left.capability_id.cmp(&right.capability_id));
    assert!(host
        .capabilities
        .iter()
        .any(|offer| { offer.kind_id.as_str() == conduit_data::MEASUREMENT_OBSERVATION_KIND }));
    let placements = default_expanded_placements(&expanded, &[host.clone()]).unwrap();
    plan_expanded_canonical_with_options(
        &expanded,
        &[host],
        &placements,
        &crate::installed_browser::local_bases(),
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: crate::installed_browser::MAXIMUM_BROWSER_VALUE_BYTES as u32,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .unwrap()
    .fragments
    .remove(0)
}

fn interactive_fragment() -> conduit_core::PlanFragment {
    let (mut startup, mut catalog) = crate::installed_browser::catalogs().unwrap();
    for (alias, kind) in [
        ("Scalar", conduit_core::SCALAR_INFO_ID),
        ("Quantity", conduit_core::QUANTITY_INFO_ID),
    ] {
        startup
            .insert_value_kind_alias(alias, conduit_core::kind_id(kind))
            .unwrap();
    }
    let sink = crate::installed_browser::test_measurement_observation_sink::offer();
    startup
        .insert(KindSignature {
            kind: sink.kind_id.as_str().into(),
            startup_parameters: Vec::new(),
        })
        .unwrap();
    catalog
        .insert(KindDefinition {
            kind_id: sink.kind_id.clone(),
            kind_contract_revision: sink.kind_contract_revision.clone(),
            inputs: sink.inputs.clone(),
            outputs: Vec::new(),
            configuration: Vec::new(),
        })
        .unwrap();
    let source = format!(
        "{NORMALIZED_MEASUREMENT}\n\nform browser-control-measurement-proof {{\n    controller: input/pointer-source\n    normalize: math/normalized-quantity-scalar\n    observe: normalized-control-measurement\n    output: {}\n    controller.pointer > project(PointerEvent.position) > project(Point2.x) > normalize.in\n    normalize.out > observe.control\n    observe.measurement > output.measurement\n}}\n",
        crate::installed_browser::test_measurement_observation_sink::KIND
    );
    let syntax = conduit_form::parse_syntax_document(&source);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = conduit_form::check_syntax_document(&syntax, &startup).unwrap();
    let selector_offers = crate::installed_browser::catalogs::install_checked_structured_selectors(
        &checked,
        &mut catalog,
    )
    .unwrap();
    let expanded = conduit_form::expand_canonical_form(
        &checked,
        "browser-control-measurement-proof",
        &catalog,
    )
    .unwrap();
    let mut host = crate::installed_browser::advertisement(
        "browser/interactive-measurement".into(),
        "boot/interactive-measurement".into(),
    );
    host.capabilities.extend(selector_offers);
    host.capabilities.push(sink);
    host.capabilities
        .sort_by(|left, right| left.capability_id.cmp(&right.capability_id));
    let placements = default_expanded_placements(&expanded, &[host.clone()]).unwrap();
    plan_expanded_canonical_with_options(
        &expanded,
        &[host],
        &placements,
        &crate::installed_browser::local_bases(),
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: crate::installed_browser::MAXIMUM_BROWSER_VALUE_BYTES as u32,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .unwrap()
    .fragments
    .remove(0)
}

fn decode_sample(bytes: &[u8]) -> conduit_data::MeasurementSample {
    let value = conduit_core::StructuredInfoValue::from_canonical_bytes(bytes).unwrap();
    assert_eq!(value.value_type(), &conduit_data::measurement_sample_type());
    let StructuredInfoValueShape::Leaf(payload) = value.shape() else {
        panic!("measurement observation is not its exact leaf type")
    };
    conduit_data::decode_measurement_sample(payload).unwrap()
}

#[test]
fn deterministic_control_runs_the_authored_measurement_form_in_the_production_kernel() {
    let fragment = fragment();
    let (mut scheduler, effect) = prepare(&fragment).unwrap();
    let capacities = scheduler.values().allocation_capacities();
    let BrowserHostEffect::Manifestation(manifestation) = &effect.effect else {
        panic!("measurement proof requested an unexpected Host effect")
    };
    let sample = decode_sample(&manifestation.canonical_value);
    assert_eq!(sample.value, Quantity::new(25, QuantityUnit::Millivolt));
    assert_eq!(sample.observed_at.ticks, 1);
    assert_eq!(sample.observed_at.scale, TemporalScale::Milliseconds);
    assert_eq!(sample.observed_at.clock_basis, "control-occurrence");
    assert_eq!(sample.uncertainty, None);
    complete_host_effect(&mut scheduler, &effect).unwrap();
    assert!(matches!(
        drive(&mut scheduler, &fragment).unwrap(),
        DriveStatus::Quiescent
    ));
    assert_eq!(scheduler.values().allocation_capacities(), capacities);
    assert!(scheduler
        .signs()
        .events()
        .any(|event| { event.kind == conduit_kernel::KernelEventKind::HostOperationCompleted }));
}

#[test]
fn browser_pointer_and_deterministic_control_share_the_exact_measurement_form() {
    let fragment = interactive_fragment();
    let (mut scheduler, pointer) = prepare(&fragment).unwrap();
    assert!(matches!(pointer.effect, BrowserHostEffect::PointerEvent));
    let pointer_value = conduit_semantic_catalog::normalized_pointer_value(
        conduit_semantic_catalog::NormalizedPointerSample {
            position_x: 250_000,
            position_y: 750_000,
            delta_x: 0,
            delta_y: 0,
            primary_pressed: false,
            coalesced: 0,
            dropped: 0,
            queue_capacity: 1,
            sequence: 7,
        },
    )
    .unwrap()
    .canonical_bytes()
    .unwrap();
    complete_host_effect_with_output(&mut scheduler, &pointer, &pointer_value).unwrap();
    let DriveStatus::Effect(effect) = drive(&mut scheduler, &fragment).unwrap() else {
        panic!("interactive measurement did not reach its typed sink")
    };
    let BrowserHostEffect::Manifestation(manifestation) = &effect.effect else {
        panic!("interactive measurement requested an unexpected Host effect")
    };
    let sample = decode_sample(&manifestation.canonical_value);
    assert_eq!(sample.value, Quantity::new(25, QuantityUnit::Millivolt));
    assert_eq!(sample.observed_at.clock_basis, "control-occurrence");
    assert!(scheduler
        .signs()
        .events()
        .any(|event| { event.kind == conduit_kernel::KernelEventKind::HostOperationCompleted }));
}
