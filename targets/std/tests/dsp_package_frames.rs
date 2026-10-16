//! Exact package Source through production local-expression and pair factories.
use conduit_core::*;
use conduit_plot::*;
use conduit_std_host::flow_activation::PreparedValuePairFactory;
use std::collections::BTreeMap;
#[path = "../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
#[path = "common_acoustic_value/kernel.rs"]
mod kernel;
fn prepare(entry: &str) -> (Plan, CheckedFront, PreparedValuePairFactory) {
    let package = conduit_semantic_catalog::prepare_dsp_source_package().unwrap();
    let stage = package
        .checked
        .native_types
        .iter()
        .find(|ty| ty.name == "SharedExcitationStage")
        .unwrap()
        .value_type
        .clone();
    let contract =
        CheckedValueContract::new(stage.profile().unwrap().value_kind().clone(), 32768, vec![])
            .unwrap();
    let authored =
        expand_canonical_plot_for_authoring(&package.checked, entry, &package.profiles).unwrap();
    let pair = PreparedValuePairFactory::new(&contract, &stage, &contract, &stage).unwrap();
    let mut offers = vec![pair.offer().clone()];
    let mut expression_capacity = 0;
    for gear in &authored.expanded.gears {
        if gear.kind_id.as_str() == conduit_semantic_catalog::VALUE_PAIR_KIND {
            continue;
        }
        let ConfigurationValue::Text(encoded) = &gear.configuration[0].value else {
            panic!("Source program")
        };
        let program = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
        expression_capacity = expression_capacity
            .max(program.maximum_prepared_input_bytes().unwrap())
            .max(program.maximum_prepared_output_bytes().unwrap());
        let offer = if gear.inputs[0].temporal == PortTemporal::Value {
            conduit_std_offers::pure_expression_local_offer(&program).unwrap()
        } else {
            conduit_std_offers::pure_expression_local_flow_offer(&program).unwrap()
        };
        assert!(gear.accepts_realization(&offer));
        assert!(offer.host_calls.is_empty());
        if !offers
            .iter()
            .any(|old| old.capability_id == offer.capability_id)
        {
            offers.push(offer);
        }
    }
    let mut host = conduit_std_host::StdHost::new().advertisement().clone();
    host.host_id = "fixture/acoustic-reference-child".into();
    host.capabilities = offers;
    let hosts = [host];
    let placements =
        conduit_planner::default_expanded_placements(&authored.expanded, &hosts).unwrap();
    let boundaries = authored
        .front
        .inputs()
        .iter()
        .chain(authored.front.outputs())
        .map(|port| {
            (
                conduit_planner::ForeBoundaryKey {
                    direction: port.direction,
                    front_port_id: port.port_id.clone(),
                    track: ConnectionTrack::Payload,
                },
                conduit_planner::ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity: 32768,
                },
            )
        })
        .collect();
    let capacity = authored
        .expanded
        .gears
        .iter()
        .flat_map(|gear| gear.semantic_contract.value_contracts())
        .map(|contract| contract.contract.maximum_bytes)
        .max()
        // Fixed-size records need no variable ValueContract; their prepared
        // codec bounds still admit the exact connection storage.
        .unwrap_or(expression_capacity);
    assert!(capacity as usize <= MAXIMUM_STRUCTURED_CANONICAL_BYTES);
    let plan = conduit_planner::plan_expanded_authoring_with_options(
        &authored,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: capacity,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundaries,
    )
    .unwrap();
    assert!(verify_plan(&plan));
    assert_eq!(
        plan.source_document_id,
        authored.expanded.source_document_id
    );
    assert_eq!(plan.checked_plot_id, authored.expanded.checked_plot_id);
    assert_eq!(plan.expanded_plot_id, authored.expanded.expanded_plot_id);
    (plan, authored.front, pair)
}
fn field(value: &StructuredInfoValue, name: &str) -> StructuredInfoValue {
    let StructuredInfoValueShape::Record(fields) = value.shape() else {
        panic!("record")
    };
    fields
        .iter()
        .find(|f| f.name() == name)
        .unwrap()
        .value()
        .clone()
}
fn payload(value: &StructuredInfoValue, expected: &str) -> StructuredInfoValue {
    let StructuredInfoValueShape::Variant { tag, payload } = value.shape() else {
        panic!("variant")
    };
    assert_eq!(tag, expected);
    payload.clone()
}
fn integer(value: &StructuredInfoValue) -> i32 {
    let StructuredInfoValueShape::Leaf(bytes) = value.shape() else {
        panic!("I32")
    };
    i32::from_le_bytes(bytes.try_into().unwrap())
}
fn initial(word: i32, b: i32, c: i32) -> Vec<u8> {
    let package = conduit_semantic_catalog::prepare_dsp_source_package().unwrap();
    let expanded = expand_canonical_plot_for_authoring(
        &package.checked,
        "shared-excitation-profile-start",
        &package.profiles,
    )
    .unwrap();
    let ConfigurationValue::Text(encoded) = &expanded.expanded.gears[0].configuration[0].value
    else {
        panic!("program")
    };
    let program = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
    let StructuredInfoTypeShape::Record { fields, .. } = program.input_type.shape() else {
        panic!("seed")
    };
    let value = StructuredInfoValue::record(
        program.input_type.clone(),
        fields
            .iter()
            .map(|f| {
                let value = match f.name() {
                    "word" => word,
                    "b" => b,
                    "c" => c,
                    _ => panic!("seed field"),
                };
                StructuredFieldValue::new(
                    f.name(),
                    StructuredInfoValue::leaf(f.value_type().clone(), value.to_le_bytes().to_vec())
                        .unwrap(),
                )
                .unwrap()
            })
            .collect(),
    )
    .unwrap();
    program.evaluate(&value.canonical_bytes().unwrap()).unwrap()
}
fn render(blocks: &[usize]) -> Vec<Vec<u8>> {
    let mut state = initial(31, 4096, -2048);
    let mut word = 31_i64;
    let (mut y1, mut y2) = (0_i64, 0_i64);
    let mut outputs = vec![];
    let mut frame = 0;
    for &block in blocks {
        let (plan, front, _) = prepare("dsp/experimental-q14-frame");
        for _ in 0..block {
            let package = conduit_semantic_catalog::prepare_dsp_source_package().unwrap();
            let stage = &package
                .checked
                .native_types
                .iter()
                .find(|ty| ty.name == "SharedExcitationStage")
                .unwrap()
                .value_type;
            let contract = CheckedValueContract::new(
                stage.profile().unwrap().value_kind().clone(),
                32768,
                vec![],
            )
            .unwrap();
            let pair = PreparedValuePairFactory::new(&contract, stage, &contract, stage).unwrap();
            let output = kernel::run(plan.clone(), &front, pair, vec![], state, false).unwrap();
            word = (word * 25173 + 13849) % 65536;
            let filtered =
                (((word - 32768) * 16384 + 4096 * y1 - 2048 * y2) / 16384).clamp(-32767, 32767);
            let expected = filtered * (frame * 32) / 256;
            let decoded = StructuredInfoValue::from_canonical_bytes(&output).unwrap();
            let control = payload(&decoded, "context");
            let cursor = field(&control, "cursor");
            assert_eq!(integer(&field(&cursor, "sample")), expected as i32);
            assert_eq!(integer(&field(&cursor, "word")), word as i32);
            assert_eq!(integer(&field(&cursor, "y1")), filtered as i32);
            assert_eq!(integer(&field(&cursor, "y2")), y1 as i32);
            assert_eq!(integer(&field(&cursor, "frame")), frame as i32 + 1);
            y2 = y1;
            y1 = filtered;
            frame += 1;
            state = output.clone();
            outputs.push(output);
        }
    }
    assert_eq!(frame, 8);
    outputs
}
#[test]
fn package_frame_matches_independent_recurrence_and_serialized_partition_resume() {
    assert_eq!(render(&[8]), render(&[1, 3, 4]));
}
#[test]
fn package_frame_can_cancel_while_output_is_held() {
    let state = initial(31, 4096, -2048);
    let (plan, front, pair) = prepare("dsp/experimental-q14-frame");
    assert!(kernel::run(plan, &front, pair, vec![], state, true).is_none());
}

#[test]
fn two_source_kernel_preserves_independent_histories_and_declared_mixer_headroom() {
    let package = conduit_semantic_catalog::prepare_dsp_source_package().unwrap();
    let dual_type = &package
        .checked
        .native_types
        .iter()
        .find(|ty| ty.name == "DspDualControl")
        .unwrap()
        .value_type;
    let stage = &package
        .checked
        .native_types
        .iter()
        .find(|ty| ty.name == "SharedExcitationStage")
        .unwrap()
        .value_type;
    let contract =
        CheckedValueContract::new(stage.profile().unwrap().value_kind().clone(), 32768, vec![])
            .unwrap();
    let left = StructuredInfoValue::from_canonical_bytes(&initial(31, 4096, -2048)).unwrap();
    let right = StructuredInfoValue::from_canonical_bytes(&initial(97, -4096, 1024)).unwrap();
    let mut controls = [payload(&left, "context"), payload(&right, "context")];
    let (plan, front, _) = prepare("dsp/experimental-q14-two-source");
    let mut words = [31_i64, 97_i64];
    let mut histories = [[0_i64; 2]; 2];
    for frame in 0..4 {
        let input = StructuredInfoValue::record(
            dual_type.clone(),
            vec![
                StructuredFieldValue::new("left", controls[0].clone()).unwrap(),
                StructuredFieldValue::new("right", controls[1].clone()).unwrap(),
            ],
        )
        .unwrap()
        .canonical_bytes()
        .unwrap();
        let pair = PreparedValuePairFactory::new(&contract, stage, &contract, stage).unwrap();
        let bytes = kernel::run(plan.clone(), &front, pair, vec![], input, false).unwrap();
        let output = StructuredInfoValue::from_canonical_bytes(&bytes).unwrap();
        let rendered = payload(&output, "rendered");
        let mut expected = 0;
        for (i, (b, c)) in [(4096, -2048), (-4096, 1024)].into_iter().enumerate() {
            words[i] = (words[i] * 25173 + 13849) % 65536;
            let filtered =
                (((words[i] - 32768) * 16384 + b * histories[i][0] + c * histories[i][1]) / 16384)
                    .clamp(-32767, 32767);
            expected += (filtered * (frame * 32) / 256) * 128 / 256;
            histories[i] = [filtered, histories[i][0]];
            controls[i] = field(&rendered, if i == 0 { "left" } else { "right" });
            let cursor = field(&controls[i], "cursor");
            assert_eq!(integer(&field(&cursor, "word")), words[i] as i32);
            assert_eq!(integer(&field(&cursor, "y1")), filtered as i32);
        }
        assert_eq!(
            integer(&field(&rendered, "sample")),
            expected.clamp(-32767, 32767) as i32
        );
        assert_ne!(words[0], words[1]);
    }
}
fn music_initial(word: i32) -> Vec<u8> {
    let package = conduit_semantic_catalog::prepare_dsp_source_package().unwrap();
    let expanded = expand_canonical_plot_for_authoring(
        &package.checked,
        "shared-excitation-music-start",
        &package.profiles,
    )
    .unwrap();
    let ConfigurationValue::Text(encoded) = &expanded.expanded.gears[0].configuration[0].value
    else {
        panic!("program")
    };
    let program = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
    let StructuredInfoTypeShape::Record { fields, .. } = program.input_type.shape() else {
        panic!("seed")
    };
    let seed = StructuredInfoValue::record(
        program.input_type.clone(),
        vec![StructuredFieldValue::new(
            "word",
            StructuredInfoValue::leaf(fields[0].value_type().clone(), word.to_le_bytes().to_vec())
                .unwrap(),
        )
        .unwrap()],
    )
    .unwrap();
    program.evaluate(&seed.canonical_bytes().unwrap()).unwrap()
}
#[test]
fn musical_profile_matches_independent_periodic_phase_and_envelope_reference() {
    let mut state = music_initial(31);
    let (plan, front, _) = prepare("dsp/experimental-q14-frame");
    let package = conduit_semantic_catalog::prepare_dsp_source_package().unwrap();
    let stage = &package
        .checked
        .native_types
        .iter()
        .find(|ty| ty.name == "SharedExcitationStage")
        .unwrap()
        .value_type;
    let contract =
        CheckedValueContract::new(stage.profile().unwrap().value_kind().clone(), 32768, vec![])
            .unwrap();
    let cubic = |x: i64| {
        if x <= 0 || x >= 256 {
            0
        } else {
            x * x * (256 - x) / 256
        }
    };
    for frame in 0..20_i64 {
        let pair = PreparedValuePairFactory::new(&contract, stage, &contract, stage).unwrap();
        let output = kernel::run(plan.clone(), &front, pair, vec![], state, false).unwrap();
        let decoded = StructuredInfoValue::from_canonical_bytes(&output).unwrap();
        let cursor = field(&payload(&decoded, "context"), "cursor");
        let phase = frame * 256 % 4096;
        let current = phase * 256 / 3072;
        let previous = if phase < 256 {
            0
        } else {
            (phase - 256) * 256 / 3072
        };
        let expected = (cubic(current) - cubic(previous)) * (frame * 32).min(256) / 256;
        assert_eq!(integer(&field(&cursor, "sample")), expected as i32);
        assert_eq!(
            integer(&field(&cursor, "phase_q8")),
            ((phase + 256) % 4096) as i32
        );
        state = output;
    }
}
#[test]
fn invalid_experimental_coefficients_preserve_the_uncommitted_cursor() {
    let input = initial(31, 8193, 0);
    let original = StructuredInfoValue::from_canonical_bytes(&input).unwrap();
    let before = field(&payload(&original, "context"), "cursor");
    let (plan, front, pair) = prepare("dsp/experimental-q14-frame");
    let bytes = kernel::run(plan, &front, pair, vec![], input, false).unwrap();
    let value = StructuredInfoValue::from_canonical_bytes(&bytes).unwrap();
    let control = payload(&value, "context");
    assert_eq!(field(&control, "cursor"), before);
    let valid = field(&control, "valid");
    let StructuredInfoValueShape::Leaf(bytes) = valid.shape() else {
        panic!("Boolean")
    };
    assert_eq!(bytes, &[0]);
}

#[path = "dsp_package_frames/independent.rs"]
mod independent;
