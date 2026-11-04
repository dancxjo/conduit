//! Actual possession and hardware are inert CPU fixtures, not MCU acceptance.
use super::*;
use crate::installation::*;
use alloc::{vec, vec::Vec};
use conduit_core::*;
use conduit_embedded_build as e;
use conduit_kernel::HostCallId;
use conduit_plan_lowering::selected_assigned_operation::{
    AssignedOperationInput, SelectedAssignedOperationPlan,
};

#[test]
fn contiguous_assignment_preserves_controller_and_executes_under_its_lease() {
    exercise(false);
}
#[test]
fn borrowed_assignment_preserves_controller_and_executes_under_its_lease() {
    exercise(true);
}
fn exercise(borrowed: bool) {
    let contract = I2cContract::prepare().unwrap();
    let mut scope = fixture::scope();
    scope.capability_id = I2C_IMPLEMENTATION.into();
    scope.implementation_id = I2C_IMPLEMENTATION.into();
    scope.authority_contract_id = I2C_AUTHORITY.into();
    let (mut fragment, _, _, _) = crate::test_support::selected(
        &scope,
        contract.kind(),
        I2C_CALL,
        I2C_MAXIMUM_BYTES,
        "i2c",
        "i2c",
    );
    fragment.placements[0].execution_profile_id = I2C_EXECUTION_PROFILE.into();
    fragment.placements[0]
        .base
        .as_mut()
        .unwrap()
        .implementation_id = "conduitos.base/i2c-controller@1".into();
    let identity = I2cNativeIdentity {
        host_id: scope.host_id.clone(),
        boot_id: scope.boot_id.clone(),
        base_id: "base/i2c".into(),
        provider_instance_id: scope.base_instance_id.clone(),
        provider_generation: scope.base_provider_generation,
        resource_pool_id: scope.resource_pool_id.clone(),
        resource_generation_id: scope.resource_generation_id.clone(),
        envelope_id: scope.envelope_id.clone(),
        artifact_id: "artifact/i2c/fixture".into(),
    };
    // SAFETY: exclusively retained inert CPU provider; no physical I2C claim.
    let mut slot = Some(
        unsafe {
            ReadyI2cBase::new(
                identity,
                I2cAttachment {
                    generation: 7,
                    minimum_address: 0x76,
                    maximum_address: 0x77,
                    resource_bytes: 4096,
                },
                Provider::default(),
            )
        }
        .unwrap(),
    );
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
    let image = e::generate_embedded_plan(
        fragment,
        &lowered,
        e::EmbeddedImageBounds {
            maximum_nodes: 1,
            maximum_cords: 1,
            maximum_routes: 1,
            maximum_route_targets: 1,
            maximum_host_calls: 1,
            maximum_resources: 1,
            maximum_sign_expectations: 16,
            maximum_configuration_entries: 1,
            maximum_ports_per_node: 8,
            maximum_remote_endpoints: 0,
            maximum_cord_value_slots: 1,
            maximum_cord_value_bytes: 4096,
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
    let records: Vec<_> = AssignedPlanRecords::new(&bytes)
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    let resources: Vec<_> = image.resources.iter().map(|r| r.resource).collect();
    let calls = [AssignedIdentity::from_text(I2C_CALL)];
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
        fragment_id: &fragment.fragment_id,
        active,
        node: lowered.nodes[0].node,
    };
    let issue = || fixture::authorization(scope.clone(), fragment, &active);
    let mut wrong = active.clone();
    wrong.boot_id = "boot/stale".into();
    let (table, handle, claim) = issue();
    assert!(ReadyI2cBase::bind_assigned_retained(
        &mut slot,
        table,
        handle,
        claim,
        &contract,
        selected(&wrong)
    )
    .is_err());
    assert!(slot.is_some());
    let (table, handle, mut claim) = issue();
    claim.resource_generation_id = ResourceGenerationId("resource/stale".into());
    assert!(ReadyI2cBase::bind_assigned_retained(
        &mut slot,
        table,
        handle,
        claim,
        &contract,
        selected(&active)
    )
    .is_err());
    assert!(slot.is_some());
    let (table, handle, mut claim) = issue();
    claim.parameter_bytes -= 1;
    assert!(ReadyI2cBase::bind_assigned_retained(
        &mut slot,
        table,
        handle,
        claim,
        &contract,
        selected(&active)
    )
    .is_err());
    assert!(slot.is_some());
    let (table, _, claim) = issue();
    assert_eq!(
        slot.as_ref()
            .unwrap()
            .check_assigned(&table, &claim, &contract, selected(&active))
            .unwrap(),
        lowered.nodes[0].node
    );
    let (table, handle, claim) = issue();
    let mut owner = ReadyI2cBase::bind_assigned_retained(
        &mut slot,
        table,
        handle,
        claim,
        &contract,
        selected(&active),
    )
    .unwrap();
    assert!(slot.is_none());
    assert_eq!(owner.provider.effects, 0);
    let (table, handle, claim) = issue();
    assert!(ReadyI2cBase::bind_assigned_retained(
        &mut slot,
        table,
        handle,
        claim,
        &contract,
        selected(&active)
    )
    .is_err());
    drop(records);
    drop(bytes);
    let forbidden = request(&contract, 0x50, &[0xd0], 1);
    let node = lowered.nodes[0].node;
    assert_eq!(
        tag(owner
            .invoke(node, HostCallId(0), RequestId(0), &forbidden)
            .unwrap()),
        "refused"
    );
    assert_eq!(owner.provider.effects, 0);
    let input = request(&contract, 0x76, &[0xd0], 1);
    assert_eq!(
        tag(owner
            .invoke(node, HostCallId(0), RequestId(1), &input)
            .unwrap()),
        "completed"
    );
    assert_eq!(owner.provider.effects, 1);
    assert_eq!(
        owner
            .invoke(node, HostCallId(0), RequestId(1), &input)
            .err(),
        Some(I2cCallRefusal::StaleRequest)
    );
    owner.revoke(node, HostCallId(0)).unwrap();
    assert!(matches!(
        owner.invoke(node, HostCallId(0), RequestId(2), &input),
        Err(I2cCallRefusal::Capability(_))
    ));
    assert_eq!(owner.provider.effects, 1);
}
