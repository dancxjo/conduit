//! Sealed-fore test wrapper: production kernel execution, without a boot claim.
use alloc::vec;
use conduit_composite::*;
use conduit_core::*;
use conduit_plot::CompositeFrontTerminal;

pub(crate) fn definition(plan: Plan, mut external: CapabilityOffer) -> KernelCompositeDefinition {
    let fragment = &plan.fragments[0];
    let mut boundary = KernelCompositeBoundary {
        input_fronts: vec![],
        output_fronts: vec![],
    };
    let mut contracts = vec![];
    for fore in &fragment.fore_ports {
        let gear = fragment
            .placements
            .iter()
            .find(|gear| gear.placement_id == fore.placement_id)
            .unwrap();
        let mut port = if fore.direction == PortDirection::Input {
            &gear.inputs
        } else {
            &gear.outputs
        }
        .iter()
        .find(|port| port.port_id == fore.gear_port_id)
        .unwrap()
        .clone();
        port.port_id = fore.front_port_id.clone();
        if let Some(contract) = &fore.value_contract {
            contracts.push(FrontValueContract {
                location: if fore.direction == PortDirection::Input {
                    FrontValueLocation::Input(fore.front_port_id.clone())
                } else {
                    FrontValueLocation::Output(fore.front_port_id.clone())
                },
                contract: contract.clone(),
            });
        }
        let binding = KernelCompositeFrontBinding {
            external_port: port,
            internal_child: fragment.host_id.clone(),
            internal_placement_id: gear.placement_id.clone(),
            internal_port_id: fore.gear_port_id.clone(),
            terminal: CompositeFrontTerminal::Independent,
        };
        if fore.direction == PortDirection::Input {
            boundary.input_fronts.push(binding);
        } else {
            boundary.output_fronts.push(binding);
        }
    }
    // A sealed-fore fixture wrapper proves kernel execution, not boot admission.
    external.kind_id = kind_id("fixture/protocol-kernel");
    external.kind_contract_revision = KindIdentity::from("fixture/protocol-kernel@1");
    external.capability_id = CapabilityId::from("fixture/protocol-kernel@1");
    external.implementation = ImplementationOffer {
        execution_profile_id: ExecutionProfileId::from("fixture/protocol-kernel@1"),
        implementation_id: ImplementationId::from("fixture/protocol-kernel@1"),
        artifact_id: ArtifactId::from("fixture/protocol-kernel@1"),
    };
    external.semantic_contract = KindSemanticContract {
        configuration: vec![],
        laws: vec![KindSemanticLaw::ValueContracts(contracts)],
    };
    external.inputs = boundary
        .input_fronts
        .iter()
        .map(|front| front.external_port.clone())
        .collect();
    external.outputs = boundary
        .output_fronts
        .iter()
        .map(|front| front.external_port.clone())
        .collect();
    KernelCompositeDefinition {
        host_id: fragment.host_id.clone(),
        boot_id: fragment.boot_id.clone(),
        offer_generation: fragment.offer_generation,
        profile: "fixture/protocol-kernel".into(),
        external_capability: external,
        internal_plan: plan,
        boundary,
        failure_translation: FailureReason::CompositeCapabilityFailed,
    }
}

/// Fixtures select exact one-item queues from their actual native Back offers.
pub(crate) fn queue_limits(
    expanded: &conduit_plot::ExpandedAuthoringPlot,
    hosts: &[HostAdvertisement],
    placements: &conduit_planner::PlacementChoices,
    maximum: u32,
) -> crate::protocol_source::ProtocolQueueLimits {
    use alloc::collections::BTreeMap;
    use conduit_planner::{ConnectionQueueLimits, ForeBoundaryKey};
    let bound = |gear: &GearId| {
        let choice = &placements.by_gear[gear];
        hosts
            .iter()
            .find(|host| host.host_id == choice.host_id)
            .unwrap()
            .capabilities
            .iter()
            .find(|offer| offer.capability_id == choice.capability_id)
            .unwrap()
            .limits
            .max_queue_bytes
            .min(maximum)
    };
    let connections = expanded
        .expanded
        .connections
        .iter()
        .map(|cord| {
            (
                (
                    cord.source_gear_id.clone(),
                    cord.source_port_id.clone(),
                    cord.sink_gear_id.clone(),
                    cord.sink_port_id.clone(),
                ),
                ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity: bound(&cord.source_gear_id).min(bound(&cord.sink_gear_id)),
                },
            )
        })
        .collect();
    let mut boundaries = BTreeMap::new();
    for (direction, bindings) in [
        (PortDirection::Input, &expanded.input_bindings),
        (PortDirection::Output, &expanded.output_bindings),
    ] {
        for binding in bindings {
            let key = ForeBoundaryKey {
                direction,
                front_port_id: binding.front_port_id.clone(),
                track: binding.track,
            };
            let bytes = bound(&binding.gear_id);
            boundaries
                .entry(key)
                .and_modify(|limit: &mut ConnectionQueueLimits| {
                    limit.byte_capacity = limit.byte_capacity.min(bytes)
                })
                .or_insert(ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity: bytes,
                });
        }
    }
    crate::protocol_source::ProtocolQueueLimits {
        connections,
        boundaries,
    }
}

// Fixture authority: never used by native production admission.
pub(crate) fn possession(
    scope: BaseCapabilityScope,
) -> (
    BaseCapabilityTable,
    BaseCapabilityHandle,
    BaseOperationClaim,
) {
    let authority = BaseCapabilityAuthority {
        grant: AuthorityGrant {
            grant_id: scope.authority_grant_id.clone(),
            contract_id: scope.authority_contract_id.clone(),
            host_call_contract_id: scope.operation_contract_id.clone(),
            subject_kind: scope.subject_kind.clone(),
            host_id: scope.host_id.clone(),
            boot_id: scope.boot_id.clone(),
            capability_id: scope.capability_id.clone(),
        },
        base_instance_id: scope.base_instance_id.clone(),
        base_provider_generation: scope.base_provider_generation,
        resource_pool_id: scope.resource_pool_id.clone(),
        resource_generation_id: scope.resource_generation_id.clone(),
        operation_contract_id: scope.operation_contract_id.clone(),
        envelope_id: scope.envelope_id.clone(),
        maximum_parameter_bytes: scope.maximum_parameter_bytes,
        maximum_result_bytes: scope.maximum_result_bytes,
        maximum_work_units: scope.maximum_work_units,
        maximum_in_flight: 1,
        maximum_operations: scope.maximum_operations,
    };
    let claim = BaseOperationClaim {
        host_id: scope.host_id.clone(),
        boot_id: scope.boot_id.clone(),
        base_instance_id: scope.base_instance_id.clone(),
        base_provider_generation: scope.base_provider_generation,
        plan_id: scope.plan_id.clone(),
        active_play_id: scope.active_play_id.clone(),
        implementation_id: scope.implementation_id.clone(),
        operation_contract_id: scope.operation_contract_id.clone(),
        subject_kind: scope.subject_kind.clone(),
        resource_pool_id: scope.resource_pool_id.clone(),
        resource_generation_id: scope.resource_generation_id.clone(),
        envelope_id: scope.envelope_id.clone(),
        parameter_bytes: scope.maximum_parameter_bytes,
        work_units: 1,
    };
    let mut table = BaseCapabilityTable::new(
        scope.host_id.clone(),
        scope.boot_id.clone(),
        scope.base_instance_id.clone(),
        scope.base_provider_generation,
        [7; 32],
        1,
    )
    .unwrap();
    let handle = table
        .issue(CapabilityIssueRequest { scope, authority })
        .unwrap();

    (table, handle, claim)
}
