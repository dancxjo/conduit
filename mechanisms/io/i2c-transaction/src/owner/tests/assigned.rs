//! Actual possession and hardware are inert CPU fixtures, not MCU acceptance.
use super::*;
use crate::installation::*;
use alloc::{vec, vec::Vec};
use conduit_core::*;
use conduit_embedded_build as e;
use conduit_kernel::HostCallId;
use conduit_plan_lowering::assigned_play_scope::AssignedPlayScope;
#[path = "assigned_body_scope.rs"]
mod body_scope;
#[path = "assigned_possession.rs"]
mod possession;
use conduit_plan_lowering::selected_assigned_operation::{
    AssignedOperationInput, SelectedAssignedForeOperationPlan, SelectedAssignedOperationPlan,
};

#[test]
fn contiguous_assignment_preserves_controller_and_executes_under_its_lease() {
    exercise(false, false, false);
}
#[test]
fn borrowed_assignment_preserves_controller_and_executes_under_its_lease() {
    exercise(true, false, false);
}
#[test]
fn local_fore_assignment_preserves_controller_and_executes_under_its_lease() {
    exercise(false, true, false);
}
#[test]
fn borrowed_local_fore_preserves_controller_and_executes_under_its_lease() {
    exercise(true, true, false);
}
#[test]
fn body_assignment_retains_actual_authority_and_controller_custody() {
    exercise(false, false, true);
    exercise(true, false, true);
}
#[test]
fn body_fore_assignment_retains_actual_authority_and_controller_custody() {
    exercise(false, true, true);
    exercise(true, true, true);
}
fn exercise(borrowed: bool, fore: bool, body: bool) {
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
        .implementation_id = "fixture/base-i2c@1".into();
    if fore {
        let gear = &fragment.placements[0];
        for (direction, port) in [
            (PortDirection::Input, &gear.inputs[0]),
            (PortDirection::Output, &gear.outputs[0]),
        ] {
            fragment.fore_ports.push(PlannedForePort {
                front_port_id: port.port_id.clone(),
                direction,
                placement_id: gear.placement_id.clone(),
                gear_port_id: port.port_id.clone(),
                value_kind: port.value_kind.clone(),
                value_contract: None,
                abnormal_kind: None,
                track: ConnectionTrack::Payload,
                temporal: port.temporal,
                pressure_policy: DeliveryPressurePolicy::PreserveOrder,
                item_capacity: 1,
                byte_capacity: I2C_MAXIMUM_BYTES,
                selected_line: None,
            });
        }
    }
    let identity = I2cNativeIdentity {
        base_implementation_id: "fixture/base-i2c@1".into(),
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
                identity.clone(),
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
    let bounds = e::EmbeddedImageBounds {
        maximum_nodes: 1,
        maximum_cords: 2,
        maximum_routes: 2,
        maximum_route_targets: 1,
        maximum_host_calls: 1,
        maximum_resources: 1,
        maximum_sign_expectations: 16,
        maximum_configuration_entries: 1,
        maximum_ports_per_node: 8,
        maximum_remote_endpoints: 0,
        maximum_cord_value_slots: if fore { 2 } else { 1 },
        maximum_cord_value_bytes: if fore { 8192 } else { 4096 },
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
            2,
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
        runtime_state_bytes: if fore { 16 * 1024 } else { 8192 },
        counts: [32; ASSIGNED_PLAN_COUNT_KINDS],
    };
    let bytes = if let Some(image) = &fore_image {
        // Two independent 4096-byte Fore queues cannot fit the old 8 KiB fixture.
        // Refuse that shortage rather than silently borrowing another grant.
        assert!(image
            .encode(AssignedPlanMaxima {
                runtime_state_bytes: 8192,
                ..maxima
            })
            .is_err());
        image.encode(maxima).unwrap()
    } else {
        e::encode_assigned_native_plan(image, maxima).unwrap()
    };
    let records: Vec<_> = AssignedPlanRecords::new(&bytes)
        .unwrap()
        .collect::<Result<_, _>>()
        .unwrap();
    let resources: Vec<_> = image.resources.iter().map(|r| r.resource).collect();
    let calls = [AssignedIdentity::from_text(I2C_CALL)];
    let context = body.then(|| body_scope::Context::new(&plan));
    let valid_scope = context
        .as_ref()
        .map_or(AssignedPlayScope::Plot(&active), |context| {
            context.scope(fragment)
        });
    let selected = |scope| SelectedAssignedOperationPlan {
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
        scope,
        node: lowered.nodes[0].node,
    };
    let bind = |slot: &mut Option<ReadyI2cBase<Provider>>, table, handle, claim, scope| {
        let prepared = if let Some(image) = &fore_image {
            ReadyI2cBase::prepare_assigned_fore_retained(
                slot,
                table,
                handle,
                claim,
                &contract,
                SelectedAssignedForeOperationPlan {
                    operation: selected(scope),
                    fore: image.fore(),
                },
            )
        } else {
            ReadyI2cBase::prepare_assigned_retained(
                slot,
                table,
                handle,
                claim,
                &contract,
                selected(scope),
            )
        }?;
        assert_eq!(prepared.node(), lowered.nodes[0].node);
        let (owner, cost) = crate::allocation_probe::observe(|| prepared.bind());
        assert_eq!(cost.allocations, 0);
        assert_eq!(cost.reallocations, 0);
        Ok(owner)
    };
    let issue = || match &context {
        Some(context) => {
            fixture::authorization_for_play(scope.clone(), context.plan_id(), context.play_id())
        }
        None => fixture::authorization(scope.clone(), fragment, &active),
    };
    let mut foreign_identity = identity.clone();
    foreign_identity.base_implementation_id = "fixture/other-base-i2c@1".into();
    // SAFETY: a second inert provider with an independently named physical
    // Base is deliberately incompatible with this exact selected Plan.
    let mut foreign = Some(unsafe {
        ReadyI2cBase::new(
            foreign_identity,
            slot.as_ref().unwrap().attachment(),
            Provider::default(),
        )
        .unwrap()
    });
    let (table, handle, claim) = issue();
    assert!(matches!(
        bind(&mut foreign, table, handle, claim, valid_scope),
        Err(I2cCallRefusal::WrongBinding)
    ));
    assert!(foreign.is_some());
    possession::reject_unusable_grants(&mut slot, &scope, &issue, |slot, table, handle, claim| {
        bind(slot, table, handle, claim, valid_scope)
    });
    if let Some(context) = &context {
        // A true Body scope cannot turn a copied claim into local possession.
        let (table, handle, mut claim) = fixture::authorization(scope.clone(), fragment, &active);
        claim.plan_id = context.plan_id().clone();
        claim.active_play_id = context.play_id().clone();
        assert!(matches!(
            bind(&mut slot, table, handle, claim, valid_scope),
            Err(I2cCallRefusal::WrongBinding)
        ));
        assert!(slot.is_some());
        let (table, handle, claim) = issue();
        assert!(matches!(
            bind(
                &mut slot,
                table,
                handle,
                claim,
                AssignedPlayScope::Plot(&active)
            ),
            Err(I2cCallRefusal::WrongBinding)
        ));
        assert!(slot.is_some());
    }
    let mut wrong = active.clone();
    wrong.boot_id = "boot/stale".into();
    let (table, handle, claim) = issue();
    assert!(bind(
        &mut slot,
        table,
        handle,
        claim,
        AssignedPlayScope::Plot(&wrong)
    )
    .is_err());
    assert!(slot.is_some());
    let (table, handle, mut claim) = issue();
    claim.resource_generation_id = ResourceGenerationId("resource/stale".into());
    assert!(bind(&mut slot, table, handle, claim, valid_scope).is_err());
    assert!(slot.is_some());
    let (table, handle, mut claim) = issue();
    claim.parameter_bytes -= 1;
    assert!(bind(&mut slot, table, handle, claim, valid_scope).is_err());
    assert!(slot.is_some());
    let (table, _, claim) = issue();
    let ready = slot.as_ref().unwrap();
    let checked = if let Some(image) = &fore_image {
        assert_eq!(image.fore().len(), 2);
        assert!(ready
            .check_assigned(&table, &claim, &contract, selected(valid_scope))
            .is_err());
        let mut wrong_abi = image.fore().to_vec();
        let first = wrong_abi[0];
        let mut abi: [u8; 32] = first.payload()[21..].try_into().unwrap();
        abi[0] ^= 1;
        wrong_abi[0] = AssignedForeRequirement::new(
            first.endpoint(),
            first.cord(),
            first.direction(),
            AssignedIdentity(first.payload()[5..21].try_into().unwrap()),
            abi,
        );
        for pins in [&[][..], &image.fore()[..1], wrong_abi.as_slice()] {
            assert!(ready
                .check_assigned_fore(
                    &table,
                    &claim,
                    &contract,
                    SelectedAssignedForeOperationPlan {
                        operation: selected(valid_scope),
                        fore: pins
                    }
                )
                .is_err());
        }
        ready.check_assigned_fore(
            &table,
            &claim,
            &contract,
            SelectedAssignedForeOperationPlan {
                operation: selected(valid_scope),
                fore: image.fore(),
            },
        )
    } else {
        ready.check_assigned(&table, &claim, &contract, selected(valid_scope))
    };
    assert_eq!(checked.unwrap(), lowered.nodes[0].node);
    if let Some(image) = &fore_image {
        let (table, handle, claim) = issue();
        assert!(ReadyI2cBase::bind_assigned_fore_retained(
            &mut slot,
            table,
            handle,
            claim,
            &contract,
            SelectedAssignedForeOperationPlan {
                operation: selected(valid_scope),
                fore: &image.fore()[..1]
            }
        )
        .is_err());
        assert!(slot.is_some());
    }
    let (table, handle, claim) = issue();
    let cancelled = if let Some(image) = &fore_image {
        ReadyI2cBase::prepare_assigned_fore_retained(
            &mut slot,
            table,
            handle,
            claim,
            &contract,
            SelectedAssignedForeOperationPlan {
                operation: selected(valid_scope),
                fore: image.fore(),
            },
        )
    } else {
        ReadyI2cBase::prepare_assigned_retained(
            &mut slot,
            table,
            handle,
            claim,
            &contract,
            selected(valid_scope),
        )
    }
    .unwrap();
    assert_eq!(cancelled.node(), lowered.nodes[0].node);
    drop(cancelled);
    assert!(slot.is_some());
    let (table, handle, claim) = issue();
    let mut owner = bind(&mut slot, table, handle, claim, valid_scope).unwrap();
    assert!(slot.is_none());
    assert_eq!(owner.provider.effects, 0);
    let (table, handle, claim) = issue();
    assert!(bind(&mut slot, table, handle, claim, valid_scope).is_err());
    if body && fore && !borrowed {
        // Independent inert fixture, not a replacement physical generation.
        let mut root_slot = Some(
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
        assigned_root::run(
            &mut root_slot,
            &contract,
            SelectedAssignedForeOperationPlan {
                operation: selected(valid_scope),
                fore: fore_image.as_ref().unwrap().fore(),
            },
            issue,
        );
    }
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

#[allow(
    dead_code,
    unused_imports,
    reason = "this fixture exercises native-only Root; mixed-profile exports are qualified by their owning tests"
)]
#[path = "../../../../../../targets/rp2040/firmware/host/src/native_preparation.rs"]
mod root_native_preparation;

#[path = "assigned_root.rs"]
mod assigned_root;
