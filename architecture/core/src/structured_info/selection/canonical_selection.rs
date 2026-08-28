use alloc::vec::Vec;

use super::{
    StructuredCanonicalSelection, StructuredInfoRefusal, StructuredInfoType, StructuredSelector,
    StructuredSelectorOperation, StructuredSelectorRefusal,
};
use crate::structured_info::canonical::Cursor;

impl StructuredSelector {
    pub fn select_canonical_into(
        &self,
        input: &[u8],
        input_type: &[u8],
        output_type: &[u8],
        output: &mut Vec<u8>,
    ) -> Result<StructuredCanonicalSelection, StructuredSelectorRefusal> {
        let Some(node) = input.strip_prefix(input_type) else {
            return Err(StructuredSelectorRefusal::WrongInputType);
        };
        let mut cursor = Cursor::new(node);
        let selected = match (&self.operation, self.input_type.shape()) {
            (
                StructuredSelectorOperation::Field(wanted),
                crate::StructuredInfoTypeShape::Record { fields, .. },
            ) => select_field(wanted, fields, &mut cursor)?,
            (
                StructuredSelectorOperation::Index(wanted),
                crate::StructuredInfoTypeShape::Collection { element, length },
            ) => select_index(*wanted, element, length, &mut cursor)?,
            (
                StructuredSelectorOperation::Variant {
                    tag: wanted,
                    unmatched,
                },
                crate::StructuredInfoTypeShape::Variant { cases, .. },
            ) => {
                expect_byte(&mut cursor, 3)?;
                let tag = cursor.bytes().map_err(malformed)?;
                let case = cases
                    .iter()
                    .find(|case| case.tag().as_bytes() == tag)
                    .ok_or(StructuredSelectorRefusal::MalformedCheckedValue)?;
                let value = take_value_node(case.payload_type(), &mut cursor)?;
                if case.tag() != wanted {
                    finish(&cursor)?;
                    return Ok(StructuredCanonicalSelection::Unmatched(*unmatched));
                }
                value
            }
            _ => return Err(StructuredSelectorRefusal::MalformedCheckedValue),
        };
        finish(&cursor)?;
        if output_type.len().saturating_add(selected.len()) > output.capacity() {
            return Err(StructuredSelectorRefusal::CanonicalEncodingTooLarge);
        }
        output.clear();
        output.extend_from_slice(output_type);
        output.extend_from_slice(selected);
        Ok(StructuredCanonicalSelection::Matched)
    }
}

fn select_field<'a>(
    wanted: &str,
    fields: &[crate::StructuredFieldType],
    cursor: &mut Cursor<'a>,
) -> Result<&'a [u8], StructuredSelectorRefusal> {
    expect_byte(cursor, 2)?;
    expect_length(cursor, fields.len())?;
    let mut selected = None;
    for field in fields {
        expect_text(cursor, field.name())?;
        let value = take_value_node(field.value_type(), cursor)?;
        if field.name() == wanted {
            selected = Some(value);
        }
    }
    selected.ok_or(StructuredSelectorRefusal::MalformedCheckedValue)
}

fn select_index<'a>(
    wanted: u16,
    element: &StructuredInfoType,
    length: u16,
    cursor: &mut Cursor<'a>,
) -> Result<&'a [u8], StructuredSelectorRefusal> {
    expect_byte(cursor, 1)?;
    expect_length(cursor, usize::from(length))?;
    let mut selected = None;
    for index in 0..length {
        let value = take_value_node(element, cursor)?;
        if index == wanted {
            selected = Some(value);
        }
    }
    selected.ok_or(StructuredSelectorRefusal::MalformedCheckedValue)
}

fn finish(cursor: &Cursor<'_>) -> Result<(), StructuredSelectorRefusal> {
    cursor
        .remaining
        .is_empty()
        .then_some(())
        .ok_or(StructuredSelectorRefusal::MalformedCheckedValue)
}

fn malformed(_: StructuredInfoRefusal) -> StructuredSelectorRefusal {
    StructuredSelectorRefusal::MalformedCheckedValue
}

fn expect_byte(cursor: &mut Cursor<'_>, expected: u8) -> Result<(), StructuredSelectorRefusal> {
    (cursor.byte().map_err(malformed)? == expected)
        .then_some(())
        .ok_or(StructuredSelectorRefusal::MalformedCheckedValue)
}

fn expect_length(
    cursor: &mut Cursor<'_>,
    expected: usize,
) -> Result<(), StructuredSelectorRefusal> {
    (cursor.length().map_err(malformed)? == expected)
        .then_some(())
        .ok_or(StructuredSelectorRefusal::MalformedCheckedValue)
}

fn expect_text(cursor: &mut Cursor<'_>, expected: &str) -> Result<(), StructuredSelectorRefusal> {
    (cursor.bytes().map_err(malformed)? == expected.as_bytes())
        .then_some(())
        .ok_or(StructuredSelectorRefusal::MalformedCheckedValue)
}

fn take_value_node<'a>(
    expected: &StructuredInfoType,
    cursor: &mut Cursor<'a>,
) -> Result<&'a [u8], StructuredSelectorRefusal> {
    let before = cursor.remaining;
    skip_value_node(expected, cursor)?;
    Ok(&before[..before.len() - cursor.remaining.len()])
}

fn skip_value_node(
    expected: &StructuredInfoType,
    cursor: &mut Cursor<'_>,
) -> Result<(), StructuredSelectorRefusal> {
    match expected.shape() {
        crate::StructuredInfoTypeShape::Leaf(_) => {
            expect_byte(cursor, 0)?;
            cursor.bytes().map_err(malformed)?;
        }
        crate::StructuredInfoTypeShape::Collection { element, length } => {
            expect_byte(cursor, 1)?;
            expect_length(cursor, usize::from(length))?;
            for _ in 0..length {
                skip_value_node(element, cursor)?;
            }
        }
        crate::StructuredInfoTypeShape::Record { fields, .. } => {
            expect_byte(cursor, 2)?;
            expect_length(cursor, fields.len())?;
            for field in fields {
                expect_text(cursor, field.name())?;
                skip_value_node(field.value_type(), cursor)?;
            }
        }
        crate::StructuredInfoTypeShape::Variant { cases, .. } => {
            expect_byte(cursor, 3)?;
            let tag = cursor.bytes().map_err(malformed)?;
            let case = cases
                .iter()
                .find(|case| case.tag().as_bytes() == tag)
                .ok_or(StructuredSelectorRefusal::MalformedCheckedValue)?;
            skip_value_node(case.payload_type(), cursor)?;
        }
    }
    Ok(())
}
