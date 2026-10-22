//! Explicit transcript fixture offers; never advertised as physical availability.
use conduit_core::*;
use conduit_plot::*;
pub const PREFIX: &str = "fixture/environment";
pub fn source() -> String {
    format!(
        "{}\n{}\n{}\n{}",
        include_str!("../../../../../plots/device-protocols/pulse-types.conduit"),
        include_str!("../../../../../plots/device-protocols/dht11-capture.conduit"),
        include_str!("../../../../../plots/device-protocols/dht11.conduit"),
        include_str!("../../../../../plots/device-protocols/dht11-read.conduit")
    )
}
fn fixture_kind(name: &str, input: Option<KindId>, maximum_bytes: u32) -> Kind {
    let port = |name, &direction| PortDescriptor {
        port_id: port_id(name),
        value_kind: input.clone().unwrap_or_else(|| kind_id(EMPTY_INFO_ID)),
        direction,
        temporal: PortTemporal::Flow { closes: true },
        abnormal_kind: None,
    };
    Kind {
        kind_id: kind_id(&format!("{PREFIX}/{name}")),
        kind_contract_revision: "fixture/environment@1".into(),
        startup_parameters: vec![],
        shorthand: None,
        inputs: if input.is_some() {
            vec![port("value", &PortDirection::Input)]
        } else {
            vec![]
        },
        outputs: if input.is_none() {
            vec![port("begin", &PortDirection::Output)]
        } else {
            vec![]
        },
        configuration: vec![],
        semantic_laws: vec![],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: maximum_bytes,
        },
    }
}
pub fn prepare(
    ready: &conduit_pulse_capture::installation::ReadyPulseBase<super::pulse::Provider>,
) -> (Plan, conduit_plan_lowering::lowering::LoweredPlanFragment) {
    prepare_for_boot(ready, &super::pulse::identity().boot_id)
}
pub fn prepare_for_boot(
    ready: &conduit_pulse_capture::installation::ReadyPulseBase<super::pulse::Provider>,
    boot: &BootId,
) -> (Plan, conduit_plan_lowering::lowering::LoweredPlanFragment) {
    prepare_profile(ready, boot, None, None)
}
#[allow(
    dead_code,
    reason = "shared by raw-capture and semantic-observation proof profiles"
)]
pub fn prepare_with_observation(
    ready: &conduit_pulse_capture::installation::ReadyPulseBase<super::pulse::Provider>,
    kind: Kind,
) -> (Plan, conduit_plan_lowering::lowering::LoweredPlanFragment) {
    prepare_profile(ready, &super::pulse::identity().boot_id, Some(kind), None)
}
#[allow(
    dead_code,
    reason = "finite cadence proof shares the exact native provider fixture"
)]
pub fn prepare_with_cadence(
    ready: &conduit_pulse_capture::installation::ReadyPulseBase<super::pulse::Provider>,
    observation: Kind,
    tick: Kind,
    count: u64,
    period_ms: u64,
) -> (Plan, conduit_plan_lowering::lowering::LoweredPlanFragment) {
    prepare_profile(
        ready,
        &super::pulse::identity().boot_id,
        Some(observation),
        Some((tick, count, period_ms)),
    )
}
fn prepare_profile(
    ready: &conduit_pulse_capture::installation::ReadyPulseBase<super::pulse::Provider>,
    boot: &BootId,
    observation_kind: Option<Kind>,
    cadence: Option<(Kind, u64, u64)>,
) -> (Plan, conduit_plan_lowering::lowering::LoweredPlanFragment) {
    let semantic = observation_kind.is_some();
    let mut startup = StartupCatalog::new();
    let mut profiles = ProfileCatalog::new();
    let pulse = conduit_pulse_capture::PulseReadContract::prepare().unwrap();
    pulse.install(&mut startup, &mut profiles).unwrap();
    if let Some(kind) = observation_kind {
        startup
            .insert(KindSignature {
                kind: kind.kind_id.as_str().into(),
                startup_parameters: vec![],
            })
            .unwrap();
        startup
            .insert_fore(kind.kind_id.as_str(), kind.checked_front())
            .unwrap();
        profiles.insert_kind(kind).unwrap();
    }
    if let Some((kind, _, _)) = &cadence {
        startup
            .insert(KindSignature {
                kind: kind.kind_id.as_str().into(),
                startup_parameters: kind
                    .startup_parameters
                    .iter()
                    .map(|p| StartupParameterSignature {
                        name: p.name.clone(),
                        value_type: "Count".into(),
                        default: kind
                            .configuration
                            .iter()
                            .find(|c| c.key == p.name)
                            .and_then(|c| {
                                if let ConfigurationValue::U64(n) = c.default_value {
                                    Some(n.to_string())
                                } else {
                                    None
                                }
                            }),
                    })
                    .collect(),
            })
            .unwrap();
        startup
            .insert_fore(kind.kind_id.as_str(), kind.checked_front())
            .unwrap();
        profiles.insert_kind(kind.clone()).unwrap();
    }
    let mut source = source();
    if semantic {
        source.push_str(include_str!(
            "../../../../../plots/device-protocols/dht11-observe.conduit"
        ));
    }
    if cadence.is_some() {
        let kind = conduit_time::tick_trigger_semantic_contract();
        startup
            .insert(KindSignature {
                kind: kind.kind_id.as_str().into(),
                startup_parameters: vec![],
            })
            .unwrap();
        startup
            .insert_fore(kind.kind_id.as_str(), kind.checked_front())
            .unwrap();
        profiles.insert_kind(kind).unwrap();
        source.push_str(include_str!("../../../profiles/sampling.conduit"));
    }
    let types = prepare_source_types(&parse_syntax_document(&source), &startup).unwrap();
    for (name, ty) in [
        ("begin", None),
        (
            "decoded",
            Some(
                types
                    .named_type(if semantic {
                        "Dht11Result"
                    } else {
                        "Dht11DecodedResult"
                    })
                    .unwrap()
                    .profile()
                    .unwrap()
                    .value_kind()
                    .clone(),
            ),
        ),
        (
            "timed-out",
            Some(
                types
                    .named_type("PulseReadCapture")
                    .unwrap()
                    .profile()
                    .unwrap()
                    .value_kind()
                    .clone(),
            ),
        ),
        (
            "provider-failed",
            Some(
                StructuredInfoType::leaf(kind_id(EMPTY_INFO_ID))
                    .unwrap()
                    .profile()
                    .unwrap()
                    .value_kind()
                    .clone(),
            ),
        ),
        (
            "geometry-refused",
            Some(
                StructuredInfoType::leaf(kind_id(EMPTY_INFO_ID))
                    .unwrap()
                    .profile()
                    .unwrap()
                    .value_kind()
                    .clone(),
            ),
        ),
    ] {
        let kind = fixture_kind(name, ty, if semantic { 4096 } else { 1024 });
        startup
            .insert(KindSignature {
                kind: kind.kind_id.as_str().into(),
                startup_parameters: vec![],
            })
            .unwrap();
        startup
            .insert_fore(kind.kind_id.as_str(), kind.checked_front())
            .unwrap();
        profiles.insert_kind(kind).unwrap();
    }
    let read_plot = if semantic {
        "dht11-observe"
    } else {
        "dht11-read"
    };
    let observation = if semantic {
        "read.observed >> reading.value"
    } else {
        "read.decoded >> reading.value"
    };
    let source = if let Some((_, count, period_ms)) = cadence {
        format!("{source}\nplot fixture {{\n read: environment-sampling({count}, {period_ms})\n reading: {PREFIX}/decoded\n timeout: {PREFIX}/timed-out\n lost: {PREFIX}/provider-failed\n bad: {PREFIX}/geometry-refused\n {observation}\n read.timed_out >> timeout.value\n read.provider_failed >> lost.value\n read.geometry_refused >> bad.value\n}}\n")
    } else {
        format!("{source}\nplot fixture {{\n start: {PREFIX}/begin\n read: {read_plot}\n reading: {PREFIX}/decoded\n timeout: {PREFIX}/timed-out\n lost: {PREFIX}/provider-failed\n bad: {PREFIX}/geometry-refused\n start.begin >> read.begin\n {observation}\n read.timed_out >> timeout.value\n read.provider_failed >> lost.value\n read.geometry_refused >> bad.value\n}}\n")
    };
    let syntax = parse_syntax_document(&source);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &startup).unwrap();
    for plot in &checked.plots {
        for cord in &plot.cords {
            for stage in &cord.stages {
                if let CheckedCordStage::StructuredSelector { selector, .. } = stage {
                    profiles
                        .insert(structured_selector_definition(
                            selector,
                            PortTemporal::Flow { closes: true },
                        ))
                        .unwrap();
                }
            }
        }
    }
    let expanded = expand_canonical_plot(&checked, "fixture", &profiles).unwrap();
    let offers = expanded
        .gears
        .iter()
        .enumerate()
        .filter(|(_, gear)| gear.kind_id.as_str() != conduit_pulse_capture::PULSE_READ_KIND)
        .map(|(index, gear)| {
            let kind = Kind {
                kind_id: gear.kind_id.clone(),
                kind_contract_revision: gear.kind_contract_revision.clone(),
                startup_parameters: gear.startup_parameters.clone(),
                shorthand: gear.shorthand.clone(),
                inputs: gear.inputs.clone(),
                outputs: gear.outputs.clone(),
                configuration: gear.semantic_contract.configuration.clone(),
                semantic_laws: gear.semantic_contract.laws.clone(),
                limits: CapabilityLimits {
                    max_active_instances: 16,
                    max_queue_items: 4,
                    max_queue_bytes: 4096,
                },
            };
            let call = if gear.kind_id.as_str() == conduit_pulse_capture::PULSE_READ_KIND {
                Some(conduit_pulse_capture::PULSE_READ_CALL)
            } else if gear.kind_id.as_str() == "time/tick" {
                Some(conduit_core::WAIT_HOST_CALL_CONTRACT)
            } else if gear.configuration.iter().any(|c| c.key == "selector") {
                Some("conduit.host/structured-selector@1")
            } else {
                None
            };
            BackOfferBuilder::new(
                kind,
                Back {
                    capability_id: format!("fixture/cap/{index}").into(),
                    execution_profile_id: "fixture/environment-kernel@1".into(),
                    implementation_id: format!("fixture/back/{index}").into(),
                    artifact_id: "fixture/environment-source@1".into(),
                    host_calls: call
                        .map(|call| {
                            vec![HostCallRequirement {
                                contract_id: call.into(),
                                target_kind: Some(gear.kind_id.clone()),
                                maximum_in_flight: 1,
                                maximum_input_bytes: 1024,
                                maximum_output_bytes: 1024,
                            }]
                        })
                        .unwrap_or_default(),
                    resource_requirements: vec![],
                    authority_requirements: vec![],
                },
            )
            .build()
        })
        .collect();
    let mut host = HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: "fixture/environment".into(),
        boot_id: boot.clone(),
        offer_generation: OfferGeneration(1),
        profile: "fixture/environment-kernel@1".into(),
        bases: vec![],
        resources: vec![],
        capabilities: offers,
        planner_capabilities: vec![],
    };
    ready.append_to_advertisement(&mut host, &pulse).unwrap();
    let hosts = [host];
    let placements = conduit_planner::default_expanded_placements(&expanded, &hosts).unwrap();
    let empty = std::collections::BTreeMap::new();
    let limits = expanded
        .connections
        .iter()
        .filter(|connection| {
            semantic
                && expanded.gears.iter().any(|gear| {
                    gear.gear_id == connection.source_gear_id
                        && gear.kind_id.as_str() == "device/dht11-observation"
                })
        })
        .map(|connection| {
            (
                (
                    connection.source_gear_id.clone(),
                    connection.source_port_id.clone(),
                    connection.sink_gear_id.clone(),
                    connection.sink_port_id.clone(),
                ),
                conduit_planner::ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity: 4096,
                },
            )
        })
        .collect();
    let assemble = |grants: &[AuthorityGrant]| {
        conduit_planner::plan_expanded_canonical_with_connection_limits(
            &expanded,
            &hosts,
            &placements,
            &[BaseImplementationId::from("conduit.base/local@1")],
            conduit_planner::PlanningOptions {
                connection_bases: &empty,
                line_candidates: &std::collections::BTreeMap::new(),
                connection_item_capacity: 1,
                connection_byte_capacity: 1024,
                authority_grants: grants,
                protected_resource_grants: &[],
                line_offers: &[],
            },
            &limits,
        )
    };
    assert!(assemble(&[]).is_err(), "availability is not authority");
    let mut grant = super::pulse::grant();
    grant.boot_id = boot.clone();
    let plan = assemble(&[grant]).unwrap();
    let lowered = conduit_plan_lowering::lowering::lower_plan_fragment(&plan.fragments[0]).unwrap();
    (plan, lowered)
}
