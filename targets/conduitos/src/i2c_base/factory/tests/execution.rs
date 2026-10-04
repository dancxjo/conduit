use super::*;
use crate::{
    expression_host_call::ExpressionOperationFactory, protocol_host_calls::PreparedProtocolCalls,
};
use alloc::sync::Arc;
use conduit_composite::*;
use conduit_plot::CompositeFrontTerminal;
use core::sync::atomic::{AtomicUsize, Ordering};

struct Scripted(Arc<AtomicUsize>);
impl I2cProvider for Scripted {
    fn transact(
        &mut self,
        transaction: &I2cTransaction<'_>,
        input: &mut [u8],
    ) -> Result<usize, I2cDisposition> {
        assert_eq!(transaction.address(), 0x55);
        assert_eq!(transaction.write(), &[0x12]);
        assert_eq!(input.len(), 1);
        self.0.fetch_add(1, Ordering::SeqCst);
        input[0] = 0xa5;
        Ok(1)
    }
    fn revoke(&mut self) {
        self.0.fetch_add(100, Ordering::SeqCst);
    }
}

#[test]
fn checked_register_plot_runs_through_kernel_and_retained_bus_owner() {
    let effects = Arc::new(AtomicUsize::new(0));
    let (plan, ready, identity) = planned(Scripted(effects.clone()));
    let fragment = &plan.fragments[0];
    let gear = fragment
        .placements
        .iter()
        .find(|gear| gear.implementation_id.as_str() == I2C_IMPLEMENTATION)
        .unwrap();
    let active = bind_active_play(&plan.plan_id, &fragment.host_id, &fragment.boot_id, 0);
    let scope = BaseCapabilityScope {
        host_id: identity.host_id.clone(),
        boot_id: identity.boot_id.clone(),
        base_instance_id: identity.provider_instance_id.clone(),
        base_provider_generation: identity.provider_generation,
        plan_id: plan.plan_id.clone(),
        active_play_id: active.active_play_id,
        authority_grant_id: gear.authority[0].grant_id.clone(),
        authority_contract_id: AuthorityContractId::from(I2C_AUTHORITY),
        capability_id: gear.capability_id.clone(),
        implementation_id: gear.implementation_id.clone(),
        operation_contract_id: HostCallContractId::from(I2C_CALL),
        subject_kind: gear.kind_id.clone(),
        resource_pool_id: identity.resource_pool_id,
        resource_generation_id: identity.resource_generation_id,
        envelope_id: identity.envelope_id,
        maximum_parameter_bytes: I2C_MAXIMUM_BYTES,
        maximum_result_bytes: I2C_MAXIMUM_BYTES,
        maximum_work_units: 1,
        maximum_in_flight: 1,
        maximum_operations: 2,
    };
    let authority = BaseCapabilityAuthority {
        grant: AuthorityGrant {
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
        maximum_in_flight: 1,
        maximum_operations: scope.maximum_operations,
    };
    let claim = BaseOperationClaim {
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
        parameter_bytes: I2C_MAXIMUM_BYTES,
        work_units: 1,
    };
    let mut table = BaseCapabilityTable::new(
        scope.host_id.clone(),
        scope.boot_id.clone(),
        scope.base_instance_id.clone(),
        scope.base_provider_generation,
        [7; 32],
        1,
    )
    .unwrap();
    let handle = table
        .issue(CapabilityIssueRequest { scope, authority })
        .unwrap();

    let mut dispatcher =
        PreparedProtocolCalls::prepare(&plan, ready, table, handle, claim).unwrap();
    let mut boundary = KernelCompositeBoundary {
        input_fronts: vec![],
        output_fronts: vec![],
    };
    for fore in &fragment.fore_ports {
        let placement = fragment
            .placements
            .iter()
            .find(|gear| gear.placement_id == fore.placement_id)
            .unwrap();
        let ports = if fore.direction == PortDirection::Input {
            &placement.inputs
        } else {
            &placement.outputs
        };
        let mut port = ports
            .iter()
            .find(|port| port.port_id == fore.gear_port_id)
            .unwrap()
            .clone();
        port.port_id = fore.front_port_id.clone();
        let front = KernelCompositeFrontBinding {
            external_port: port,
            internal_child: fragment.host_id.clone(),
            internal_placement_id: fore.placement_id.clone(),
            internal_port_id: fore.gear_port_id.clone(),
            terminal: CompositeFrontTerminal::Independent,
        };
        if fore.direction == PortDirection::Input {
            boundary.input_fronts.push(front);
        } else {
            boundary.output_fronts.push(front);
        }
    }
    assert_eq!(boundary.input_fronts.len(), 1);
    assert_eq!(boundary.output_fronts.len(), 1);
    let input_port = boundary.input_fronts[0].external_port.clone();
    let output_port = boundary.output_fronts[0].external_port.clone();
    let expression = fragment
        .placements
        .iter()
        .find(|gear| gear.implementation_id.as_str() == expression_host_call::IMPLEMENTATION)
        .unwrap();
    let ConfigurationValue::Text(encoded) = &expression.configuration[0].value else {
        panic!("program")
    };
    let program = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
    let mut external =
        expression_host_call::offer(&program, PortTemporal::Flow { closes: true }).unwrap();
    external.inputs = vec![input_port.clone()];
    external.outputs = vec![output_port.clone()];
    external.host_calls.clear();
    external.limits.max_queue_items = 1;
    external.limits.max_queue_bytes = I2C_MAXIMUM_BYTES;
    // Fixture wrapper exposes only the exact sealed ordinary Source fore ports.
    let definition = KernelCompositeDefinition {
        host_id: fragment.host_id.clone(),
        boot_id: fragment.boot_id.clone(),
        offer_generation: fragment.offer_generation,
        profile: HostProfileId::from("fixture/native-register@1"),
        external_capability: external,
        internal_plan: plan,
        boundary,
        failure_translation: FailureReason::CompositeCapabilityFailed,
    };
    let mut registry = KernelOperationRegistry::new();
    registry
        .install(ExpressionOperationFactory::default())
        .unwrap();
    registry
        .install(I2cOperationFactory::prepare_contract().unwrap())
        .unwrap();
    let mut kernel = KernelCompositeHost::prepare(definition, &registry).unwrap();
    kernel.start().unwrap();
    let ty = program.input_type.clone();
    let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        panic!("query")
    };
    let values = fields
        .iter()
        .map(|field| {
            StructuredFieldValue::new(
                field.name(),
                StructuredInfoValue::leaf(
                    field.value_type().clone(),
                    vec![if field.name() == "address" {
                        0x55
                    } else {
                        0x12
                    }],
                )
                .unwrap(),
            )
            .unwrap()
        })
        .collect();
    let encoded = StructuredInfoValue::record(ty, values)
        .unwrap()
        .canonical_bytes()
        .unwrap();
    kernel
        .admit_input(
            &input_port.port_id,
            0,
            &ValuePayload {
                value_kind: input_port.value_kind,
                encoded,
            },
        )
        .unwrap();
    kernel.close_input(&input_port.port_id).unwrap();
    let mut output = ValuePayload {
        value_kind: output_port.value_kind,
        encoded: alloc::vec::Vec::with_capacity(I2C_MAXIMUM_BYTES as usize),
    };
    let mut calls = 0;
    let mut sequence = None;
    for _ in 0..64 {
        kernel.step().unwrap();
        if let Some(request) = kernel.next_host_request() {
            dispatcher.dispatch(&mut kernel, &request).unwrap();
            calls += 1;
        }
        sequence = kernel
            .output_into(&output_port.port_id, &mut output)
            .unwrap();
        if sequence.is_some() {
            break;
        }
    }
    assert_eq!(calls, 2);
    assert_eq!(sequence, Some(0));
    assert_eq!(effects.load(Ordering::SeqCst), 1);
    let mut encoder =
        crate::i2c_base::result::PreparedI2cResultEncoder::new(&I2cContract::prepare().unwrap())
            .unwrap();
    assert_eq!(output.encoded, encoder.completed(1, &[0xa5]).unwrap());
    let result = StructuredInfoValue::from_canonical_bytes(&output.encoded).unwrap();
    assert!(
        matches!(result.shape(), StructuredInfoValueShape::Variant { tag, .. } if tag == "completed")
    );
    for _ in 0..100 {
        kernel.step().unwrap();
        assert!(kernel.next_host_request().is_none());
    }
    assert_eq!(effects.load(Ordering::SeqCst), 1);
    kernel.complete_output(&output_port.port_id, 0).unwrap();
    dispatcher.cancel(&mut kernel).unwrap();
    assert_eq!(effects.load(Ordering::SeqCst), 101);
}
