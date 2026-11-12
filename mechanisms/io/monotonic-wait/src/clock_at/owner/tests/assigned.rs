//! Real encoded constrained plans; inert clock ticks, no physical-clock claim.
use super::*;
use crate::clock_at::{assigned::PreparedAssignedClockAt, retained::*};
use alloc::vec::Vec;
use conduit_embedded_build as e;
use conduit_plan_lowering::{
    assigned_play_scope::AssignedPlayScope, selected_assigned_operation::*,
};

fn ready() -> ReadyClockAtBase<Provider> {
    let s = fixture::scope();
    // SAFETY: owned inert provider with finite calls and quiescent revocation.
    unsafe {
        ReadyClockAtBase::new(
            ClockAtNativeIdentity {
                host_id: s.host_id,
                boot_id: s.boot_id,
                base_id: "base/clock".into(),
                provider_instance_id: s.base_instance_id,
                provider_generation: s.base_provider_generation,
                resource_pool_id: s.resource_pool_id,
                resource_generation_id: s.resource_generation_id,
                envelope_id: s.envelope_id,
                artifact_id: "artifact/clock/fixture".into(),
                implementation_id: s.implementation_id,
                base_implementation_id: "base/clock/fixture@1".into(),
                capability_id: s.capability_id,
                execution_profile_id: "conduitos/cooperative-bounded-step@1".into(),
                authority_contract_id: s.authority_contract_id,
                mechanism_family: "machine/clock-attachment".into(),
                resource_class: "machine/clock-attachment".into(),
            },
            ClockAtAttachment { resource_bytes: 1 },
            Provider::default(),
        )
    }
    .unwrap()
}
fn prepare<'a>(
    contract: &MonotonicClockContract,
    fore: bool,
    slot: &'a mut Option<ReadyClockAtBase<Provider>>,
    table: BaseCapabilityTable,
    handle: BaseCapabilityHandle,
    claim: BaseOperationClaim,
    selected: SelectedAssignedOperationPlan<'_>,
) -> Result<PreparedAssignedClockAt<'a, Provider>, ClockCallRefusal> {
    if fore {
        ReadyClockAtBase::prepare_assigned_fore_retained(
            slot,
            table,
            handle,
            claim,
            contract,
            SelectedAssignedForeOperationPlan {
                operation: selected,
                fore: &[],
            },
        )
    } else {
        ReadyClockAtBase::prepare_assigned_retained(slot, table, handle, claim, contract, selected)
    }
}

#[test]
fn contiguous_assignment_transfers_original_clock_after_complete_preflight() {
    exercise(false, false);
}
#[test]
fn borrowed_assignment_transfers_original_clock_after_complete_preflight() {
    exercise(true, false);
}
#[test]
fn local_fore_assignment_checks_exact_trusted_pins() {
    exercise(false, true);
}
#[test]
fn borrowed_local_fore_assignment_checks_exact_trusted_pins() {
    exercise(true, true);
}

fn exercise(borrowed: bool, fore: bool) {
    let contract = MonotonicClockContract::prepare().unwrap();
    let (fragment, lowered, active, _) = fixture::selected();
    let bounds = e::EmbeddedImageBounds {
        maximum_nodes: 1,
        maximum_cords: 0,
        maximum_routes: 1,
        maximum_route_targets: 1,
        maximum_host_calls: 1,
        maximum_resources: 1,
        maximum_sign_expectations: 16,
        maximum_configuration_entries: 0,
        maximum_ports_per_node: 8,
        maximum_remote_endpoints: 0,
        maximum_cord_value_slots: 1,
        maximum_cord_value_bytes: 1024,
        maximum_sign_items: 64,
        maximum_sign_bytes: 8192,
    };
    let image = e::generate_embedded_plan(&fragment, &lowered, bounds).unwrap();
    let maxima = AssignedPlanMaxima {
        encoded_bytes: 8192,
        runtime_state_bytes: 8192,
        counts: [32; ASSIGNED_PLAN_COUNT_KINDS],
    };
    let bytes = if fore {
        let plan = seal_plan(
            PlotIdentity {
                source_document_id: fragment.source_document_id.clone(),
                checked_plot_id: fragment.checked_plot_id.clone(),
                expanded_plot_id: fragment.expanded_plot_id.clone(),
            },
            alloc::vec![fragment.clone()],
        );
        e::generate_fore_native_plan(
            &plan,
            &fragment.fragment_id,
            bounds,
            e::EmbeddedComputationBounds {
                maximum_program_bytes: 32768,
                maximum_total_program_bytes: 32768,
            },
            0,
        )
        .unwrap()
        .encode(maxima)
        .unwrap()
    } else {
        e::encode_assigned_native_plan(&image, maxima).unwrap()
    };
    let records: Vec<_> = AssignedPlanRecords::new(&bytes)
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    let calls = [AssignedIdentity::from_text(CLOCK_CALL)];
    let resources: Vec<_> = image
        .resources
        .iter()
        .map(|resource| resource.resource)
        .collect();
    let selected = |active| SelectedAssignedOperationPlan {
        input: if borrowed {
            AssignedOperationInput::Records {
                header: &bytes[..ASSIGNED_PLAN_HEADER_BYTES],
                records: &records,
            }
        } else {
            AssignedOperationInput::Contiguous(&bytes)
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
        fragment_id: fragment.fragment_id.as_str(),
        scope: AssignedPlayScope::Plot(active),
        node: lowered.nodes[0].node,
    };
    let issue = || fixture::authorization(fixture::scope(), &fragment, &active, [7; 32]);
    let mut slot = Some(ready());
    // Trusted retained facts cannot be replaced by matching received IDs.
    for mutation in 0..9 {
        let mut candidate = ready();
        match mutation {
            0 => candidate.identity.host_id = "foreign/host".into(),
            1 => candidate.identity.boot_id = "stale/boot".into(),
            2 => candidate.identity.provider_generation += 1,
            3 => candidate.identity.execution_profile_id = "foreign/profile".into(),
            4 => candidate.identity.capability_id = "foreign/capability".into(),
            5 => candidate.identity.artifact_id = "foreign/artifact".into(),
            6 => candidate.identity.resource_generation_id.0 = "stale/resource".into(),
            7 => candidate.identity.base_implementation_id = "foreign/base".into(),
            _ => candidate.identity.authority_contract_id = "foreign/authority".into(),
        }
        let mut rejected = Some(candidate);
        let (table, handle, claim) = issue();
        assert!(prepare(
            &contract,
            fore,
            &mut rejected,
            table,
            handle,
            claim,
            selected(&active)
        )
        .is_err());
        assert_eq!(rejected.as_ref().unwrap().provider.calls, 0);
    }
    let mut stale_host = active.clone();
    stale_host.host_id = "stale/host".into();
    let mut stale_boot = active.clone();
    stale_boot.boot_id = "stale/boot".into();
    for stale in [&stale_host, &stale_boot] {
        let (table, handle, claim) = issue();
        assert!(prepare(
            &contract,
            fore,
            &mut slot,
            table,
            handle,
            claim,
            selected(stale)
        )
        .is_err());
        assert!(slot.is_some());
    }
    if fore {
        let pins = [AssignedForeRequirement::new(
            0,
            0,
            AssignedForeDirection::Input,
            AssignedIdentity::from_text("counterfeit"),
            [7; 32],
        )];
        let (table, handle, claim) = issue();
        assert!(ReadyClockAtBase::prepare_assigned_fore_retained(
            &mut slot,
            table,
            handle,
            claim,
            &contract,
            SelectedAssignedForeOperationPlan {
                operation: selected(&active),
                fore: &pins
            }
        )
        .is_err());
        assert!(slot.is_some());
    }
    if borrowed {
        for truncated in [true, false] {
            let mut altered = records.clone();
            if truncated {
                altered.pop();
            } else {
                altered.push(records[0]);
            }
            let mut candidate = selected(&active);
            candidate.input = AssignedOperationInput::Records {
                header: &bytes[..ASSIGNED_PLAN_HEADER_BYTES],
                records: &altered,
            };
            let (table, handle, claim) = issue();
            assert!(prepare(&contract, fore, &mut slot, table, handle, claim, candidate).is_err());
            assert!(slot.is_some());
        }
    }
    // Neither a smaller result envelope nor an underfunded poll budget binds.
    for result_bound in [true, false] {
        let mut scope = fixture::scope();
        if result_bound {
            scope.maximum_result_bytes = CLOCK_MAXIMUM_BYTES - 1;
        } else {
            scope.maximum_work_units = MAXIMUM_POLL_STEPS - 1;
        }
        let (table, handle, claim) = fixture::authorization(scope, &fragment, &active, [7; 32]);
        assert!(prepare(
            &contract,
            fore,
            &mut slot,
            table,
            handle,
            claim,
            selected(&active)
        )
        .is_err());
        assert!(slot.is_some());
    }
    let (table, _, claim) = issue();
    let (_, foreign_handle, _) =
        fixture::authorization(fixture::scope(), &fragment, &active, [8; 32]);
    assert!(prepare(
        &contract,
        fore,
        &mut slot,
        table,
        foreign_handle,
        claim,
        selected(&active)
    )
    .is_err());
    assert!(slot.is_some());
    // A ready ticket starts no observation; dropping it preserves original P.
    let (table, handle, claim) = issue();
    drop(
        prepare(
            &contract,
            fore,
            &mut slot,
            table,
            handle,
            claim,
            selected(&active),
        )
        .unwrap(),
    );
    assert_eq!(slot.as_ref().unwrap().provider.calls, 0);
    slot.as_mut().unwrap().provider.next = Some(10);
    let (table, handle, claim) = issue();
    let ticket: PreparedAssignedClockAt<'_, _> = prepare(
        &contract,
        fore,
        &mut slot,
        table,
        handle,
        claim,
        selected(&active),
    )
    .unwrap();
    assert_eq!(ticket.node(), lowered.nodes[0].node);
    let (reported, report_work) = allocation_probe::observe(|| ticket.retained_backing_bytes());
    assert!(reported.unwrap() >= CLOCK_MAXIMUM_BYTES as usize);
    assert_eq!((report_work.allocations, report_work.reallocations), (0, 0));
    let input = request(10);
    let ((mut owner, output), memory) = allocation_probe::observe(|| {
        let mut owner = ticket.bind();
        owner
            .start(NodeId(0), HostCallId(0), RequestId(0), &input)
            .unwrap();
        let bytes = owner
            .poll(NodeId(0), HostCallId(0), RequestId(0))
            .unwrap()
            .unwrap();
        let value = validate_canonical_structured_value(bytes).unwrap();
        assert_eq!(value.variant_tag().unwrap(), "completed");
        let instant = value.variant_payload("completed").unwrap().unwrap();
        assert_eq!(
            instant.primitive_bytes("value/u64").unwrap(),
            10u64.to_le_bytes()
        );
        let output = bytes.len();
        (owner, output)
    });
    assert!(slot.is_none());
    assert!(output > 0 && output <= CLOCK_MAXIMUM_BYTES as usize);
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
    assert_eq!(owner.provider.calls, 1);
    assert_eq!(owner.inspection().in_flight_operations, 0);
    // Cancel an actually pending second observation; late provider data cannot
    // become a new completion, and revocation retires its in-flight lease.
    owner
        .start(NodeId(0), HostCallId(0), RequestId(1), &request(20))
        .unwrap();
    assert!(owner
        .poll(NodeId(0), HostCallId(0), RequestId(1))
        .unwrap()
        .is_none());
    assert_eq!(owner.provider.calls, 2);
    owner.revoke(NodeId(0), HostCallId(0)).unwrap();
    assert_eq!(owner.inspection().in_flight_operations, 0);
    owner.provider.next = Some(20);
    assert_eq!(
        owner.poll(NodeId(0), HostCallId(0), RequestId(1)),
        Err(ClockCallRefusal::Cancelled)
    );
    assert_eq!(owner.provider.calls, 2);
    assert!(owner.provider.revoked);
}
