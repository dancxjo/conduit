//! Shared input construction only; no registered tests.
use super::{plan::SourceSchema, state::vector};
use conduit_core::*;
use std::collections::BTreeMap;
pub fn input(
    schema: &SourceSchema,
    features: &[f32; 20],
    history: &[f32; 128],
    period: u16,
    previous: u16,
) -> BTreeMap<String, Vec<u8>> {
    let mut result = BTreeMap::new();
    for (name, values) in [
        ("features", features.as_slice()),
        ("history", history.as_slice()),
    ] {
        result.insert(
            name.into(),
            vector(schema.ty(name), values).canonical_bytes().unwrap(),
        );
    }
    for (name, value) in [("period", period), ("previous_period", previous)] {
        let leaf = StructuredInfoValue::leaf(
            StructuredInfoType::leaf(kind_id("value/u16")).unwrap(),
            value.to_le_bytes().to_vec(),
        )
        .unwrap();
        result.insert(
            name.into(),
            StructuredInfoValue::nominal(schema.ty(name).clone(), leaf)
                .unwrap()
                .canonical_bytes()
                .unwrap(),
        );
    }
    result
}
