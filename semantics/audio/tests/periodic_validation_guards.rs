use conduit_audio::*;
use conduit_core::{StructuredInfoValue, StructuredInfoValueShape};
use conduit_plot::rust_binding::NativeRustBinding;
#[path = "common/periodic.rs"]
mod fixture;
use fixture::{request, started, Pipeline};
fn replace(value: &StructuredInfoValue, path: &[&str], raw: &[u8]) -> StructuredInfoValue {
    if path.is_empty() {
        return StructuredInfoValue::leaf(value.value_type().clone(), raw.to_vec()).unwrap();
    }
    let StructuredInfoValueShape::Record(fields) = value.shape() else {
        panic!("record path")
    };
    StructuredInfoValue::record(
        value.value_type().clone(),
        fields
            .iter()
            .map(|field| {
                let child = if field.name() == path[0] {
                    replace(field.value(), &path[1..], raw)
                } else {
                    field.value().clone()
                };
                conduit_core::StructuredFieldValue::new(field.name(), child).unwrap()
            })
            .collect(),
    )
    .unwrap()
}
#[test]
fn invalid_projection_skips_unsafe_later_cursor_laws_in_portable_execution() {
    let mut start = Pipeline::new(
        "dsp/oscillator-start",
        AudioPeriodicProjectionCandidate::semantic_type().unwrap(),
    );
    let req = request(125, 1, i64::MAX, u64::MAX, 0, 1, 3);
    let projection = AudioPeriodicProjectionCandidate::new(0, 0, 16384, 0, req).unwrap();
    let cursor = started(&start.run(projection.encode().unwrap()));
    let mut step = Pipeline::new(
        "dsp/oscillator-step",
        AudioPeriodicStepInput::semantic_type().unwrap(),
    );
    for period in [0i128, i128::MIN, i128::MAX] {
        for cancelled in [false, true] {
            let input = AudioPeriodicStepInput::new(cancelled, cursor.clone())
                .unwrap()
                .encode()
                .unwrap();
            let input = StructuredInfoValue::from_canonical_bytes(&input).unwrap();
            let input = replace(
                &input,
                &["cursor", "profile", "projection", "period_q8"],
                &period.to_le_bytes(),
            )
            .canonical_bytes()
            .unwrap();
            assert!(matches!(
                AudioPeriodicOutcome::decode(&step.run(input)).unwrap(),
                AudioPeriodicOutcome::InvalidCursor
            ));
        }
    }
}

#[test]
fn malformed_request_domains_refuse_without_evaluating_projection_divisions() {
    let mut pipeline = Pipeline::new(
        "dsp/oscillator-prepare",
        AudioPeriodicRequest::semantic_type().unwrap(),
    );
    let valid = request(125, 1, 0, 1, 1, 1, 3).encode().unwrap();
    let original = StructuredInfoValue::from_canonical_bytes(&valid).unwrap();
    for path in [
        vec!["frequency", "value", "numerator_hz"],
        vec!["frequency", "value", "denominator"],
        vec!["amplitude", "denominator"],
        vec!["phase", "denominator"],
        vec!["phase", "at", "denominator"],
    ] {
        let input = replace(&original, &path, &0u64.to_le_bytes())
            .canonical_bytes()
            .unwrap();
        assert!(matches!(
            AudioPeriodicStartOutcome::decode(&pipeline.run(input)).unwrap(),
            AudioPeriodicStartOutcome::InvalidRequest
        ));
    }
}
