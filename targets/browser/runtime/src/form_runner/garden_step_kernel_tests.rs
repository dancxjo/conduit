//! Canonical reusable Garden reducer through the installed browser kernel.

use super::super::protocol::decode_manifestation;
use super::*;
use conduit_core::{
    process_owned_line_offer_with_limits, BaseImplementationId, CapabilityOffer, LinkLimits,
    PortDescriptor, PortDirection, PortTemporal, Scalar,
};
use conduit_form::{
    check_syntax_document, expand_canonical_form, parse_syntax_document, KindDefinition,
    KindSignature,
};
use conduit_planner::{PlacementChoice, PlacementChoices, PlanningOptions};
use std::collections::BTreeMap;

const SOURCE_KIND: &str = "fixture/garden-observations";

fn source_offer(interactive: bool) -> CapabilityOffer {
    let reducer = conduit_semantic_catalog::garden_minimal_step_definition();
    let output = |name: &str, input: &PortDescriptor| PortDescriptor {
        port_id: conduit_core::port_id(name),
        value_kind: input.value_kind.clone(),
        direction: PortDirection::Output,
        temporal: PortTemporal::Value,
    };
    let mut offer = crate::installed_browser::advertisement(
        "fixture-template".into(),
        "fixture-template-boot".into(),
    )
    .capabilities
    .into_iter()
    .find(|offer| offer.kind_id.as_str() == conduit_semantic_catalog::GARDEN_MINIMAL_STEP_KIND)
    .unwrap();
    offer.kind_id = SOURCE_KIND.into();
    offer.kind_contract_revision = "fixture/garden-observations@1".into();
    offer.capability_id = SOURCE_KIND.into();
    offer.inputs.clear();
    offer.outputs = vec![
        output("prior", &reducer.inputs[0]),
        output("clock", &reducer.inputs[1]),
    ];
    if interactive {
        let enriched = crate::installed_browser::catalogs()
            .unwrap()
            .1
            .get(&conduit_core::kind_id(
                conduit_semantic_catalog::GARDEN_ENRICHED_STEP_KIND,
            ))
            .unwrap()
            .clone();
        offer.outputs.push(output("contact", &enriched.inputs[2]));
    }
    offer.host_operations.clear();
    offer.implementation.implementation_id = SOURCE_KIND.into();
    offer.implementation.execution_profile_id = SOURCE_KIND.into();
    offer.implementation.artifact_id = SOURCE_KIND.into();
    offer
}

fn fragment(interactive: bool) -> PlanFragment {
    let (mut startup, mut catalog) = crate::installed_browser::catalogs().unwrap();
    let browser =
        crate::installed_browser::advertisement("garden-browser".into(), "garden-boot".into());

    let source = source_offer(interactive);
    startup
        .insert(KindSignature {
            kind: SOURCE_KIND.into(),
            startup_parameters: Vec::new(),
        })
        .unwrap();
    catalog
        .insert(KindDefinition {
            kind_id: source.kind_id.clone(),
            kind_contract_revision: source.kind_contract_revision.clone(),
            inputs: Vec::new(),
            outputs: source.outputs.clone(),
            configuration: Vec::new(),
        })
        .unwrap();
    let mut source_host = browser.clone();
    source_host.host_id = "fixture/garden-source".into();
    source_host.boot_id = "fixture/garden-source-boot".into();
    source_host.capabilities = vec![source];

    let contact_cord = if interactive {
        " source.contact > evolve.contact\n"
    } else {
        ""
    };
    let evolve_form = if interactive {
        "garden-state-step-contact"
    } else {
        "garden-state-step"
    };
    let source = format!(
        "{}\nform garden-kernel-proof {{\n source: {}\n evolve: {}\n result: {}\n source.prior > evolve.prior\n source.clock > evolve.clock\n{} evolve.next > result.state\n}}\n",
        include_str!("../../../../../forms/signal-garden/main.conduit"),
        SOURCE_KIND,
        evolve_form,
        conduit_semantic_catalog::GARDEN_STATE_PRESENTATION_KIND,
        contact_cord,
    );
    let checked = check_syntax_document(&parse_syntax_document(&source), &startup).unwrap();
    let expanded = expand_canonical_form(&checked, "garden-kernel-proof", &catalog).unwrap();
    let hosts = [source_host.clone(), browser.clone()];
    let placements = PlacementChoices {
        by_gear: expanded
            .gears
            .iter()
            .map(|gear| {
                let host = if gear.kind_id.as_str() == SOURCE_KIND {
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
    let crossings = expanded
        .connections
        .iter()
        .filter(|cord| cord.source_gear_id.as_str().ends_with("/source"))
        .collect::<Vec<_>>();
    assert_eq!(crossings.len(), if interactive { 3 } else { 2 });
    let lines = crossings
        .iter()
        .enumerate()
        .map(|(index, _)| {
            process_owned_line_offer_with_limits(
                &format!("fixture/garden-line-{index}"),
                &format!("fixture/garden-binding-{index}"),
                BaseImplementationId::from("conduit.base/websocket-rfc6455@1"),
                &format!("fixture/garden-base-{index}"),
                &source_host,
                &browser,
                LinkLimits {
                    maximum_in_flight_items: 1,
                    maximum_payload_bytes: maximum,
                    maximum_buffered_bytes: maximum * 2,
                    maximum_frame_bytes: maximum * 2,
                },
            )
        })
        .collect::<Vec<_>>();
    let candidates = crossings
        .iter()
        .zip(&lines)
        .map(|(cord, line)| {
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
            line_offers: &lines,
        },
    )
    .unwrap()
    .fragments
    .into_iter()
    .find(|fragment| fragment.host_id == browser.host_id)
    .unwrap()
}

#[test]
fn canonical_minimal_reducer_executes_through_the_production_kernel() {
    let fragment = fragment(false);
    let lowered = conduit_plan_lowering::lowering::lower_plan_fragment(&fragment).unwrap();
    assert_eq!(lowered.remote_endpoints.len(), 2);
    let mut scheduler = prepare_scheduler(&fragment, &lowered).unwrap();
    let state_kind = conduit_semantic_catalog::garden_state_type()
        .profile()
        .unwrap()
        .value_kind()
        .clone();
    let state_endpoint = lowered
        .remote_endpoints
        .iter()
        .find(|endpoint| endpoint.value_kind == state_kind)
        .unwrap();
    let clock_endpoint = lowered
        .remote_endpoints
        .iter()
        .find(|endpoint| endpoint.value_kind != state_kind)
        .unwrap();
    let prior =
        conduit_semantic_catalog::garden_state_value(conduit_semantic_catalog::GardenState {
            vitality: Scalar::from_raw_microunits(400_000),
            activity: Scalar::ZERO,
            step: 0,
        })
        .unwrap()
        .canonical_bytes()
        .unwrap();
    let clock = conduit_semantic_catalog::garden_clock_observation_value(
        conduit_semantic_catalog::GardenClockObservation {
            phase: Scalar::from_raw_microunits(800_000),
        },
    )
    .unwrap()
    .canonical_bytes()
    .unwrap();
    for (endpoint, value) in [(state_endpoint, prior), (clock_endpoint, clock)] {
        scheduler
            .admit_remote_input(endpoint.endpoint, endpoint.cord, 0, &value)
            .unwrap();
        scheduler
            .close_remote_input(endpoint.endpoint, endpoint.cord)
            .unwrap();
    }
    let DriveStatus::Effect(pending) = drive(&mut scheduler, &fragment).unwrap() else {
        panic!("expected Garden state manifestation")
    };
    let BrowserHostEffect::Manifestation(output) = &pending.effect else {
        panic!("expected Garden state manifestation")
    };
    let next = conduit_semantic_catalog::decode_garden_state(&output.canonical_value).unwrap();
    assert_eq!(next.vitality.raw_microunits(), 500_000);
    assert_eq!(next.activity.raw_microunits(), 400_000);
    assert_eq!(next.step, 1);
    assert_eq!(
        decode_manifestation(output).unwrap().2.as_deref(),
        Some("garden step 1 · vitality 0.500000 · activity 0.400000")
    );
    complete_host_effect(&mut scheduler, &pending).unwrap();
    assert!(matches!(
        drive(&mut scheduler, &fragment).unwrap(),
        DriveStatus::Complete
    ));
}

#[test]
fn canonical_interactive_reducer_composes_contact_through_one_production_kernel() {
    let fragment = fragment(true);
    assert!(fragment.placements.iter().any(|placement| {
        placement.kind_id.as_str() == conduit_semantic_catalog::GARDEN_OBSERVATION_COMBINE_KIND
    }));
    assert!(fragment.placements.iter().any(|placement| {
        placement.kind_id.as_str() == conduit_semantic_catalog::GARDEN_ENRICHED_REDUCER_KIND
    }));
    assert!(fragment
        .placements
        .iter()
        .all(|placement| placement.host_operations.len() <= 2));

    let lowered = conduit_plan_lowering::lowering::lower_plan_fragment(&fragment).unwrap();
    assert_eq!(lowered.remote_endpoints.len(), 3);
    let mut scheduler = prepare_scheduler(&fragment, &lowered).unwrap();
    let prior =
        conduit_semantic_catalog::garden_state_value(conduit_semantic_catalog::GardenState {
            vitality: Scalar::from_raw_microunits(400_000),
            activity: Scalar::ZERO,
            step: 0,
        })
        .unwrap()
        .canonical_bytes()
        .unwrap();
    let clock = conduit_semantic_catalog::garden_clock_observation_value(
        conduit_semantic_catalog::GardenClockObservation {
            phase: Scalar::from_raw_microunits(800_000),
        },
    )
    .unwrap()
    .canonical_bytes()
    .unwrap();
    let contact = conduit_semantic_catalog::garden_contact_observation_value(
        conduit_semantic_catalog::GardenContactObservation {
            intensity: Scalar::from_raw_microunits(800_000),
        },
    )
    .unwrap()
    .canonical_bytes()
    .unwrap();
    let state_kind = conduit_semantic_catalog::garden_state_type()
        .profile()
        .unwrap()
        .value_kind()
        .clone();
    let clock_kind = conduit_semantic_catalog::garden_clock_observation_type()
        .profile()
        .unwrap()
        .value_kind()
        .clone();
    for endpoint in &lowered.remote_endpoints {
        let value = if endpoint.value_kind == state_kind {
            &prior
        } else if endpoint.value_kind == clock_kind {
            &clock
        } else {
            &contact
        };
        scheduler
            .admit_remote_input(endpoint.endpoint, endpoint.cord, 0, value)
            .unwrap();
        scheduler
            .close_remote_input(endpoint.endpoint, endpoint.cord)
            .unwrap();
    }
    let DriveStatus::Effect(pending) = drive(&mut scheduler, &fragment).unwrap() else {
        panic!("expected enriched Garden state manifestation")
    };
    let BrowserHostEffect::Manifestation(output) = &pending.effect else {
        panic!("expected enriched Garden state manifestation")
    };
    let next = conduit_semantic_catalog::decode_garden_state(&output.canonical_value).unwrap();
    assert_eq!(next.vitality.raw_microunits(), 600_000);
    assert_eq!(next.activity.raw_microunits(), 400_000);
    assert_eq!(next.step, 1);
    assert_eq!(
        decode_manifestation(output).unwrap().2.as_deref(),
        Some("garden step 1 · vitality 0.600000 · activity 0.400000")
    );
    complete_host_effect(&mut scheduler, &pending).unwrap();
    assert!(matches!(
        drive(&mut scheduler, &fragment).unwrap(),
        DriveStatus::Complete
    ));
}
