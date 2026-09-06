use super::*;

/// Membership observers may still hold a biography from before local execution.
/// Adopt only their exact membership extension; never replace local Body/Wakes.
pub(super) fn merge_membership_extension(
    local: &BodyBiographyEvidence,
    remote: &BodyBiographyEvidence,
) -> Result<BodyBiographyEvidence, ServerError> {
    let refusal =
        || ServerError::Interaction("membership observer contradicts retained history".into());
    local.validate().map_err(|_| refusal())?;
    remote.validate().map_err(|_| refusal())?;
    let prior_events = local.membership.events.len();
    if remote.schema != local.schema
        || remote.body_id != local.body_id
        || remote.friendly_name != local.friendly_name
        || remote.graduation != local.graduation
        || !local.body.events.starts_with(&remote.body.events)
        || !remote
            .membership
            .events
            .starts_with(&local.membership.events)
        || remote.wakes.len() > local.wakes.len()
    {
        return Err(refusal());
    }
    for (observed, retained) in remote.wakes.iter().zip(&local.wakes) {
        if observed.wake_id != retained.wake_id
            || !retained.events.starts_with(&observed.events)
            || observed.plans.len() > retained.plans.len()
            || observed
                .plans
                .iter()
                .zip(&retained.plans)
                .any(|(old, current)| {
                    old.plan_id != current.plan_id
                        || old.hold != current.hold
                        || old
                            .active_play_id
                            .as_ref()
                            .is_some_and(|id| current.active_play_id.as_ref() != Some(id))
                })
        {
            return Err(refusal());
        }
    }
    let count = remote.membership.events.len() - prior_events;
    if !(1..=2).contains(&count) || remote.records.len() < count {
        return Err(refusal());
    }
    let split = remote.records.len() - count;
    // The observer's old records must be known in the same order. Membership
    // sequence numbers are observer-local after a merge; exact Signs and
    // change identities remain unchanged. Non-membership sequences stay exact.
    let mut known = local.records.iter();
    for record in &remote.records[..split] {
        if !known.any(|prior| {
            prior.kind == record.kind
                && prior.sign_id == record.sign_id
                && (membership_record(record) || prior.sequence == record.sequence)
        }) {
            return Err(refusal());
        }
    }
    let first_sequence = local
        .records
        .last()
        .ok_or_else(refusal)?
        .sequence
        .checked_add(1)
        .ok_or_else(refusal)?;
    let events = remote.membership.events[prior_events..]
        .iter()
        .enumerate()
        .map(|(offset, event)| {
            Ok((
                event.change_id.clone(),
                first_sequence
                    .checked_add(offset as u64)
                    .ok_or_else(refusal)?,
            ))
        })
        .collect::<Result<Vec<_>, ServerError>>()?;
    let mut merged = local.clone();
    merged
        .append_membership_events(remote.membership.clone(), &events)
        .map_err(|_| refusal())?;
    for (offered, appended) in remote.records[split..]
        .iter()
        .zip(&merged.records[local.records.len()..])
    {
        if !membership_record(offered)
            || offered.kind != appended.kind
            || offered.sign_id != appended.sign_id
        {
            return Err(refusal());
        }
    }
    super::validate_membership_extension(local, &merged)?;
    Ok(merged)
}

fn membership_record(record: &conduit_body::BodyBiographyRecord) -> bool {
    matches!(
        record.kind,
        BodyBiographyRecordKind::PartAdmitted { .. }
            | BodyBiographyRecordKind::HostJoined { .. }
            | BodyBiographyRecordKind::HostLeft { .. }
            | BodyBiographyRecordKind::PartRevoked { .. }
    )
}
