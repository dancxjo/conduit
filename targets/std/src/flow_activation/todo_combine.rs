//! Exact std child Back for the authored Todo scan.

use conduit_composite::{KernelOperationBudget, KernelOperationFactory};
use conduit_core::{
    ArtifactId, Back, BackOfferBuilder, CapabilityId, CapabilityOffer, ExecutionProfileId,
    ImplementationId, PlannedGear, PlannedScanActivation, PortDirection, PortTemporal,
};
use conduit_kernel::{scheduler::StepBack, HostedValueStore};
use conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
use conduit_todo_plot::{
    todo_combine_kind, TodoCombineBack, COMMAND_MAX_BYTES, STATE_MAX_BYTES, TODO_COMBINE_KIND,
    TODO_COMBINE_REVISION,
};

pub const EXECUTION_PROFILE: &str = "conduit.std/todo-combine-kernel@1";
pub const IMPLEMENTATION: &str = "std/kernel-todo-combine@1";
pub const ARTIFACT: &str = "conduit-std-host/todo-combine@1";

pub struct TodoCombineFactory;

pub fn offer() -> CapabilityOffer {
    BackOfferBuilder::new(
        todo_combine_kind(),
        Back {
            capability_id: CapabilityId::from("std/todo-combine@1"),
            execution_profile_id: ExecutionProfileId::from(EXECUTION_PROFILE),
            implementation_id: ImplementationId::from(IMPLEMENTATION),
            artifact_id: ArtifactId::from(ARTIFACT),
            host_calls: vec![],
            resource_requirements: vec![],
            authority_requirements: vec![],
        },
    )
    .build()
}

fn validate(placement: &PlannedGear) -> Result<(), String> {
    let expected = offer();
    if placement.kind_id.as_str() != TODO_COMBINE_KIND
        || placement.kind_contract_revision.as_str() != TODO_COMBINE_REVISION
        || placement.capability_id != expected.capability_id
        || placement.execution_profile_id != expected.implementation.execution_profile_id
        || placement.implementation_id != expected.implementation.implementation_id
        || placement.artifact_id != expected.implementation.artifact_id
        || placement.inputs != expected.inputs
        || placement.outputs != expected.outputs
        || placement.semantic_contract != expected.semantic_contract
        || placement.limits != expected.limits
        || !placement.configuration.is_empty()
        || placement.base.is_some()
        || !placement.realization_characteristics.is_empty()
        || !placement.realization_properties.is_empty()
        || !placement.terminal_transductions.is_empty()
        || !placement.host_calls.is_empty()
        || !placement.resources.is_empty()
        || !placement.authority.is_empty()
        || !placement.pool_references.is_empty()
    {
        return Err("planned Todo combine Back differs from the exact std offer".into());
    }
    Ok(())
}

/// The exact one-node Todo child has one productive Step and one close Step.
/// Both inputs are admitted and closed before its first scheduler Step, its
/// single output cord has one reserved slot, and there are no internal links
/// or Host Calls that could introduce another wait or retry.
pub(crate) fn maximum_scan_child_steps(planned: &PlannedScanActivation) -> Result<u32, String> {
    let child = planned.selected_plan.as_ref();
    if !conduit_core::verify_plan(child) || child.fragments.len() != 1 {
        return Err("Todo scan child Step bound requires one sealed local Plan fragment".into());
    }
    let fragment = &child.fragments[0];
    if fragment.placements.len() != 1
        || !fragment.connections.is_empty()
        || !fragment.states.is_empty()
        || !fragment.execution_regions.is_empty()
        || !fragment.execution_fusions.is_empty()
        || !fragment.shared_pools.is_empty()
        || fragment.expected_terminals
            != [
                conduit_core::ExpectedTerminal::PlacementCompleted(
                    fragment.placements[0].placement_id.clone(),
                ),
                conduit_core::ExpectedTerminal::PlanCompleted,
            ]
        || fragment.terminal_policy
            != conduit_core::TerminalPolicy::RequireAllPlacementsAndConnections
        || fragment.cancellation_policy
            != conduit_core::CancellationPolicy::CancelAllAndRejectLateCompletion
        || fragment.fore_ports.len() != 3
    {
        return Err("Todo scan child Step bound requires one pure unlinked combine Gear".into());
    }
    validate(&fragment.placements[0])?;
    for (expected, direction, maximum_bytes) in [
        (
            &planned.accumulator_input,
            PortDirection::Input,
            STATE_MAX_BYTES,
        ),
        (&planned.item_input, PortDirection::Input, COMMAND_MAX_BYTES),
        (&planned.output, PortDirection::Output, STATE_MAX_BYTES),
    ] {
        if !fragment.fore_ports.iter().any(|front| {
            front.front_port_id == expected.front_port_id
                && front.direction == direction
                && front.placement_id == fragment.placements[0].placement_id
                && front.gear_port_id == expected.front_port_id
                && front.value_kind == expected.value_kind
                && front.abnormal_kind.is_none()
                && front.track == conduit_core::ConnectionTrack::Payload
                && front.temporal == PortTemporal::Value
                && front.item_capacity == 1
                && front.byte_capacity >= maximum_bytes as u32
                && front.selected_line.is_none()
        }) {
            return Err("Todo scan child Step bound requires exact local Value Fore".into());
        }
    }
    u32::from(planned.limits.maximum_items)
        .checked_mul(2)
        .ok_or_else(|| "Todo scan child Step bound overflow".into())
}

impl KernelOperationFactory for TodoCombineFactory {
    fn implementation_id(&self) -> &ImplementationId {
        static ID: std::sync::OnceLock<ImplementationId> = std::sync::OnceLock::new();
        ID.get_or_init(|| ImplementationId::from(IMPLEMENTATION))
    }

    fn budget(&self, placement: &PlannedGear) -> Result<KernelOperationBudget, String> {
        validate(placement)?;
        Ok(KernelOperationBudget {
            value_items: 3,
            value_bytes: (2 * STATE_MAX_BYTES + COMMAND_MAX_BYTES) as u32,
            maximum_value_bytes: STATE_MAX_BYTES as u32,
            host_requests: 0,
            sign_items: 3,
        })
    }

    fn prepare(
        &self,
        placement: &PlannedGear,
        _: &mut HostedValueStore,
    ) -> Result<Box<dyn StepBack<{ FIXED_KERNEL_STORAGE_PORTS_PER_NODE }> + Send>, String> {
        validate(placement)?;
        Ok(Box::new(TodoCombineBack::new()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flow_activation::{
        authored_todo_plan, install_pure_todo_scan, standard_child_registry, StdActivationHost,
    };
    use conduit_composite::{BoundedScanAdmission, BoundedScanState};
    use conduit_core::{
        prepare_plan_on_hosts, ActivePlayId, PlannedActivationEntry, PreparationHostIdentity,
        ValuePayload,
    };
    use conduit_todo_plot::TodoCommand;

    #[test]
    fn exact_todo_child_takes_two_steps_per_item_even_with_output_held() {
        let plan = authored_todo_plan();
        let PlannedActivationEntry::Scan(planned) = &plan.activations[0] else {
            panic!("authored Todo must select scan")
        };
        assert_eq!(maximum_scan_child_steps(planned).unwrap(), 128);
        let fragment = &plan.fragments[0];
        let mut host = StdActivationHost::new(
            PreparationHostIdentity {
                host_id: fragment.host_id.clone(),
                boot_id: fragment.boot_id.clone(),
                offer_generation: fragment.offer_generation,
            },
            standard_child_registry().unwrap(),
        );
        let mut prepared = prepare_plan_on_hosts(&plan, &mut [&mut host]).unwrap();
        let mut scan =
            install_pure_todo_scan(&plan, &mut prepared, &planned.activation_id, &mut host)
                .unwrap();
        scan.bind_parent_play(&ActivePlayId::from("body/play/step-bound"))
            .unwrap();
        let mut output = ValuePayload {
            value_kind: planned.output.value_kind.clone(),
            encoded: Vec::with_capacity(STATE_MAX_BYTES),
        };
        let mut total_child_steps = 0;
        for index in 0..64 {
            let command = if index == 0 {
                TodoCommand::Add {
                    text: "Milk".into(),
                }
            } else {
                TodoCommand::SetComplete {
                    id: "task-1".into(),
                    complete: index % 2 == 1,
                }
            };
            assert_eq!(
                scan.admit(&ValuePayload {
                    value_kind: planned.item_input.value_kind.clone(),
                    encoded: command.encode_info().unwrap(),
                })
                .unwrap(),
                BoundedScanAdmission::Accepted
            );
            let mut invocation_steps = 0;
            while scan.has_active_child() {
                invocation_steps += 1;
                assert!(invocation_steps <= 2, "exact child exceeded admitted steps");
                scan.step().unwrap();
            }
            assert_eq!(invocation_steps, 2);
            total_child_steps += invocation_steps;
            // Holding the parent output for longer than the entire child
            // budget cannot execute another child or consume its allowance.
            for _ in 0..129 {
                assert!(!scan.has_active_child());
                assert_eq!(scan.step().unwrap(), &BoundedScanState::OutputReady);
            }
            assert!(scan.has_pending_output());
            assert!(scan.output_into(&mut output).unwrap());
            scan.complete_output().unwrap();
        }
        assert_eq!(total_child_steps, 128);
        scan.close_input().unwrap();
        assert!(!scan.has_active_child());
        assert_eq!(scan.step().unwrap(), &BoundedScanState::Complete);
    }

    #[test]
    fn child_step_bound_refuses_a_second_gear_even_when_its_plan_is_sealed() {
        let plan = authored_todo_plan();
        let PlannedActivationEntry::Scan(planned) = &plan.activations[0] else {
            panic!("authored Todo must select scan")
        };
        let child = planned.selected_plan.as_ref();
        let mut fragment = child.fragments[0].clone();
        let mut second = fragment.placements[0].clone();
        second.placement_id = conduit_core::PlacementId::from("second-combine");
        second.gear_id = conduit_core::GearId::from("second-combine");
        fragment.startup_order.push(second.placement_id.clone());
        fragment.placements.push(second);
        let expanded = conduit_core::seal_plan_with_activation_entries(
            conduit_core::PlotIdentity {
                source_document_id: child.source_document_id.clone(),
                checked_plot_id: child.checked_plot_id.clone(),
                expanded_plot_id: child.expanded_plot_id.clone(),
            },
            child.completion_policy,
            child.realization_backs.clone(),
            vec![],
            vec![fragment],
        );
        assert!(conduit_core::verify_plan(&expanded));
        let mut altered = planned.clone();
        altered.selected_plan_id = expanded.plan_id.clone();
        *altered.selected_plan = expanded;
        assert!(maximum_scan_child_steps(&altered).is_err());
    }
}
