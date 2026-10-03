use super::*;
mod fixture;

#[test]
fn kernel_request_replay_cannot_repeat_a_completed_native_effect() {
    let mut owner = fixture::owner(7);
    let input = fixture::input();
    let node = owner.node;
    assert_eq!(
        owner.begin(node, HostCallId(0), RequestId(1), &input).err(),
        Some(ControlOwnerRefusal::StaleTransfer)
    );
    let submission = owner
        .begin(node, HostCallId(0), RequestId(0), &input)
        .unwrap();
    assert_eq!(
        owner.begin(node, HostCallId(0), RequestId(0), &input).err(),
        Some(ControlOwnerRefusal::Pressure)
    );
    assert!(
        unsafe {
            owner.finish_quiesced(
                &submission,
                NativeControlObservation::Completed {
                    actual: 8,
                    input: &[0; 8],
                },
            )
        }
        .is_ok()
    );
    assert_eq!(
        owner.begin(node, HostCallId(0), RequestId(0), &input).err(),
        Some(ControlOwnerRefusal::StaleTransfer)
    );
    assert_eq!(
        owner
            .table
            .inspections()
            .next()
            .unwrap()
            .completed_operations,
        1
    );
    assert!(
        owner
            .begin(node, HostCallId(0), RequestId(1), &input)
            .is_ok()
    );
}

#[test]
fn matching_ids_cannot_substitute_call_tables_or_semantic_value_contracts() {
    let (fragment, mut lowered, active, placement) = fixture::selected();
    lowered.host_calls[0].binding.maximum_output_bytes += 1;
    assert_eq!(
        fixture::bind(
            SelectedOperationPlan {
                fragment: &fragment,
                lowered: &lowered,
                active: &active,
                placement_id: &placement
            },
            7
        )
        .err(),
        Some(ControlOwnerRefusal::WrongBinding)
    );
    let mut fragment = fragment;
    let conduit_core::KindSemanticLaw::ValueContracts(contracts) =
        &mut fragment.placements[0].semantic_contract.laws[0]
    else {
        panic!("value contracts")
    };
    contracts[0].contract.maximum_bytes -= 1;
    let identity = conduit_core::PlotIdentity {
        source_document_id: fragment.source_document_id.clone(),
        checked_plot_id: fragment.checked_plot_id.clone(),
        expanded_plot_id: fragment.expanded_plot_id.clone(),
    };
    let fragment = conduit_core::seal_plan(identity, alloc::vec![fragment])
        .fragments
        .remove(0);
    let active =
        conduit_core::bind_active_play(&fragment.plan_id, &fragment.host_id, &fragment.boot_id, 0);
    let lowered = conduit_plan_lowering::lowering::lower_plan_fragment(&fragment).unwrap();
    assert_eq!(
        fixture::bind(
            SelectedOperationPlan {
                fragment: &fragment,
                lowered: &lowered,
                active: &active,
                placement_id: &placement
            },
            7
        )
        .err(),
        Some(ControlOwnerRefusal::WrongBinding)
    );
}

#[test]
fn foreign_handle_and_every_changed_operation_scope_refuse_before_submission() {
    let input = fixture::input();
    let mut owner = fixture::owner(7);
    let foreign = fixture::owner(8);
    owner.handle = foreign.handle;
    assert_eq!(
        owner
            .begin(
                owner.node,
                HostCallId(0),
                RequestId(owner.next_request),
                &input
            )
            .err(),
        Some(ControlOwnerRefusal::Capability(
            BaseCapabilityRefusal::UnknownCapability
        ))
    );
    assert!(owner.pending.is_none());
    type Change = fn(&mut BaseOperationClaim);
    let changes: [Change; 13] = [
        |claim| claim.host_id = "host/foreign".into(),
        |claim| claim.boot_id = "boot/foreign".into(),
        |claim| claim.base_instance_id = "base/foreign".into(),
        |claim| claim.base_provider_generation += 1,
        |claim| claim.plan_id = "plan/foreign".into(),
        |claim| claim.active_play_id = "play/foreign".into(),
        |claim| claim.implementation_id = "implementation/foreign".into(),
        |claim| claim.operation_contract_id = "call/foreign".into(),
        |claim| claim.subject_kind = "kind/foreign".into(),
        |claim| claim.resource_pool_id = "resource/foreign".into(),
        |claim| {
            claim.resource_generation_id =
                conduit_core::ResourceGenerationId("generation/foreign".into())
        },
        |claim| claim.envelope_id = "envelope/foreign".into(),
        |claim| {
            claim.plan_id = "plan/replacement".into();
            claim.active_play_id = "play/replacement".into();
        },
    ];
    for change in changes {
        let mut owner = fixture::owner(7);
        change(&mut owner.claim);
        assert_eq!(
            owner
                .begin(
                    owner.node,
                    HostCallId(0),
                    RequestId(owner.next_request),
                    &input
                )
                .err(),
            Some(ControlOwnerRefusal::Capability(
                BaseCapabilityRefusal::WrongScope
            ))
        );
        assert!(owner.pending.is_none());
        assert_eq!(
            owner
                .table
                .inspections()
                .next()
                .unwrap()
                .in_flight_operations,
            0
        );
    }
}

#[test]
fn stale_physical_attachment_cannot_release_a_genuine_pending_transfer() {
    let mut owner = fixture::owner(7);
    let input = fixture::input();
    let mut submission = owner
        .begin(
            owner.node,
            HostCallId(0),
            RequestId(owner.next_request),
            &input,
        )
        .unwrap();
    submission.attachment.generation += 1;
    assert_eq!(
        unsafe {
            owner.finish_quiesced(
                &submission,
                NativeControlObservation::Disposition(ControlTransferDisposition::ProviderLost),
            )
        },
        Err(ControlOwnerRefusal::StaleTransfer)
    );
    assert!(owner.pending.is_some());
    submission.attachment.generation -= 1;
    submission.attachment.slot += 1;
    assert_eq!(
        unsafe {
            owner.finish_quiesced(
                &submission,
                NativeControlObservation::Disposition(ControlTransferDisposition::ProviderLost),
            )
        },
        Err(ControlOwnerRefusal::StaleTransfer)
    );
    assert!(owner.pending.is_some());
    submission.attachment.slot -= 1;
    assert!(
        unsafe {
            owner.finish_quiesced(
                &submission,
                NativeControlObservation::Disposition(ControlTransferDisposition::ProviderLost),
            )
        }
        .is_ok()
    );
}

#[test]
fn selected_control_owner_requires_exact_binding_and_current_possession() {
    let mut owner = fixture::owner(7);
    let node = owner.node;
    let input = fixture::input();
    assert_eq!(
        owner
            .begin(
                NodeId(node.0 + 1),
                HostCallId(0),
                RequestId(owner.next_request),
                &input
            )
            .err(),
        Some(ControlOwnerRefusal::WrongBinding)
    );
    assert_eq!(
        owner
            .begin(node, HostCallId(1), RequestId(owner.next_request), &input)
            .err(),
        Some(ControlOwnerRefusal::WrongBinding)
    );
    assert!(
        owner
            .begin(
                node,
                HostCallId(0),
                RequestId(owner.next_request),
                &input[..input.len() - 1]
            )
            .is_err()
    );
    assert_eq!(
        owner
            .table
            .inspections()
            .next()
            .unwrap()
            .in_flight_operations,
        0
    );
    owner.claim.boot_id = conduit_core::BootId::from("boot/foreign");
    assert_eq!(
        owner
            .begin(node, HostCallId(0), RequestId(owner.next_request), &input)
            .err(),
        Some(ControlOwnerRefusal::Capability(
            BaseCapabilityRefusal::WrongScope
        ))
    );
    assert!(owner.pending.is_none());
}

#[test]
fn pending_transfer_exerts_pressure_until_acknowledged_quiescence() {
    let mut owner = fixture::owner(7);
    let input = fixture::input();
    let node = owner.node;
    let submission = owner
        .begin(node, HostCallId(0), RequestId(owner.next_request), &input)
        .unwrap();
    assert_eq!(submission.request().unwrap().length(), 8);
    assert_eq!(
        owner
            .begin(node, HostCallId(0), RequestId(owner.next_request), &input)
            .err(),
        Some(ControlOwnerRefusal::Pressure)
    );
    assert_eq!(
        owner
            .table
            .inspections()
            .next()
            .unwrap()
            .in_flight_operations,
        1
    );
    let result = unsafe {
        owner.finish_quiesced(
            &submission,
            NativeControlObservation::Completed {
                actual: 3,
                input: &[1, 2, 3],
            },
        )
    }
    .unwrap();
    assert!(conduit_core::validate_canonical_structured_value(result).is_ok());
    assert_eq!(
        owner
            .table
            .inspections()
            .next()
            .unwrap()
            .completed_operations,
        1
    );
    assert!(owner.pending.is_none());
    assert_eq!(
        unsafe {
            owner.finish_quiesced(
                &submission,
                NativeControlObservation::Completed {
                    actual: 3,
                    input: &[1, 2, 3],
                },
            )
        },
        Err(ControlOwnerRefusal::StaleTransfer)
    );
}

#[test]
fn revocation_keeps_physical_pending_and_rejects_late_software_completion() {
    let mut owner = fixture::owner(7);
    let input = fixture::input();
    let node = owner.node;
    let submission = owner
        .begin(node, HostCallId(0), RequestId(owner.next_request), &input)
        .unwrap();
    owner.revoke(node, HostCallId(0)).unwrap();
    assert!(owner.pending.is_some());
    assert_eq!(
        owner.table.inspections().next().unwrap().lifecycle,
        CapabilityLifecycle::Revoked
    );
    assert_eq!(
        owner
            .begin(node, HostCallId(0), RequestId(owner.next_request), &input)
            .err(),
        Some(ControlOwnerRefusal::Capability(
            BaseCapabilityRefusal::Revoked
        ))
    );
    assert_eq!(
        unsafe {
            owner.finish_quiesced(
                &submission,
                NativeControlObservation::Completed {
                    actual: 8,
                    input: &[0; 8],
                },
            )
        },
        Err(ControlOwnerRefusal::Capability(
            BaseCapabilityRefusal::StaleCompletion
        ))
    );
    assert!(owner.pending.is_none());
    assert_eq!(
        owner
            .table
            .inspections()
            .next()
            .unwrap()
            .completed_operations,
        0
    );
}

#[test]
fn identical_descriptive_ids_cannot_substitute_another_issuers_transfer() {
    let mut owner = fixture::owner(7);
    let mut foreign = fixture::owner(8);
    let input = fixture::input();
    let node = owner.node;
    let genuine = owner
        .begin(node, HostCallId(0), RequestId(owner.next_request), &input)
        .unwrap();
    let forged = foreign
        .begin(
            foreign.node,
            HostCallId(0),
            RequestId(foreign.next_request),
            &input,
        )
        .unwrap();
    assert_eq!(
        unsafe {
            owner.finish_quiesced(
                &forged,
                NativeControlObservation::Completed {
                    actual: 8,
                    input: &[0; 8],
                },
            )
        },
        Err(ControlOwnerRefusal::StaleTransfer)
    );
    assert!(owner.pending.is_some());
    assert_eq!(
        owner
            .table
            .inspections()
            .next()
            .unwrap()
            .in_flight_operations,
        1
    );
    assert!(
        unsafe {
            owner.finish_quiesced(
                &genuine,
                NativeControlObservation::Completed {
                    actual: 8,
                    input: &[0; 8],
                },
            )
        }
        .is_ok()
    );
}

#[test]
fn reusable_possession_and_malformed_physical_results_do_not_hide_work_or_retry() {
    let mut owner = fixture::owner(7);
    let input = fixture::input();
    let node = owner.node;
    for _ in 0..10_000 {
        let submission = owner
            .begin(node, HostCallId(0), RequestId(owner.next_request), &input)
            .unwrap();
        assert!(
            unsafe {
                owner.finish_quiesced(
                    &submission,
                    NativeControlObservation::Disposition(ControlTransferDisposition::Stalled),
                )
            }
            .is_ok()
        );
    }
    assert_eq!(
        owner
            .table
            .inspections()
            .next()
            .unwrap()
            .completed_operations,
        10_000
    );
    let submission = owner
        .begin(node, HostCallId(0), RequestId(owner.next_request), &input)
        .unwrap();
    assert_eq!(
        unsafe {
            owner.finish_quiesced(
                &submission,
                NativeControlObservation::Completed {
                    actual: 9,
                    input: &[0; 9],
                },
            )
        },
        Err(ControlOwnerRefusal::Result(
            ControlResultRefusal::ActualLength
        ))
    );
    assert!(owner.pending.is_none());
    assert_eq!(
        owner
            .table
            .inspections()
            .next()
            .unwrap()
            .completed_operations,
        10_001
    );
}
