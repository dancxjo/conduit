//! Exact adaptation between portable text Info and the typed-record waist.

use conduit_core::{kind_id, StructuredInfoType, StructuredInfoValue};

use crate::{typed_record_value, value_from_typed_record, TypedRecordFrameRefusal};

pub const TEXT_INFO_ID: &str = "value/text@1";

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum TextRecordRefusal {
    WrongTextValueType,
    WrongTypedRecordValueType,
    TypedRecord(TypedRecordFrameRefusal),
}

/// Wrap one exact portable text value without interpreting or normalizing it.
pub fn typed_record_from_text(
    text: &StructuredInfoValue,
) -> Result<StructuredInfoValue, TextRecordRefusal> {
    if text.value_type() != &text_type() {
        return Err(TextRecordRefusal::WrongTextValueType);
    }
    typed_record_value(text).map_err(TextRecordRefusal::TypedRecord)
}

/// Recover text only when the record's retained payload identity is exactly text.
pub fn text_from_typed_record(
    record: &StructuredInfoValue,
) -> Result<StructuredInfoValue, TextRecordRefusal> {
    let value = value_from_typed_record(record).map_err(|error| {
        if error == TypedRecordFrameRefusal::WrongTypedRecordValueType {
            TextRecordRefusal::WrongTypedRecordValueType
        } else {
            TextRecordRefusal::TypedRecord(error)
        }
    })?;
    if value.value_type() != &text_type() {
        return Err(TextRecordRefusal::WrongTextValueType);
    }
    Ok(value)
}

pub fn text_type() -> StructuredInfoType {
    StructuredInfoType::leaf(kind_id(TEXT_INFO_ID)).expect("the text identity is finite")
}
