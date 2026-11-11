//! Assigned service factory through original signed workset admission and Start.
use super::*;
use conduit_body as b;
use conduit_core as c;
use conduit_mcu_host_runtime::{
    execution::{Policy, PreparedBacks, PreparedFragment},
    execution_factory::{
        BoxedRecordServiceBack, Driver, PreparedCalls, PreparedDrivers, SoftwareInventory,
    },
    execution_workset::PreparedLocalWorkset,
};
use conduit_plan_lowering::{
    assigned_play_scope::AssignedPlayScope, selected_assigned_operation::*,
    selected_assigned_operation_set::*,
};
use conduit_resource_snapshot::{
    native_append::service_back::RecordServiceBack, native_service::*,
};
use ed25519_dalek::{Signer, SigningKey};
use service_fixture::{operation, ready, Fixture};

fn maxima() -> c::AssignedPlanMaxima {
    c::AssignedPlanMaxima {
        encoded_bytes: 8192,
        runtime_state_bytes: 16384,
        counts: [32; c::ASSIGNED_PLAN_COUNT_KINDS],
    }
}
fn image(f: &Fixture) -> Vec<u8> {
    use conduit_embedded_build::*;
    let image = generate_embedded_plan(
        &f.fragment,
        &f.lowered,
        EmbeddedImageBounds {
            maximum_nodes: 1,
            maximum_cords: 0,
            maximum_routes: 4,
            maximum_route_targets: 4,
            maximum_host_calls: 2,
            maximum_resources: 1,
            maximum_sign_expectations: 32,
            maximum_configuration_entries: 0,
            maximum_ports_per_node: 8,
            maximum_remote_endpoints: 0,
            maximum_cord_value_slots: 4,
            maximum_cord_value_bytes: 4096,
            maximum_sign_items: 64,
            maximum_sign_bytes: 8192,
        },
    )
    .unwrap();
    encode_assigned_native_plan(&image, maxima()).unwrap()
}
fn prepared<'a>(
    f: &'a Fixture,
    slot: &'a mut Option<ReadyRecordService<fixture::Storage>>,
    bytes: &[u8],
    scope: AssignedPlayScope<'_>,
) -> PreparedAssignedRecordService<'a, fixture::Storage> {
    let mut scopes = f.scopes.clone();
    if let AssignedPlayScope::Delegated(fragment) = scope {
        for s in &mut scopes {
            s.plan_id = fragment.metadata().body_plan.into();
            s.active_play_id = fragment.metadata().body_play.into();
        }
    }
    let calls = scopes
        .each_ref()
        .map(|s| c::AssignedIdentity::from_text(s.operation_contract_id.as_str()));
    ReadyRecordService::prepare_assigned_retained(
        slot,
        scopes.map(operation),
        &f.resource,
        SelectedAssignedServicePlan {
            operation: SelectedAssignedOperationPlan {
                input: AssignedOperationInput::Contiguous(bytes),
                maxima: maxima(),
                requirements: c::AssignedPlanRequirements {
                    host: c::AssignedIdentity::from_text(f.fragment.host_id.as_str()),
                    boot: c::AssignedIdentity::from_text(f.fragment.boot_id.as_str()),
                    host_calls: &calls,
                    resources: &[0],
                    remote_bindings: &[],
                },
                computations: c::AssignedComputationProfile {
                    maximum_text_bytes: 0,
                    maximum_total_text_bytes: 0,
                    requirements: &[],
                },
                fragment_id: f.fragment.fragment_id.as_str(),
                scope,
                node: NodeId(0),
            },
            fore: (u16::from_le_bytes(bytes[8..10].try_into().unwrap())
                == c::ASSIGNED_FORE_NATIVE_PLAN_SCHEMA)
                .then_some(&[]),
            resource: 0,
        },
    )
    .unwrap_or_else(|_| panic!("actual assigned service"))
}
fn native() -> PreparedCalls<'static, NoHardware, NoHardware, NoHardware, 0> {
    PreparedCalls {
        pulse: None,
        write: None,
        waits: [],
    }
}
#[test]
fn assigned_factory_refusals_preserve_ready_and_charge_all_retained_backing() {
    use super::super::native_factory_appender::Residents;
    let f = Fixture::new();
    let bytes = image(&f);
    let storage = fixture::Storage::new();
    for (node, backing, error) in [(0, 0, "duplicate-driver"), (1, usize::MAX, "backing")] {
        let mut slot = Some(ready(&f, storage.clone()));
        let ticket = prepared(&f, &mut slot, &bytes, AssignedPlayScope::Plot(&f.active));
        // SAFETY: empty checked software inventory, no runtime effect owner.
        let mut software =
            unsafe { SoftwareInventory::new([None, None], [None, None], 0, &[]) }.unwrap();
        let selected = unsafe { software.prepare([false, false]) }.unwrap();
        // SAFETY: this refused candidate never binds; production caller must use
        // original authenticated Start with the exact ticket resource admitted.
        let result = unsafe {
            PreparedDrivers::with_record_service::<_, 2, READ_RESULT_BYTES>(
                Residents {
                    nodes: [Some(node)],
                    backing,
                },
                [Some(node)],
                selected,
                native(),
                ticket,
            )
        };
        assert_eq!(result.err(), Some(error));
        assert!(slot.is_some());
        assert_eq!(storage.programs.get(), 0);
    }
    let mut slot = Some(ready(&f, storage.clone()));
    let expected_ready = slot.as_ref().unwrap().retained_backing_bytes().unwrap();
    let ticket = prepared(&f, &mut slot, &bytes, AssignedPlayScope::Plot(&f.active));
    let expected_operations = ticket.operation_backing_bytes().unwrap();
    assert_eq!(
        ticket.retained_backing_bytes(),
        Some(expected_ready + expected_operations)
    );
    let mut software =
        unsafe { SoftwareInventory::new([None, None], [None, None], 0, &[]) }.unwrap();
    let selected = unsafe { software.prepare([false, false]) }.unwrap();
    let candidate = unsafe {
        PreparedDrivers::with_record_service::<_, 2, READ_RESULT_BYTES>(
            Residents {
                nodes: [Some(1)],
                backing: 37,
            },
            [Some(1)],
            selected,
            native(),
            ticket,
        )
    }
    .unwrap();
    assert_eq!(
        PreparedBacks::<2, 2>::retained_backing_bytes(&candidate),
        Some(
            37 + expected_ready
                + expected_operations
                + core::mem::size_of::<RecordServiceBack<2, READ_RESULT_BYTES>>()
        )
    );
    assert_eq!(
        core::mem::size_of::<Driver<'_, HostCallBack>>(),
        core::mem::size_of::<Driver<'_, HostCallBack, BoxedRecordServiceBack<2, READ_RESULT_BYTES>>>(
        )
    );
    println!("service factory CPU64: ready_heap={expected_ready}, operation_heap={expected_operations}, boxed_back={}, old_driver={}, service_driver={}",
        core::mem::size_of::<RecordServiceBack<2,READ_RESULT_BYTES>>(),
        core::mem::size_of::<Driver<'_,HostCallBack>>(),
        core::mem::size_of::<Driver<'_,HostCallBack,BoxedRecordServiceBack<2,READ_RESULT_BYTES>>>());
    assert!(PreparedBacks::<2, 2>::requires_terminal_gate(&candidate));
    assert!(
        PreparedBacks::<2, 2>::terminal_transductions(&candidate).is_none(),
        "an arbitrary resident owner must explicitly refuse inspection"
    );
    drop(candidate);
    assert!(slot.is_some());
    assert_eq!(storage.programs.get(), 0);
}
#[test]
fn original_signed_start_binds_actual_service_once_without_allocation_or_media_effect() {
    check_signed_start::<2>(true, None, false);
    check_signed_start::<1>(false, None, false);
    check_signed_start::<2>(false, Some(0), false);
    check_signed_start::<2>(false, Some(1), false);
    check_signed_start::<2>(false, None, true);
}
// A terminal-bearing initialized resident owner, independent of expected Plan
// data. Production expression residents currently support only empty laws.
struct TerminalResident;
impl<const P: usize> StepBack<P> for TerminalResident {
    fn terminal_transductions(&self) -> [Option<AssignedTerminalTransduction>; P] {
        core::array::from_fn(|port| {
            (port < 2).then_some(AssignedTerminalTransduction {
                input: PortId(port as u16),
                output: PortId(port as u16),
                normal_close: AssignedNormalCloseTransduction::FlushThenPropagateWhenAllClose(
                    AssignedFiniteTerminalEmission {
                        maximum_items: 2,
                        maximum_bytes: conduit_resource_snapshot::append_result::APPEND_RESULT_BYTES
                            + READ_RESULT_BYTES as u32,
                    },
                ),
                abnormal: AssignedAbnormalTransduction::NotAccepted,
                cancellation: AssignedCancellationTransduction::NotCancellable,
            })
        })
    }
    fn step(&mut self, _: &mut StepIo<P>, _: &StepInputBytes<'_, P>) -> StepOutcome {
        StepOutcome::Await
    }
}
struct ResidentLoan<'a>(&'a mut Option<TerminalResident>);
// SAFETY: exclusively borrowed actual owner, exact node/report, inline state;
// inspection and drop do not withdraw it; bind transfers that same owner once.
unsafe impl conduit_mcu_host_runtime::execution_factory::PreparedResidentEntry
    for ResidentLoan<'_>
{
    type Back = TerminalResident;
    fn node(&self) -> u16 {
        1
    }
    fn terminal_transductions<const P: usize>(
        &self,
    ) -> Option<[Option<AssignedTerminalTransduction>; P]> {
        Some(self.0.as_ref()?.terminal_transductions())
    }
    fn retained_backing_bytes(&self) -> Option<usize> {
        Some(0)
    }
    fn bind(self) -> (u16, Self::Back) {
        (1, self.0.take().unwrap())
    }
}
fn check_signed_start<const HOST_CALLS: usize>(
    can_start: bool,
    mismatch: Option<usize>,
    legacy: bool,
) {
    let mut f = Fixture::new();
    let mut resident = f.fragment.placements[0].clone();
    resident.placement_id = "terminal-resident".into();
    resident.kind_id = "test/terminal-resident".into();
    resident.host_calls.clear();
    resident.resources.clear();
    resident.authority.clear();
    resident.base = None;
    f.fragment.startup_order.push(resident.placement_id.clone());
    f.fragment.placements.push(resident);
    let storage = fixture::Storage::new();
    let plot = c::seal_plan(
        c::PlotIdentity {
            source_document_id: f.fragment.source_document_id.clone(),
            checked_plot_id: f.fragment.checked_plot_id.clone(),
            expanded_plot_id: f.fragment.expanded_plot_id.clone(),
        },
        vec![f.fragment.clone()],
    );
    f.fragment = plot.fragments[0].clone();
    let generated = conduit_embedded_build::generate_fore_native_plan(
        &plot,
        &f.fragment.fragment_id,
        conduit_embedded_build::EmbeddedImageBounds {
            maximum_nodes: 2,
            maximum_cords: 0,
            maximum_routes: 4,
            maximum_route_targets: 4,
            maximum_host_calls: 2,
            maximum_resources: 1,
            maximum_sign_expectations: 32,
            maximum_configuration_entries: 0,
            maximum_ports_per_node: 8,
            maximum_remote_endpoints: 0,
            maximum_cord_value_slots: 4,
            maximum_cord_value_bytes: 4096,
            maximum_sign_items: 64,
            maximum_sign_bytes: 8192,
        },
        conduit_embedded_build::EmbeddedComputationBounds {
            maximum_program_bytes: 0,
            maximum_total_program_bytes: 0,
        },
        16,
    )
    .unwrap();
    assert!(generated.fore().is_empty());
    let projection = generated.encode(maxima()).unwrap();
    let resident = b::ResidentPlot::new(
        plot.source_document_id.clone(),
        plot.checked_plot_id.clone(),
    );
    let body = b::Body::born_with_plots(
        b::BodyWorkset::from_plots([resident.clone()]).unwrap(),
        1,
        "born".into(),
    )
    .unwrap();
    let wake = body.wake(1, "wake".into()).unwrap().1;
    let plan = b::BodyPlan::seal(
        &wake,
        vec![b::BodyPlotPlan {
            plot: resident,
            plan: plot,
        }],
    )
    .unwrap();
    let play = b::BodyPlayIdentity::bind(&plan, 1);
    let scope =
        b::PreparedBodyFragment::prepare(&wake, &plan, &play, &plan.plots[0].plan.fragments[0])
            .unwrap();
    let mut resources = Vec::new();
    b::PlainFragmentResource {
        node: 0,
        resource: 0,
        placement: f.placement.as_str(),
        pool: f.resource.pool_id.as_str(),
        class: f.resource.class_id.as_str(),
        units: 1,
    }
    .append_to(&mut resources)
    .unwrap();
    let metadata =
        b::BodyFragmentApprovalMetadata::from_prepared(scope, &projection, &resources, &[])
            .unwrap();
    let document = b::encode_body_fragment_document(&metadata, &projection, 16384).unwrap();
    let payload = b::encode_body_workset_document(&[&document], 16384).unwrap();
    let actual = f.owner.host();
    let member:b::MembershipCredential=serde_json::from_value(serde_json::json!({"credential_id":"credential/test","body_id":body.body_id,"part_id":"part/test","host_id":actual.host_id,"boot_id":actual.boot_id,"issued_at_millis":1})).unwrap();
    let key = SigningKey::from_bytes(&[9; 32]);
    let owner = b::FederationPeer {
        host_id: "owner".into(),
        boot_id: "owner/boot".into(),
        offer_generation: c::OfferGeneration(1),
        verifying_key: key.verifying_key().to_bytes(),
    };
    let bootstrap = b::MemberBootstrap {
        schema: b::MEMBER_BOOTSTRAP_SCHEMA.into(),
        body_id: body.body_id.clone(),
        host_id: actual.host_id.as_str().into(),
        image_id: "image".into(),
        configuration_id: "config".into(),
        owner: owner.clone(),
        admission: b::MemberBootstrapMaterial::Retained {
            credential: member.clone(),
            secret: [3; 32],
        },
    };
    let challenge = b::IssuedHostCommandChallenge::issue(
        &member,
        actual,
        &owner,
        b::HostCommandReceiveBounds {
            payload_contract: b::HOST_PREPARED_WORKSET_CONTRACT,
            maximum_payload_bytes: 16384,
            lifetime_millis: 100,
        },
        [1; 32],
        10,
    )
    .unwrap();
    let signature = key
        .sign(&b::host_command_attribution_transcript(challenge.challenge(), &payload).unwrap())
        .to_bytes();
    let attributed = challenge.authenticate(&payload, &signature, 11).unwrap();
    let mut input = b::HostCommandInput::<16384>::new();
    input.writable().unwrap()[..payload.len()].copy_from_slice(&payload);
    input
        .retain(attributed.into_retained_stamp().unwrap())
        .unwrap();
    let context = b::NativePreparationContext {
        bootstrap: &bootstrap,
        member: &member,
        actual,
        image_id: "image",
        configuration_id: "config",
        now_millis: 11,
    };
    let preparation = b::HostPreparationDelegation {
        authorizer: b::HostPreparationAuthorizer::SeedOwner,
        maximum_assignment_bytes: 16384,
        maximum_runtime_state_bytes: 16384,
    };
    let execution = b::HostExecutionDelegation {
        authorizer: b::HostExecutionAuthorizer::SeedOwner,
        maximum_active_worksets: 1,
        continuation: None,
    };
    let policy = Policy {
        preparation: Some(&preparation),
        execution: Some(&execution),
        active_worksets: 0,
        maximum_retained_bytes: 131072,
    };
    let calls = f
        .scopes
        .each_ref()
        .map(|s| c::AssignedIdentity::from_text(s.operation_contract_id.as_str()));
    let profile = || b::AssignedBodyPreparationProfile {
        maxima: maxima(),
        requirements: c::AssignedPlanRequirements {
            host: c::AssignedIdentity::from_text(actual.host_id.as_str()),
            boot: c::AssignedIdentity::from_text(actual.boot_id.as_str()),
            host_calls: &calls,
            resources: &[0],
            remote_bindings: &[],
        },
        computations: c::AssignedComputationProfile {
            maximum_text_bytes: 0,
            maximum_total_text_bytes: 0,
            requirements: &[],
        },
        fore: Some(&[]),
    };
    let records = generated.layout_requirements(maxima()).unwrap();
    let view = c::decode_assigned_plan_with_fore(
        &projection,
        maxima(),
        profile().requirements,
        profile().computations,
        &[],
    )
    .unwrap();
    let layout = conduit_kernel::assigned::ResidentLayout {
        records: &records,
        counts: view.counts,
    };
    let mut slot = Some(ready(&f, storage.clone()));
    let mut resident_ready = Some(TerminalResident);
    let mut software =
        unsafe { SoftwareInventory::new([None, None], [None, None], 0, &[]) }.unwrap();
    let prepared_result = PreparedLocalWorkset::<_, 1>::prepare::<16384, 2, _>(
        &input,
        context,
        policy,
        [profile()],
        |approved, _| {
            let ticket = prepared(
                &f,
                &mut slot,
                approved[0].projection(),
                AssignedPlayScope::Delegated(&approved[0]),
            );
            let selected = unsafe { software.prepare([false, false]) }.unwrap();
            // SAFETY: retained only in this original workset; its complete resource
            // admission and authenticated Start below gate sole owner withdrawal.
            let backs = unsafe {
                PreparedDrivers::with_record_service::<_, 2, READ_RESULT_BYTES>(
                    conduit_mcu_host_runtime::execution_factory::PreparedResidentSubset([Some(
                        ResidentLoan(&mut resident_ready),
                    )]),
                    [Some(1)],
                    selected,
                    native(),
                    ticket,
                )
            }
            .unwrap();
            let expected = core::array::from_fn(|node| {
                core::array::from_fn(|port| {
                    if mismatch == Some(node) {
                        None
                    } else {
                        generated.kernel().nodes[node].terminal_transductions[port]
                    }
                })
            });
            if legacy {
                return PreparedFragment::<_, 2, 1, 2, 4, 4, HOST_CALLS, 64>::prepare(
                    &approved[0],
                    profile(),
                    &[],
                    &layout,
                    HostedValueStore::new(8, 837, 4096).unwrap(),
                    FixedSignLog::<64>::new(64 * core::mem::size_of::<KernelEvent>() as u32)
                        .unwrap(),
                    131072,
                    || Ok::<_, ()>(backs),
                );
            }
            PreparedFragment::<_, 2, 1, 2, 4, 4, HOST_CALLS, 64>::prepare_with_terminal_transductions(
                &approved[0],
                profile(),
                &[],
                &layout,
                HostedValueStore::new(8, 837, 4096).unwrap(),
                FixedSignLog::<64>::new(64 * core::mem::size_of::<KernelEvent>() as u32).unwrap(),
                131072,
                || Ok::<_, ()>(backs),
                expected,
            )
        },
    );
    let mut prepared_slot = prepared_result
        .map_err(|error| {
            assert!(!can_start, "unexpected prepare failure: {error:?}");
            match error {
                conduit_mcu_host_runtime::execution::Refusal::Back(
                    conduit_mcu_host_runtime::execution::Refusal::Tables(
                        conduit_kernel::assigned::AssignedKernelRefusal::Capacity,
                    ),
                ) => assert_eq!(HOST_CALLS, 1),
                conduit_mcu_host_runtime::execution::Refusal::Back(
                    conduit_mcu_host_runtime::execution::Refusal::TerminalContracts,
                ) => assert!(legacy),
                conduit_mcu_host_runtime::execution::Refusal::Back(
                    conduit_mcu_host_runtime::execution::Refusal::Storage(
                        conduit_kernel::scheduler::KernelPreparationError::Topology(
                            conduit_kernel::scheduler::SchedulerError::InvalidPlan,
                        ),
                    ),
                ) => assert!(mismatch.is_some()),
                other => panic!("unexpected refusal: {other:?}"),
            }
        })
        .ok();
    if !can_start {
        assert!(prepared_slot.is_none());
        drop(prepared_slot);
        assert!(
            slot.is_some(),
            "underfunded actual call table leaves Ready intact"
        );
        assert_eq!(storage.programs.get(), 0);
        assert!(
            resident_ready.is_some(),
            "refusal preserves the original resident Ready owner"
        );
        return;
    }
    let prepared = prepared_slot.take().unwrap();
    drop(prepared_slot);
    let mut ledger = c::ResourceAdmissionOwner::new(actual.clone());
    let pool = &actual.resources[0];
    let observation = c::ResourceObservation {
        host_id: actual.host_id.clone(),
        boot_id: actual.boot_id.clone(),
        offer_generation: actual.offer_generation,
        pool_id: pool.pool_id.clone(),
        class_id: pool.class_id.clone(),
        health: c::ResourceHealth::Ready,
        unreserved_units: 1,
        utilized_units: 0,
        sign_id: "ready".into(),
    };
    let mut admitted = prepared
        .admit::<()>(&mut ledger, &[observation], 0)
        .unwrap_or_else(|_| panic!("admitted actual service"));
    let missing = c::ResourceAdmissionOwner::new(actual.clone());
    let mut started = None;
    assert!(admitted
        .start_into::<16384, ()>(&input, context, policy, &missing, &mut started)
        .is_err());
    assert!(started.is_none());
    assert_eq!(storage.programs.get(), 0);
    allocations::without_allocations(|| {
        admitted.start_into::<16384, ()>(&input, context, policy, &ledger, &mut started)
    })
    .unwrap();
    assert!(admitted
        .start_into::<16384, ()>(&input, context, policy, &ledger, &mut started)
        .is_err());
    drop(admitted);
    assert!(slot.is_none());
    assert!(
        resident_ready.is_none(),
        "original Start transfers the inspected resident once"
    );
    assert_eq!(ledger.admissions().len(), 1);
    assert_eq!(storage.programs.get(), 0);
    let (kernel, mut calls) = started.unwrap().into_kernels();
    assert_eq!(calls.service.node(), NodeId(0));
    assert_eq!(calls.service.routes().len(), 2);
    assert!(
        calls.base.pulse.is_none() && calls.base.write.is_none() && calls.base.waits.is_empty()
    );
    assert_eq!(
        kernel.drivers()[1].terminal_transductions(),
        TerminalResident.terminal_transductions()
    );
    drop(kernel);
    calls.service.revoke().unwrap();
}
