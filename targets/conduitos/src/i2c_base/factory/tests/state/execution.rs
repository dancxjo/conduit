use super::*;
use alloc::vec::Vec;
use conduit_composite::*;
use conduit_plot::{CompositeFrontTerminal, PreparedPortableExpressionEvaluator};

fn source_state(address: u8) -> Vec<u8> {
    let contract = I2cContract::prepare().unwrap();
    let (startup, profile) = contract.catalogs();
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
        panic!("initializer program")
    };
    let program = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
    let StructuredInfoTypeShape::Record { fields, .. } = program.input_type.shape() else {
        panic!("begin record")
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
    PreparedPortableExpressionEvaluator::new(&program)
        .unwrap()
        .evaluate(&begin.canonical_bytes().unwrap())
        .unwrap()
        .to_vec()
}

#[test]
fn finite_state_sampler_runs_in_kernel_under_output_pressure_and_closes_after_drain() {
    let (plan, _, _) = planned_named_source(Provider, SOURCE, "native-state-sample");
    let fragment = &plan.fragments[0];
    let sampler = fragment
        .placements
        .iter()
        .find(|gear| gear.implementation_id.as_str() == crate::current_sample::IMPLEMENTATION)
        .unwrap();
    let contract_at = |name| {
        &sampler
            .semantic_contract
            .value_contracts()
            .iter()
            .find(|entry| entry.location == FrontValueLocation::Input(port_id(name)))
            .unwrap()
            .contract
    };
    let mut external =
        crate::current_sample::offer(contract_at("current"), contract_at("trigger")).unwrap();
    let mut boundary = KernelCompositeBoundary {
        input_fronts: vec![],
        output_fronts: vec![],
    };
    for fore in &fragment.fore_ports {
        let gear = fragment
            .placements
            .iter()
            .find(|gear| gear.placement_id == fore.placement_id)
            .unwrap();
        let ports = if fore.direction == PortDirection::Input {
            &gear.inputs
        } else {
            &gear.outputs
        };
        let mut port = ports
            .iter()
            .find(|port| port.port_id == fore.gear_port_id)
            .unwrap()
            .clone();
        port.port_id = fore.front_port_id.clone();
        let binding = KernelCompositeFrontBinding {
            external_port: port,
            internal_child: fragment.host_id.clone(),
            internal_placement_id: fore.placement_id.clone(),
            internal_port_id: fore.gear_port_id.clone(),
            terminal: CompositeFrontTerminal::Independent,
        };
        if fore.direction == PortDirection::Input {
            boundary.input_fronts.push(binding);
        } else {
            boundary.output_fronts.push(binding);
        }
    }
    // Fixture wrapper exposes the sealed Source fronts; this is kernel proof, not boot admission.
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
    let current = external
        .inputs
        .iter()
        .find(|port| port.port_id == port_id("current"))
        .unwrap()
        .clone();
    let trigger = external
        .inputs
        .iter()
        .find(|port| port.port_id == port_id("trigger"))
        .unwrap()
        .clone();
    let sampled = external
        .outputs
        .iter()
        .find(|port| port.port_id == port_id("sampled"))
        .unwrap()
        .clone();
    let definition = KernelCompositeDefinition {
        host_id: fragment.host_id.clone(),
        boot_id: fragment.boot_id.clone(),
        offer_generation: fragment.offer_generation,
        profile: HostProfileId::from("fixture/native-state@1"),
        external_capability: external,
        internal_plan: plan,
        boundary,
        failure_translation: FailureReason::CompositeCapabilityFailed,
    };
    let mut registry = KernelOperationRegistry::new();
    registry
        .install(crate::current_sample::CurrentSampleOperationFactory::default())
        .unwrap();
    registry
        .install(I2cOperationFactory::prepare_contract().unwrap())
        .unwrap();
    let mut play = KernelCompositeHost::prepare(definition, &registry).unwrap();
    play.start().unwrap();
    play.close_input(&port_id("request")).unwrap();
    let a = source_state(0x76);
    let b = source_state(0x77);
    assert_ne!(a, b);
    let mut output = ValuePayload {
        value_kind: sampled.value_kind.clone(),
        encoded: Vec::with_capacity(4096),
    };
    for (sequence, bytes) in [(0, &a), (1, &b)] {
        play.admit_input(
            &current.port_id,
            sequence,
            &ValuePayload {
                value_kind: current.value_kind.clone(),
                encoded: bytes.clone(),
            },
        )
        .unwrap();
        play.admit_input(
            &trigger.port_id,
            sequence,
            &ValuePayload {
                value_kind: trigger.value_kind.clone(),
                encoded: sequence.to_le_bytes().to_vec(),
            },
        )
        .unwrap();
        for _ in 0..32 {
            play.step().unwrap();
            assert!(play.next_host_request().is_none());
        }
        if sequence == 1 {
            assert_eq!(
                play.output_into(&sampled.port_id, &mut output).unwrap(),
                Some(0)
            );
            assert_eq!(output.encoded, a);
            play.complete_output(&sampled.port_id, 0).unwrap();
            for _ in 0..32 {
                play.step().unwrap();
                assert!(play.next_host_request().is_none());
            }
        }
        assert_eq!(
            play.output_into(&sampled.port_id, &mut output).unwrap(),
            Some(sequence)
        );
        assert_eq!(&output.encoded, bytes);
    }
    play.close_input(&trigger.port_id).unwrap();
    for _ in 0..32 {
        play.step().unwrap();
        assert!(play.next_host_request().is_none());
    }
    assert_eq!(
        play.output_into(&sampled.port_id, &mut output).unwrap(),
        Some(1)
    );
    assert_eq!(output.encoded, b);
    play.complete_output(&sampled.port_id, 1).unwrap();
    for _ in 0..32 {
        play.step().unwrap();
        assert!(play.next_host_request().is_none());
    }
    assert_eq!(
        play.output_into(&sampled.port_id, &mut output).unwrap(),
        None
    );
    assert_eq!(
        play.output_terminal_into(&sampled.port_id, &mut output)
            .unwrap(),
        Some(KernelCompositeTerminal::Normal)
    );
}
