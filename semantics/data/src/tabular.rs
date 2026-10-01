//! Host-neutral finite typed rows and query outcomes without SQL or JSON semantics.

use crate::{
    TabularColumnSpec, TabularColumnType, TabularOptionalText, TabularPersonRow,
    TabularPersonRowSlot, TabularPersonRowsFour, TabularQueryResultFour, TabularQueryStatus,
    TabularSchemaFour,
};
use alloc::vec;
use conduit_core::{
    kind_id, StructuredInfoRefusal, StructuredInfoType, StructuredVariantCase,
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

fn native(
    value: Result<StructuredInfoType, conduit_form::rust_binding::NativeBindingRefusal>,
) -> StructuredInfoType {
    value.expect("generated tabular semantic type")
}
pub fn tabular_selected_text_type() -> StructuredInfoType {
    StructuredInfoType::leaf(kind_id("value/text")).expect("reviewed tabular text")
}
pub fn tabular_column_type() -> StructuredInfoType {
    native(TabularColumnType::semantic_type())
}
pub fn tabular_column_type_spec() -> StructuredInfoType {
    native(TabularColumnSpec::semantic_type())
}
pub fn tabular_schema_type() -> StructuredInfoType {
    native(TabularSchemaFour::semantic_type())
}
pub fn tabular_optional_text_type() -> StructuredInfoType {
    native(TabularOptionalText::semantic_type())
}
pub fn tabular_person_row_type() -> StructuredInfoType {
    native(TabularPersonRow::semantic_type())
}
pub fn tabular_row_slot_type() -> StructuredInfoType {
    native(TabularPersonRowSlot::semantic_type())
}
pub fn tabular_rows_four_type() -> StructuredInfoType {
    native(TabularPersonRowsFour::semantic_type())
}
pub fn tabular_query_status_type() -> StructuredInfoType {
    native(TabularQueryStatus::semantic_type())
}
pub fn tabular_query_result_type() -> StructuredInfoType {
    native(TabularQueryResultFour::semantic_type())
}

pub fn tabular_query_outcome_type() -> StructuredInfoType {
    StructuredInfoType::variant(
        kind_id("tabular/query-outcome-four@1"),
        vec![
            StructuredVariantCase::new("inline", tabular_query_result_type())
                .expect("reviewed inline case"),
            StructuredVariantCase::new(
                "materialized",
                StructuredInfoType::leaf(kind_id(RESOURCE_REFERENCE_INFO_ID))
                    .expect("reviewed resource reference"),
            )
            .expect("reviewed materialized case"),
        ],
    )
    .expect("reviewed query outcomes")
}
