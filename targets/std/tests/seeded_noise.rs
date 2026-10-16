//! Ordinary Source/Plan/Play entrance for generic seeded excitation, no device.
use conduit_core::{
    BaseImplementationId, ConfigurationValue, ConnectionTrack, PortDirection, StructuredFieldValue,
    StructuredInfoTypeShape, StructuredInfoValue, StructuredInfoValueShape,
};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    PortableExpressionProgram, ProfileCatalog, StartupCatalog,
};
use conduit_std_host::{
    ExternalForeDelivery, ExternalForeInput, ExternalForeOutputAdapter, StdHost, TimerAdapter,
};
use std::{collections::BTreeMap, time::Duration};
const SOURCE: &str = include_str!("../../../semantics/audio/dsp/seeded-noise.conduit");
struct NoTimer;
impl TimerAdapter for NoTimer {
    fn wait(&mut self, _: Duration) {
        panic!("noise has no timer/device effect");
    }
}
#[derive(Default)]
struct Outputs(Vec<Vec<u8>>);
impl ExternalForeOutputAdapter for Outputs {
    fn deliver(&mut self, value: ExternalForeDelivery) -> Result<(), String> {
        if value.track == ConnectionTrack::Payload {
            self.0.push(value.bytes);
        }
        Ok(())
    }
}
fn prepare() -> (StdHost, conduit_core::Plan, PortableExpressionProgram) {
    let checked =
        check_syntax_document(&parse_syntax_document(SOURCE), &StartupCatalog::new()).unwrap();
    let authored =
        expand_canonical_plot_for_authoring(&checked, "dsp/noise-step", &ProfileCatalog::new())
            .unwrap();
    let ConfigurationValue::Text(encoded) = &authored.expanded.gears[0].configuration[0].value
    else {
        panic!("program")
    };
    let program = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
    let mut advertisement = StdHost::new().advertisement().clone();
    for gear in &authored.expanded.gears {
        let ConfigurationValue::Text(encoded) = &gear.configuration[0].value else {
            panic!("program")
        };
        let component = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
        let offer =
            conduit_std_offers::pure_expression_std_offer(&component, gear.inputs[0].temporal)
                .unwrap();
        assert!(gear.accepts_realization(&offer));
        if !advertisement
            .capabilities
            .iter()
            .any(|existing| existing.capability_id == offer.capability_id)
        {
            advertisement.capabilities.push(offer);
        }
    }
    advertisement
        .capabilities
        .sort_by(|a, b| a.capability_id.cmp(&b.capability_id));
    let host = StdHost::from_advertisement(advertisement).unwrap();
    let hosts = [host.advertisement().clone()];
    let placements =
        conduit_planner::default_expanded_placements(&authored.expanded, &hosts).unwrap();
    let boundaries = [
        (PortDirection::Input, "state"),
        (PortDirection::Output, "next"),
    ]
    .into_iter()
    .map(|(direction, port)| {
        (
            conduit_planner::ForeBoundaryKey {
                direction,
                front_port_id: port.into(),
                track: ConnectionTrack::Payload,
            },
            conduit_planner::ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: 4096,
            },
        )
    })
    .collect();
    let plan = conduit_planner::plan_expanded_authoring_with_options(
        &authored,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: 4096,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundaries,
    )
    .unwrap();
    (host, plan, program)
}

fn input(program: &PortableExpressionProgram, word: i32, remaining: u32) -> Vec<u8> {
    let StructuredInfoTypeShape::Record { fields, .. } = program.input_type.shape() else {
        panic!("record")
    };
    StructuredInfoValue::record(
        program.input_type.clone(),
        fields
            .iter()
            .map(|field| {
                let bytes = if field.name() == "word" {
                    word.to_le_bytes().to_vec()
                } else {
                    remaining.to_le_bytes().to_vec()
                };
                StructuredFieldValue::new(
                    field.name(),
                    StructuredInfoValue::leaf(field.value_type().clone(), bytes).unwrap(),
                )
                .unwrap()
            })
            .collect(),
    )
    .unwrap()
    .canonical_bytes()
    .unwrap()
}

fn result(bytes: &[u8]) -> (String, Option<(i32, u32, i32)>) {
    let value = StructuredInfoValue::from_canonical_bytes(bytes).unwrap();
    let StructuredInfoValueShape::Variant { tag, payload } = value.shape() else {
        panic!("outcome")
    };
    if tag != "advanced" {
        return (tag.into(), None);
    }
    let StructuredInfoValueShape::Record(fields) = payload.shape() else {
        panic!("record")
    };
    let leaf = |name| {
        let value = fields
            .iter()
            .find(|field| field.name() == name)
            .unwrap()
            .value();
        let StructuredInfoValueShape::Leaf(bytes) = value.shape() else {
            panic!("leaf")
        };
        <[u8; 4]>::try_from(bytes).unwrap()
    };
    (
        tag.into(),
        Some((
            i32::from_le_bytes(leaf("word")),
            u32::from_le_bytes(leaf("remaining")),
            i32::from_le_bytes(leaf("sample")),
        )),
    )
}

fn run(host: &mut StdHost, plan: &conduit_core::Plan, inputs: Vec<Vec<u8>>) -> Vec<Vec<u8>> {
    let count = inputs.len();
    let inputs = inputs
        .into_iter()
        .map(|bytes| ExternalForeInput {
            front_port_id: "state".into(),
            track: ConnectionTrack::Payload,
            bytes,
        })
        .collect::<Vec<_>>();
    let mut output = Outputs(Vec::with_capacity(count));
    let report = host
        .run_external_plot_sequence_to(
            plan.fragments[0].clone(),
            &inputs,
            &mut output,
            &mut Vec::new(),
            &mut NoTimer,
        )
        .unwrap();
    assert_eq!(output.0.len(), count);
    let kernel = report.kernel.unwrap();
    assert_eq!(
        kernel.value_allocation_capacity_before,
        kernel.value_allocation_capacity_after
    );
    assert!(kernel
        .kernel_sign
        .iter()
        .any(|event| event.kind == conduit_kernel::KernelEventKind::HostCallRequested));
    assert!(kernel
        .kernel_sign
        .iter()
        .any(|event| event.kind == conduit_kernel::KernelEventKind::HostCallCompleted));
    output.0
}

fn reference(word: i32) -> i32 {
    ((u64::try_from(word).unwrap() * 25173 + 13849) & 0xffff) as i32
}

fn sequence(partitions: &[u32]) -> (Vec<Vec<u8>>, (i32, u32)) {
    let mut state = (1i32, 64u32);
    let mut outputs = Vec::with_capacity(64);
    for &count in partitions {
        // A fresh installed Host/Play resumes only from explicit checked state.
        let (mut host, plan, program) = prepare();
        assert_eq!(
            plan.source_document_id,
            parse_syntax_document(SOURCE).source_document_id()
        );
        let mut expected = state;
        let mut inputs = Vec::with_capacity(count as usize);
        for _ in 0..count {
            inputs.push(input(&program, expected.0, expected.1));
            expected = (reference(expected.0), expected.1 - 1);
        }
        let block = run(&mut host, &plan, inputs);
        for bytes in &block {
            let (_, Some((word, remaining, sample))) = result(bytes) else {
                panic!("advance")
            };
            assert_eq!(
                (word, remaining, sample),
                (reference(state.0), state.1 - 1, reference(state.0) - 32768)
            );
            state = (word, remaining);
        }
        assert_eq!(state, expected);
        outputs.extend(block);
    }
    (outputs, state)
}

#[test]
fn partition_reset_and_fresh_host_resume_preserve_exact_noise_bytes() {
    let whole = sequence(&[64]);
    assert_eq!(whole, sequence(&[16, 24, 24]));
    assert_eq!(whole, sequence(&[1, 31, 32]));
    assert_eq!(whole, sequence(&[64]));
    assert_eq!(whole.1 .1, 0);
}

#[test]
fn installed_source_returns_precise_terminal_refusals_without_a_sample() {
    let (mut host, plan, program) = prepare();
    for (word, remaining, wanted) in [
        (i32::MIN, 8, "invalid_seed"),
        (65536, 8, "invalid_seed"),
        (1, 0, "exhausted"),
        (1, u32::MAX, "invalid_profile"),
    ] {
        let output = run(&mut host, &plan, vec![input(&program, word, remaining)]);
        assert_eq!(result(&output[0]), (wanted.into(), None));
    }
}
