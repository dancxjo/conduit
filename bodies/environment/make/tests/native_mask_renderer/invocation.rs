use super::*;
use conduit_plan_lowering::selected_operation::*;
#[test]
fn capsule_reuses_verified_selection_only_after_exact_child_retirement() {
    let pins = Rc::new(native::Pins::default());
    let ready = native::ready(pins.clone());
    let planned = planned(&ready);
    let context = f::PreparedMaskShowContext::from_wire(
        serde_json::to_vec(&planned).unwrap().into_boxed_slice(),
        8192,
        65536,
    )
    .unwrap();
    let mut metadata = planned.plan.clone();
    let fragment_id = metadata.fragments[0].fragment_id.clone();
    let fore = metadata.fragments[0].fore_ports.clone();
    let configurations: std::collections::BTreeMap<_, _> = metadata
        .fragments
        .iter_mut()
        .flat_map(|fragment| &mut fragment.placements)
        .map(|gear| {
            (
                gear.placement_id.clone(),
                core::mem::take(&mut gear.configuration),
            )
        })
        .collect();
    let verified = c::verify_plan_metadata_with_configurations(metadata, |gear| {
        Ok::<_, ()>(configurations[&gear.placement_id].clone())
    })
    .unwrap();
    let lowered = conduit_plan_lowering::lowering::lower_verified_plan_metadata_fragment(
        &verified,
        &fragment_id,
    )
    .unwrap();
    let gear = planned.show_placement();
    let first = c::bind_active_play(
        verified.plan_id(),
        &ready.identity().host_id,
        &ready.identity().boot_id,
        1,
    );
    let selected = || SelectedVerifiedForeOperationPlan {
        fore: &fore,
        operation: SelectedVerifiedOperationPlan {
            plan: &verified,
            fragment_id: &fragment_id,
            lowered: &lowered,
            active: &first,
            placement_id: &gear.placement_id,
        },
    };
    for failure in ["fore", "configuration", "lowering", "revoked"] {
        let (mut table, handle, claim) = native::issue(&ready, &first);
        let mut wrong_fore = fore.clone();
        let mut wrong_lowered = lowered.clone();
        if failure == "fore" {
            wrong_fore[0].byte_capacity -= 1;
        }
        if failure == "lowering" {
            wrong_lowered.fore_ports[0].byte_capacity -= 1;
        }
        if failure == "revoked" {
            table.revoke(&handle).unwrap();
        }
        assert!(
            ready
                .prepare_verified_invocation(
                    &table,
                    &claim,
                    if failure == "configuration" {
                        &[]
                    } else {
                        &gear.configuration
                    },
                    SelectedVerifiedForeOperationPlan {
                        fore: &wrong_fore,
                        operation: SelectedVerifiedOperationPlan {
                            plan: &verified,
                            fragment_id: &fragment_id,
                            lowered: &wrong_lowered,
                            active: &first,
                            placement_id: &gear.placement_id
                        }
                    }
                )
                .is_err(),
            "{failure}"
        );
    }
    let (table, _, claim) = native::issue(&ready, &first);
    assert!(ready
        .prepare_verified_invocation(&table, &claim, &[], selected())
        .is_err());
    let (capsule, memory) = allocation_probe::observe(|| {
        ready
            .prepare_verified_invocation(&table, &claim, &gear.configuration, selected())
            .unwrap()
    });
    assert_eq!(capsule.fore(), fore);
    assert_eq!(capsule.configuration(), gear.configuration);
    assert_eq!(
        capsule.numeric_node(),
        lowered
            .nodes
            .iter()
            .find(|node| node.node == capsule.node())
            .unwrap()
    );
    assert_eq!(
        capsule.numeric_call(),
        lowered
            .host_calls
            .iter()
            .find(|call| call.node == capsule.node())
            .unwrap()
    );
    println!(
        "owned renderer invocation capsule preparation peak={} retained={} allocations={}",
        memory.peak_bytes,
        memory.retained_bytes(),
        memory.allocations
    );
    // Independently retain the exact comparison values the former execution
    // capsule cloned. The current capsule keeps fixed digests and no such heap.
    let (legacy_comparison, comparison_memory) = allocation_probe::observe(|| {
        (
            ready.identity().clone(),
            table.inspections().next().unwrap().scope.clone(),
            first.clone(),
        )
    });
    drop(legacy_comparison);
    #[allow(dead_code)]
    struct LegacyExecutionLayout {
        identity: RendererIdentity,
        bounds: Bounds,
        scope: c::BaseCapabilityScope,
        initial: c::ActivePlayIdentity,
        last: Option<c::ActivePlayIdentity>,
        bound: bool,
        node: conduit_kernel::NodeId,
    }
    let inline = core::mem::size_of::<LegacyExecutionLayout>()
        - core::mem::size_of::<PreparedRendererExecution>();
    let (last, last_memory) = allocation_probe::observe(|| first.clone());
    drop(last);
    let initial_saving = comparison_memory.retained_bytes() + inline;
    println!("renderer fingerprint execution removed_initial_heap={} removed_last_heap={} old_inline={} fixed_execution_bytes={} initial_CPU_saving={} bound_CPU_saving={}", comparison_memory.retained_bytes(), last_memory.retained_bytes(), core::mem::size_of::<LegacyExecutionLayout>(), core::mem::size_of::<PreparedRendererExecution>(), initial_saving, initial_saving + last_memory.retained_bytes());
    assert!(
        initial_saving > 1000,
        "measured heap and inline reduction is substantial"
    );
    let (mut capsule, retirement) = allocation_probe::observe(|| capsule.into_execution());
    assert_eq!((retirement.allocations, retirement.reallocations), (0, 0));
    println!(
        "renderer cold retirement freed_heap={} rich_size={} execution_size={}",
        retirement.deallocated_bytes,
        core::mem::size_of::<PreparedRendererInvocation>(),
        core::mem::size_of::<PreparedRendererExecution>()
    );
    // The old capsule additionally retained this immutable Kind, whose limits
    // changed scalars only. Measure its actual owned allocations separately.
    let (old_contract, contract_memory) = allocation_probe::observe(f::renderer_kind);
    let freed_heap = retirement.deallocated_bytes + contract_memory.retained_bytes();
    println!(
        "renderer compact execution freed_heap_total={} old_contract_heap={}",
        freed_heap,
        contract_memory.retained_bytes()
    );
    assert!(
        freed_heap >= 1512,
        "measured heap saving must cover observed admission deficit"
    );
    drop(old_contract);
    drop(verified);
    drop(lowered);
    drop(configurations);
    drop(planned);
    let mut owner = Some(ready);
    let face = fixture::face();
    let face_wire = serde_json::to_vec(&face).unwrap();
    for sequence in 1..=2 {
        let active = c::bind_active_play(&first.plan_id, &first.host_id, &first.boot_id, sequence);
        for failure in [
            "play",
            "host",
            "boot",
            "plan",
            "active-id",
            "claim",
            "revoked",
            "inflight",
            "spent",
        ] {
            let (mut table, handle, mut claim) = native::issue(owner.as_ref().unwrap(), &active);
            let mut wrong = active.clone();
            if failure == "play" {
                wrong.play_sequence += 1;
            }
            match failure {
                "host" => {
                    wrong = c::bind_active_play(
                        &active.plan_id,
                        &"host/foreign".into(),
                        &active.boot_id,
                        active.play_sequence,
                    )
                }
                "boot" => {
                    wrong = c::bind_active_play(
                        &active.plan_id,
                        &active.host_id,
                        &"boot/foreign".into(),
                        active.play_sequence,
                    )
                }
                "plan" => {
                    wrong = c::bind_active_play(
                        &"plan/foreign".into(),
                        &active.host_id,
                        &active.boot_id,
                        active.play_sequence,
                    )
                }
                "active-id" => wrong.active_play_id = "active/foreign".into(),
                _ => (),
            }
            if failure == "claim" {
                claim.resource_generation_id.0.push_str("-foreign");
            }
            if failure == "revoked" {
                table.revoke(&handle).unwrap();
            }
            if matches!(failure, "inflight" | "spent") {
                let lease = table.authorize(&handle, &claim).unwrap();
                if failure == "spent" {
                    let mut handle = handle;
                    table.complete(&mut handle, lease, 0).unwrap();
                    assert!(capsule
                        .bind_retained(&mut owner, table, handle, claim, &wrong)
                        .is_err());
                    assert!(owner.is_some());
                    continue;
                }
            }
            assert!(
                capsule
                    .bind_retained(&mut owner, table, handle, claim, &wrong)
                    .is_err(),
                "{failure}"
            );
            assert!(owner.is_some());
        }
        for failure in ["operations", "result", "grant", "generation"] {
            let ready = owner.as_ref().unwrap();
            let (original, _, mut claim) = native::issue(ready, &active);
            let mut scope = original.inspections().next().unwrap().scope.clone();
            if failure == "operations" {
                scope.maximum_operations = 3;
            }
            if failure == "result" {
                scope.maximum_result_bytes -= 1;
            }
            if failure == "grant" {
                scope.authority_grant_id = "foreign/grant".into();
            }
            if failure == "generation" {
                scope.resource_generation_id.0.push_str("-renewed");
                claim.resource_generation_id = scope.resource_generation_id.clone();
            }
            let mut grant = native::grant(ready);
            grant.grant_id = scope.authority_grant_id.clone();
            let authority = c::BaseCapabilityAuthority {
                grant,
                base_instance_id: scope.base_instance_id.clone(),
                base_provider_generation: scope.base_provider_generation,
                resource_pool_id: scope.resource_pool_id.clone(),
                resource_generation_id: scope.resource_generation_id.clone(),
                operation_contract_id: scope.operation_contract_id.clone(),
                envelope_id: scope.envelope_id.clone(),
                maximum_parameter_bytes: scope.maximum_parameter_bytes,
                maximum_result_bytes: scope.maximum_result_bytes,
                maximum_work_units: scope.maximum_work_units,
                maximum_in_flight: scope.maximum_in_flight,
                maximum_operations: scope.maximum_operations,
            };
            let mut table = c::BaseCapabilityTable::new(
                scope.host_id.clone(),
                scope.boot_id.clone(),
                scope.base_instance_id.clone(),
                scope.base_provider_generation,
                [9; 32],
                1,
            )
            .unwrap();
            let handle = table
                .issue(c::CapabilityIssueRequest { scope, authority })
                .unwrap();
            assert!(
                capsule
                    .bind_retained(&mut owner, table, handle, claim, &active)
                    .is_err(),
                "{failure}"
            );
            assert!(owner.is_some());
        }
        let (table, handle, claim) = native::issue(owner.as_ref().unwrap(), &active);
        let (bound, memory) = allocation_probe::observe(|| {
            capsule
                .bind_retained(&mut owner, table, handle, claim, &active)
                .unwrap()
        });
        println!(
            "renderer child bind peak={} retained={}",
            memory.peak_bytes,
            memory.retained_bytes()
        );
        assert!(memory.peak_bytes < 4096);
        let mut call = Some(bound);
        assert!(capsule.retire_invocation(&mut call).is_err());
        assert!(call.is_some());
        let presentation = call
            .as_mut()
            .unwrap()
            .prepare_presentation(PresentationBasis {
                face: &face,
                face_wire: &face_wire,
                context: &context,
                elapsed_ms: (sequence - 1) * 3000,
                front: "environment".into(),
                target: "fixture/display".into(),
                prepared_sign: "fixture/prepared".into(),
                available_sign: "fixture/ack".into(),
            })
            .unwrap();
        let mut output = vec![0; 8192];
        let node = capsule.node();
        // Request sequence is retained by the physical owner across child calls.
        assert!(call
            .as_mut()
            .unwrap()
            .present_encoded(
                node,
                HostCallId(0),
                RequestId(99),
                &face_wire,
                &presentation,
                &mut output,
                encode,
                || false
            )
            .is_err());
        call.as_mut()
            .unwrap()
            .present_encoded(
                node,
                HostCallId(0),
                RequestId((sequence - 1) as u32),
                &face_wire,
                &presentation,
                &mut output,
                encode,
                || false,
            )
            .unwrap();
        assert!(capsule.retire_invocation(&mut call).is_err());
        call.as_mut().unwrap().withdraw(|| false).unwrap();
        assert_eq!(call.as_ref().unwrap().inspection().completed_operations, 2);
        owner = Some(capsule.retire_invocation(&mut call).unwrap());
        assert!(call.is_none());
        let (table, handle, claim) = native::issue(owner.as_ref().unwrap(), &active);
        assert!(capsule
            .bind_retained(&mut owner, table, handle, claim, &active)
            .is_err());
    }
    let active = c::bind_active_play(&first.plan_id, &first.host_id, &first.boot_id, 3);
    let (table, handle, claim) = native::issue(owner.as_ref().unwrap(), &active);
    let mut call = Some(
        capsule
            .bind_retained(&mut owner, table, handle, claim, &active)
            .unwrap(),
    );
    call.as_mut().unwrap().cancel_before_delivery().unwrap();
    owner = Some(capsule.retire_invocation(&mut call).unwrap());
    let active = c::bind_active_play(&first.plan_id, &first.host_id, &first.boot_id, 4);
    let (table, handle, claim) = native::issue(owner.as_ref().unwrap(), &active);
    let mut call = Some(
        capsule
            .bind_retained(&mut owner, table, handle, claim, &active)
            .unwrap(),
    );
    call.as_mut().unwrap().cancel_before_delivery().unwrap();
    // Bypassing capsule retirement cannot restore capsule readiness.
    owner = Some(RendererCall::retire_retained(&mut call).unwrap());
    let next = c::bind_active_play(&first.plan_id, &first.host_id, &first.boot_id, 5);
    let (table, handle, claim) = native::issue(owner.as_ref().unwrap(), &next);
    assert!(capsule
        .bind_retained(&mut owner, table, handle, claim, &next)
        .is_err());
    assert!(owner.is_some());
    assert!(pins.edges.get() > 400);
}
