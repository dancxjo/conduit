//! Explicit fixture seeds: portable preparation precedes the measured child Play.
//! No oscillator or resonator advancement is implemented in this helper.
use conduit_audio::*;
use conduit_core::*;
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_plot::{
    expand_canonical_plot_for_authoring, CheckedSyntaxDocument, PortableExpressionProgram,
    PreparedPortableExpressionEvaluator, ProfileCatalog,
};

fn ty(checked: &CheckedSyntaxDocument, name: &str) -> StructuredInfoType {
    checked
        .native_types
        .iter()
        .find(|ty| ty.name == name)
        .unwrap()
        .value_type
        .clone()
}
fn field_type(checked: &CheckedSyntaxDocument, name: &str, field: &str) -> StructuredInfoType {
    let ty = ty(checked, name);
    let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        panic!("record")
    };
    fields
        .iter()
        .find(|f| f.name() == field)
        .unwrap()
        .value_type()
        .clone()
}
fn record(
    checked: &CheckedSyntaxDocument,
    name: &str,
    fields: Vec<(&str, StructuredInfoValue)>,
) -> StructuredInfoValue {
    StructuredInfoValue::record(
        ty(checked, name),
        fields
            .into_iter()
            .map(|(name, value)| StructuredFieldValue::new(name, value).unwrap())
            .collect(),
    )
    .unwrap()
}
fn leaf(
    checked: &CheckedSyntaxDocument,
    name: &str,
    field: &str,
    bytes: Vec<u8>,
) -> StructuredInfoValue {
    StructuredInfoValue::leaf(field_type(checked, name, field), bytes).unwrap()
}
fn empty_case(checked: &CheckedSyntaxDocument, name: &str, tag: &str) -> StructuredInfoValue {
    let ty = ty(checked, name);
    let StructuredInfoTypeShape::Variant { cases, .. } = ty.shape() else {
        panic!("variant")
    };
    let payload = cases
        .iter()
        .find(|case| case.tag() == tag)
        .unwrap()
        .payload_type()
        .clone();
    StructuredInfoValue::variant(ty, tag, StructuredInfoValue::leaf(payload, vec![]).unwrap())
        .unwrap()
}
fn anchor() -> AudioTrajectoryAnchor {
    AudioTrajectoryAnchor::new(
        AudioOriginIdentity::new(1).unwrap(),
        AudioTimelineIdentity::new(1).unwrap(),
    )
    .unwrap()
}
fn provenance() -> AudioTrajectoryProvenance {
    AudioTrajectoryProvenance::new(
        AudioTrajectoryProvenanceKind::Authored,
        "acoustic child fixture".into(),
        None,
    )
    .unwrap()
}
fn prepare(
    checked: &CheckedSyntaxDocument,
    profiles: &ProfileCatalog,
    entry: &str,
    input: StructuredInfoValue,
) -> StructuredInfoValue {
    // Reference evaluation constructs a fixture seed before child admission.
    // The production 63-Gear child remains the sole measured advancement path.
    let expanded = expand_canonical_plot_for_authoring(checked, entry, profiles).unwrap();
    assert_eq!(expanded.input_bindings.len(), 1);
    let mut next = expanded.input_bindings[0].gear_id.clone();
    let mut bytes = input.canonical_bytes().unwrap();
    let mut visited = 0;
    loop {
        let gear = expanded
            .expanded
            .gears
            .iter()
            .find(|gear| gear.gear_id == next)
            .unwrap();
        let ConfigurationValue::Text(encoded) = &gear.configuration[0].value else {
            panic!("expression")
        };
        let program = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
        let mut evaluator = PreparedPortableExpressionEvaluator::new(&program).unwrap();
        bytes = evaluator.evaluate(&bytes).unwrap().to_vec();
        visited += 1;
        assert!(visited <= expanded.expanded.gears.len());
        let outgoing = expanded
            .expanded
            .connections
            .iter()
            .filter(|cord| cord.source_gear_id == next)
            .collect::<Vec<_>>();
        if outgoing.is_empty() {
            break;
        }
        assert_eq!(outgoing.len(), 1);
        next = outgoing[0].sink_gear_id.clone();
    }
    assert_eq!(visited, expanded.expanded.gears.len());
    let result = StructuredInfoValue::from_canonical_bytes(&bytes).unwrap();
    let StructuredInfoValueShape::Variant { tag, payload } = result.shape() else {
        panic!("outcome")
    };
    assert_eq!(tag, "started");
    payload.clone()
}

pub fn seeds(checked: &CheckedSyntaxDocument, profiles: &ProfileCatalog) -> (Vec<u8>, Vec<u8>) {
    let basis = AudioSampleRateBasis::new(anchor(), AudioFrameQuantization::Floor, 8000).unwrap();
    let request = AudioPeriodicRequest::new(
        AudioRelativeAmplitude::new(1, 1).unwrap(),
        basis.clone(),
        4,
        AudioFrequencyControl::new(
            anchor(),
            provenance(),
            AudioFrequencyHz::new(1, 125).unwrap(),
        )
        .unwrap(),
        AudioPhaseReference::new(
            anchor(),
            AudioExactTimeOffset::new(1, 0).unwrap(),
            64,
            1,
            provenance(),
        )
        .unwrap(),
        AudioPeriodicReference::UnscaledCubicFlow,
    )
    .unwrap();
    let oscillator = prepare(
        checked,
        profiles,
        "dsp/oscillator-prepare",
        request.into_structured().unwrap(),
    );
    let resonator_request = record(
        checked,
        "ResonatorRequest",
        vec![
            (
                "profile",
                empty_case(checked, "ResonatorProfile", "polynomial_q30_to_q14_v1"),
            ),
            (
                "generation",
                leaf(
                    checked,
                    "ResonatorRequest",
                    "generation",
                    1_u64.to_le_bytes().to_vec(),
                ),
            ),
            (
                "control",
                AudioResonator::new(
                    AudioFrequencyHz::new(1, 100).unwrap(),
                    AudioFrequencyHz::new(1, 1000).unwrap(),
                )
                .unwrap()
                .into_structured()
                .unwrap(),
            ),
            ("basis", basis.into_structured().unwrap()),
            (
                "frames",
                leaf(
                    checked,
                    "ResonatorRequest",
                    "frames",
                    4_u32.to_le_bytes().to_vec(),
                ),
            ),
        ],
    );
    let resonator = prepare(
        checked,
        profiles,
        "dsp/resonator-prepare",
        resonator_request,
    );
    let cursor = record(
        checked,
        "AcousticFrameCursor",
        vec![
            ("oscillator", oscillator),
            ("resonator", resonator),
            (
                "sample",
                leaf(
                    checked,
                    "AcousticFrameCursor",
                    "sample",
                    0_i128.to_le_bytes().to_vec(),
                ),
            ),
            (
                "clipped",
                leaf(checked, "AcousticFrameCursor", "clipped", vec![0]),
            ),
            (
                "status",
                empty_case(checked, "AcousticFrameStatus", "waiting"),
            ),
            (
                "history_count",
                leaf(
                    checked,
                    "AcousticFrameCursor",
                    "history_count",
                    0_u64.to_le_bytes().to_vec(),
                ),
            ),
            (
                "history_a",
                leaf(
                    checked,
                    "AcousticFrameCursor",
                    "history_a",
                    0_i128.to_le_bytes().to_vec(),
                ),
            ),
            (
                "history_b",
                leaf(
                    checked,
                    "AcousticFrameCursor",
                    "history_b",
                    0_i128.to_le_bytes().to_vec(),
                ),
            ),
            (
                "history_c",
                leaf(
                    checked,
                    "AcousticFrameCursor",
                    "history_c",
                    0_i128.to_le_bytes().to_vec(),
                ),
            ),
            (
                "history_d",
                leaf(
                    checked,
                    "AcousticFrameCursor",
                    "history_d",
                    0_i128.to_le_bytes().to_vec(),
                ),
            ),
        ],
    );
    let context = record(
        checked,
        "AcousticFrameControl",
        vec![
            ("cursor", cursor),
            (
                "control_status",
                empty_case(checked, "AcousticControlStatus", "ready"),
            ),
            (
                "cancelled",
                leaf(checked, "AcousticFrameControl", "cancelled", vec![0]),
            ),
            (
                "oscillator_ready",
                leaf(checked, "AcousticFrameControl", "oscillator_ready", vec![0]),
            ),
            (
                "drive",
                leaf(
                    checked,
                    "AcousticFrameControl",
                    "drive",
                    0_i128.to_le_bytes().to_vec(),
                ),
            ),
        ],
    );
    let accumulator =
        StructuredInfoValue::variant(ty(checked, "AcousticStage"), "context", context).unwrap();
    let stage = ty(checked, "AcousticStage");
    let StructuredInfoTypeShape::Variant { cases, .. } = stage.shape() else {
        panic!("stage")
    };
    let tick = cases
        .iter()
        .find(|case| case.tag() == "tick")
        .unwrap()
        .payload_type()
        .clone();
    let item = StructuredInfoValue::variant(
        stage,
        "tick",
        StructuredInfoValue::leaf(tick, vec![0]).unwrap(),
    )
    .unwrap();
    (
        accumulator.canonical_bytes().unwrap(),
        item.canonical_bytes().unwrap(),
    )
}
