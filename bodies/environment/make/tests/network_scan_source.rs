//! Checked authored scan meaning; this test does not initialize a radio.
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
#[path = "../../../../architecture/plan-lowering/tests/common/native_possession.rs"]
mod possession;
#[test]
fn scan_source_checks_with_a_connected_typed_evidence_sink() {
    let plot = conduit_environment_make::compile_network_scan().unwrap();
    assert_eq!(plot.name, "environment-network-scan");
    assert_eq!(plot.gears.len(), 2);
    assert_eq!(plot.connections.len(), 1);
    // Invocation generation and deadlines belong to the retained provider,
    // rather than an invented authored input or remote request port.
    assert!(conduit_net::network_scan_contract().inputs.is_empty());
    let sink = conduit_net::network_scan_sign_contract();
    assert_eq!(sink.inputs.len(), 1);
    assert!(sink.outputs.is_empty());
    // The existing installed join Source must still check with this catalog.
    conduit_environment_make::compile_network_setup().unwrap();
}

// This journey uses ordinary checked Source, initialized CPU-provider offers,
// real planner/lowering and the production scheduler. Host completions below
// are explicitly CPU inputs: physical selection/cleanup is tested separately.
struct Inert;
impl conduit_network_join_call::scan::NetworkScanProvider for Inert {
    fn now_ms(&self) -> u64 {
        1
    }
    async fn scan(
        &mut self,
        _: conduit_net::NetworkScanRequest,
        _: &mut [conduit_net::ScanObservation; 16],
    ) -> Result<conduit_net::ScanCompletion, conduit_net::ScanFailure> {
        panic!("kernel preparation cannot scan")
    }
    async fn cleanup(&mut self, _: u64) -> Result<(), conduit_net::ScanFailure> {
        panic!("kernel preparation cannot clean a radio")
    }
}
fn scan_plan() -> conduit_core::Plan {
    scan_fixture().0
}
fn scan_fixture() -> (
    conduit_core::Plan,
    conduit_network_join_call::scan::ReadyNetworkScanBase<Inert>,
) {
    use conduit_core as c;
    use conduit_net::*;
    use conduit_network_join_call::{installation::NetworkNativeIdentity, scan::*};
    use std::collections::BTreeMap;
    let mut host = c::HostAdvertisement {
        protocol_version: c::PROTOCOL_VERSION,
        host_id: "cpu/host".into(),
        boot_id: "cpu/boot".into(),
        offer_generation: c::OfferGeneration(1),
        profile: "cpu/scan".into(),
        bases: vec![],
        resources: vec![],
        capabilities: vec![],
        planner_capabilities: vec![],
    };
    // SAFETY: exclusively owned inert CPU provider; methods panic above.
    let ready = unsafe {
        ReadyNetworkScanBase::new(
            NetworkNativeIdentity {
                host: host.host_id.clone(),
                boot: host.boot_id.clone(),
                base: "cpu/base".into(),
                instance: "cpu/instance".into(),
                provider_generation: 1,
                resource: "cpu/radio".into(),
                resource_generation: c::ResourceGenerationId("cpu/radio/1".into()),
                envelope: "cpu/envelope".into(),
                artifact: "cpu/artifact".into(),
                implementation: "cpu/scan@1".into(),
                base_implementation: "cpu/base@1".into(),
            },
            NetworkScanRequest {
                generation: 1,
                scan_deadline_ms: 10,
                cleanup_deadline_ms: 20,
            },
            Inert,
        )
    }
    .unwrap_or_else(|_| panic!("valid CPU initialization"));
    ready.append_to_advertisement(&mut host).unwrap();
    host.capabilities.push(network_scan_sign_offer(
        "cpu/sign".into(),
        "cpu/sign@1".into(),
        "cpu/sign-artifact".into(),
    ));
    let grants = [c::AuthorityGrant {
        grant_id: "cpu/grant".into(),
        contract_id: NETWORK_SCAN_AUTHORITY.into(),
        host_call_contract_id: NETWORK_SCAN_HOST_CALL.into(),
        subject_kind: NETWORK_SCAN_SUBJECT.into(),
        host_id: host.host_id.clone(),
        boot_id: host.boot_id.clone(),
        capability_id: SCAN_CAPABILITY.into(),
    }];
    let source = conduit_environment_make::compile_network_scan().unwrap();
    let hosts = [host];
    let placements = conduit_planner::default_expanded_placements(&source, &hosts).unwrap();
    let plan = conduit_planner::plan_expanded_canonical_with_options(
        &source,
        &hosts,
        &placements,
        &["conduit.base/local@1".into(), "cpu/base@1".into()],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: MAXIMUM_SCAN_OUTPUT_BYTES,
            authority_grants: &grants,
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .unwrap();
    (plan, ready)
}

#[test]
fn actual_checked_scan_kernel_delivers_one_bounded_result_and_retires_original_signs() {
    let plan = scan_plan();
    assert_eq!(plan.fragments.len(), 1);
    let parts =
        conduit_network_join_call::scan::PreparedScanKernelParts::prepare(&plan.fragments[0])
            .unwrap();
    exercise_kernel(parts);
}

#[test]
fn borrowed_verified_scan_tables_keep_the_same_execution_without_heap_preparation() {
    use conduit_network_join_call::scan::*;
    let plan = scan_plan();
    let verified = conduit_core::FlatPlanRef::from_plan(&plan)
        .unwrap()
        .verify()
        .unwrap();
    let (parts, memory) = allocation_probe::observe(|| {
        let tables = ScanFlatDirectTables::prepare(&verified).unwrap();
        PreparedScanKernelParts::check_flat_direct(&tables).unwrap();
        PreparedScanKernelParts::from_flat_direct(tables).unwrap()
    });
    assert_eq!(memory.allocations, 0);
    assert_eq!(memory.reallocations, 0);
    exercise_kernel(parts);
}

fn exercise_kernel(parts: conduit_network_join_call::scan::PreparedScanKernelParts) {
    use conduit_kernel::{self as k, scheduler::SchedulerStatus};
    use conduit_net::*;
    use conduit_network_join_call::scan::*;
    let mut prepared = PreparedScanKernelStorage::try_new()
        .unwrap()
        .finish(parts)
        .unwrap();
    let original = (&*prepared) as *const _;
    prepared = match prepared.try_retire_signs() {
        Ok(_) => panic!("unplayed kernel cannot retire"),
        Err(retained) => retained,
    };
    assert_eq!((&*prepared) as *const _, original);
    let mut observations = [ScanObservation::EMPTY; 16];
    for (i, r) in observations.iter_mut().enumerate() {
        *r = ScanObservation::new(
            format!("cpu-network-{i}").as_bytes(),
            ObservedNetworkSecurity::Known(KnownSecurity::Wpa2Personal),
            -45,
            1,
        )
        .unwrap();
    }
    let mut wire = [0; MAXIMUM_SCAN_OUTPUT_BYTES as usize];
    let len = ScanCompletion {
        generation: 1,
        completed_ms: 1,
        count: 16,
        total: 17,
    }
    .encode(&observations, &mut wire)
    .unwrap();
    assert_eq!(len, wire.len());
    let ((calls, drained), allocation) = allocation_probe::observe(|| {
        let mut calls = [0u8; 2];
        let mut drained = false;
        for _ in 0..32 {
            let status = prepared.kernel.step().unwrap();
            while let Some(request) = prepared.kernel.next_host_request() {
                let output = if request.node == prepared.roles.scan {
                    calls[0] += 1;
                    assert!(prepared
                        .kernel
                        .host_value(request.input.value)
                        .unwrap()
                        .is_empty());
                    let value = prepared.kernel.store_host_value(&wire).unwrap();
                    Some(k::BoundedValueRef::new(value, MAXIMUM_SCAN_OUTPUT_BYTES).unwrap())
                } else {
                    assert_eq!(request.node, prepared.roles.evidence);
                    calls[1] += 1;
                    assert_eq!(
                        prepared.kernel.host_value(request.input.value).unwrap(),
                        wire
                    );
                    None
                };
                prepared
                    .kernel
                    .complete_host_call(
                        request.node,
                        request.request,
                        k::HostCallOutcome {
                            disposition: k::HostCallDisposition::Completed,
                            output,
                            failure: None,
                        },
                    )
                    .unwrap();
            }
            if status == SchedulerStatus::Drained {
                drained = true;
                break;
            }
        }
        (calls, drained)
    });
    assert_eq!(allocation.allocations, 0);
    assert_eq!(allocation.reallocations, 0);
    assert!(drained);
    assert_eq!(calls, [1, 1]);
    assert_eq!(prepared.kernel.pending_host_call_count(), 0);
    assert!(prepared.kernel.can_retire());
    assert!(prepared.try_retire_signs().is_ok());
    println!("CPU64 scan storage bytes: {}", SCAN_KERNEL_BYTES);
}

#[test]
fn scan_kernel_refuses_unverified_plan_identity_before_storage() {
    use conduit_network_join_call::scan::*;
    let mut plan = scan_plan();
    plan.fragments[0].boot_id = "foreign/boot".into();
    assert!(matches!(
        PreparedScanKernelParts::prepare(&plan.fragments[0]),
        Err(ScanKernelRefusal::Identity)
    ));
}

#[test]
fn scan_storage_refuses_each_allocation_without_retaining_partial_backing() {
    use conduit_network_join_call::scan::*;
    for index in 0..2 {
        let (refused, memory) = allocation_probe::observe(|| {
            let _refusal = allocation_probe::refuse_nth_allocation(index);
            matches!(
                PreparedScanKernelStorage::try_new(),
                Err(ScanKernelRefusal::Storage)
            )
        });
        assert!(refused, "allocation {index} must refuse");
        assert_eq!(memory.retained_bytes(), 0);
    }
}

fn issued_scope(
    ready: &conduit_network_join_call::scan::ReadyNetworkScanBase<Inert>,
    active: &conduit_core::ActivePlayIdentity,
) -> conduit_core::BaseCapabilityScope {
    use conduit_net::*;
    let i = ready.native_identity();
    let mut scope = possession::scope(NETWORK_SCAN_SUBJECT);
    scope.host_id = i.host.clone();
    scope.boot_id = i.boot.clone();
    scope.base_instance_id = i.instance.clone();
    scope.base_provider_generation = i.provider_generation;
    scope.plan_id = active.plan_id.clone();
    scope.active_play_id = active.active_play_id.clone();
    scope.authority_grant_id = "cpu/grant".into();
    scope.authority_contract_id = NETWORK_SCAN_AUTHORITY.into();
    scope.capability_id = conduit_network_join_call::scan::SCAN_CAPABILITY.into();
    scope.implementation_id = i.implementation.clone();
    scope.operation_contract_id = NETWORK_SCAN_HOST_CALL.into();
    scope.resource_pool_id = i.resource.clone();
    scope.resource_generation_id = i.resource_generation.clone();
    scope.envelope_id = i.envelope.clone();
    scope.maximum_parameter_bytes = MAXIMUM_SCAN_INPUT_BYTES;
    scope.maximum_result_bytes = MAXIMUM_SCAN_OUTPUT_BYTES;
    scope
}

#[test]
fn flat_scan_binding_uses_borrowed_contract_and_returns_actual_possession_on_refusal() {
    use conduit_core as c;
    use conduit_net::*;
    use conduit_network_join_call::scan::*;
    use conduit_plan_lowering::selected_operation::SelectedFlatDirectOperationPlan;
    for substitution in 0..7 {
        let (mut plan, ready) = scan_fixture();
        if [1, 5, 6].contains(&substitution) {
            let scan = plan.fragments[0]
                .placements
                .iter_mut()
                .find(|g| g.kind_id.as_str() == NETWORK_SCAN_OPERATION)
                .unwrap();
            match substitution {
                1 => scan.artifact_id = "foreign/artifact".into(),
                5 => scan.execution_profile_id = "foreign/profile".into(),
                _ => scan.base.as_mut().unwrap().mechanism_family = "foreign/family".into(),
            }
            plan = c::seal_plan_with_completion(
                c::PlotIdentity {
                    source_document_id: plan.source_document_id.clone(),
                    checked_plot_id: plan.checked_plot_id.clone(),
                    expanded_plot_id: plan.expanded_plot_id.clone(),
                },
                plan.completion_policy,
                plan.fragments,
            );
        }
        let active = c::bind_active_play(
            &plan.plan_id,
            &ready.native_identity().host,
            &ready.native_identity().boot,
            1,
        );
        let mut scope = issued_scope(&ready, &active);
        match substitution {
            2 => scope.boot_id = "foreign/boot".into(),
            3 => scope.subject_kind = "authority/foreign".into(),
            4 => {
                scope.resource_generation_id = c::ResourceGenerationId("foreign/generation".into())
            }
            _ => (),
        }
        let foreign = c::bind_active_play(&plan.plan_id, &active.host_id, &active.boot_id, 2);
        let (table, handle, claim) = possession::possession(scope);
        let verified = c::FlatPlanRef::from_plan(&plan).unwrap().verify().unwrap();
        let tables = ScanFlatDirectTables::prepare(&verified).unwrap();
        let placement = plan.fragments[0]
            .placements
            .iter()
            .find(|g| g.kind_id.as_str() == NETWORK_SCAN_OPERATION)
            .unwrap()
            .placement_id
            .as_str();
        let mut slot = Some(ready);
        let original = slot.as_ref().unwrap() as *const _;
        let (result, memory) = allocation_probe::observe(|| {
            ReadyNetworkScanBase::bind_flat_direct_retained(
                &mut slot,
                table,
                handle,
                claim,
                SelectedFlatDirectOperationPlan {
                    tables: &tables,
                    active: if substitution == 0 { &foreign } else { &active },
                    placement_id: placement,
                },
            )
        });
        assert_eq!(memory.allocations, 0);
        assert_eq!(memory.reallocations, 0);
        let (_, table, handle, claim) = result.err().expect("foreign scope must refuse");
        assert_eq!(slot.as_ref().unwrap() as *const _, original);
        // Even a foreign-but-valid CPU fixture bearer remains exactly usable
        // in its own original scope; rejection did not discard/spend it.
        c::BoundBaseOperation::check(&table, &handle, &claim).unwrap();
        assert_eq!(table.inspections().next().unwrap().completed_operations, 0);
    }
    let (plan, ready) = scan_fixture();
    let active = c::bind_active_play(
        &plan.plan_id,
        &ready.native_identity().host,
        &ready.native_identity().boot,
        1,
    );
    let (table, handle, claim) = possession::possession(issued_scope(&ready, &active));
    let verified = c::FlatPlanRef::from_plan(&plan).unwrap().verify().unwrap();
    let tables = ScanFlatDirectTables::prepare(&verified).unwrap();
    let roles = PreparedScanKernelParts::check_flat_direct(&tables).unwrap();
    let placement = plan.fragments[0]
        .placements
        .iter()
        .find(|g| g.kind_id.as_str() == NETWORK_SCAN_OPERATION)
        .unwrap()
        .placement_id
        .as_str();
    let mut slot = Some(ready);
    let (result, memory) = allocation_probe::observe(|| {
        ReadyNetworkScanBase::bind_flat_direct_retained(
            &mut slot,
            table,
            handle,
            claim,
            SelectedFlatDirectOperationPlan {
                tables: &tables,
                active: &active,
                placement_id: placement,
            },
        )
    });
    let owner = result.unwrap_or_else(|_| panic!("valid borrowed binding"));
    assert!(slot.is_none());
    assert_eq!(memory.allocations, 0);
    assert_eq!(memory.reallocations, 0);
    let mut selected = Some(owner);
    selected
        .as_mut()
        .unwrap()
        .revoke(roles.scan, conduit_kernel::HostCallId(0))
        .unwrap();
    assert!(NetworkScanHostCall::retire_retained(&mut selected).is_ok());
    assert!(selected.is_none());
}
