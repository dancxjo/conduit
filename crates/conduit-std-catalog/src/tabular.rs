//! Finite typed rows and query outcomes without SQL or JSON semantics.

use alloc::{vec, vec::Vec};
use conduit_core::{
    kind_id, StructuredFieldType, StructuredInfoRefusal, StructuredInfoType, StructuredVariantCase,
    RESOURCE_REFERENCE_INFO_ID,
};

pub const TABULAR_SCHEMA_TYPE: &str = "TabularPersonSchema";
pub const TABULAR_PERSON_ROW_TYPE: &str = "TabularPersonRow";
pub const TABULAR_ROW_SLOT_TYPE: &str = "TabularPersonRowSlot";
pub const TABULAR_ROWS_FOUR_TYPE: &str = "TabularPersonRowsFour";
pub const TABULAR_QUERY_RESULT_TYPE: &str = "TabularQueryResultFour";
pub const TABULAR_QUERY_OUTCOME_TYPE: &str = "TabularQueryOutcomeFour";
pub const TABULAR_SELECTED_TEXT_TYPE: &str = "TabularSelectedText";
pub const TABULAR_MAXIMUM_ROWS: u16 = 4;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TabularRefusal {
    TooManyRows { maximum: u16, actual: usize },
    MalformedInfo,
    Structured(StructuredInfoRefusal),
}

impl From<StructuredInfoRefusal> for TabularRefusal {
    fn from(value: StructuredInfoRefusal) -> Self {
        Self::Structured(value)
    }
}

fn leaf(kind: &str) -> StructuredInfoType {
    StructuredInfoType::leaf(kind_id(kind)).expect("reviewed tabular leaf")
}

fn field(name: &str, value_type: StructuredInfoType) -> StructuredFieldType {
    StructuredFieldType::new(name, value_type).expect("reviewed tabular field")
}

fn case(name: &str, payload_type: StructuredInfoType) -> StructuredVariantCase {
    StructuredVariantCase::new(name, payload_type).expect("reviewed tabular case")
}

fn record(kind: &str, fields: Vec<StructuredFieldType>) -> StructuredInfoType {
    StructuredInfoType::record(kind_id(kind), fields).expect("reviewed tabular record")
}

fn bounded(value_type: StructuredInfoType, length: u16) -> StructuredInfoType {
    StructuredInfoType::collection(value_type, Some(length)).expect("bounded tabular collection")
}

fn unit_type() -> StructuredInfoType {
    leaf("value/unit@1")
}

fn text_type() -> StructuredInfoType {
    leaf("value/text@1")
}

fn count_type() -> StructuredInfoType {
    leaf("value/count@1")
}

fn bool_type() -> StructuredInfoType {
    leaf(conduit_core::BOOL_INFO_ID)
}

pub fn tabular_selected_text_type() -> StructuredInfoType {
    text_type()
}

pub fn tabular_column_type() -> StructuredInfoType {
    StructuredInfoType::variant(
        kind_id("tabular/column-type@1"),
        vec![
            case("boolean", unit_type()),
            case("count", unit_type()),
            case("optional_text", unit_type()),
            case("text", unit_type()),
        ],
    )
    .expect("reviewed column types")
}

pub fn tabular_column_type_spec() -> StructuredInfoType {
    record(
        "tabular/column-spec@1",
        vec![
            field("name", text_type()),
            field("value_type", tabular_column_type()),
        ],
    )
}

pub fn tabular_schema_type() -> StructuredInfoType {
    record(
        "tabular/person-schema@1",
        vec![
            field(
                "columns",
                bounded(tabular_column_type_spec(), TABULAR_MAXIMUM_ROWS),
            ),
            field("identity", text_type()),
        ],
    )
}

pub fn tabular_optional_text_type() -> StructuredInfoType {
    StructuredInfoType::variant(
        kind_id("tabular/optional-text@1"),
        vec![case("null", unit_type()), case("value", text_type())],
    )
    .expect("explicit nullable text")
}

pub fn tabular_person_row_type() -> StructuredInfoType {
    record(
        "tabular/person-row@1",
        vec![
            field("active", bool_type()),
            field("id", count_type()),
            field("name", text_type()),
            field("nickname", tabular_optional_text_type()),
        ],
    )
}

pub fn tabular_row_slot_type() -> StructuredInfoType {
    StructuredInfoType::variant(
        kind_id("tabular/person-row-slot@1"),
        vec![
            case("row", tabular_person_row_type()),
            case("unused", unit_type()),
        ],
    )
    .expect("reviewed row slots")
}

pub fn tabular_rows_four_type() -> StructuredInfoType {
    bounded(tabular_row_slot_type(), TABULAR_MAXIMUM_ROWS)
}

fn query_completion_type() -> StructuredInfoType {
    record(
        "tabular/query-completion@1",
        vec![
            field("emitted_rows", count_type()),
            field("end_of_results", bool_type()),
        ],
    )
}

fn query_error_type() -> StructuredInfoType {
    record(
        "tabular/query-error@1",
        vec![field("code", text_type()), field("message", text_type())],
    )
}

pub fn tabular_query_status_type() -> StructuredInfoType {
    StructuredInfoType::variant(
        kind_id("tabular/query-status@1"),
        vec![
            case("complete", query_completion_type()),
            case("error", query_error_type()),
        ],
    )
    .expect("reviewed query states")
}

pub fn tabular_query_result_type() -> StructuredInfoType {
    record(
        "tabular/query-result-four@1",
        vec![
            field("rows", tabular_rows_four_type()),
            field("schema", tabular_schema_type()),
            field("status", tabular_query_status_type()),
        ],
    )
}

pub fn tabular_query_outcome_type() -> StructuredInfoType {
    StructuredInfoType::variant(
        kind_id("tabular/query-outcome-four@1"),
        vec![
            case("inline", tabular_query_result_type()),
            case("materialized", leaf(RESOURCE_REFERENCE_INFO_ID)),
        ],
    )
    .expect("reviewed query outcomes")
}

pub(crate) fn tabular_unit_type() -> StructuredInfoType {
    unit_type()
}

pub(crate) fn tabular_text_type() -> StructuredInfoType {
    text_type()
}

pub(crate) fn tabular_count_type() -> StructuredInfoType {
    count_type()
}

pub(crate) fn tabular_bool_type() -> StructuredInfoType {
    bool_type()
}

pub(crate) fn tabular_query_completion_type() -> StructuredInfoType {
    query_completion_type()
}

pub(crate) fn tabular_query_error_type() -> StructuredInfoType {
    query_error_type()
}
