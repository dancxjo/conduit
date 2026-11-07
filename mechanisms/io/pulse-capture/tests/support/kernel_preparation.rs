//! CPU reference admission proof, not production authority or a physical capture.
use super::*;
use conduit_embedded_build as e;
use conduit_plan_lowering::{
    assigned_play_scope::AssignedPlayScope,
    selected_assigned_operation::{
        AssignedOperationInput, SelectedAssignedForeOperationPlan, SelectedAssignedOperationPlan,
    },
};
use conduit_pulse_capture::{installation::*, kernel::*};
// Direct stderr also retains measurements when a runner captures test macros.
macro_rules! report {
    ($($argument:tt)*) => {{
        use std::io::Write;
        writeln!(std::io::stderr(), $($argument)*).unwrap();
    }};
}
fn owner(state: Rc<State>) -> ReadyPulseBase<Provider> {
    let scope = fixture::scope();
    // SAFETY: inert exclusively owned fixture with observed cancellation law.
    unsafe {
        ReadyPulseBase::new(
            PulseNativeIdentity {
                host_id: scope.host_id,
                boot_id: scope.boot_id,
                base_id: "base/pulse".into(),
                provider_instance_id: scope.base_instance_id,
                provider_generation: scope.base_provider_generation,
                resource_pool_id: scope.resource_pool_id,
                resource_generation_id: scope.resource_generation_id,
                envelope_id: scope.envelope_id,
                artifact_id: "artifact/pulse/fixture".into(),
                implementation_id: scope.implementation_id,
                base_implementation_id: "base/pulse/fixture@1".into(),
            },
            PulseAttachment {
                generation: 7,
                resource_bytes: 4096,
                limits: limits(),
            },
            Provider(state),
        )
    }
    .unwrap()
}
#[test]
fn generated_pulse_preparation_preserves_owner_until_exact_bounded_transfer() {
    let (mut fragment, _, _, _) = fixture::selected();
    let gear = &mut fragment.placements[0];
    gear.capability_id = PULSE_CAPABILITY.into();
    gear.execution_profile_id = PULSE_EXECUTION_PROFILE.into();
    gear.authority[0].contract_id = PULSE_AUTHORITY.into();
    gear.authority[0].capability_id = PULSE_CAPABILITY.into();
    for port in gear.inputs.iter().chain(&gear.outputs) {
        fragment.fore_ports.push(PlannedForePort {
            front_port_id: port.port_id.clone(),
            direction: port.direction,
            placement_id: gear.placement_id.clone(),
            gear_port_id: port.port_id.clone(),
            value_kind: port.value_kind.clone(),
            value_contract: None,
            abnormal_kind: None,
            track: ConnectionTrack::Payload,
            temporal: port.temporal,
            pressure_policy: DeliveryPressurePolicy::PreserveOrder,
            item_capacity: 1,
            byte_capacity: PULSE_READ_MAXIMUM_BYTES,
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
    let image = e::generate_fore_native_plan(
        &plan,
        &fragment.fragment_id,
        e::EmbeddedImageBounds {
            maximum_nodes: 1,
            maximum_cords: 2,
            maximum_routes: 1,
            maximum_route_targets: 2,
            maximum_host_calls: 1,
            maximum_resources: 1,
            maximum_sign_expectations: 16,
            maximum_configuration_entries: 2,
            maximum_ports_per_node: 8,
            maximum_remote_endpoints: 0,
            maximum_cord_value_slots: 2,
            maximum_cord_value_bytes: 2048,
            maximum_sign_items: 64,
            maximum_sign_bytes: 8192,
        },
        e::EmbeddedComputationBounds {
            maximum_program_bytes: 32768,
            maximum_total_program_bytes: 32768,
        },
        2,
    )
    .unwrap();
    let maxima = AssignedPlanMaxima {
        encoded_bytes: 8192,
        runtime_state_bytes: 8192,
        counts: [32; ASSIGNED_PLAN_COUNT_KINDS],
    };
    let bytes = image.encode(maxima).unwrap();
    // Trusted fixture Make output, independently retained from runtime assignment.
    let pins = image.layout_requirements(maxima).unwrap();
    let calls = [AssignedIdentity::from_text(PULSE_READ_CALL)];
    let resources: Vec<_> = image
        .kernel()
        .resources
        .iter()
        .map(|r| r.resource)
        .collect();
    let requirements = AssignedPlanRequirements {
        host: AssignedIdentity::from_text(fragment.host_id.as_str()),
        boot: AssignedIdentity::from_text(fragment.boot_id.as_str()),
        host_calls: &calls,
        resources: &resources,
        remote_bindings: &[],
    };
    let computations = AssignedComputationProfile {
        maximum_text_bytes: 32768,
        maximum_total_text_bytes: 32768,
        requirements: &[],
    };
    let counts =
        decode_assigned_plan_with_fore(&bytes, maxima, requirements, computations, image.fore())
            .unwrap()
            .counts;
    let layout = conduit_kernel::assigned::ResidentLayout {
        records: &pins,
        counts,
    };
    let selected = || SelectedAssignedForeOperationPlan {
        operation: SelectedAssignedOperationPlan {
            input: AssignedOperationInput::Contiguous(&bytes),
            maxima,
            requirements,
            computations,
            fragment_id: fragment.fragment_id.as_str(),
            scope: AssignedPlayScope::Plot(&active),
            node: NodeId(0),
        },
        fore: image.fore(),
    };
    let state = Rc::new(State::default());
    let mut slot = Some(owner(state.clone()));
    let contract = PulseReadContract::prepare().unwrap();
    let codec = PreparedPulseCodec::prepare(&contract).unwrap();
    report!("pulse CPU fixture pointer_bits={} codec_reachable_backing_bytes={} scheduler_inline_bytes={}",
        usize::BITS, codec.retained_backing_bytes().unwrap(), core::mem::size_of::<PulseKernel>());
    drop(codec);

    for capacity in [0, core::mem::size_of::<PulseKernel>()] {
        let (table, handle, claim) = assigned_retained::issued(fragment, &active);
        assert!(PreparedPulseKernel::prepare(
            PulsePossession {
                slot: &mut slot,
                table,
                handle,
                claim
            },
            &contract,
            selected(),
            &layout,
            capacity
        )
        .is_err());
        assert!(slot.is_some());
        assert_eq!(state.effects.get(), 0);
    }
    let mut wrong_pins = pins.clone();
    wrong_pins[0].payload_digest[0] ^= 1;
    let foreign_layout = conduit_kernel::assigned::ResidentLayout {
        records: &wrong_pins,
        counts,
    };
    let (table, handle, claim) = assigned_retained::issued(fragment, &active);
    assert!(PreparedPulseKernel::prepare(
        PulsePossession {
            slot: &mut slot,
            table,
            handle,
            claim
        },
        &contract,
        selected(),
        &foreign_layout,
        65536
    )
    .is_err());
    assert!(slot.is_some());
    assert_eq!(state.effects.get(), 0);
    let (table, handle, mut claim) = assigned_retained::issued(fragment, &active);
    claim.boot_id = "stale".into();
    assert!(PreparedPulseKernel::prepare(
        PulsePossession {
            slot: &mut slot,
            table,
            handle,
            claim
        },
        &contract,
        selected(),
        &layout,
        65536
    )
    .is_err());
    assert!(slot.is_some());
    let (table, handle, claim) = assigned_retained::issued(fragment, &active);
    report!(
        "pulse CPU fixture capability_table_backing_bytes={} claim_backing_bytes={}",
        table.retained_backing_bytes().unwrap(),
        claim.retained_backing_bytes().unwrap()
    );
    let prepared = PreparedPulseKernel::prepare(
        PulsePossession {
            slot: &mut slot,
            table,
            handle,
            claim,
        },
        &contract,
        selected(),
        &layout,
        65536,
    )
    .unwrap();
    assert!(prepared.storage_bytes() > core::mem::size_of::<PulseKernel>());
    report!("pulse CPU fixture prepared_retained_storage_bytes={} (scheduler + codec/table/claim reachable backing; excludes retained contract, provider, temporary workspace, inline ticket and allocator overhead)", prepared.storage_bytes());

    drop(prepared);
    assert!(slot.is_some());
    assert_eq!(state.effects.get(), 0);
    let (table, handle, claim) = assigned_retained::issued(fragment, &active);
    let prepared = PreparedPulseKernel::prepare(
        PulsePossession {
            slot: &mut slot,
            table,
            handle,
            claim,
        },
        &contract,
        selected(),
        &layout,
        65536,
    )
    .unwrap();
    let (mut bound, cost) = allocation_probe::observe(|| prepared.bind());
    assert_eq!((cost.allocations, cost.reallocations), (0, 0));
    assert!(slot.is_none());
    assert_eq!(state.effects.get(), 0);
    bound.call.revoke(NodeId(0), HostCallId(0)).unwrap();
    let (provider, _) = PulseHostCall::retire_retained(&mut Some(bound.call)).unwrap();
    assert!(Rc::ptr_eq(&provider.0, &state));
}
