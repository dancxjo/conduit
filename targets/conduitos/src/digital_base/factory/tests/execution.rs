//! Ordinary checked Source through the production kernel and retained contact owner.
use super::*;
use crate::protocol_play::{DigitalAdmission, PeripheralAdmission, PreparedPeripheralPlay};
use alloc::{rc::Rc, vec::Vec};
use core::cell::Cell;

struct Scripted {
    effects: Rc<Cell<u32>>,
    revoked: bool,
}
impl DigitalProvider for Scripted {
    fn access(&mut self, request: DigitalRequest) -> Result<bool, DigitalDisposition> {
        assert_eq!(request, DigitalRequest::Drive(true));
        self.effects.set(self.effects.get() + 1);
        Ok(true)
    }
    fn revoke(&mut self) {
        if !self.revoked {
            self.effects.set(self.effects.get() + 100);
            self.revoked = true;
        }
    }
}
fn admission() -> (
    conduit_composite::KernelCompositeDefinition,
    PeripheralAdmission,
    Rc<Cell<u32>>,
) {
    let effects = Rc::new(Cell::new(0));
    let (plan, ready, identity, mut external) = planned(Scripted {
        effects: effects.clone(),
        revoked: false,
    });
    assert_eq!(effects.get(), 0);
    let fragment = &plan.fragments[0];
    let gear = &fragment.placements[0];
    let active = bind_active_play(&plan.plan_id, &fragment.host_id, &fragment.boot_id, 0);
    let scope = BaseCapabilityScope {
        host_id: identity.host_id,
        boot_id: identity.boot_id,
        base_instance_id: identity.provider_instance_id,
        base_provider_generation: identity.provider_generation,
        plan_id: plan.plan_id.clone(),
        active_play_id: active.active_play_id,
        authority_grant_id: gear.authority[0].grant_id.clone(),
        authority_contract_id: AuthorityContractId::from(DIGITAL_AUTHORITY),
        capability_id: gear.capability_id.clone(),
        implementation_id: gear.implementation_id.clone(),
        operation_contract_id: HostCallContractId::from(DIGITAL_CALL),
        subject_kind: gear.kind_id.clone(),
        resource_pool_id: identity.resource_pool_id,
        resource_generation_id: identity.resource_generation_id,
        envelope_id: identity.envelope_id,
        maximum_parameter_bytes: DIGITAL_MAXIMUM_BYTES,
        maximum_result_bytes: DIGITAL_MAXIMUM_BYTES,
        maximum_work_units: 1,
        maximum_in_flight: 1,
        maximum_operations: 2,
    };
    let (table, handle, claim) = crate::protocol_kernel_fixture::possession(scope);
    let admission = PeripheralAdmission {
        spi: None,
        digital: vec![DigitalAdmission {
            placement: gear.placement_id.clone(),
            ready: ready.erase(),
            table,
            handle,
            claim,
        }],
    };
    external.host_calls.clear();
    external.resource_requirements.clear();
    external.authority_requirements.clear();
    external.shorthand = None;
    let definition = crate::protocol_kernel_fixture::definition(plan, external);
    (definition, admission, effects)
}

#[test]
fn checked_digital_source_executes_in_the_existing_kernel_without_allocating() {
    let (definition, admission, effects) = admission();
    let input_port = definition.boundary.input_fronts[0].external_port.clone();
    let output_port = definition.boundary.output_fronts[0].external_port.clone();
    let mut play = PreparedPeripheralPlay::prepare_peripheral(
        definition,
        admission,
        crate::protocol_operations::ProtocolOperations::default(),
    )
    .unwrap();
    assert_eq!(effects.get(), 0);
    play.start().unwrap();
    let request = crate::digital_base::tests::request(
        &DigitalContract::prepare().unwrap(),
        DigitalRequest::Drive(true),
    );
    play.admit_input(
        &input_port.port_id,
        0,
        &ValuePayload {
            value_kind: input_port.value_kind,
            encoded: request,
        },
    )
    .unwrap();
    play.close_input(&input_port.port_id).unwrap();
    let mut output = ValuePayload {
        value_kind: output_port.value_kind,
        encoded: Vec::with_capacity(DIGITAL_MAXIMUM_BYTES as usize),
    };
    let mut sequence = None;
    let allocations = crate::test_allocations::allocations(|| {
        for _ in 0..64 {
            play.step().unwrap();
            sequence = play.output_into(&output_port.port_id, &mut output).unwrap();
            if sequence.is_some() {
                break;
            }
        }
    });
    assert_eq!(allocations, 0);
    assert_eq!(sequence, Some(0));
    assert_eq!(effects.get(), 1);
    let mut encoder = crate::digital_base::result::PreparedDigitalResultEncoder::new(
        &DigitalContract::prepare().unwrap(),
    )
    .unwrap();
    assert_eq!(output.encoded, encoder.completed(true).unwrap());
    play.complete_output(&output_port.port_id, 0).unwrap();
    play.cancel().unwrap();
    assert_eq!(effects.get(), 101);
    drop(play);
    assert_eq!(effects.get(), 101);
}

#[test]
fn preparation_failure_revokes_retained_physical_ownership() {
    for failure in 0..3 {
        let (mut definition, mut admission, effects) = admission();
        match failure {
            0 => definition.boot_id = BootId::from("boot/stale"),
            1 => admission.digital[0].claim.boot_id = BootId::from("boot/stale"),
            _ => admission.digital[0].claim.parameter_bytes += 1,
        }
        let result = PreparedPeripheralPlay::prepare_peripheral(
            definition,
            admission,
            crate::protocol_operations::ProtocolOperations::default(),
        );
        assert!(result.is_err(), "failure fixture {failure} must refuse");
        assert_eq!(
            effects.get(),
            100,
            "failure fixture {failure} must revoke without traffic"
        );
    }
}

#[test]
fn dropping_prepared_play_revokes_its_physical_owner() {
    let (definition, admission, effects) = admission();
    let play = PreparedPeripheralPlay::prepare_peripheral(
        definition,
        admission,
        crate::protocol_operations::ProtocolOperations::default(),
    )
    .unwrap();
    assert_eq!(effects.get(), 0);
    drop(play);
    assert_eq!(effects.get(), 100);
}
