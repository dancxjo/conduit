//! Five independent finite Info specimens for cross-domain Form conformance.
//!
//! These are deliberately separate nominal schemas. Shared structured machinery
//! supplies only shape and bounds; it does not turn them into one document type.

use alloc::vec;
use conduit_core::{
    kind_id, InfoBool, Quantity, QuantityUnit, StructuredFieldType, StructuredFieldValue,
    StructuredInfoType, StructuredInfoValue, StructuredVariantCase,
};

pub const GEOMETRY_REGION_TYPE: &str = "GeometryRegion";
pub const ROBOTICS_RANGE_TYPE: &str = "RoboticsRangeSample";
pub const LANGUAGE_ANNOTATION_TYPE: &str = "LanguageAnnotation";
pub const MESSAGE_ENVELOPE_TYPE: &str = "MessageEnvelope";
pub const EDUCATION_FEEDBACK_TYPE: &str = "EducationFeedback";

fn text_type() -> StructuredInfoType {
    StructuredInfoType::leaf(kind_id("value/text@1")).expect("text leaf is finite")
}

fn count_type() -> StructuredInfoType {
    StructuredInfoType::leaf(kind_id("value/count@1")).expect("count leaf is finite")
}

fn bool_type() -> StructuredInfoType {
    StructuredInfoType::leaf(kind_id(conduit_core::BOOL_INFO_ID)).expect("Boolean leaf is finite")
}

fn quantity_type() -> StructuredInfoType {
    StructuredInfoType::leaf(kind_id(conduit_core::QUANTITY_INFO_ID))
        .expect("quantity leaf is finite")
}

fn field(name: &str, value_type: StructuredInfoType) -> StructuredFieldType {
    StructuredFieldType::new(name, value_type).expect("reviewed field name is finite and unique")
}

fn value_field(name: &str, value: StructuredInfoValue) -> StructuredFieldValue {
    StructuredFieldValue::new(name, value).expect("reviewed field name is finite and unique")
}

fn text(value: &str) -> StructuredInfoValue {
    StructuredInfoValue::leaf(text_type(), value.as_bytes().to_vec())
        .expect("reviewed text specimen is bounded")
}

fn count(value: u64) -> StructuredInfoValue {
    StructuredInfoValue::leaf(count_type(), value.to_le_bytes().to_vec())
        .expect("count encoding is bounded")
}

fn boolean(value: bool) -> StructuredInfoValue {
    StructuredInfoValue::leaf(bool_type(), InfoBool::new(value).encode().to_vec())
        .expect("Boolean encoding is bounded")
}

fn quantity(value: i64, unit: QuantityUnit) -> StructuredInfoValue {
    StructuredInfoValue::leaf(
        quantity_type(),
        Quantity::new(value, unit).encode().to_vec(),
    )
    .expect("quantity encoding is bounded")
}

pub fn geometry_region_type() -> StructuredInfoType {
    StructuredInfoType::record(
        kind_id("geometry/region-2d@1"),
        vec![
            field("frame", text_type()),
            field("height", quantity_type()),
            field("width", quantity_type()),
            field("x", quantity_type()),
            field("y", quantity_type()),
        ],
    )
    .expect("reviewed geometry schema is finite")
}

pub fn geometry_region_example() -> StructuredInfoValue {
    StructuredInfoValue::record(
        geometry_region_type(),
        vec![
            value_field("frame", text("image/content")),
            value_field("height", quantity(480, QuantityUnit::Millimeter)),
            value_field("width", quantity(640, QuantityUnit::Millimeter)),
            value_field("x", quantity(12, QuantityUnit::Millimeter)),
            value_field("y", quantity(24, QuantityUnit::Millimeter)),
        ],
    )
    .expect("reviewed geometry specimen matches its schema")
}

pub fn robotics_range_sample_type() -> StructuredInfoType {
    StructuredInfoType::record(
        kind_id("robotics/range-sample@2"),
        vec![
            field("distance", quantity_type()),
            field("frame", text_type()),
            field("uncertainty", quantity_type()),
        ],
    )
    .expect("reviewed robotics schema is finite")
}

pub fn robotics_range_sample_example() -> StructuredInfoValue {
    StructuredInfoValue::record(
        robotics_range_sample_type(),
        vec![
            value_field("distance", quantity(850, QuantityUnit::Millimeter)),
            value_field("frame", text("sensor/forward")),
            value_field("uncertainty", quantity(5, QuantityUnit::Millimeter)),
        ],
    )
    .expect("reviewed robotics specimen matches its schema")
}

pub fn language_annotation_type() -> StructuredInfoType {
    let tokens = StructuredInfoType::collection(text_type(), Some(2))
        .expect("two-token collection is finite");
    StructuredInfoType::record(
        kind_id("language/annotation@1"),
        vec![
            field("end", count_type()),
            field("label", text_type()),
            field("start", count_type()),
            field("tokens", tokens),
        ],
    )
    .expect("reviewed language schema is finite")
}

pub fn language_annotation_example() -> StructuredInfoValue {
    let tokens_type = StructuredInfoType::collection(text_type(), Some(2))
        .expect("two-token collection is finite");
    let tokens = StructuredInfoValue::collection(tokens_type, vec![text("bright"), text("star")])
        .expect("reviewed tokens match their finite collection");
    StructuredInfoValue::record(
        language_annotation_type(),
        vec![
            value_field("end", count(11)),
            value_field("label", text("noun-phrase")),
            value_field("start", count(0)),
            value_field("tokens", tokens),
        ],
    )
    .expect("reviewed annotation specimen matches its schema")
}

fn delivery_state_type() -> StructuredInfoType {
    StructuredInfoType::variant(
        kind_id("messaging/delivery-state@1"),
        vec![
            StructuredVariantCase::new("delivered", bool_type()).expect("reviewed variant tag"),
            StructuredVariantCase::new("queued", count_type()).expect("reviewed variant tag"),
        ],
    )
    .expect("reviewed delivery state is finite")
}

pub fn message_envelope_type() -> StructuredInfoType {
    StructuredInfoType::record(
        kind_id("messaging/envelope@1"),
        vec![
            field("message_id", text_type()),
            field("state", delivery_state_type()),
            field("subject", text_type()),
        ],
    )
    .expect("reviewed messaging schema is finite")
}

pub fn message_envelope_example() -> StructuredInfoValue {
    let state = StructuredInfoValue::variant(delivery_state_type(), "delivered", boolean(true))
        .expect("reviewed delivery state matches its schema");
    StructuredInfoValue::record(
        message_envelope_type(),
        vec![
            value_field("message_id", text("message/7")),
            value_field("state", state),
            value_field("subject", text("lesson/feedback")),
        ],
    )
    .expect("reviewed envelope specimen matches its schema")
}

fn assessment_outcome_type() -> StructuredInfoType {
    StructuredInfoType::variant(
        kind_id("education/assessment-outcome@1"),
        vec![
            StructuredVariantCase::new("passed", bool_type()).expect("reviewed variant tag"),
            StructuredVariantCase::new("retry", text_type()).expect("reviewed variant tag"),
        ],
    )
    .expect("reviewed assessment outcome is finite")
}

pub fn education_feedback_type() -> StructuredInfoType {
    StructuredInfoType::record(
        kind_id("education/feedback@1"),
        vec![
            field("outcome", assessment_outcome_type()),
            field("prompt_id", text_type()),
            field("score", quantity_type()),
        ],
    )
    .expect("reviewed education schema is finite")
}

pub fn education_feedback_example() -> StructuredInfoValue {
    let outcome = StructuredInfoValue::variant(assessment_outcome_type(), "passed", boolean(true))
        .expect("reviewed outcome matches its schema");
    StructuredInfoValue::record(
        education_feedback_type(),
        vec![
            value_field("outcome", outcome),
            value_field("prompt_id", text("question/3")),
            value_field("score", quantity(88, QuantityUnit::Percent)),
        ],
    )
    .expect("reviewed feedback specimen matches its schema")
}
