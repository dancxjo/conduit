//! Typed completion substitution fixture, independent of descriptor policy.
use conduit_core::{StructuredFieldValue, StructuredInfoValue, StructuredInfoValueShape};

pub(super) fn replace_completed_leaf(bytes: &[u8], field: &str, changed: &[u8]) -> Vec<u8> {
    let value = StructuredInfoValue::from_canonical_bytes(bytes).unwrap();
    let StructuredInfoValueShape::Variant { payload, .. } = value.shape() else {
        panic!("variant")
    };
    let StructuredInfoValueShape::Record(fields) = payload.shape() else {
        panic!("record")
    };
    let fields = fields
        .iter()
        .map(|member| {
            StructuredFieldValue::new(
                member.name(),
                if member.name() == field {
                    StructuredInfoValue::leaf(member.value().value_type().clone(), changed.to_vec())
                        .unwrap()
                } else {
                    member.value().clone()
                },
            )
            .unwrap()
        })
        .collect();
    let payload = StructuredInfoValue::record(payload.value_type().clone(), fields).unwrap();
    StructuredInfoValue::variant(value.value_type().clone(), "completed", payload)
        .unwrap()
        .canonical_bytes()
        .unwrap()
}
