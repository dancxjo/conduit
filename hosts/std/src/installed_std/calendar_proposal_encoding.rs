//! Canonical bounded encoding of an inert meeting proposal.

use conduit_core::{StructuredInfoType, StructuredInfoTypeShape, StructuredInfoValue};
use conduit_time::{
    AvailabilityState, MeetingProposal, TemporalInstant, TemporalScale, TemporalWindow,
};

pub(crate) fn encode(proposal: &MeetingProposal) -> Result<Vec<u8>, String> {
    let value_type = conduit_std_catalog::calendar_proposal_result_type();
    let value = conduit_std_catalog::record_value(
        value_type.clone(),
        vec![
            (
                "availability_basis_identities",
                string_slots(
                    &value_type,
                    "availability_basis_identities",
                    "basis",
                    &proposal.availability_basis_identities,
                )?,
            ),
            (
                "candidates",
                value_slots(
                    &value_type,
                    "candidates",
                    "candidate",
                    proposal
                        .candidates
                        .iter()
                        .map(proposed_slot)
                        .collect::<Result<Vec<_>, _>>()?,
                )?,
            ),
            (
                "identity",
                conduit_std_catalog::leaf_value("value/text@1", &proposal.identity)?,
            ),
            ("reference_at", instant(&proposal.reference_at)?),
            (
                "rejected",
                value_slots(
                    &value_type,
                    "rejected",
                    "rejected",
                    proposal
                        .rejected
                        .iter()
                        .map(|rejected| {
                            let rejected_type = conduit_std_catalog::calendar_rejected_slot_type();
                            let conflicts = rejected
                                .conflicts
                                .iter()
                                .map(|conflict| {
                                    let conflict_collection =
                                        record_field_type(&rejected_type, "conflicts")?;
                                    let conflict_type =
                                        collection_payload_type(&conflict_collection, "conflict")?;
                                    conduit_std_catalog::record_value(
                                        conflict_type,
                                        vec![
                                            (
                                                "participant_identity",
                                                conduit_std_catalog::leaf_value(
                                                    "value/text@1",
                                                    &conflict.participant_identity,
                                                )?,
                                            ),
                                            (
                                                "state",
                                                conduit_std_catalog::leaf_value(
                                                    "calendar/availability-state@1",
                                                    state_name(conflict.state),
                                                )?,
                                            ),
                                        ],
                                    )
                                })
                                .collect::<Result<Vec<_>, String>>()?;
                            conduit_std_catalog::record_value(
                                rejected_type.clone(),
                                vec![
                                    (
                                        "candidate_identity",
                                        conduit_std_catalog::leaf_value(
                                            "value/text@1",
                                            &rejected.candidate_identity,
                                        )?,
                                    ),
                                    (
                                        "conflicts",
                                        value_slots(
                                            &rejected_type,
                                            "conflicts",
                                            "conflict",
                                            conflicts,
                                        )?,
                                    ),
                                ],
                            )
                        })
                        .collect::<Result<Vec<_>, String>>()?,
                )?,
            ),
        ],
    )?;
    value
        .canonical_bytes()
        .map_err(|error| format!("encode calendar proposal: {error:?}"))
}

fn proposed_slot(value: &conduit_time::ProposedMeetingSlot) -> Result<StructuredInfoValue, String> {
    let value_type = conduit_std_catalog::calendar_proposed_slot_type();
    conduit_std_catalog::record_value(
        value_type.clone(),
        vec![
            (
                "candidate_identity",
                conduit_std_catalog::leaf_value("value/text@1", &value.candidate_identity)?,
            ),
            ("interval", window(&value.interval)?),
            (
                "rationale",
                conduit_std_catalog::leaf_value("value/text@1", &value.rationale)?,
            ),
            (
                "tentative_participants",
                string_slots(
                    &value_type,
                    "tentative_participants",
                    "participant",
                    &value.tentative_participants,
                )?,
            ),
        ],
    )
}

fn window(value: &TemporalWindow) -> Result<StructuredInfoValue, String> {
    conduit_std_catalog::record_value(
        conduit_std_catalog::calendar_window_type(),
        vec![
            ("end", instant(value.end())?),
            ("start", instant(value.start())?),
        ],
    )
}

fn instant(value: &TemporalInstant) -> Result<StructuredInfoValue, String> {
    conduit_std_catalog::record_value(
        conduit_std_catalog::calendar_instant_type(),
        vec![
            (
                "basis",
                conduit_std_catalog::leaf_value("value/text@1", &value.clock_basis)?,
            ),
            (
                "resolution_ticks",
                conduit_std_catalog::leaf_value(
                    "value/count@1",
                    &value.resolution_ticks.to_string(),
                )?,
            ),
            (
                "scale",
                conduit_std_catalog::leaf_value("time/scale@1", scale_name(value.scale))?,
            ),
            (
                "ticks",
                conduit_std_catalog::leaf_value("value/count@1", &value.ticks.to_string())?,
            ),
            (
                "uncertainty_ticks",
                conduit_std_catalog::leaf_value(
                    "value/count@1",
                    &value.uncertainty_ticks.to_string(),
                )?,
            ),
        ],
    )
}

fn string_slots(
    record_type: &StructuredInfoType,
    field: &str,
    tag: &str,
    values: &[String],
) -> Result<StructuredInfoValue, String> {
    value_slots(
        record_type,
        field,
        tag,
        values
            .iter()
            .map(|value| conduit_std_catalog::leaf_value("value/text@1", value))
            .collect::<Result<Vec<_>, _>>()?,
    )
}

fn value_slots(
    record_type: &StructuredInfoType,
    field: &str,
    tag: &str,
    values: Vec<StructuredInfoValue>,
) -> Result<StructuredInfoValue, String> {
    let collection_type = record_field_type(record_type, field)?;
    let StructuredInfoTypeShape::Collection { element, length } = collection_type.shape() else {
        return Err("calendar output slots are not a collection".into());
    };
    if values.len() > usize::from(length) {
        return Err("calendar output exceeds installed slot capacity".into());
    }
    let mut slots = values
        .into_iter()
        .map(|value| {
            StructuredInfoValue::variant(element.clone(), tag, value)
                .map_err(|error| format!("calendar output slot refusal: {error:?}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    while slots.len() < usize::from(length) {
        slots.push(
            StructuredInfoValue::variant(
                element.clone(),
                "unused",
                conduit_std_catalog::leaf_value("value/unit@1", "")?,
            )
            .map_err(|error| format!("calendar unused slot refusal: {error:?}"))?,
        );
    }
    StructuredInfoValue::collection(collection_type, slots)
        .map_err(|error| format!("calendar output collection refusal: {error:?}"))
}

fn record_field_type(
    record_type: &StructuredInfoType,
    name: &str,
) -> Result<StructuredInfoType, String> {
    let StructuredInfoTypeShape::Record { fields, .. } = record_type.shape() else {
        return Err("calendar output expected record type".into());
    };
    fields
        .iter()
        .find(|field| field.name() == name)
        .map(|field| field.value_type().clone())
        .ok_or_else(|| format!("calendar output field '{name}' is missing"))
}

fn collection_payload_type(
    collection_type: &StructuredInfoType,
    tag: &str,
) -> Result<StructuredInfoType, String> {
    let StructuredInfoTypeShape::Collection { element, .. } = collection_type.shape() else {
        return Err("calendar output expected collection type".into());
    };
    let StructuredInfoTypeShape::Variant { cases, .. } = element.shape() else {
        return Err("calendar output expected slot variant".into());
    };
    cases
        .iter()
        .find(|case| case.tag() == tag)
        .map(|case| case.payload_type().clone())
        .ok_or_else(|| format!("calendar output slot '{tag}' is missing"))
}

fn scale_name(value: TemporalScale) -> &'static str {
    match value {
        TemporalScale::Seconds => "seconds",
        TemporalScale::Milliseconds => "milliseconds",
        TemporalScale::Microseconds => "microseconds",
        TemporalScale::Nanoseconds => "nanoseconds",
    }
}

fn state_name(value: AvailabilityState) -> &'static str {
    match value {
        AvailabilityState::Free => "free",
        AvailabilityState::Tentative => "tentative",
        AvailabilityState::Busy => "busy",
        AvailabilityState::Unavailable => "unavailable",
    }
}
