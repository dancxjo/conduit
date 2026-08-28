//! Canonical finite occurrence-batch encoding after recurrence expansion.

use super::recurrence_codec::{count, leaf, occurrence_instant, structured, value_field};
use conduit_core::{StructuredInfoType, StructuredInfoValue};
use conduit_time::RecurrenceOccurrence;

pub(super) fn encode_batch(occurrences: &[RecurrenceOccurrence]) -> Result<Vec<u8>, String> {
    if occurrences.len() > usize::from(conduit_semantic_catalog::RECURRENCE_MAXIMUM_RESULTS) {
        return Err("recurrence result exceeds the installed batch profile".into());
    }
    let result_type = conduit_semantic_catalog::recurrence_result_type();
    let slot_type = collection_element(&result_type, "occurrences")?;
    let mut slots = occurrences
        .iter()
        .map(|occurrence| {
            StructuredInfoValue::variant(
                slot_type.clone(),
                "occurrence",
                occurrence_value(occurrence)?,
            )
            .map_err(structured)
        })
        .collect::<Result<Vec<_>, String>>()?;
    while slots.len() < usize::from(conduit_semantic_catalog::RECURRENCE_MAXIMUM_RESULTS) {
        slots.push(
            StructuredInfoValue::variant(slot_type.clone(), "unused", leaf("value/unit@1", "")?)
                .map_err(structured)?,
        );
    }
    let slots = StructuredInfoValue::collection(
        StructuredInfoType::collection(
            slot_type,
            Some(conduit_semantic_catalog::RECURRENCE_MAXIMUM_RESULTS),
        )
        .map_err(structured)?,
        slots,
    )
    .map_err(structured)?;
    let batch = StructuredInfoValue::record(
        result_type,
        vec![
            value_field("count", count(occurrences.len() as u64)?),
            value_field("occurrences", slots),
        ],
    )
    .map_err(structured)?;
    batch.canonical_bytes().map_err(structured)
}

fn occurrence_value(occurrence: &RecurrenceOccurrence) -> Result<StructuredInfoValue, String> {
    StructuredInfoValue::record(
        conduit_semantic_catalog::recurrence_occurrence_type(),
        vec![
            value_field("identity", leaf("value/text@1", &occurrence.identity)?),
            value_field("instant", occurrence_instant(&occurrence.at)?),
            value_field("ordinal", count(u64::from(occurrence.ordinal))?),
            value_field(
                "recurrence_identity",
                leaf("value/text@1", &occurrence.recurrence_identity)?,
            ),
        ],
    )
    .map_err(structured)
}

fn collection_element(
    record_type: &StructuredInfoType,
    name: &str,
) -> Result<StructuredInfoType, String> {
    let conduit_core::StructuredInfoTypeShape::Record { fields, .. } = record_type.shape() else {
        return Err("recurrence batch is not a record type".into());
    };
    let field = fields
        .iter()
        .find(|field| field.name() == name)
        .ok_or_else(|| "recurrence batch slot field is missing".to_string())?;
    let conduit_core::StructuredInfoTypeShape::Collection { element, .. } =
        field.value_type().shape()
    else {
        return Err("recurrence batch slots are not a collection".into());
    };
    Ok(element.clone())
}
