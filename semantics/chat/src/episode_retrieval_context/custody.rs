use super::*;
pub(super) fn field<'a>(
    v: ValidatedCanonicalStructuredValue<'a>,
    key: &str,
) -> Result<ValidatedCanonicalStructuredValue<'a>, R> {
    v.record_field(key)
        .map_err(|_| R::Malformed)?
        .ok_or(R::Malformed)
}
fn leaf(v: ValidatedCanonicalStructuredValue<'_>) -> Result<&[u8], R> {
    let n = v.value_node();
    if n.len() < 5
        || n[0] != 0
        || u32::from_le_bytes(n[1..5].try_into().unwrap()) as usize != n.len() - 5
    {
        return Err(R::Malformed);
    }
    Ok(&n[5..])
}
fn number(v: ValidatedCanonicalStructuredValue<'_>) -> Result<u64, R> {
    Ok(u64::from_le_bytes(
        leaf(v)?.try_into().map_err(|_| R::Malformed)?,
    ))
}
fn optional(v: ValidatedCanonicalStructuredValue<'_>) -> Result<Option<u64>, R> {
    if let Some(p) = v.variant_payload("some").map_err(|_| R::Malformed)? {
        number(p).map(Some)
    } else if v
        .variant_payload("none")
        .map_err(|_| R::Malformed)?
        .is_some()
    {
        Ok(None)
    } else {
        Err(R::Malformed)
    }
}
pub(super) fn source_span(source: &[u8], node: &[u8]) -> Result<(u64, u64), R> {
    let start = (node.as_ptr() as usize)
        .checked_sub(source.as_ptr() as usize)
        .ok_or(R::SourceSpan)?;
    let end = start.checked_add(node.len()).ok_or(R::SourceSpan)?;
    if source.get(start..end) != Some(node) {
        return Err(R::SourceSpan);
    }
    Ok((start as u64, end as u64))
}
pub(super) fn temporal<'a>(
    candidate: ValidatedCanonicalStructuredValue<'_>,
    snapshot: &BorrowedEpisodeRetrievalSnapshot<'a>,
    identity: &'a str,
) -> Result<BorrowedContextTemporal<'a>, R> {
    let provenance = field(candidate, "provenance")?;
    let observed = optional(field(provenance, "observed_at")?)?.ok_or(R::Malformed)?;
    let uncertainty = optional(field(provenance, "uncertainty_millis")?)?;
    let input = snapshot.input();
    let instant = BorrowedTemporalInstant {
        ticks: observed,
        scale: TemporalScale::Milliseconds,
        clock_basis: input.reference.clock_basis,
        resolution_ticks: 1,
        uncertainty_ticks: uncertainty.unwrap_or(0),
    };
    let relation = instant
        .relation_to(&input.reference)
        .map_err(|_| R::Malformed)?;
    Ok(BorrowedContextTemporal {
        evidence_identity: identity,
        source: TemporalSource::Observed,
        source_at: Some(observed),
        valid_from: None,
        valid_until: None,
        retrieved_at: input.retrieved.ticks,
        reference_at: input.reference.ticks,
        clock_basis: input.reference.clock_basis,
        uncertainty_millis: uncertainty,
        context_source: TemporalSource::Observed,
        context_relation: relation,
        context_validity: TemporalValidity::UnknownWhetherCurrent,
    })
}
