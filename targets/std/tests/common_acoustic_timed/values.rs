//! Typed controls prepared before Play; no recurrence computation.
use conduit_audio::{
    AudioExactTimeOffset, AudioOriginIdentity, AudioTimelineIdentity, AudioTrajectoryAnchor,
};
use conduit_core::*;
use conduit_plot::rust_binding::NativeRustBinding;
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
pub fn tick(stage: &StructuredInfoType, origin: u64, frame: u64) -> Vec<u8> {
    let ty = ty_case(stage, "timed");
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
    StructuredInfoValue::variant(stage.clone(), "timed", value)
        .unwrap()
        .canonical_bytes()
        .unwrap()
}
