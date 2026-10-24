//! Explicit CPU Root authority fixture; the production owner services requests.
use conduit_core as c;
use conduit_tm1637_wire::{installation::*, owner::*, *};
use std::{cell::Cell, rc::Rc};
pub struct Pins(pub Rc<Cell<usize>>);
impl OpenDrain for Pins {
    type Error = core::convert::Infallible;
    fn clock(&mut self, _: bool) -> Result<(), Self::Error> {
        self.0.set(self.0.get() + 1);
        Ok(())
    }
    fn data(&mut self, _: bool) -> Result<(), Self::Error> {
        self.0.set(self.0.get() + 1);
        Ok(())
    }
    fn read_data(&mut self) -> Result<bool, Self::Error> {
        Ok(false)
    }
    fn half_period(&mut self) -> Result<(), Self::Error> {
        Ok(())
    }
}
pub fn bind(
    prepared: &conduit_environment_make::PreparedDisplayOutput,
    pins: Pins,
) -> WriteHostCall<Pins> {
    bind_profile(prepared, pins, false)
}
pub fn bind_fore(
    prepared: &conduit_environment_make::PreparedDisplayOutput,
    pins: Pins,
) -> WriteHostCall<Pins> {
    bind_profile(prepared, pins, true)
}
fn bind_profile(
    prepared: &conduit_environment_make::PreparedDisplayOutput,
    pins: Pins,
    fore: bool,
) -> WriteHostCall<Pins> {
    let fragment = &prepared.plan().fragments[0];
    let gear = fragment
        .placements
        .iter()
        .find(|p| p.kind_id.as_str() == WRITE_KIND)
        .unwrap();
    let active = c::bind_active_play(&fragment.plan_id, &fragment.host_id, &fragment.boot_id, 0);
    let scope = c::BaseCapabilityScope {
        host_id: fragment.host_id.clone(),
        boot_id: fragment.boot_id.clone(),
        base_instance_id: "fixture/provider".into(),
        base_provider_generation: 1,
        plan_id: fragment.plan_id.clone(),
        active_play_id: active.active_play_id.clone(),
        authority_grant_id: "fixture/display".into(),
        authority_contract_id: WRITE_AUTHORITY.into(),
        capability_id: WRITE_CAPABILITY.into(),
        implementation_id: "fixture/driver".into(),
        operation_contract_id: WRITE_CALL.into(),
        subject_kind: c::kind_id(WRITE_KIND),
        resource_pool_id: "fixture/pins".into(),
        resource_generation_id: c::ResourceGenerationId("fixture/generation".into()),
        envelope_id: "fixture/envelope".into(),
        maximum_parameter_bytes: MAXIMUM_BYTES,
        maximum_result_bytes: 0,
        maximum_work_units: 8,
        maximum_in_flight: 1,
        maximum_operations: 2,
    };
    let authority = c::BaseCapabilityAuthority {
        grant: c::AuthorityGrant {
            grant_id: scope.authority_grant_id.clone(),
            contract_id: scope.authority_contract_id.clone(),
            host_call_contract_id: scope.operation_contract_id.clone(),
            subject_kind: scope.subject_kind.clone(),
            host_id: scope.host_id.clone(),
            boot_id: scope.boot_id.clone(),
            capability_id: scope.capability_id.clone(),
        },
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
    let claim = c::BaseOperationClaim {
        host_id: scope.host_id.clone(),
        boot_id: scope.boot_id.clone(),
        base_instance_id: scope.base_instance_id.clone(),
        base_provider_generation: scope.base_provider_generation,
        plan_id: scope.plan_id.clone(),
        active_play_id: scope.active_play_id.clone(),
        implementation_id: scope.implementation_id.clone(),
        operation_contract_id: scope.operation_contract_id.clone(),
        subject_kind: scope.subject_kind.clone(),
        resource_pool_id: scope.resource_pool_id.clone(),
        resource_generation_id: scope.resource_generation_id.clone(),
        envelope_id: scope.envelope_id.clone(),
        parameter_bytes: MAXIMUM_BYTES,
        work_units: 1,
    };
    let mut table = c::BaseCapabilityTable::new(
        scope.host_id.clone(),
        scope.boot_id.clone(),
        scope.base_instance_id.clone(),
        1,
        [7; 32],
        1,
    )
    .unwrap();
    let handle = table
        .issue(c::CapabilityIssueRequest { scope, authority })
        .unwrap();
    if fore {
        return bind_fore_owner(prepared, pins, table, handle, claim, &active);
    }
    // SAFETY: the fixture exclusively owns Pins and explicitly issues current
    // Plan/Play authority above. This proves no physical pins or Root admission.
    unsafe {
        WriteHostCall::bind_selected(
            table,
            handle,
            claim,
            WriteAttachment {
                generation: 1,
                resource_bytes: 64,
            },
            pins,
            &WriteContract::prepare().unwrap(),
            conduit_plan_lowering::selected_operation::SelectedOperationPlan {
                fragment,
                lowered: prepared.lowered(),
                active: &active,
                placement_id: &gear.placement_id,
            },
        )
        .unwrap()
    }
}

fn bind_fore_owner(
    prepared: &conduit_environment_make::PreparedDisplayOutput,
    pins: Pins,
    table: c::BaseCapabilityTable,
    handle: c::BaseCapabilityHandle,
    claim: c::BaseOperationClaim,
    active: &c::ActivePlayIdentity,
) -> WriteHostCall<Pins> {
    use conduit_embedded_build as build;
    use conduit_plan_lowering::selected_assigned_operation::*;
    let fragment = &prepared.plan().fragments[0];
    let generated = build::generate_fore_native_plan(
        prepared.plan(),
        &fragment.fragment_id,
        build::EmbeddedImageBounds::HOST_TOOLING,
        build::EmbeddedComputationBounds {
            maximum_program_bytes: 32 * 1024,
            maximum_total_program_bytes: 96 * 1024,
        },
        2,
    )
    .unwrap();
    let maxima = c::AssignedPlanMaxima {
        encoded_bytes: 16 * 1024,
        runtime_state_bytes: u16::MAX,
        counts: [32; c::ASSIGNED_PLAN_COUNT_KINDS],
    };
    let bytes = generated.encode(maxima).unwrap();
    let records = c::AssignedPlanRecords::new(&bytes)
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    let calls: Vec<_> = generated
        .kernel()
        .host_calls
        .iter()
        .map(|c| c::AssignedIdentity::from_text(&c.contract_id))
        .collect();
    let resources: Vec<_> = generated
        .kernel()
        .resources
        .iter()
        .map(|r| r.resource)
        .collect();
    let computations = build::assigned_computation_requirements(generated.kernel()).unwrap();
    let requirements = c::AssignedPlanRequirements {
        host: c::AssignedIdentity::from_text(fragment.host_id.as_str()),
        boot: c::AssignedIdentity::from_text(fragment.boot_id.as_str()),
        host_calls: &calls,
        resources: &resources,
        remote_bindings: &[],
    };
    let node = conduit_kernel::NodeId(
        fragment
            .placements
            .iter()
            .position(|p| p.kind_id.as_str() == WRITE_KIND)
            .unwrap() as u16,
    );
    let selected = || SelectedAssignedOperationPlan {
        input: AssignedOperationInput::Records {
            header: &bytes[..125],
            records: &records,
        },
        maxima,
        requirements,
        computations: c::AssignedComputationProfile {
            maximum_text_bytes: u16::MAX,
            maximum_total_text_bytes: u32::MAX,
            requirements: &computations,
        },
        fragment_id: &fragment.fragment_id,
        active,
        node,
    };
    let identity = WriteNativeIdentity {
        host_id: fragment.host_id.clone(),
        boot_id: fragment.boot_id.clone(),
        base_id: "fixture/pins".into(),
        provider_instance_id: "fixture/provider".into(),
        provider_generation: 1,
        resource_pool_id: "fixture/pins".into(),
        resource_generation_id: c::ResourceGenerationId("fixture/generation".into()),
        envelope_id: "fixture/envelope".into(),
        artifact_id: "fixture/driver".into(),
        implementation_id: "fixture/driver".into(),
        base_implementation_id: "fixture/base".into(),
    };
    let edges = pins.0.clone();
    // SAFETY: this explicit CPU Root fixture transfers its actual counter pins.
    // No descriptor or successful probe constructs their initialized owner.
    let mut slot = Some(
        unsafe {
            ReadyWriteBase::new(
                identity,
                WriteAttachment {
                    generation: 1,
                    resource_bytes: 64,
                },
                pins,
            )
        }
        .unwrap(),
    );
    let contract = WriteContract::prepare().unwrap();
    assert!(slot
        .as_ref()
        .unwrap()
        .check_assigned(&table, &claim, &contract, selected())
        .is_err());
    assert!(slot
        .as_ref()
        .unwrap()
        .check_assigned_fore(
            &table,
            &claim,
            &contract,
            SelectedAssignedForeOperationPlan {
                operation: selected(),
                fore: &generated.fore()[..1]
            }
        )
        .is_err());
    let empty = c::BaseCapabilityTable::new(
        claim.host_id.clone(),
        claim.boot_id.clone(),
        claim.base_instance_id.clone(),
        1,
        [7; 32],
        1,
    )
    .unwrap();
    assert!(slot
        .as_ref()
        .unwrap()
        .check_assigned_fore(
            &empty,
            &claim,
            &contract,
            SelectedAssignedForeOperationPlan {
                operation: selected(),
                fore: generated.fore()
            }
        )
        .is_err());
    assert_eq!(edges.get(), 0);
    let owner = ReadyWriteBase::bind_assigned_fore_retained(
        &mut slot,
        table,
        handle,
        claim,
        &contract,
        SelectedAssignedForeOperationPlan {
            operation: selected(),
            fore: generated.fore(),
        },
    )
    .unwrap();
    assert!(slot.is_none());
    assert_eq!(edges.get(), 0);
    owner
}
