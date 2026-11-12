//! Fabrication-host definition of finite descriptive media read outcomes.
use alloc::vec;
use conduit_core::{
    kind_id, StructuredFieldType as Field, StructuredInfoType as Type,
    StructuredVariantCase as Case,
};
pub fn count_definition() -> Type {
    Type::leaf(kind_id("value/u64")).expect("Count")
}
pub fn bytes_definition() -> Type {
    Type::leaf(kind_id("value/bytes")).expect("Bytes")
}
pub fn unit_definition() -> Type {
    Type::leaf(kind_id("value/empty")).expect("Empty")
}
pub fn gap_reason_definition() -> Type {
    Type::variant(
        kind_id("resource/read-record/gap-reason@1"),
        vec![
            Case::new("vacant", unit_definition()).unwrap(),
            Case::new("unpublished-or-damaged", unit_definition()).unwrap(),
            Case::new("corrupt", unit_definition()).unwrap(),
        ],
    )
    .expect("finite gap reason")
}
pub fn recorded_definition() -> Type {
    Type::record(
        kind_id("resource/read-record/recorded@1"),
        vec![
            Field::new("index", count_definition()).unwrap(),
            Field::new("captured-end", count_definition()).unwrap(),
            Field::new("record", bytes_definition()).unwrap(),
        ],
    )
    .expect("finite recorded result")
}
pub fn gap_definition() -> Type {
    Type::record(
        kind_id("resource/read-record/gap@1"),
        vec![
            Field::new("index", count_definition()).unwrap(),
            Field::new("captured-end", count_definition()).unwrap(),
            Field::new("reason", gap_reason_definition()).unwrap(),
        ],
    )
    .expect("finite gap result")
}
pub fn definition() -> Type {
    Type::variant(
        kind_id("resource/read-record/result@1"),
        vec![
            Case::new("recorded", recorded_definition()).unwrap(),
            Case::new("gap", gap_definition()).unwrap(),
            Case::new("end", count_definition()).unwrap(),
            Case::new("cancelled", unit_definition()).unwrap(),
            Case::new("busy", unit_definition()).unwrap(),
            Case::new("recovery-required", unit_definition()).unwrap(),
            Case::new("unavailable", unit_definition()).unwrap(),
        ],
    )
    .expect("finite read result")
}
