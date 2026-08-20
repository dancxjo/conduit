//! Deterministic provider and filter for the finite tabular contract.

use alloc::{string::ToString, vec, vec::Vec};
use conduit_core::{
    BoundedResourceRef, InfoBool, StructuredFieldValue, StructuredInfoType,
    StructuredInfoTypeShape, StructuredInfoValue, StructuredInfoValueShape,
    RESOURCE_REFERENCE_INFO_ID,
};

use super::tabular::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersonRow<'a> {
    pub id: u64,
    pub name: &'a str,
    pub nickname: Option<&'a str>,
    pub active: bool,
}

pub fn deterministic_query_result(
    rows: &[PersonRow<'_>],
) -> Result<StructuredInfoValue, TabularRefusal> {
    if rows.len() > usize::from(TABULAR_MAXIMUM_ROWS) {
        return Err(TabularRefusal::TooManyRows {
            maximum: TABULAR_MAXIMUM_ROWS,
            actual: rows.len(),
        });
    }
    let emitted = rows.len();
    let mut slots = rows
        .iter()
        .map(person_row_slot)
        .collect::<Result<Vec<_>, _>>()?;
    while slots.len() < usize::from(TABULAR_MAXIMUM_ROWS) {
        slots.push(unit_variant(tabular_row_slot_type(), "unused")?);
    }
    record_value(
        tabular_query_result_type(),
        vec![
            ("rows", collection_value(tabular_row_slot_type(), slots)?),
            ("schema", schema_value()?),
            ("status", completion_value(emitted as u64)?),
        ],
    )
}

pub fn deterministic_person_provider() -> Result<StructuredInfoValue, TabularRefusal> {
    deterministic_query_result(&[
        PersonRow {
            id: 1,
            name: "Ada",
            nickname: None,
            active: true,
        },
        PersonRow {
            id: 2,
            name: "Grace",
            nickname: Some("Amazing Grace"),
            active: false,
        },
        PersonRow {
            id: 3,
            name: "Edsger",
            nickname: None,
            active: true,
        },
    ])
}

pub fn deterministic_query_error(
    code: &str,
    message: &str,
) -> Result<StructuredInfoValue, TabularRefusal> {
    let slots = (0..TABULAR_MAXIMUM_ROWS)
        .map(|_| unit_variant(tabular_row_slot_type(), "unused"))
        .collect::<Result<Vec<_>, _>>()?;
    let payload = record_value(
        tabular_query_error_type(),
        vec![("code", text_value(code)), ("message", text_value(message))],
    )?;
    let status = StructuredInfoValue::variant(tabular_query_status_type(), "error", payload)?;
    record_value(
        tabular_query_result_type(),
        vec![
            ("rows", collection_value(tabular_row_slot_type(), slots)?),
            ("schema", schema_value()?),
            ("status", status),
        ],
    )
}

pub fn filter_active_rows(
    result: &StructuredInfoValue,
) -> Result<StructuredInfoValue, TabularRefusal> {
    if result.value_type() != &tabular_query_result_type() {
        return Err(TabularRefusal::MalformedInfo);
    }
    let status = record_field(result, "status")?;
    let StructuredInfoValueShape::Variant { tag, .. } = status.shape() else {
        return Err(TabularRefusal::MalformedInfo);
    };
    if tag == "error" {
        return Ok(result.clone());
    }
    if tag != "complete" {
        return Err(TabularRefusal::MalformedInfo);
    }
    let rows = collection_field(result, "rows")?;
    let mut kept = Vec::new();
    for slot in rows {
        let StructuredInfoValueShape::Variant { tag, payload } = slot.shape() else {
            return Err(TabularRefusal::MalformedInfo);
        };
        if tag == "row" && leaf_bool(record_field(payload, "active")?)? {
            kept.push(slot.clone());
        }
    }
    let count = kept.len();
    while kept.len() < usize::from(TABULAR_MAXIMUM_ROWS) {
        kept.push(unit_variant(tabular_row_slot_type(), "unused")?);
    }
    record_value(
        tabular_query_result_type(),
        vec![
            ("rows", collection_value(tabular_row_slot_type(), kept)?),
            ("schema", record_field(result, "schema")?.clone()),
            ("status", completion_value(count as u64)?),
        ],
    )
}

pub fn materialized_query_outcome(
    reference: &BoundedResourceRef,
) -> Result<StructuredInfoValue, TabularRefusal> {
    let encoded = reference
        .encode()
        .map_err(|_| TabularRefusal::MalformedInfo)?;
    let resource = StructuredInfoValue::leaf(
        StructuredInfoType::leaf(conduit_core::kind_id(RESOURCE_REFERENCE_INFO_ID))?,
        encoded,
    )?;
    Ok(StructuredInfoValue::variant(
        tabular_query_outcome_type(),
        "materialized",
        resource,
    )?)
}

fn schema_value() -> Result<StructuredInfoValue, TabularRefusal> {
    let columns = [
        ("active", "boolean"),
        ("id", "count"),
        ("name", "text"),
        ("nickname", "optional_text"),
    ]
    .into_iter()
    .map(|(name, tag)| {
        record_value(
            tabular_column_type_spec(),
            vec![
                ("name", text_value(name)),
                ("value_type", unit_variant(tabular_column_type(), tag)?),
            ],
        )
    })
    .collect::<Result<Vec<_>, _>>()?;
    record_value(
        tabular_schema_type(),
        vec![
            (
                "columns",
                collection_value(tabular_column_type_spec(), columns)?,
            ),
            ("identity", text_value("tabular/person@1")),
        ],
    )
}

fn person_row_slot(row: &PersonRow<'_>) -> Result<StructuredInfoValue, TabularRefusal> {
    let nickname = match row.nickname {
        Some(value) => StructuredInfoValue::variant(
            tabular_optional_text_type(),
            "value",
            text_value(value),
        )?,
        None => unit_variant(tabular_optional_text_type(), "null")?,
    };
    let value = record_value(
        tabular_person_row_type(),
        vec![
            ("active", bool_value(row.active)),
            ("id", count_value(row.id)),
            ("name", text_value(row.name)),
            ("nickname", nickname),
        ],
    )?;
    Ok(StructuredInfoValue::variant(
        tabular_row_slot_type(),
        "row",
        value,
    )?)
}

fn completion_value(emitted: u64) -> Result<StructuredInfoValue, TabularRefusal> {
    let status_type = tabular_query_status_type();
    let payload = record_value(
        tabular_query_completion_type(),
        vec![
            ("emitted_rows", count_value(emitted)),
            ("end_of_results", bool_value(true)),
        ],
    )?;
    Ok(StructuredInfoValue::variant(
        status_type,
        "complete",
        payload,
    )?)
}

fn unit_variant(
    value_type: StructuredInfoType,
    tag: &str,
) -> Result<StructuredInfoValue, TabularRefusal> {
    Ok(StructuredInfoValue::variant(
        value_type,
        tag,
        StructuredInfoValue::leaf(tabular_unit_type(), Vec::new())?,
    )?)
}

fn record_value(
    value_type: StructuredInfoType,
    fields: Vec<(&str, StructuredInfoValue)>,
) -> Result<StructuredInfoValue, TabularRefusal> {
    Ok(StructuredInfoValue::record(
        value_type,
        fields
            .into_iter()
            .map(|(name, value)| StructuredFieldValue::new(name, value))
            .collect::<Result<Vec<_>, _>>()?,
    )?)
}

fn collection_value(
    element_type: StructuredInfoType,
    values: Vec<StructuredInfoValue>,
) -> Result<StructuredInfoValue, TabularRefusal> {
    let length = u16::try_from(values.len()).map_err(|_| TabularRefusal::MalformedInfo)?;
    Ok(StructuredInfoValue::collection(
        StructuredInfoType::collection(element_type, Some(length))?,
        values,
    )?)
}

fn text_value(value: &str) -> StructuredInfoValue {
    StructuredInfoValue::leaf(tabular_text_type(), value.as_bytes().to_vec())
        .expect("bounded provider text")
}

fn count_value(value: u64) -> StructuredInfoValue {
    StructuredInfoValue::leaf(tabular_count_type(), value.to_string().into_bytes())
        .expect("bounded provider count")
}

fn bool_value(value: bool) -> StructuredInfoValue {
    StructuredInfoValue::leaf(
        tabular_bool_type(),
        InfoBool::new(value).encode().to_vec(),
    )
    .expect("bounded provider Boolean")
}

fn record_field<'a>(
    value: &'a StructuredInfoValue,
    name: &str,
) -> Result<&'a StructuredInfoValue, TabularRefusal> {
    let StructuredInfoValueShape::Record(fields) = value.shape() else {
        return Err(TabularRefusal::MalformedInfo);
    };
    fields
        .iter()
        .find(|field| field.name() == name)
        .map(StructuredFieldValue::value)
        .ok_or(TabularRefusal::MalformedInfo)
}

fn collection_field<'a>(
    value: &'a StructuredInfoValue,
    name: &str,
) -> Result<&'a [StructuredInfoValue], TabularRefusal> {
    let StructuredInfoValueShape::Collection(values) = record_field(value, name)?.shape() else {
        return Err(TabularRefusal::MalformedInfo);
    };
    Ok(values)
}

fn leaf_bool(value: &StructuredInfoValue) -> Result<bool, TabularRefusal> {
    let StructuredInfoValueShape::Leaf(bytes) = value.shape() else {
        return Err(TabularRefusal::MalformedInfo);
    };
    InfoBool::decode(bytes)
        .map(InfoBool::get)
        .map_err(|_| TabularRefusal::MalformedInfo)
}

pub fn tabular_variant_payload_type(
    value_type: &StructuredInfoType,
    tag: &str,
) -> Result<StructuredInfoType, TabularRefusal> {
    let StructuredInfoTypeShape::Variant { cases, .. } = value_type.shape() else {
        return Err(TabularRefusal::MalformedInfo);
    };
    cases
        .iter()
        .find(|case| case.tag() == tag)
        .map(|case| case.payload_type().clone())
        .ok_or(TabularRefusal::MalformedInfo)
}
