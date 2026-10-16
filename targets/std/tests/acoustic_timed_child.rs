//! Actual Source/kernel control refusal and exact recurrence retention.
use conduit_audio::{
    AudioExactTimeOffset, AudioOriginIdentity, AudioTimelineIdentity, AudioTrajectoryAnchor,
};
use conduit_core::*;
use conduit_plot::rust_binding::NativeRustBinding;
#[path = "../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
mod common_acoustic_timed;
#[path = "common_acoustic_value/kernel.rs"]
mod kernel;
#[path = "common_acoustic_value/seed.rs"]
mod seed;
use common_acoustic_timed::Fixture;
fn field(value: &StructuredInfoValue, name: &str) -> StructuredInfoValue {
    let StructuredInfoValueShape::Record(fields) = value.shape() else {
        panic!("record")
    };
    fields
        .iter()
        .find(|field| field.name() == name)
        .unwrap()
        .value()
        .clone()
}
fn payload(value: &StructuredInfoValue, wanted: &str) -> StructuredInfoValue {
    let StructuredInfoValueShape::Variant { tag, payload } = value.shape() else {
        panic!("variant")
    };
    assert_eq!(tag, wanted);
    payload.clone()
}
fn ty_field(ty: &StructuredInfoType, name: &str) -> StructuredInfoType {
    let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        panic!("record type")
    };
    fields
        .iter()
        .find(|field| field.name() == name)
        .unwrap()
        .value_type()
        .clone()
}
fn ty_case(ty: &StructuredInfoType, name: &str) -> StructuredInfoType {
    let StructuredInfoTypeShape::Variant { cases, .. } = ty.shape() else {
        panic!("variant type")
    };
    cases
        .iter()
        .find(|case| case.tag() == name)
        .unwrap()
        .payload_type()
        .clone()
}
fn record(ty: StructuredInfoType, fields: Vec<(&str, StructuredInfoValue)>) -> StructuredInfoValue {
    StructuredInfoValue::record(
        ty,
        fields
            .into_iter()
            .map(|(name, value)| StructuredFieldValue::new(name, value).unwrap())
            .collect(),
    )
    .unwrap()
}
fn initial(fixture: &Fixture) -> Vec<u8> {
    seed::seeds(&fixture.checked, &fixture.profiles).0
}
fn tick(fixture: &Fixture, origin: u64, frame: u64) -> Vec<u8> {
    let ty = ty_case(&fixture.stage, "timed");
    let anchor = AudioTrajectoryAnchor::new(
        AudioOriginIdentity::new(origin).unwrap(),
        AudioTimelineIdentity::new(1).unwrap(),
    )
    .unwrap();
    let time = AudioExactTimeOffset::new(8000, frame).unwrap();
    let value = record(
        ty.clone(),
        vec![
            (
                "anchor",
                StructuredInfoValue::from_canonical_bytes(&anchor.encode().unwrap()).unwrap(),
            ),
            (
                "time",
                StructuredInfoValue::from_canonical_bytes(&time.encode().unwrap()).unwrap(),
            ),
            (
                "cancelled",
                StructuredInfoValue::leaf(ty_field(&ty, "cancelled"), vec![0]).unwrap(),
            ),
        ],
    );
    StructuredInfoValue::variant(fixture.stage.clone(), "timed", value)
        .unwrap()
        .canonical_bytes()
        .unwrap()
}
fn advance(fixture: &Fixture, state: &[u8], origin: u64, frame: u64) -> Vec<u8> {
    kernel::run(
        fixture.plan.clone(),
        &fixture.front,
        fixture.pair(),
        state.to_vec(),
        tick(fixture, origin, frame),
        false,
    )
    .unwrap()
}
fn history(encoded: &[u8]) -> StructuredInfoValue {
    field(
        &payload(
            &StructuredInfoValue::from_canonical_bytes(encoded).unwrap(),
            "context",
        ),
        "cursor",
    )
}
fn status(cursor: &StructuredInfoValue) -> String {
    let status = field(cursor, "status");
    let StructuredInfoValueShape::Variant { tag, .. } = status.shape() else {
        panic!("status")
    };
    tag.into()
}
#[test]
fn timed_controls_refuse_stale_late_and_future_without_advancing_the_recurrence() {
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(check)
        .unwrap()
        .join()
        .unwrap();
}
fn check() {
    let fixture = common_acoustic_timed::prepare();
    let mut state = advance(&fixture, &initial(&fixture), 1, 0);
    let original = history(&state);
    assert_eq!(status(&original), "sample");
    for (origin, frame, expected) in [
        (2, 1, "stale_epoch"),
        (1, 0, "late_control"),
        (1, 3, "future_control"),
    ] {
        let rejected_bytes = advance(&fixture, &state, origin, frame);
        let rejected = history(&rejected_bytes);
        assert_eq!(status(&rejected), expected);
        for name in [
            "history_count",
            "history_a",
            "history_b",
            "history_c",
            "history_d",
        ] {
            assert_eq!(field(&rejected, name), field(&original, name));
        }
        let before = original.clone();
        let after = rejected.clone();
        for name in ["oscillator", "resonator", "sample", "clipped"] {
            assert_eq!(field(&after, name), field(&before, name));
        }
        state = rejected_bytes;
    }
    for frame in 1..4 {
        state = advance(&fixture, &state, 1, frame);
    }
    let final_state = history(&state);
    assert_eq!(status(&final_state), "sample");
    for (name, expected) in ["history_a", "history_b", "history_c", "history_d"]
        .into_iter()
        .zip([24_i128, 104, 263, 425])
    {
        let value = field(&final_state, name);
        let StructuredInfoValueShape::Leaf(bytes) = value.shape() else {
            panic!("sample")
        };
        assert_eq!(i128::from_le_bytes(bytes.try_into().unwrap()), expected);
    }
}
