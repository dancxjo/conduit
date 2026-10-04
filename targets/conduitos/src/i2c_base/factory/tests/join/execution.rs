use super::*;
use crate::protocol_play::PreparedProtocolPlay;
use alloc::vec::Vec;
use conduit_composite::*;
use conduit_plot::PreparedPortableExpressionEvaluator;

fn inputs(address: u8, now: u64) -> (Vec<u8>, Vec<u8>) {
    let (startup, profile) = I2cContract::prepare().unwrap().catalogs();
    let checked = check_syntax_document(
        &parse_syntax_document(include_str!(
            "../../../../../../../plots/device-protocols/bme280-lifecycle.conduit"
        )),
        &startup,
    )
    .unwrap();
    let expanded =
        expand_canonical_plot_for_authoring(&checked, "bme280-protocol-initialize", &profile)
            .unwrap();
    let ConfigurationValue::Text(encoded) = &expanded.expanded.gears[0].configuration[0].value
    else {
        panic!("initializer");
    };
    let program = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
    let StructuredInfoTypeShape::Record { fields, .. } = program.input_type.shape() else {
        panic!("begin");
    };
    let begin = StructuredInfoValue::record(
        program.input_type.clone(),
        vec![
            StructuredFieldValue::new(
                "address",
                StructuredInfoValue::leaf(fields[0].value_type().clone(), vec![address]).unwrap(),
            )
            .unwrap(),
        ],
    )
    .unwrap();
    let state = PreparedPortableExpressionEvaluator::new(&program)
        .unwrap()
        .evaluate(&begin.canonical_bytes().unwrap())
        .unwrap()
        .to_vec();
    let event_type = checked
        .native_types
        .iter()
        .find(|ty| ty.name == "BmeProtocolEvent")
        .unwrap()
        .value_type
        .clone();
    let StructuredInfoTypeShape::Variant { cases, .. } = event_type.shape() else {
        panic!("event");
    };
    let tick = cases
        .iter()
        .find(|case| case.tag() == "tick")
        .unwrap()
        .payload_type()
        .clone();
    let event = StructuredInfoValue::variant(
        event_type,
        "tick",
        StructuredInfoValue::leaf(tick, now.to_le_bytes().to_vec()).unwrap(),
    )
    .unwrap()
    .canonical_bytes()
    .unwrap();
    (state, event)
}

fn assert_pair(output: &ValuePayload, state: &[u8], event: &[u8], exposed: bool) {
    let value = StructuredInfoValue::from_canonical_bytes(&output.encoded).unwrap();
    let StructuredInfoValueShape::Record(fields) = value.shape() else {
        panic!("transition");
    };
    let names = if exposed {
        ["item-00000", "item-00001"]
    } else {
        ["state", "event"]
    };
    for (name, expected) in [(names[0], state), (names[1], event)] {
        assert_eq!(
            fields
                .iter()
                .find(|field| field.name() == name)
                .unwrap()
                .value()
                .canonical_bytes()
                .unwrap(),
            expected
        );
    }
}

#[test]
fn bme280_source_state_event_join_runs_in_native_kernel_and_drains_without_bus_effects() {
    run_join(false);
}

#[test]
fn retained_native_join_exposes_a_typed_pair_without_a_downstream_schema_owner() {
    run_join(true);
}

fn run_join(exposed: bool) {
    let (mut startup, mut profile) = I2cContract::prepare().unwrap().catalogs();
    let (offer, _) = install(&mut startup, &mut profile);
    let pair_bound = offer
        .semantic_contract
        .value_contracts()
        .iter()
        .find(|entry| entry.location == FrontValueLocation::Output(port_id("paired")))
        .unwrap()
        .contract
        .maximum_bytes;
    let bare = alloc::format!(
        "{SOURCE}\nplot native-bare-pair (\n >> state: BmeProtocolState...| <= 4096B\n >> event: BmeProtocolEvent...| <= 4096B\n >> request: I2cTransaction...|\n paired: BmePair...| <= {pair_bound}B >>\n result: I2cResult...| >>\n) {{\n join: flow/zip/finite\n bus: machine/i2c/transact\n state >> join.left\n event >> join.right\n join.paired >> paired\n request >> bus >> result\n}}\n"
    );
    let source = if exposed { bare.as_str() } else { SOURCE };
    let name = if exposed {
        "native-bare-pair"
    } else {
        "native-state-event"
    };
    let (plan, ready, identity, joins) = planned_named_source_with_joins(Provider, source, name);
    let fragment = &plan.fragments[0];
    let bus = fragment
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
        authority_grant_id: bus.authority[0].grant_id.clone(),
        authority_contract_id: AuthorityContractId::from(I2C_AUTHORITY),
        capability_id: bus.capability_id.clone(),
        implementation_id: bus.implementation_id.clone(),
        operation_contract_id: HostCallContractId::from(I2C_CALL),
        subject_kind: bus.kind_id.clone(),
        resource_pool_id: identity.resource_pool_id,
        resource_generation_id: identity.resource_generation_id,
        envelope_id: identity.envelope_id,
        maximum_parameter_bytes: I2C_MAXIMUM_BYTES,
        maximum_result_bytes: I2C_MAXIMUM_BYTES,
        maximum_work_units: 1,
        maximum_in_flight: 1,
        maximum_operations: 2,
    };
    let (table, handle, claim) = super::super::common::possession(scope);
    let boundary = super::super::common::boundary(fragment);
    let state_port = boundary
        .input_fronts
        .iter()
        .find(|front| front.external_port.port_id == port_id("state"))
        .unwrap()
        .external_port
        .clone();
    let event_port = boundary
        .input_fronts
        .iter()
        .find(|front| front.external_port.port_id == port_id("event"))
        .unwrap()
        .external_port
        .clone();
    let output_port = boundary
        .output_fronts
        .iter()
        .find(|front| {
            front.external_port.port_id == port_id(if exposed { "paired" } else { "transition" })
        })
        .unwrap()
        .external_port
        .clone();
    let mut external = joins.offers().next().unwrap().clone();
    // Fixture wrapper exposes only sealed Source fronts; it is not boot admission.
    external.inputs = boundary
        .input_fronts
        .iter()
        .map(|front| front.external_port.clone())
        .collect();
    external.outputs = boundary
        .output_fronts
        .iter()
        .map(|front| front.external_port.clone())
        .collect();
    external.host_calls.clear();
    let definition = KernelCompositeDefinition {
        host_id: fragment.host_id.clone(),
        boot_id: fragment.boot_id.clone(),
        offer_generation: fragment.offer_generation,
        profile: HostProfileId::from("fixture/native-state-event@1"),
        external_capability: external,
        internal_plan: plan,
        boundary,
        failure_translation: FailureReason::CompositeCapabilityFailed,
    };
    let mut play =
        PreparedProtocolPlay::prepare_with_joins(definition, ready, table, handle, claim, joins)
            .unwrap();
    let (state_a, event_a) = inputs(0x76, 8);
    let (state_b, event_b) = inputs(0x77, 9);
    let payload = |port: &PortDescriptor, encoded: &[u8]| ValuePayload {
        value_kind: port.value_kind.clone(),
        encoded: encoded.to_vec(),
    };
    let mut output = ValuePayload {
        value_kind: output_port.value_kind.clone(),
        encoded: Vec::with_capacity(crate::flow_zip::MAXIMUM_PAIR_BYTES as usize),
    };
    play.start().unwrap();
    play.close_input(&port_id("request")).unwrap();
    play.admit_input(&event_port.port_id, 0, &payload(&event_port, &event_a))
        .unwrap();
    for _ in 0..32 {
        play.step().unwrap();
    }
    assert_eq!(
        play.output_into(&output_port.port_id, &mut output).unwrap(),
        None
    );
    play.admit_input(&state_port.port_id, 0, &payload(&state_port, &state_a))
        .unwrap();
    for _ in 0..32 {
        play.step().unwrap();
    }
    assert_eq!(
        play.output_into(&output_port.port_id, &mut output).unwrap(),
        Some(0)
    );
    assert_pair(&output, &state_a, &event_a, exposed);
    play.admit_input(&state_port.port_id, 1, &payload(&state_port, &state_b))
        .unwrap();
    play.admit_input(&event_port.port_id, 1, &payload(&event_port, &event_b))
        .unwrap();
    // Admit and form the second pair before closing either finite stream.
    // Zip closure intentionally discards a still-unmatched member.
    for _ in 0..64 {
        play.step().unwrap();
    }
    play.close_input(&state_port.port_id).unwrap();
    play.close_input(&event_port.port_id).unwrap();
    for _ in 0..64 {
        play.step().unwrap();
    }
    assert_eq!(
        play.output_into(&output_port.port_id, &mut output).unwrap(),
        Some(0)
    );
    assert_pair(&output, &state_a, &event_a, exposed);
    play.complete_output(&output_port.port_id, 0).unwrap();
    for _ in 0..64 {
        play.step().unwrap();
    }
    assert_eq!(
        play.output_into(&output_port.port_id, &mut output).unwrap(),
        Some(1)
    );
    assert_pair(&output, &state_b, &event_b, exposed);
    play.complete_output(&output_port.port_id, 1).unwrap();
    let mut status = KernelCompositeStatus::Active;
    for _ in 0..64 {
        status = play.step().unwrap();
    }
    assert_eq!(
        play.output_into(&output_port.port_id, &mut output).unwrap(),
        None
    );
    assert_eq!(
        play.kernel()
            .output_terminal_into(&output_port.port_id, &mut output)
            .unwrap(),
        Some(KernelCompositeTerminal::Normal)
    );
    assert_eq!(status, KernelCompositeStatus::Complete);
}
