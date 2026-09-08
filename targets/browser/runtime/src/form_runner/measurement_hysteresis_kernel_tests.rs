//! Explicit hysteresis initialization through a planned browser production kernel.

use super::*;
use conduit_core::{
    process_owned_line_offer_with_limits, BaseImplementationId, LinkLimits, PortDirection,
    Quantity, QuantityUnit, StructuredInfoValue, TemporalInstant, TemporalScale,
};
use conduit_data::{
    MeasurementHysteresisProfile, MeasurementSummary, MeasurementThresholdPolicy,
    MeasurementThresholdState, MeasurementThresholdTransition,
};
use conduit_form::{
    check_syntax_document, expand_canonical_form, parse_syntax_document, KindDefinition,
    KindSignature,
};
use conduit_planner::{PlacementChoice, PlacementChoices, PlanningOptions};
use std::collections::BTreeMap;

fn fragment() -> PlanFragment {
    let (mut startup, mut catalog) = crate::installed_browser::catalogs().unwrap();
    let mut browser = crate::installed_browser::advertisement(
        "hysteresis-browser".into(),
        "hysteresis-boot".into(),
    );
    let sink = crate::installed_browser::test_measurement_decision_sink::offer();
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
    browser.capabilities.push(sink);

    let hysteresis = browser
        .capabilities
        .iter()
        .find(|offer| offer.kind_id.as_str() == conduit_data::MEASUREMENT_HYSTERESIS_KIND)
        .unwrap();
    let mut source_offer = hysteresis.clone();
    source_offer.kind_id = "fixture/measurement-hysteresis-inputs".into();
    source_offer.kind_contract_revision = "fixture/measurement-hysteresis-inputs@1".into();
    source_offer.capability_id = "fixture/measurement-hysteresis-inputs".into();
    source_offer.outputs = source_offer.inputs.clone();
    for output in &mut source_offer.outputs {
        output.direction = PortDirection::Output;
    }
    source_offer.inputs.clear();
    source_offer.host_operations.clear();
    source_offer.implementation.implementation_id =
        "fixture/measurement-hysteresis-inputs@1".into();
    source_offer.implementation.artifact_id = "fixture/measurement-hysteresis-inputs@1".into();
    startup
        .insert(KindSignature {
            kind: source_offer.kind_id.as_str().into(),
            startup_parameters: Vec::new(),
        })
        .unwrap();
    catalog
        .insert(KindDefinition {
            kind_id: source_offer.kind_id.clone(),
            kind_contract_revision: source_offer.kind_contract_revision.clone(),
            inputs: Vec::new(),
            outputs: source_offer.outputs.clone(),
            configuration: Vec::new(),
        })
        .unwrap();
    let mut source_host = browser.clone();
    source_host.host_id = "fixture/hysteresis-source".into();
    source_host.boot_id = "fixture/hysteresis-source-boot".into();
    source_host.capabilities = vec![source_offer];

    let syntax = parse_syntax_document("form decide {\n source: fixture/measurement-hysteresis-inputs\n decide: data/measurement-hysteresis\n result: conduit-test/measurement-decision-sink\n source.profile > decide.profile\n source.summary > decide.summary\n decide.decision > result.decision\n}\n");
    let checked = check_syntax_document(&syntax, &startup).unwrap();
    let expanded = expand_canonical_form(&checked, "decide", &catalog).unwrap();
    let hosts = [source_host.clone(), browser.clone()];
    let placements = PlacementChoices {
        by_gear: expanded
            .gears
            .iter()
            .map(|gear| {
                let host = if gear.kind_id.as_str() == "fixture/measurement-hysteresis-inputs" {
                    &source_host
                } else {
                    &browser
                };
                (
                    gear.gear_id.clone(),
                    PlacementChoice {
                        host_id: host.host_id.clone(),
                        capability_id: host
                            .capabilities
                            .iter()
                            .find(|offer| offer.kind_id == gear.kind_id)
                            .unwrap()
                            .capability_id
                            .clone(),
                    },
                )
            })
            .collect(),
    };
    let maximum = MAXIMUM_BROWSER_VALUE_BYTES as u32;
    let line = process_owned_line_offer_with_limits(
        "fixture/hysteresis-line",
        "fixture/hysteresis-binding",
        BaseImplementationId::from("conduit.base/websocket-rfc6455@1"),
        "fixture/hysteresis-base",
        &source_host,
        &browser,
        LinkLimits {
            maximum_in_flight_items: 2,
            maximum_payload_bytes: maximum,
            maximum_buffered_bytes: maximum * 2,
            maximum_frame_bytes: maximum * 2,
        },
    );
    let candidates = expanded
        .connections
        .iter()
        .filter(|cord| cord.source_gear_id.as_str() == "decide/source")
        .map(|cord| {
            (
                (cord.source_gear_id.clone(), cord.sink_gear_id.clone()),
                vec![line.line_id.clone()],
            )
        })
        .collect::<BTreeMap<_, _>>();
    conduit_planner::plan_expanded_canonical_with_options(
        &expanded,
        &hosts,
        &placements,
        &[
            BaseImplementationId::from("conduit.base/local@1"),
            BaseImplementationId::from("conduit.base/websocket-rfc6455@1"),
        ],
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &candidates,
            connection_item_capacity: 1,
            connection_byte_capacity: maximum,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[line],
        },
    )
    .unwrap()
    .fragments
    .into_iter()
    .find(|fragment| fragment.host_id == browser.host_id)
    .unwrap()
}

fn leaf(value_type: conduit_core::StructuredInfoType, payload: Vec<u8>) -> Vec<u8> {
    StructuredInfoValue::leaf(value_type, payload)
        .unwrap()
        .canonical_bytes()
        .unwrap()
}

fn drain_to_remote_idle(scheduler: &mut TourScheduler, fragment: &PlanFragment) {
    match drive(scheduler, fragment) {
        Err(error) => assert_eq!(error, "Tour Play became idle"),
        Ok(_) => panic!("expected the kernel to await the summary"),
    }
}

#[test]
fn planned_browser_hysteresis_uses_exact_profile_and_initial_state() {
    let fragment = fragment();
    let lowered = conduit_plan_lowering::lowering::lower_plan_fragment(&fragment).unwrap();
    assert_eq!(lowered.remote_endpoints.len(), 2);
    let mut scheduler = prepare_scheduler(&fragment, &lowered).unwrap();
    let profile_type = conduit_data::measurement_hysteresis_profile_type();
    let profile_kind = profile_type.profile().unwrap().value_kind().clone();
    let profile_endpoint = lowered
        .remote_endpoints
        .iter()
        .find(|endpoint| endpoint.value_kind == profile_kind)
        .unwrap();
    let summary_type = conduit_data::measurement_summary_type();
    let summary_kind = summary_type.profile().unwrap().value_kind().clone();
    let summary_endpoint = lowered
        .remote_endpoints
        .iter()
        .find(|endpoint| endpoint.value_kind == summary_kind)
        .unwrap();

    let profile = leaf(
        profile_type,
        conduit_data::encode_measurement_hysteresis_profile(MeasurementHysteresisProfile {
            policy: MeasurementThresholdPolicy {
                lower: Quantity::new(40, QuantityUnit::Millivolt),
                upper: Quantity::new(60, QuantityUnit::Millivolt),
            },
            initial_state: MeasurementThresholdState::Above,
        })
        .unwrap(),
    );
    scheduler
        .admit_remote_input(
            profile_endpoint.endpoint,
            profile_endpoint.cord,
            0,
            &profile,
        )
        .unwrap();
    scheduler
        .close_remote_input(profile_endpoint.endpoint, profile_endpoint.cord)
        .unwrap();
    drain_to_remote_idle(&mut scheduler, &fragment);

    let instant = TemporalInstant {
        ticks: 7,
        scale: TemporalScale::Milliseconds,
        clock_basis: "fixture-clock".into(),
        resolution_ticks: 1,
        uncertainty_ticks: 0,
    };
    let summary = MeasurementSummary {
        unit: QuantityUnit::Millivolt,
        sample_count: 2,
        first_observed_at: instant.clone(),
        last_observed_at: instant,
        minimum: Quantity::new(40, QuantityUnit::Millivolt),
        maximum: Quantity::new(40, QuantityUnit::Millivolt),
        range: Quantity::new(0, QuantityUnit::Millivolt),
        mean: Quantity::new(40, QuantityUnit::Millivolt),
    };
    let summary = leaf(
        summary_type,
        conduit_data::encode_measurement_summary(&summary).unwrap(),
    );
    scheduler
        .admit_remote_input(
            summary_endpoint.endpoint,
            summary_endpoint.cord,
            0,
            &summary,
        )
        .unwrap();
    scheduler
        .close_remote_input(summary_endpoint.endpoint, summary_endpoint.cord)
        .unwrap();
    let DriveStatus::Effect(pending) = drive(&mut scheduler, &fragment).unwrap() else {
        panic!("expected decision manifestation")
    };
    let BrowserHostEffect::Manifestation(output) = &pending.effect else {
        panic!("expected decision manifestation")
    };
    let value = StructuredInfoValue::from_canonical_bytes(&output.canonical_value).unwrap();
    let conduit_core::StructuredInfoValueShape::Leaf(payload) = value.shape() else {
        panic!("expected decision leaf")
    };
    let decision = conduit_data::decode_measurement_threshold_decision(payload).unwrap();
    assert_eq!(decision.state, MeasurementThresholdState::Below);
    assert_eq!(
        decision.transition,
        Some(MeasurementThresholdTransition::FellBelow)
    );
    complete_host_effect(&mut scheduler, &pending).unwrap();
    assert!(matches!(
        drive(&mut scheduler, &fragment).unwrap(),
        DriveStatus::Quiescent
    ));
}
