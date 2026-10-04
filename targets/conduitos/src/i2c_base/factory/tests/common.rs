//! Inert authority and exact sealed-fore wrappers for native kernel fixtures.
use super::*;
use conduit_composite::*;
use conduit_plot::CompositeFrontTerminal;

pub(super) fn boundary(fragment: &PlanFragment) -> KernelCompositeBoundary {
    let mut boundary = KernelCompositeBoundary {
        input_fronts: vec![],
        output_fronts: vec![],
    };
    for fore in &fragment.fore_ports {
        let placement = fragment
            .placements
            .iter()
            .find(|gear| gear.placement_id == fore.placement_id)
            .unwrap();
        let ports = if fore.direction == PortDirection::Input {
            &placement.inputs
        } else {
            &placement.outputs
        };
        let mut port = ports
            .iter()
            .find(|port| port.port_id == fore.gear_port_id)
            .unwrap()
            .clone();
        port.port_id = fore.front_port_id.clone();
        let front = KernelCompositeFrontBinding {
            external_port: port,
            internal_child: fragment.host_id.clone(),
            internal_placement_id: fore.placement_id.clone(),
            internal_port_id: fore.gear_port_id.clone(),
            terminal: CompositeFrontTerminal::Independent,
        };
        if fore.direction == PortDirection::Input {
            boundary.input_fronts.push(front);
        } else {
            boundary.output_fronts.push(front);
        }
    }
    boundary
}

// Fixture authority: never used by native production admission.
pub(super) fn possession(
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
        parameter_bytes: I2C_MAXIMUM_BYTES,
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
