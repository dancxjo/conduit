//! Ordinary Source/Plan/Play entrance for generic envelope and gain, no device.
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
const GAIN: &str = include_str!("../../../semantics/audio/dsp/gain.conduit");
const ENVELOPE: &str = include_str!("../../../semantics/audio/dsp/envelope.conduit");
struct NoTimer;
impl TimerAdapter for NoTimer {
    fn wait(&mut self, _: Duration) {
        panic!("envelope/gain has no timer/device effect");
    }
}
#[derive(Default)]
struct Outputs(Vec<Vec<u8>>);
impl ExternalForeOutputAdapter for Outputs {
    fn deliver(&mut self, value: ExternalForeDelivery) -> Result<(), String> {
        if value.track == ConnectionTrack::Payload {
            self.0.push(value.bytes.to_vec());
        }
        Ok(())
    }
}
fn prepare(source: &str, name: &str) -> (StdHost, conduit_core::Plan, PortableExpressionProgram) {
    let checked =
        check_syntax_document(&parse_syntax_document(source), &StartupCatalog::new()).unwrap();
    let authored =
        expand_canonical_plot_for_authoring(&checked, name, &ProfileCatalog::new()).unwrap();
    let ConfigurationValue::Text(encoded) = &authored.expanded.gears[0].configuration[0].value
    else {
        panic!("program")
    };
    let program = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
    let mut advertisement = StdHost::new().advertisement().clone();
    let offer = conduit_std_offers::pure_expression_std_offer(
        &program,
        authored.expanded.gears[0].inputs[0].temporal,
    )
    .unwrap();
    assert!(authored.expanded.gears[0].accepts_realization(&offer));
    advertisement.capabilities.push(offer);
    advertisement
        .capabilities
        .sort_by(|a, b| a.capability_id.cmp(&b.capability_id));
    let host = StdHost::from_advertisement(advertisement).unwrap();
    let hosts = [host.advertisement().clone()];
    let placements =
        conduit_planner::default_expanded_placements(&authored.expanded, &hosts).unwrap();
    let boundaries = [
        (PortDirection::Input, "control"),
        (PortDirection::Output, "result"),
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

fn input(program: &PortableExpressionProgram, values: &[(&str, i32)], cancelled: bool) -> Vec<u8> {
    let StructuredInfoTypeShape::Record { fields, .. } = program.input_type.shape() else {
        panic!("record")
    };
    StructuredInfoValue::record(
        program.input_type.clone(),
        fields
            .iter()
            .map(|field| {
                let bytes = if field.name() == "cancelled" {
                    vec![u8::from(cancelled)]
                } else {
                    values
                        .iter()
                        .find(|(name, _)| *name == field.name())
                        .unwrap()
                        .1
                        .to_le_bytes()
                        .to_vec()
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
fn run(source: &str, name: &str, values: &[(&str, i32)], cancelled: bool) -> (String, Option<i32>) {
    let (mut host, plan, program) = prepare(source, name);
    assert_eq!(
        plan.source_document_id,
        parse_syntax_document(source).source_document_id()
    );
    let mut output = Outputs(Vec::with_capacity(1));
    let report = host
        .run_external_plot_sequence_to(
            plan.fragments[0].clone(),
            &[ExternalForeInput {
                front_port_id: "control".into(),
                track: ConnectionTrack::Payload,
                bytes: input(&program, values, cancelled),
            }],
            &mut output,
            &mut Vec::new(),
            &mut NoTimer,
        )
        .unwrap();
    assert_eq!(output.0.len(), 1);
    let kernel = report.kernel.unwrap();
    assert_eq!(
        kernel.value_allocation_capacity_before,
        kernel.value_allocation_capacity_after
    );
    for kind in [
        conduit_kernel::KernelEventKind::HostCallRequested,
        conduit_kernel::KernelEventKind::HostCallCompleted,
    ] {
        assert!(kernel.kernel_sign.iter().any(|event| event.kind == kind));
    }
    let value = StructuredInfoValue::from_canonical_bytes(&output.0[0]).unwrap();
    let StructuredInfoValueShape::Variant { tag, payload } = value.shape() else {
        panic!("outcome")
    };
    let sample = match payload.shape() {
        StructuredInfoValueShape::Leaf(bytes) if bytes.len() == 4 => {
            Some(i32::from_le_bytes(bytes.try_into().unwrap()))
        }
        _ => None,
    };
    (tag.into(), sample)
}
#[test]
fn installed_gain_saturates_and_cancels_without_emitting_a_sample() {
    for sample in [-32768, 0, 32767] {
        assert_eq!(
            run(
                GAIN,
                "dsp/gain",
                &[("sample", sample), ("gain_q8", 4096)],
                false
            ),
            (
                "sample".into(),
                Some((i64::from(sample) * 16).clamp(-32767, 32767) as i32)
            )
        );
    }
    let values = [("sample", i32::MAX), ("gain_q8", i32::MAX)];
    assert_eq!(
        run(GAIN, "dsp/gain", &values, false),
        ("invalid_profile".into(), None)
    );
    assert_eq!(
        run(GAIN, "dsp/gain", &values, true),
        ("cancelled".into(), None)
    );
}
#[test]
fn installed_envelope_preserves_endpoints_and_explicit_restart() {
    let controls = |frame| {
        [
            ("frame", frame),
            ("total", 64),
            ("attack", 16),
            ("release", 16),
        ]
    };
    for (frame, expected) in [(0, 0), (8, 128), (16, 256), (48, 256), (56, 128), (64, 0)] {
        assert_eq!(
            run(ENVELOPE, "dsp/envelope", &controls(frame), false),
            ("level_q8".into(), Some(expected))
        );
    }
    assert_eq!(
        run(ENVELOPE, "dsp/envelope", &controls(32), true),
        ("cancelled".into(), None)
    );
    assert_eq!(
        run(ENVELOPE, "dsp/envelope", &controls(0), false),
        ("level_q8".into(), Some(0))
    );
    assert_eq!(
        run(ENVELOPE, "dsp/envelope", &controls(65), false),
        ("invalid_profile".into(), None)
    );
}
