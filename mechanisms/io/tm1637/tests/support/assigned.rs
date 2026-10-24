//! Constrained assignment and actual possession are CPU fixtures here.
use super::*;
use conduit_embedded_build as e;
use conduit_plan_lowering::selected_assigned_operation::{
    SelectedAssignedForeOperationPlan, SelectedAssignedOperationPlan,
};
use conduit_tm1637_wire::installation::*;
#[test]
fn assigned_output_preserves_slot_on_refusal_then_transfers_and_writes() {
    assigned_output_control(false, false);
}
#[test]
fn borrowed_assignment_preserves_slot_then_transfers_and_writes() {
    assigned_output_control(true, false);
}
#[test]
fn local_fore_assignment_preserves_slot_then_transfers_and_writes() {
    assigned_output_control(false, true);
}
#[test]
fn borrowed_local_fore_preserves_slot_then_transfers_and_writes() {
    assigned_output_control(true, true);
}
fn assigned_output_control(borrowed: bool, fore: bool) {
    let state = Rc::new(State::default());
    let mut slot = Some(installation::ready(state.clone()));
    let (mut fragment, _, _, _) = fixture::selected();
    let gear = &mut fragment.placements[0];
    gear.capability_id = WRITE_CAPABILITY.into();
    gear.execution_profile_id = WRITE_EXECUTION_PROFILE.into();
    gear.authority[0].contract_id = WRITE_AUTHORITY.into();
    gear.authority[0].capability_id = WRITE_CAPABILITY.into();
    gear.base.as_mut().unwrap().mechanism_family = WRITE_ATTACHMENT_CLASS.into();
    gear.resources[0].class_id = WRITE_ATTACHMENT_CLASS.into();
    if fore {
        let port = &gear.inputs[0];
        fragment.fore_ports.push(PlannedForePort {
            front_port_id: port_id("packets"),
            direction: PortDirection::Input,
            placement_id: gear.placement_id.clone(),
            gear_port_id: port.port_id.clone(),
            value_kind: port.value_kind.clone(),
            value_contract: None,
            abnormal_kind: None,
            track: ConnectionTrack::Payload,
            temporal: port.temporal,
            pressure_policy: DeliveryPressurePolicy::PreserveOrder,
            item_capacity: 1,
            byte_capacity: MAXIMUM_BYTES,
            selected_line: None,
        });
    }
    let plan = seal_plan(
        PlotIdentity {
            source_document_id: fragment.source_document_id.clone(),
            checked_plot_id: fragment.checked_plot_id.clone(),
            expanded_plot_id: fragment.expanded_plot_id.clone(),
        },
        vec![fragment],
    );
    let fragment = &plan.fragments[0];
    let active = bind_active_play(&fragment.plan_id, &fragment.host_id, &fragment.boot_id, 0);
    let (lowered, _) = conduit_plan_lowering::lowering::lower_plan_fragment_from_plan(
        &plan,
        &fragment.fragment_id,
    )
    .unwrap();
    let bounds = e::EmbeddedImageBounds {
        maximum_nodes: 1,
        maximum_cords: 1,
        maximum_routes: 1,
        maximum_route_targets: 1,
        maximum_host_calls: 1,
        maximum_resources: 1,
        maximum_sign_expectations: 16,
        maximum_configuration_entries: 2,
        maximum_ports_per_node: 8,
        maximum_remote_endpoints: 0,
        maximum_cord_value_slots: 1,
        maximum_cord_value_bytes: 1024,
        maximum_sign_items: 64,
        maximum_sign_bytes: 8192,
    };
    let fore_image = fore.then(|| {
        e::generate_fore_native_plan(
            &plan,
            &fragment.fragment_id,
            bounds,
            e::EmbeddedComputationBounds {
                maximum_program_bytes: 32768,
                maximum_total_program_bytes: 32768,
            },
            1,
        )
        .unwrap()
    });
    let plain_image =
        (!fore).then(|| e::generate_embedded_plan(fragment, &lowered, bounds).unwrap());
    let image = fore_image
        .as_ref()
        .map(|image| image.kernel())
        .or(plain_image.as_ref())
        .unwrap();
    let maxima = AssignedPlanMaxima {
        encoded_bytes: 8192,
        runtime_state_bytes: 8192,
        counts: [32; ASSIGNED_PLAN_COUNT_KINDS],
    };
    let bytes = if let Some(image) = &fore_image {
        image.encode(maxima).unwrap()
    } else {
        e::encode_assigned_native_plan(image, maxima).unwrap()
    };
    let calls = [AssignedIdentity::from_text(WRITE_CALL)];
    let resources: Vec<_> = image.resources.iter().map(|r| r.resource).collect();
    let records: Vec<_> = AssignedPlanRecords::new(&bytes)
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    let selected = |active| SelectedAssignedOperationPlan {
        input: if borrowed {
            conduit_plan_lowering::selected_assigned_operation::AssignedOperationInput::Records {
                header: &bytes[..ASSIGNED_PLAN_HEADER_BYTES],
                records: &records,
            }
        } else {
            conduit_plan_lowering::selected_assigned_operation::AssignedOperationInput::Contiguous(
                &bytes,
            )
        },
        maxima,
        requirements: AssignedPlanRequirements {
            host: AssignedIdentity::from_text(fragment.host_id.as_str()),
            boot: AssignedIdentity::from_text(fragment.boot_id.as_str()),
            host_calls: &calls,
            resources: &resources,
            remote_bindings: &[],
        },
        computations: AssignedComputationProfile {
            maximum_text_bytes: 32768,
            maximum_total_text_bytes: 32768,
            requirements: &[],
        },
        fragment_id: &fragment.fragment_id,
        active,
        node: lowered.nodes[0].node,
    };
    let contract = WriteContract::prepare().unwrap();
    let bind = |slot: &mut Option<ReadyWriteBase<Provider>>, table, handle, claim, active| {
        if let Some(image) = &fore_image {
            ReadyWriteBase::bind_assigned_fore_retained(
                slot,
                table,
                handle,
                claim,
                &contract,
                SelectedAssignedForeOperationPlan {
                    operation: selected(active),
                    fore: image.fore(),
                },
            )
        } else {
            ReadyWriteBase::bind_assigned_retained(
                slot,
                table,
                handle,
                claim,
                &contract,
                selected(active),
            )
        }
    };
    let issue = || {
        let mut scope = fixture::scope();
        scope.capability_id = WRITE_CAPABILITY.into();
        scope.authority_contract_id = WRITE_AUTHORITY.into();
        fixture::authorization(scope, fragment, &active)
    };
    if let Some(image) = &fore_image {
        let (table, _, claim) = issue();
        assert!(slot
            .as_ref()
            .unwrap()
            .check_assigned(&table, &claim, &contract, selected(&active))
            .is_err());
        let pin = image.fore()[0];
        let wrong = [AssignedForeRequirement::new(
            pin.endpoint(),
            pin.cord(),
            pin.direction(),
            AssignedIdentity(pin.payload()[5..21].try_into().unwrap()),
            assigned_plan_payload_digest(b"foreign ABI"),
        )];
        assert!(slot
            .as_ref()
            .unwrap()
            .check_assigned_fore(
                &table,
                &claim,
                &contract,
                SelectedAssignedForeOperationPlan {
                    operation: selected(&active),
                    fore: &wrong
                }
            )
            .is_err());
        assert!(slot.is_some());
        assert_eq!(state.edges.get(), 0);
    }
    let mut foreign = active.clone();
    foreign.boot_id = "stale/boot".into();
    let (table, handle, claim) = issue();
    assert!(bind(&mut slot, table, handle, claim, &foreign).is_err());
    assert!(slot.is_some());
    assert_eq!(state.edges.get(), 0);
    let (table, handle, mut claim) = issue();
    claim.resource_generation_id = ResourceGenerationId("foreign/resource-generation".into());
    assert!(bind(&mut slot, table, handle, claim, &active).is_err());
    assert!(slot.is_some());
    assert_eq!(state.edges.get(), 0);
    let (table, handle, claim) = issue();
    let mut owner = bind(&mut slot, table, handle, claim, &active).unwrap();
    assert!(slot.is_none());
    assert_eq!(state.edges.get(), 0);
    let (table, handle, claim) = issue();
    assert!(bind(&mut slot, table, handle, claim, &active).is_err());
    assert!(slot.is_none());
    assert_eq!(state.edges.get(), 0);
    drop(records);
    drop(bytes);
    let canonical = encoded();
    let allocations = allocation_probe::observe(|| {
        owner.invoke(
            lowered.nodes[0].node,
            HostCallId(0),
            RequestId(0),
            &canonical,
            || false,
        )
    });
    assert_eq!(allocations.0, Ok(()));
    assert_eq!(
        (allocations.1.allocations, allocations.1.reallocations),
        (0, 0)
    );
    assert!(state.edges.get() > 200);
    assert_eq!(state.released.get(), [true; 2]);
    owner.revoke(lowered.nodes[0].node, HostCallId(0)).unwrap();
    let (provider, attachment) = WriteHostCall::retire_retained(&mut Some(owner)).unwrap();
    assert!(Rc::ptr_eq(&provider.0, &state));
    assert_eq!(attachment.generation, 7);
}
