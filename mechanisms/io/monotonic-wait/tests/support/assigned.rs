//! Constrained assignment and actual possession are CPU fixtures here.
use super::*;
use conduit_embedded_build as e;
use conduit_plan_lowering::selected_assigned_operation::SelectedAssignedOperationPlan;
#[test]
fn assigned_timer_preserves_slot_on_refusal_then_transfers_and_waits() {
    assigned_timer_control(false);
}
#[test]
fn borrowed_assignment_preserves_slot_then_transfers_and_waits() {
    assigned_timer_control(true);
}
fn assigned_timer_control(borrowed: bool) {
    let state = Rc::new(State::default());
    let mut slot = Some(installation::ready(state.clone()));
    let (mut fragment, _, _, _) = fixture::selected();
    let gear = &mut fragment.placements[0];
    gear.capability_id = WAIT_CAPABILITY.into();
    gear.execution_profile_id = WAIT_EXECUTION_PROFILE.into();
    gear.authority[0].contract_id = WAIT_AUTHORITY.into();
    gear.authority[0].capability_id = WAIT_CAPABILITY.into();
    gear.base.as_mut().unwrap().mechanism_family = WAIT_ATTACHMENT_CLASS.into();
    gear.resources[0].class_id = WAIT_ATTACHMENT_CLASS.into();
    let fragment = seal_plan(
        PlotIdentity {
            source_document_id: fragment.source_document_id.clone(),
            checked_plot_id: fragment.checked_plot_id.clone(),
            expanded_plot_id: fragment.expanded_plot_id.clone(),
        },
        vec![fragment],
    )
    .fragments
    .remove(0);
    let active = bind_active_play(&fragment.plan_id, &fragment.host_id, &fragment.boot_id, 0);
    let lowered = conduit_plan_lowering::lowering::lower_plan_fragment(&fragment).unwrap();
    let image = e::generate_embedded_plan(
        &fragment,
        &lowered,
        e::EmbeddedImageBounds {
            maximum_nodes: 1,
            maximum_cords: 0,
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
        },
    )
    .unwrap();
    let maxima = AssignedPlanMaxima {
        encoded_bytes: 8192,
        runtime_state_bytes: 8192,
        counts: [32; ASSIGNED_PLAN_COUNT_KINDS],
    };
    let bytes = e::encode_assigned_native_plan(&image, maxima).unwrap();
    let calls = [AssignedIdentity::from_text(WAIT_HOST_CALL_CONTRACT)];
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
    let issue = || {
        let mut scope = fixture::scope();
        scope.capability_id = WAIT_CAPABILITY.into();
        scope.authority_contract_id = WAIT_AUTHORITY.into();
        fixture::authorization(scope, &fragment, &active)
    };
    let mut foreign = active.clone();
    foreign.boot_id = "stale/boot".into();
    for (config, active) in [(fixture::configuration(), &foreign), (vec![], &active)] {
        let (table, handle, claim) = issue();
        assert!(ReadyMonotonicWaitBase::bind_assigned_retained(
            &mut slot,
            table,
            handle,
            claim,
            &config,
            selected(active)
        )
        .is_err());
        assert!(slot.is_some());
        assert_eq!(state.calls.get(), 0);
    }
    if borrowed {
        // A borrowed view is storage, not evidence: missing and duplicate
        // records must pass through Core validation before custody transfers.
        let mut duplicate = records.clone();
        duplicate.push(records[0]);
        for invalid in [&records[..records.len() - 1], duplicate.as_slice()] {
            let mut candidate = selected(&active);
            candidate.input = conduit_plan_lowering::selected_assigned_operation::AssignedOperationInput::Records {
                header: &bytes[..ASSIGNED_PLAN_HEADER_BYTES],
                records: invalid,
            };
            let (table, handle, claim) = issue();
            assert!(ReadyMonotonicWaitBase::bind_assigned_retained(
                &mut slot,
                table,
                handle,
                claim,
                &fixture::configuration(),
                candidate,
            )
            .is_err());
            assert!(slot.is_some());
            assert_eq!(state.calls.get(), 0);
        }
    }
    let (table, handle, claim) = issue();
    let mut owner = ReadyMonotonicWaitBase::bind_assigned_retained(
        &mut slot,
        table,
        handle,
        claim,
        &fixture::configuration(),
        selected(&active),
    )
    .unwrap();
    assert!(slot.is_none());
    drop(records);
    drop(bytes);
    let allocations = allocation_probe::observe(|| {
        immediate(owner.invoke(
            lowered.nodes[0].node,
            HostCallId(0),
            RequestId(0),
            &encode_monotonic_duration(250),
        ))
    });
    assert_eq!(allocations.0, Ok(()));
    assert_eq!(allocations.1.allocations, 0);
    assert_eq!(allocations.1.reallocations, 0);
    assert_eq!(state.duration.get(), 250);
}
