use super::*;
pub(super) fn field<'a>(v: View<'a>, key: &str) -> Result<View<'a>, R> {
    v.record_field(key)
        .map_err(|_| R::Malformed)?
        .ok_or(R::Malformed)
}
fn leaf(v: View<'_>) -> Result<&[u8], R> {
    let n = v.value_node();
    if n.len() < 5 || n[0] != 0 {
        return Err(R::Malformed);
    }
    if u32::from_le_bytes(n[1..5].try_into().unwrap()) as usize != n.len() - 5 {
        return Err(R::Malformed);
    }
    Ok(&n[5..])
}
pub(super) fn number(v: View<'_>) -> Result<u64, R> {
    Ok(u64::from_le_bytes(
        leaf(v)?.try_into().map_err(|_| R::Malformed)?,
    ))
}
pub(super) fn boolean(v: View<'_>) -> Result<bool, R> {
    match leaf(v)? {
        [0] => Ok(false),
        [1] => Ok(true),
        _ => Err(R::Malformed),
    }
}
fn case(v: View<'_>, tag: &str) -> Result<bool, R> {
    match v.variant_payload(tag).map_err(|_| R::Malformed)? {
        None => Ok(false),
        Some(payload) => {
            if leaf(payload)?.is_empty() {
                Ok(true)
            } else {
                Err(R::Malformed)
            }
        }
    }
}
fn payload<'a>(v: View<'a>, tag: &str) -> Result<Option<View<'a>>, R> {
    v.variant_payload(tag).map_err(|_| R::Malformed)
}
fn text(v: View<'_>) -> Result<&str, R> {
    core::str::from_utf8(leaf(v)?).map_err(|_| R::Malformed)
}
fn optional(v: View<'_>) -> Result<Option<View<'_>>, R> {
    if let Some(value) = payload(v, "some")? {
        Ok(Some(value))
    } else if case(v, "none")? {
        Ok(None)
    } else {
        Err(R::Malformed)
    }
}
fn optional_number(v: View<'_>) -> Result<Option<u64>, R> {
    optional(v)?.map(number).transpose()
}
pub(super) fn checked_identity(value: View<'_>, maximum: usize) -> Result<&str, R> {
    let value = text(value)?;
    if value.is_empty() || value.len() > maximum {
        return Err(R::Malformed);
    }
    Ok(value)
}
pub(super) fn clock(v: View<'_>) -> Result<BorrowedTemporalClockBasis<'_>, R> {
    if case(v, "unix_epoch_milliseconds")? {
        return Ok(BorrowedTemporalClockBasis::UnixEpochMilliseconds);
    }
    let value = payload(v, "monotonic_milliseconds")?.ok_or(R::Malformed)?;
    Ok(BorrowedTemporalClockBasis::MonotonicMilliseconds(
        checked_identity(field(value, "identity")?, 128)?,
    ))
}
fn source(v: View<'_>) -> Result<TemporalSource, R> {
    for (tag, value) in [
        ("event", TemporalSource::Event),
        ("valid_from", TemporalSource::ValidFrom),
        ("valid_until", TemporalSource::ValidUntil),
        ("observed", TemporalSource::Observed),
        ("recorded", TemporalSource::Recorded),
        ("ingested", TemporalSource::Ingested),
        ("retrieved", TemporalSource::Retrieved),
    ] {
        if case(v, tag)? {
            return Ok(value);
        }
    }
    Err(R::Malformed)
}
fn boundary(v: View<'_>) -> Result<EntityBoundary, R> {
    for (tag, value) in [
        ("created", EntityBoundary::Created),
        ("first_observed", EntityBoundary::FirstObserved),
        ("first_user_mention", EntityBoundary::FirstUserMention),
        ("born", EntityBoundary::Born),
        ("started", EntityBoundary::Started),
        ("current_phase_started", EntityBoundary::CurrentPhaseStarted),
        ("last_changed", EntityBoundary::LastChanged),
    ] {
        if case(v, tag)? {
            return Ok(value);
        }
    }
    Err(R::Malformed)
}
fn transition(v: View<'_>) -> Result<TransitionDirection, R> {
    if case(v, "into_state")? {
        Ok(TransitionDirection::IntoState)
    } else if case(v, "out_of_state")? {
        Ok(TransitionDirection::OutOfState)
    } else {
        Err(R::Malformed)
    }
}
pub(super) fn decode_candidate(v: View<'_>) -> Result<BorrowedTemporalEvidenceCandidate<'_>, R> {
    let validity = field(v, "validity")?;
    let mut known = false;
    for tag in [
        "current",
        "historical",
        "superseded",
        "unknown_whether_current",
    ] {
        if case(validity, tag)? {
            known = true;
            break;
        }
    }
    if !known {
        return Err(R::Malformed);
    }
    let p = field(v, "provenance")?;
    Ok(BorrowedTemporalEvidenceCandidate {
        identity: text(field(v, "identity")?)?,
        provenance: BorrowedTemporalProvenance {
            event_at: optional_number(field(p, "event_at")?)?,
            valid_from: optional_number(field(p, "valid_from")?)?,
            valid_until: optional_number(field(p, "valid_until")?)?,
            observed_at: optional_number(field(p, "observed_at")?)?,
            recorded_at: optional_number(field(p, "recorded_at")?)?,
            ingested_at: optional_number(field(p, "ingested_at")?)?,
            retrieved_at: number(field(p, "retrieved_at")?)?,
            reference_at: number(field(p, "reference_at")?)?,
            clock_basis: clock(field(p, "clock_basis")?)?,
            uncertainty_millis: optional_number(field(p, "uncertainty_millis")?)?,
        },
        source: source(field(v, "source")?)?,
        boundary: optional(field(v, "boundary")?)?.map(boundary).transpose()?,
        transition: optional(field(v, "transition")?)?
            .map(transition)
            .transpose()?,
    })
}
pub(super) fn decode_intent(v: View<'_>) -> Result<BorrowedTemporalRetrievalIntent, R> {
    for (tag, value) in [
        (
            "earliest_evidence",
            BorrowedTemporalRetrievalIntent::EarliestEvidence,
        ),
        (
            "latest_evidence",
            BorrowedTemporalRetrievalIntent::LatestEvidence,
        ),
        (
            "event_ordering",
            BorrowedTemporalRetrievalIntent::EventOrdering,
        ),
    ] {
        if case(v, tag)? {
            return Ok(value);
        }
    }
    if let Some(value) = payload(v, "state_valid_at")? {
        return Ok(BorrowedTemporalRetrievalIntent::StateValidAt(number(
            field(value, "instant")?,
        )?));
    }
    if let Some(value) = payload(v, "transition")? {
        return Ok(BorrowedTemporalRetrievalIntent::Transition(transition(
            field(value, "direction")?,
        )?));
    }
    if let Some(value) = payload(v, "duration_since")? {
        return Ok(BorrowedTemporalRetrievalIntent::DurationSince(boundary(
            field(value, "boundary")?,
        )?));
    }
    if let Some(value) = payload(v, "evidence_within")? {
        let window = field(value, "window")?;
        return Ok(BorrowedTemporalRetrievalIntent::EvidenceWithin {
            start: number(field(window, "start")?)?,
            end: number(field(window, "end")?)?,
        });
    }
    Err(R::Malformed)
}
