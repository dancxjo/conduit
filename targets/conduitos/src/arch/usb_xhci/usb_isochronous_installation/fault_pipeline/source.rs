//! Checked Source consumes native-encoded observations; no synthetic completion constructor.
use alloc::{format, string::String, vec, vec::Vec};
use conduit_core::*;
use conduit_plot::*;
fn source() -> String {
    let audio = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../semantics/audio/types.conduit"
    ));
    let start = audio.find("type PcmSampleRepresentation").unwrap();
    let end = audio.find("type AudioRenderDemand").unwrap();
    let negotiation = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/plots/usb/audio-negotiation-back.conduit"
    ));
    let setup_types = &negotiation[negotiation.find("type UsbAudioSetupState").unwrap()
        ..negotiation.find("plot usb-audio-setup-start").unwrap()];
    let setup_observation = &negotiation[negotiation
        .find("plot usb-audio-setup-observation")
        .unwrap()
        ..negotiation.find("plot usb-audio-setup-accept").unwrap()];
    let usb = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/plots/usb/protocol.conduit"
    ));
    let cadence_start = usb.find("type UsbPcmCadence").unwrap();
    let cadence_end = usb.find("# A stale DMA completion").unwrap();
    [
        &audio[start..end],
        setup_types,
        setup_observation,
        &usb[cadence_start..cadence_end],
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/plots/usb/control-types.conduit"
        )),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/plots/usb/isochronous-out-types.conduit"
        )),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/plots/usb/audio-types.conduit"
        )),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/plots/usb/audio-block-types.conduit"
        )),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/plots/usb/audio-block-profile.conduit"
        )),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/plots/usb/audio-profile.conduit"
        )),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/plots/usb/audio-completion-profile.conduit"
        )),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/plots/usb/audio-repeat-playback-policy.conduit"
        )),
        include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/plots/usb/audio-playback-result.conduit"
        )),
    ]
    .join("\n")
}
fn block_value(
    ty: &conduit_core::StructuredInfoType,
    path: &str,
    packet: usize,
) -> conduit_core::StructuredInfoValue {
    block_value_with(ty, path, packet, None)
}
fn block_value_with(
    ty: &conduit_core::StructuredInfoType,
    path: &str,
    packet: usize,
    replacement: Option<(&str, u64)>,
) -> conduit_core::StructuredInfoValue {
    use conduit_core::{
        StructuredFieldValue, StructuredInfoTypeShape as Shape, StructuredInfoValue as V,
    };
    match ty.shape() {
        Shape::Record { fields, .. } => V::record(
            ty.clone(),
            fields
                .iter()
                .map(|field| {
                    let next = format!("{path}/{}", field.name());
                    StructuredFieldValue::new(
                        field.name(),
                        block_value_with(field.value_type(), &next, packet, replacement),
                    )
                    .unwrap()
                })
                .collect(),
        )
        .unwrap(),
        Shape::Variant { cases, .. } => {
            let tag = if path.ends_with("representation") {
                "signed16-little-endian"
            } else {
                "stereo-left-right"
            };
            let case = cases.iter().find(|case| case.tag() == tag).unwrap();
            V::variant(
                ty.clone(),
                tag,
                V::leaf(case.payload_type().clone(), vec![]).unwrap(),
            )
            .unwrap()
        }
        Shape::Sequence { element, .. } => V::sequence(
            ty.clone(),
            (0..8)
                .map(|index| block_value_with(element, path, index, replacement))
                .collect(),
        )
        .unwrap(),
        Shape::Leaf(kind) => {
            let value: u64 = match path.rsplit('/').next().unwrap() {
                "sample-rate-hz" | "sample_rate" => 48000,
                "frame-count" => 384,
                "clock-id" => 1,
                "payload-bytes" => 1536,
                "channels" | "sample_bytes" => 2,
                "frame" => 100 + packet as u64,
                "start-frame" | "discontinuity" | "phase" | "wire" => 0,
                other => panic!("unknown fixture field {other}"),
            };
            let value = replacement
                .filter(|(target, _)| *target == path)
                .map_or(value, |(_, replacement)| replacement);
            let width = match kind.as_str() {
                "value/u64" => 8,
                "value/u32" => 4,
                "value/u16" => 2,
                "value/u8" | "value/bool" => 1,
                "value/bytes" => 0,
                other => panic!("kind {other}"),
            };
            let bytes = if width == 0 {
                (0..192)
                    .map(|offset| ((packet * 192 + offset) % 251) as u8)
                    .collect()
            } else {
                value.to_le_bytes()[..width].to_vec()
            };
            V::leaf(ty.clone(), bytes).unwrap()
        }
        _ => panic!("unsupported block shape"),
    }
}

pub(super) struct Pipeline {
    pub request: Vec<u8>,
    previous: Vec<u8>,
    seed: PreparedPortableExpressionEvaluator,
    stages: Vec<PreparedPortableExpressionEvaluator>,
    finish: PreparedPortableExpressionEvaluator,
    projection: PreparedPortableExpressionEvaluator,
    observation: PreparedStructuredComposer,
    scratch: [Vec<u8>; 2],
}
fn program(checked: &CheckedSyntaxDocument, name: &str) -> PortableExpressionProgram {
    let expanded = expand_canonical_plot_for_authoring(checked, name, &ProfileCatalog::new())
        .unwrap()
        .expanded;
    assert_eq!(expanded.gears.len(), 1);
    let ConfigurationValue::Text(encoded) = &expanded.gears[0].configuration[0].value else {
        panic!("program")
    };
    PortableExpressionProgram::from_canonical_hex(encoded).unwrap()
}
impl Pipeline {
    pub fn prepare() -> Self {
        let checked =
            check_syntax_document(&parse_syntax_document(&source()), &StartupCatalog::new())
                .unwrap();
        let request = program(&checked, "usb-audio-playback-request");
        let StructuredInfoTypeShape::Record { fields, .. } = request.input_type.shape() else {
            panic!("state")
        };
        let previous = StructuredInfoValue::record(
            request.input_type.clone(),
            fields
                .iter()
                .map(|field| {
                    let value = match field.name() {
                        "pcm" => block_value(field.value_type(), "", 0),
                        "first-frame" => StructuredInfoValue::leaf(
                            field.value_type().clone(),
                            8_u64.to_le_bytes().to_vec(),
                        )
                        .unwrap(),
                        "phase" => StructuredInfoValue::leaf(
                            field.value_type().clone(),
                            1_u64.to_le_bytes().to_vec(),
                        )
                        .unwrap(),
                        "terminal" => {
                            StructuredInfoValue::leaf(field.value_type().clone(), vec![0]).unwrap()
                        }
                        _ => panic!("state field"),
                    };
                    StructuredFieldValue::new(field.name(), value).unwrap()
                })
                .collect(),
        )
        .unwrap();
        let request = PreparedPortableExpressionEvaluator::new(&request)
            .unwrap()
            .evaluate(&previous.canonical_bytes().unwrap())
            .unwrap()
            .to_vec();
        let seed = program(&checked, "usb-audio-completion-validation-seed");
        let observation = seed.input_type.clone();
        let previous = previous.canonical_bytes().unwrap();
        Self {
            request,
            previous,
            observation: PreparedStructuredComposer::new(&observation, 32768).unwrap(),
            scratch: [Vec::with_capacity(32768), Vec::with_capacity(32768)],
            seed: PreparedPortableExpressionEvaluator::new(&seed).unwrap(),
            stages: (0..9)
                .map(|index| {
                    PreparedPortableExpressionEvaluator::new(&program(
                        &checked,
                        &format!("usb-audio-completion-validation-{index}"),
                    ))
                    .unwrap()
                })
                .collect(),
            finish: PreparedPortableExpressionEvaluator::new(&program(
                &checked,
                "usb-audio-completion-validation-finish",
            ))
            .unwrap(),
            projection: PreparedPortableExpressionEvaluator::new(&program(
                &checked,
                "usb-audio-playback-result",
            ))
            .unwrap(),
        }
    }
    pub fn wire(&self, index: u16) -> &[u8] {
        validate_canonical_structured_value(&self.request)
            .unwrap()
            .variant_payload("batch")
            .unwrap()
            .unwrap()
            .record_field("packets")
            .unwrap()
            .unwrap()
            .collection_index(index)
            .unwrap()
            .unwrap()
            .record_field("wire")
            .unwrap()
            .unwrap()
            .primitive_bytes("value/bytes")
            .unwrap()
    }
    pub fn check(&mut self, native: &[u8], tag: &str, prefix: usize) {
        let observation = self
            .observation
            .record(&[
                validate_canonical_structured_value(&self.previous).unwrap(),
                validate_canonical_structured_value(native).unwrap(),
            ])
            .unwrap();
        let projected = self.projection.evaluate(observation).unwrap();
        assert_eq!(
            projected, native,
            "public result preserves exact native bytes"
        );
        let result = validate_canonical_structured_value(projected).unwrap();
        assert_eq!(result.variant_tag().unwrap(), tag);
        let payload = result.variant_payload(tag).unwrap().unwrap();
        assert_eq!(
            payload
                .record_field("completed")
                .unwrap()
                .unwrap()
                .collection_length()
                .unwrap(),
            prefix
        );
        let completed = payload.record_field("completed").unwrap().unwrap();
        for index in 0..prefix {
            let packet = completed.collection_index(index as u16).unwrap().unwrap();
            for (name, expected) in [
                ("frame", 8 + index as u64),
                ("requested", 192),
                ("actual", 192),
            ] {
                assert_eq!(
                    packet
                        .record_field(name)
                        .unwrap()
                        .unwrap()
                        .primitive_bytes("value/u64")
                        .unwrap(),
                    &expected.to_le_bytes()
                );
            }
        }
        if tag == "short" {
            let packet = payload.record_field("packet").unwrap().unwrap();
            for (name, expected) in [("frame", 12_u64), ("requested", 192), ("actual", 188)] {
                assert_eq!(
                    packet
                        .record_field(name)
                        .unwrap()
                        .unwrap()
                        .primitive_bytes("value/u64")
                        .unwrap(),
                    &expected.to_le_bytes()
                );
            }
        } else if tag == "missed-service" {
            assert_eq!(
                payload
                    .record_field("frame")
                    .unwrap()
                    .unwrap()
                    .primitive_bytes("value/u64")
                    .unwrap(),
                &12_u64.to_le_bytes()
            );
        }
        self.scratch[0].clear();
        self.scratch[0].extend_from_slice(self.seed.evaluate(observation).unwrap());
        let mut current = 0;
        for stage in &mut self.stages {
            let (left, right) = self.scratch.split_at_mut(1);
            let (input, output) = if current == 0 {
                (&left[0], &mut right[0])
            } else {
                (&right[0], &mut left[0])
            };
            let result = stage.evaluate(input).unwrap();
            assert!(result.len() <= output.capacity());
            output.clear();
            output.extend_from_slice(result);
            current ^= 1;
        }
        let state = self.finish.evaluate(&self.scratch[current]).unwrap();
        let state = validate_canonical_structured_value(state).unwrap();
        assert_eq!(
            state
                .record_field("terminal")
                .unwrap()
                .unwrap()
                .primitive_bytes("value/bool")
                .unwrap(),
            &[1]
        );
        assert_eq!(
            state
                .record_field("phase")
                .unwrap()
                .unwrap()
                .primitive_bytes("value/u64")
                .unwrap(),
            &1_u64.to_le_bytes()
        );
    }
}
