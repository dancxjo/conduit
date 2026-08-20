//! Exact conversion between structured calendar request Info and core semantics.

use conduit_core::{
    AvailabilityBasis, AvailabilityInterval, AvailabilityState, MeetingCandidate,
    MeetingProposalRequest, NamedTimeZone, ParticipantAvailability, StructuredFieldValue,
    StructuredInfoValue, StructuredInfoValueShape, TemporalBoundary, TemporalInstant,
    TemporalScale, TemporalWindow,
};

pub(super) struct DecodedCalendarProposal {
    pub(super) request: MeetingProposalRequest,
    pub(super) availability: Vec<ParticipantAvailability>,
}

pub(super) fn decode(value: &StructuredInfoValue) -> Result<DecodedCalendarProposal, String> {
    if value.value_type() != &conduit_std_catalog::calendar_proposal_request_type() {
        return Err("calendar proposal request differs from installed contract".into());
    }
    let fields = record(value)?;
    let participant_identities =
        active_slots(field(fields, "participant_identities")?, "participant")?
            .map(|value| text(value.1))
            .collect::<Result<Vec<_>, _>>()?;
    let candidates = active_slots(field(fields, "candidates")?, "candidate")?
        .map(|value| candidate(value.1))
        .collect::<Result<Vec<_>, _>>()?;
    let availability = active_slots(field(fields, "availability")?, "participant")?
        .map(|value| participant_availability(value.1))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(DecodedCalendarProposal {
        request: MeetingProposalRequest {
            identity: text(field(fields, "identity")?)?,
            reference_at: instant(field(fields, "reference_at")?)?,
            participant_identities,
            candidates,
            maximum_results: u16_value(field(fields, "maximum_results")?)?,
        },
        availability,
    })
}

fn candidate(value: &StructuredInfoValue) -> Result<MeetingCandidate, String> {
    let fields = record(value)?;
    Ok(MeetingCandidate {
        identity: text(field(fields, "identity")?)?,
        interval: window(field(fields, "interval")?)?,
        rationale: text(field(fields, "rationale")?)?,
    })
}

fn participant_availability(
    value: &StructuredInfoValue,
) -> Result<ParticipantAvailability, String> {
    let fields = record(value)?;
    let participant_identity = text(field(fields, "participant_identity")?)?;
    let intervals = active_slots(field(fields, "intervals")?, "interval")?
        .map(|value| availability_interval(value.1))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(ParticipantAvailability {
        participant_identity,
        zone: NamedTimeZone::new(
            text(field(fields, "zone")?)?,
            text(field(fields, "zone_rule_set")?)?,
        )
        .map_err(|error| format!("calendar zone refusal: {error:?}"))?,
        basis: AvailabilityBasis {
            identity: text(field(fields, "basis_identity")?)?,
            observed_at: instant(field(fields, "observed_at")?)?,
            usable_until: instant(field(fields, "usable_until")?)?,
        },
        intervals,
    })
}

fn availability_interval(value: &StructuredInfoValue) -> Result<AvailabilityInterval, String> {
    let fields = record(value)?;
    Ok(AvailabilityInterval {
        participant_identity: text(field(fields, "participant_identity")?)?,
        interval: TemporalWindow::new(
            prefixed_instant(fields, "start")?,
            boundary(field(fields, "start_boundary")?)?,
            prefixed_instant(fields, "end")?,
            boundary(field(fields, "end_boundary")?)?,
        )
        .map_err(|error| format!("calendar availability window refusal: {error:?}"))?,
        state: match text(field(fields, "state")?)?.as_str() {
            "free" => AvailabilityState::Free,
            "tentative" => AvailabilityState::Tentative,
            "busy" => AvailabilityState::Busy,
            "unavailable" => AvailabilityState::Unavailable,
            _ => return Err("unknown calendar availability state".into()),
        },
    })
}

fn prefixed_instant(
    fields: &[StructuredFieldValue],
    prefix: &str,
) -> Result<TemporalInstant, String> {
    let value = TemporalInstant {
        ticks: count(field(fields, &format!("{prefix}_ticks"))?)?,
        scale: scale(field(fields, &format!("{prefix}_scale"))?)?,
        clock_basis: text(field(fields, &format!("{prefix}_basis"))?)?,
        resolution_ticks: count(field(fields, &format!("{prefix}_resolution_ticks"))?)?,
        uncertainty_ticks: count(field(fields, &format!("{prefix}_uncertainty_ticks"))?)?,
    };
    value
        .validate()
        .map_err(|error| format!("calendar instant refusal: {error:?}"))?;
    Ok(value)
}

fn boundary(value: &StructuredInfoValue) -> Result<TemporalBoundary, String> {
    match text(value)?.as_str() {
        "inclusive" => Ok(TemporalBoundary::Inclusive),
        "exclusive" => Ok(TemporalBoundary::Exclusive),
        _ => Err("unknown calendar temporal boundary".into()),
    }
}

fn window(value: &StructuredInfoValue) -> Result<TemporalWindow, String> {
    let fields = record(value)?;
    TemporalWindow::new(
        instant(field(fields, "start")?)?,
        TemporalBoundary::Inclusive,
        instant(field(fields, "end")?)?,
        TemporalBoundary::Exclusive,
    )
    .map_err(|error| format!("calendar window refusal: {error:?}"))
}

fn instant(value: &StructuredInfoValue) -> Result<TemporalInstant, String> {
    let fields = record(value)?;
    let value = TemporalInstant {
        ticks: count(field(fields, "ticks")?)?,
        scale: scale(field(fields, "scale")?)?,
        clock_basis: text(field(fields, "basis")?)?,
        resolution_ticks: count(field(fields, "resolution_ticks")?)?,
        uncertainty_ticks: count(field(fields, "uncertainty_ticks")?)?,
    };
    value
        .validate()
        .map_err(|error| format!("calendar instant refusal: {error:?}"))?;
    Ok(value)
}

fn scale(value: &StructuredInfoValue) -> Result<TemporalScale, String> {
    let scale = text(value)?;
    match scale.as_str() {
        "seconds" => Ok(TemporalScale::Seconds),
        "milliseconds" => Ok(TemporalScale::Milliseconds),
        "microseconds" => Ok(TemporalScale::Microseconds),
        "nanoseconds" => Ok(TemporalScale::Nanoseconds),
        _ => Err(format!("unknown calendar temporal scale '{scale}'")),
    }
}

fn active_slots<'a>(
    value: &'a StructuredInfoValue,
    active_tag: &'static str,
) -> Result<impl Iterator<Item = (&'a str, &'a StructuredInfoValue)>, String> {
    let StructuredInfoValueShape::Collection(values) = value.shape() else {
        return Err("calendar slots are not a collection".into());
    };
    let parsed = values.iter().map(variant).collect::<Result<Vec<_>, _>>()?;
    let mut unused_seen = false;
    for (tag, _) in &parsed {
        match *tag {
            "unused" => unused_seen = true,
            tag if tag == active_tag => {
                if unused_seen {
                    return Err("active calendar slot follows unused capacity".into());
                }
            }
            _ => return Err("unknown calendar slot tag".into()),
        }
    }
    Ok(parsed.into_iter().filter(|(tag, _)| *tag != "unused"))
}

fn record(value: &StructuredInfoValue) -> Result<&[StructuredFieldValue], String> {
    match value.shape() {
        StructuredInfoValueShape::Record(fields) => Ok(fields),
        _ => Err("calendar value is not the required record".into()),
    }
}

fn variant(value: &StructuredInfoValue) -> Result<(&str, &StructuredInfoValue), String> {
    match value.shape() {
        StructuredInfoValueShape::Variant { tag, payload } => Ok((tag, payload)),
        _ => Err("calendar value is not the required variant".into()),
    }
}

fn field<'a>(
    fields: &'a [StructuredFieldValue],
    name: &str,
) -> Result<&'a StructuredInfoValue, String> {
    fields
        .iter()
        .find(|field| field.name() == name)
        .map(StructuredFieldValue::value)
        .ok_or_else(|| format!("calendar field '{name}' is missing"))
}

fn text(value: &StructuredInfoValue) -> Result<String, String> {
    let StructuredInfoValueShape::Leaf(bytes) = value.shape() else {
        return Err("calendar text is not a leaf".into());
    };
    let raw = core::str::from_utf8(bytes).map_err(|_| "calendar text is not UTF-8")?;
    if raw.starts_with('"') {
        serde_json::from_str(raw).map_err(|_| "calendar quoted leaf is malformed".into())
    } else {
        Ok(raw.to_owned())
    }
}

fn count(value: &StructuredInfoValue) -> Result<u64, String> {
    text(value)?
        .parse()
        .map_err(|_| "calendar count is malformed".into())
}

fn u16_value(value: &StructuredInfoValue) -> Result<u16, String> {
    count(value)?
        .try_into()
        .map_err(|_| "calendar count exceeds u16".into())
}
