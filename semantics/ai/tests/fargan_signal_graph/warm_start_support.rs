//! Shared warm initialization/comparison helpers; no registered tests.
//! Initialization executes from Source zero state; C state is comparison only.
use super::{plan, runtime, state};
use conduit_core::{StructuredInfoValue, StructuredInfoValueShape};
use std::collections::BTreeMap;
pub(super) const ENTRY: &str = "speech/fargan-warm-start";

pub(super) fn source() -> String {
    [
        plan::source(),
        include_str!("../../../speech/fargan_warm_policy.conduit").into(),
        include_str!("../../../speech/fargan_warm_start.conduit").into(),
    ]
    .join("\n")
}
pub(super) fn period(value: &StructuredInfoValue) -> u16 {
    match value.shape() {
        StructuredInfoValueShape::Leaf(bytes) => u16::from_le_bytes(bytes.try_into().unwrap()),
        _ => panic!("exact period"),
    }
}
pub(super) fn inputs(
    schema: &plan::SourceSchema,
    context: &[f32; 320],
    features: &[[f32; 20]; 5],
    periods: [u16; 5],
) -> BTreeMap<String, Vec<u8>> {
    let mut result = BTreeMap::from([(
        "pcm_context".into(),
        state::vector(schema.ty("pcm_context"), context)
            .canonical_bytes()
            .unwrap(),
    )]);
    for i in 0..5 {
        let name = format!("features{i}");
        result.insert(
            name.clone(),
            state::vector(schema.ty(&name), &features[i])
                .canonical_bytes()
                .unwrap(),
        );
        let name = format!("period{i}");
        let leaf = StructuredInfoValue::leaf(
            conduit_core::StructuredInfoType::leaf(conduit_core::kind_id("value/u16")).unwrap(),
            periods[i].to_le_bytes().to_vec(),
        )
        .unwrap();
        result.insert(
            name.clone(),
            StructuredInfoValue::nominal(schema.ty(&name).clone(), leaf)
                .unwrap()
                .canonical_bytes()
                .unwrap(),
        );
    }
    assert!(!result.contains_key("state") && !result.contains_key("history"));
    result
}
pub(super) fn compare(
    result: &runtime::EncodedResultAndTiming,
    expected: &state::State,
    history: &[f32; 128],
    expected_period: u16,
) {
    let actual = state::State::from_state(&result.outputs["next_state"]);
    let actual_history = state::floats(&result.outputs["next_history"]);
    assert_eq!(actual.flattened().len() + actual_history.len(), 965);
    let state_error = actual
        .flattened()
        .iter()
        .zip(expected.flattened())
        .map(|(a, b)| (a - b).abs())
        .fold(0f32, f32::max);
    let history_error = actual_history
        .iter()
        .zip(history)
        .map(|(a, b)| (a - b).abs())
        .fold(0f32, f32::max);
    // Existing explicit full-float/libm differential guards; not a global theorem.
    assert!(
        state_error < 0.05 && history_error < 0.001,
        "state={state_error} history={history_error}"
    );
    assert_eq!(period(&result.outputs["previous_period"]), expected_period);
    assert_eq!(
        actual.pitch.map(f32::to_bits),
        expected.pitch.map(f32::to_bits)
    );
    assert_eq!(actual.deemphasis.to_bits(), expected.deemphasis.to_bits());
    assert!(!result.signs.is_empty());
    eprintln!(
        "warm965+period Source-owned initialization: nodes={} cords={} state_max_abs={state_error} history_max_abs={history_error}; no oracle state supplied to Plan",
        result.nodes, result.cords
    );
}
