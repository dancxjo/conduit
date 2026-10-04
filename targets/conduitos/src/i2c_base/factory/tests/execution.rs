use super::common::possession;
use super::*;
use crate::protocol_play::PreparedProtocolPlay;
use alloc::sync::Arc;
use conduit_composite::*;
use core::sync::atomic::{AtomicUsize, Ordering};

struct Scripted {
    effects: Arc<AtomicUsize>,
    address: u8,
    register: u8,
    response: u8,
}
impl I2cProvider for Scripted {
    fn transact(
        &mut self,
        transaction: &I2cTransaction<'_>,
        input: &mut [u8],
    ) -> Result<usize, I2cDisposition> {
        assert_eq!(transaction.address(), self.address);
        assert_eq!(transaction.write(), &[self.register]);
        assert_eq!(input.len(), 1);
        self.effects.fetch_add(1, Ordering::SeqCst);
        input[0] = self.response;
        Ok(1)
    }
    fn revoke(&mut self) {
        self.effects.fetch_add(100, Ordering::SeqCst);
    }
}

#[test]
fn checked_register_plot_runs_through_kernel_and_retained_bus_owner() {
    run_register(Wrapping::None);
}

#[test]
fn checked_selector_feeds_register_plot_through_the_same_native_kernel() {
    run_register(Wrapping::Field);
}

#[test]
fn checked_variant_routing_feeds_register_plot() {
    run_register(Wrapping::Read);
}
#[test]
fn checked_unmatched_variant_drains_without_a_bus_effect() {
    run_register(Wrapping::Drop);
}
#[derive(Clone, Copy)]
enum Wrapping {
    None,
    Field,
    Read,
    Drop,
    Bme,
}
#[test]
fn bme280_source_initializer_and_action_issue_identity_read_through_native_kernel() {
    run_register(Wrapping::Bme);
}

fn run_register(wrapping: Wrapping) {
    let wrapped = !matches!(wrapping, Wrapping::None | Wrapping::Bme);
    let bme = matches!(wrapping, Wrapping::Bme);
    let dropped = matches!(wrapping, Wrapping::Drop);

    let effects = Arc::new(AtomicUsize::new(0));
    let provider = || Scripted {
        effects: effects.clone(),
        address: if bme { 0x76 } else { 0x55 },
        register: if bme { 0xd0 } else { 0x12 },
        response: if bme { 0x60 } else { 0xa5 },
    };
    let source = include_str!("../../../../../../plots/device-protocols/i2c-register.conduit");
    let declaration = if matches!(wrapping, Wrapping::Field) {
        "type WrappedRead = {\n query: I2cRegisterRead\n}\ntype I2cRegisterWrite"
    } else {
        "type WrappedRead =\n read I2cRegisterRead\n | idle\ntype I2cRegisterWrite"
    };
    let selector = if matches!(wrapping, Wrapping::Field) {
        "    query >> project(WrappedRead.query) >> ({"
    } else {
        "    query >> select(WrappedRead.read, unmatched=drop) >> ({"
    };
    let wrapped_source = source
        .replace("type I2cRegisterWrite", declaration)
        .replace("query: I2cRegisterRead...|", "query: WrappedRead...|")
        .replacen("    query >> ({", selector, 1);
    let source = if bme {
        BME_FIRST_CALL_SOURCE
    } else if wrapped {
        wrapped_source.as_str()
    } else {
        source
    };
    let name = if bme {
        "bme280-first-call"
    } else {
        "i2c-register-read"
    };
    let (plan, ready, identity) = planned_named_source(provider(), source, name);
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
    let (table, handle, claim) = possession(scope.clone());
    let boundary = super::common::boundary(fragment);
    assert_eq!(boundary.input_fronts.len(), 1);
    assert_eq!(boundary.output_fronts.len(), 1);
    let input_port = boundary.input_fronts[0].external_port.clone();
    let output_port = boundary.output_fronts[0].external_port.clone();
    let expression = fragment
        .placements
        .iter()
        .find(|gear| {
            gear.implementation_id.as_str() == expression_host_call::IMPLEMENTATION
                && (!bme
                    || fragment.fore_ports.iter().any(|fore| {
                        fore.direction == PortDirection::Input
                            && fore.placement_id == gear.placement_id
                    }))
        })
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
    let selector_input = fragment
        .placements
        .iter()
        .find(|gear| {
            gear.implementation_id.as_str() == crate::structured_selector_host_call::IMPLEMENTATION
        })
        .map(|gear| {
            let ConfigurationValue::Text(encoded) = &gear.configuration[0].value else {
                panic!("selector")
            };
            StructuredSelector::from_canonical_hex(encoded)
                .unwrap()
                .input_type()
                .clone()
        })
        .filter(|_| wrapped);
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
    let mut wrong_route = definition.clone();
    wrong_route.boundary.input_fronts[0].internal_port_id = port_id("forged");
    let mut stale_boot = definition.clone();
    stale_boot.boot_id = BootId::from("boot/stale");
    let mut duplicated_front = definition.clone();
    duplicated_front
        .boundary
        .input_fronts
        .push(duplicated_front.boundary.input_fronts[0].clone());
    for refused in [wrong_route, stale_boot, duplicated_front] {
        let (_, inert_ready, _) = planned_named_source(provider(), source, name);
        let (refused_table, refused_handle, refused_claim) = possession(scope.clone());
        assert!(matches!(
            PreparedProtocolPlay::prepare(
                refused,
                inert_ready,
                refused_table,
                refused_handle,
                refused_claim
            ),
            Err(crate::protocol_host_calls::ProtocolCallRefusal::InvalidPlan)
        ));
        assert_eq!(effects.load(Ordering::SeqCst), 0);
    }
    let mut play = PreparedProtocolPlay::prepare(definition, ready, table, handle, claim).unwrap();
    play.start().unwrap();
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
                        if bme { 0x76 } else { 0x55 }
                    } else {
                        0x12
                    }],
                )
                .unwrap(),
            )
            .unwrap()
        })
        .collect();
    let query = StructuredInfoValue::record(ty, values).unwrap();
    let query = if let Some(ty) = selector_input {
        if matches!(wrapping, Wrapping::Field) {
            StructuredInfoValue::record(
                ty,
                vec![StructuredFieldValue::new("query", query).unwrap()],
            )
            .unwrap()
        } else if dropped {
            let unit = StructuredInfoValue::leaf(
                StructuredInfoType::leaf(kind_id("value/unit")).unwrap(),
                vec![],
            )
            .unwrap();
            StructuredInfoValue::variant(ty, "idle", unit).unwrap()
        } else {
            StructuredInfoValue::variant(ty, "read", query).unwrap()
        }
    } else {
        query
    };
    let encoded = query.canonical_bytes().unwrap();
    play.admit_input(
        &input_port.port_id,
        0,
        &ValuePayload {
            value_kind: input_port.value_kind,
            encoded,
        },
    )
    .unwrap();
    play.close_input(&input_port.port_id).unwrap();
    let mut output = ValuePayload {
        value_kind: output_port.value_kind,
        encoded: alloc::vec::Vec::with_capacity(I2C_MAXIMUM_BYTES as usize),
    };
    let mut sequence = None;
    let mut status = KernelCompositeStatus::Active;
    for _ in 0..64 {
        status = play.step().unwrap();
        sequence = play.output_into(&output_port.port_id, &mut output).unwrap();
        if sequence.is_some() {
            break;
        }
    }
    if dropped {
        assert_eq!(sequence, None);
        assert_eq!(status, KernelCompositeStatus::Complete);
        assert_eq!(effects.load(Ordering::SeqCst), 0);
        play.cancel().unwrap();
        assert_eq!(effects.load(Ordering::SeqCst), 100);
        return;
    }
    assert_eq!(sequence, Some(0));
    assert_eq!(effects.load(Ordering::SeqCst), 1);
    let mut encoder =
        crate::i2c_base::result::PreparedI2cResultEncoder::new(&I2cContract::prepare().unwrap())
            .unwrap();
    assert_eq!(
        output.encoded,
        encoder
            .completed(1, &[if bme { 0x60 } else { 0xa5 }])
            .unwrap()
    );
    let result = StructuredInfoValue::from_canonical_bytes(&output.encoded).unwrap();
    assert!(
        matches!(result.shape(), StructuredInfoValueShape::Variant { tag, .. } if tag == "completed")
    );
    for _ in 0..100 {
        play.step().unwrap();
    }
    assert_eq!(effects.load(Ordering::SeqCst), 1);
    play.complete_output(&output_port.port_id, 0).unwrap();
    play.cancel().unwrap();
    assert_eq!(effects.load(Ordering::SeqCst), 101);
}
