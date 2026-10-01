//! Deterministic reference provider and filter for the finite tabular contract.

use crate::tabular::*;
use crate::{
    TabularColumnSpec, TabularColumnType, TabularOptionalText, TabularPersonRowSlot,
    TabularPersonRowsFour, TabularQueryResultFour, TabularQueryStatus, TabularSchemaFour,
};
use alloc::{string::ToString, vec, vec::Vec};
use conduit_core::{
    BoundedResourceRef, StructuredInfoType, StructuredInfoTypeShape, StructuredInfoValue,
    RESOURCE_REFERENCE_INFO_ID,
};
use conduit_form::rust_binding::NativeRustBinding;

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
    slots.resize_with(
        usize::from(TABULAR_MAXIMUM_ROWS),
        TabularPersonRowSlot::unused,
    );
    query_result(
        slots,
        TabularQueryStatus::complete(emitted as u64, true).map_err(native_error)?,
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
    query_result(
        vec![TabularPersonRowSlot::unused(); usize::from(TABULAR_MAXIMUM_ROWS)],
        TabularQueryStatus::error(code.to_string(), message.to_string()).map_err(native_error)?,
    )
}

pub fn filter_active_rows(
    result: &StructuredInfoValue,
) -> Result<StructuredInfoValue, TabularRefusal> {
    let decoded = TabularQueryResultFour::from_structured(result.clone()).map_err(native_error)?;
    if matches!(decoded.status(), TabularQueryStatus::Error(_)) {
        return Ok(result.clone());
    }
    let mut kept = decoded
        .rows()
        .get()
        .iter()
        .filter(|slot| matches!(slot, TabularPersonRowSlot::Row(row) if *row.active()))
        .cloned()
        .collect::<Vec<_>>();
    let count = kept.len();
    kept.resize_with(
        usize::from(TABULAR_MAXIMUM_ROWS),
        TabularPersonRowSlot::unused,
    );
    TabularQueryResultFour::new(
        fixed_rows(kept)?,
        decoded.schema().clone(),
        TabularQueryStatus::complete(count as u64, true).map_err(native_error)?,
    )
    .map_err(native_error)?
    .into_structured()
    .map_err(native_error)
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

fn query_result(
    slots: Vec<TabularPersonRowSlot>,
    status: TabularQueryStatus,
) -> Result<StructuredInfoValue, TabularRefusal> {
    TabularQueryResultFour::new(fixed_rows(slots)?, schema()?, status)
        .map_err(native_error)?
        .into_structured()
        .map_err(native_error)
}
fn fixed_rows(slots: Vec<TabularPersonRowSlot>) -> Result<TabularPersonRowsFour, TabularRefusal> {
    TabularPersonRowsFour::new(
        slots
            .try_into()
            .map_err(|_| TabularRefusal::MalformedInfo)?,
    )
    .map_err(native_error)
}
fn schema() -> Result<TabularSchemaFour, TabularRefusal> {
    let columns = [
        ("active", TabularColumnType::boolean()),
        ("id", TabularColumnType::count()),
        ("name", TabularColumnType::text()),
        ("nickname", TabularColumnType::optional_text()),
    ]
    .map(|(name, value_type)| {
        TabularColumnSpec::new(name.to_string(), value_type).map_err(native_error)
    })
    .into_iter()
    .collect::<Result<Vec<_>, _>>()?;
    TabularSchemaFour::new(
        columns
            .try_into()
            .map_err(|_| TabularRefusal::MalformedInfo)?,
        "tabular/person@1".to_string(),
    )
    .map_err(native_error)
}
fn person_row_slot(row: &PersonRow<'_>) -> Result<TabularPersonRowSlot, TabularRefusal> {
    let nickname = match row.nickname {
        Some(value) => TabularOptionalText::value(value.to_string()).map_err(native_error)?,
        None => TabularOptionalText::null(),
    };
    TabularPersonRowSlot::row(row.active, row.id, row.name.to_string(), nickname)
        .map_err(native_error)
}
fn native_error(_: conduit_form::rust_binding::NativeBindingRefusal) -> TabularRefusal {
    TabularRefusal::MalformedInfo
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
