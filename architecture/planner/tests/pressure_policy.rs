use conduit_core::{
    kind_id, port_id, BaseImplementationId, CapabilityId, CapabilityLimits, CapabilityOffer,
    ExecutionProfileId, ImplementationId, ImplementationOffer, KindIdentity, PortDescriptor,
    PortDirection, PortTemporal,
};
use conduit_form::{
    KindConfigurationField, KindConfigurationRule, KindProjection, KindSignature, ProfileCatalog,
    StartupCatalog, StartupParameterSignature,
};
use conduit_planner::{default_placements, plan_with_connection_limits};

mod common;

fn tick_current_sink_offer() -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters: vec![],
        shorthand: None,
        capability_id: CapabilityId::from("fixture-tick-current-sink"),
        kind_id: kind_id("fixture/tick-current-sink"),
        kind_contract_revision: KindIdentity::from("fixture/tick-current-sink@1"),
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from("fixture/tick-current-sink@1"),
            implementation_id: ImplementationId::from("fixture/tick-current-sink@1"),
            artifact_id: conduit_core::ArtifactId::from("fixture/planning-only@1"),
        },
        inputs: vec![PortDescriptor {
            port_id: port_id("in"),
            value_kind: conduit_std_offers::tick_capability_offer().outputs[0]
                .value_kind
                .clone(),
            direction: PortDirection::Input,
            temporal: PortTemporal::Current,
        }],
        outputs: vec![],
        host_calls: vec![],
        resource_requirements: vec![],
        authority_requirements: vec![],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: conduit_std_offers::tick_capability_offer()
                .limits
                .max_queue_bytes,
        },
    }
}

#[test]
fn coalesce_latest_seals_distinct_downstream_pressure_policy_in_the_plan() {
    let tick = conduit_std_offers::tick_capability_offer();
    let tick_kind = tick.outputs[0].value_kind.clone();
    let tick_bytes = tick.limits.max_queue_bytes;
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    let tick_contract = conduit_semantic_catalog::tick_contract();
    startup
        .insert(KindSignature {
            kind: tick_contract.kind_id.as_str().into(),
            startup_parameters: vec![
                StartupParameterSignature {
                    name: "count".into(),
                    value_type: "Count".into(),
                    default: Some("4".into()),
                },
                StartupParameterSignature {
                    name: "period-ms".into(),
                    value_type: "Count".into(),
                    default: Some("1000".into()),
                },
            ],
        })
        .unwrap();
    profile
        .insert(KindProjection {
            kind_id: tick_contract.kind_id,
            kind_contract_revision: tick.kind_contract_revision.clone(),
            inputs: vec![],
            outputs: tick_contract.outputs,
            configuration: tick_contract
                .configuration
                .into_iter()
                .map(|field| KindConfigurationField {
                    key: field.key,
                    default_value: field.default_value,
                    rule: match field.rule {
                        conduit_semantic_catalog::KindConfigurationRule::U64Range {
                            minimum,
                            maximum,
                        } => KindConfigurationRule::U64Range { minimum, maximum },
                        rule => panic!("unexpected tick configuration rule: {rule:?}"),
                    },
                })
                .collect(),
        })
        .unwrap();
    conduit_semantic_catalog::install_flow_pressure_kind(
        conduit_semantic_catalog::flow_coalesce_latest_contract(&tick_kind, tick_bytes),
        &mut startup,
        &mut profile,
    )
    .unwrap();
    let sink = tick_current_sink_offer();
    startup
        .insert(KindSignature {
            kind: sink.kind_id.as_str().into(),
            startup_parameters: vec![],
        })
        .unwrap();
    profile
        .insert(KindProjection {
            kind_id: sink.kind_id.clone(),
            kind_contract_revision: sink.kind_contract_revision.clone(),
            inputs: sink.inputs.clone(),
            outputs: vec![],
            configuration: vec![],
        })
        .unwrap();

    let form = conduit_form::parse_with_startup(
        "form coalesced_ticks {\n    clock: time/tick(count = 2, period-ms = 1)\n    latest: flow/coalesce-latest\n    sink: fixture/tick-current-sink\n    clock.tick > latest.in\n    latest.out > sink.in\n}\n",
        &startup,
        &profile,
    )
    .unwrap();

    let mut host = common::standard_planning_fixture("pressure-host", "pressure-boot");
    host.capabilities = vec![
        tick,
        conduit_std_offers::flow_coalesce_latest_std_offer(&tick_kind, tick_bytes),
        sink,
    ];
    let hosts = [host];
    let placements = default_placements(&form, &hosts).unwrap();
    let plan = plan_with_connection_limits(
        &form,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        1,
        tick_bytes,
    )
    .unwrap();

    let latest = plan.fragments[0]
        .placements
        .iter()
        .find(|placement| {
            placement.kind_id == kind_id(conduit_semantic_catalog::FLOW_COALESCE_LATEST_KIND)
        })
        .unwrap();
    let upstream = plan.fragments[0]
        .connections
        .iter()
        .find(|connection| connection.sink_placement_id == latest.placement_id)
        .unwrap();
    let downstream = plan.fragments[0]
        .connections
        .iter()
        .find(|connection| connection.source_placement_id == latest.placement_id)
        .unwrap();

    assert_eq!(
        upstream.pressure_policy,
        conduit_core::DeliveryPressurePolicy::PreserveOrder
    );
    assert_eq!(
        downstream.pressure_policy,
        conduit_core::DeliveryPressurePolicy::CoalesceLatest
    );
}
