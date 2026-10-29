//! Ordered canonical replay over borrowed fixed-capacity state; no second owner.
use super::*;
pub(super) fn validate(value: &BodyMembership) -> Result<(), MembershipRefusal> {
    validate_ids(&[value.body_id.as_str()])?;
    if let Some(sign_id) = &value.fulfilled_sign_id {
        validate_ids(&[sign_id.as_str()])?;
    }
    if value.parts.len() > MAX_BODY_PARTS || value.events.len() > MAX_MEMBERSHIP_EVENTS {
        return Err(MembershipRefusal::MalformedState);
    }
    let base_revision = value
        .checkpoint
        .as_ref()
        .map_or(0, |checkpoint| checkpoint.revision.0);
    if value.events.len() as u64 != value.revision.0.saturating_sub(base_revision)
        || base_revision > value.revision.0
    {
        return Err(MembershipRefusal::MalformedState);
    }
    for (index, event) in value.events.iter().enumerate() {
        if event.body_id != value.body_id
            || event.revision != BodyMembershipRevision(base_revision + index as u64 + 1)
            || event.change_id.as_str().as_bytes()
                != crate::identity::bound_identity_bytes(
                    "membership-change",
                    &[
                        value.body_id.as_str(),
                        event.part_id.as_str(),
                        event.sign_id.as_str(),
                    ],
                    event.revision.0,
                )
        {
            return Err(MembershipRefusal::MalformedState);
        }
        validate_ids(&[
            event.part_id.as_str(),
            event.sign_id.as_str(),
            event.change_id.as_str(),
        ])?;
        if value.events[..index]
            .iter()
            .any(|prior| prior.change_id == event.change_id)
        {
            return Err(MembershipRefusal::DuplicateChange);
        }
        if value.events[..index]
            .iter()
            .any(|prior| prior.sign_id == event.sign_id)
        {
            return Err(MembershipRefusal::DuplicateSign);
        }
    }
    let base_parts = value
        .checkpoint
        .as_ref()
        .map_or(&[][..], |checkpoint| checkpoint.parts.as_slice());
    validate_parts(base_parts)?;
    if !replay_matches(&value.body_id, base_parts, &value.events, &value.parts)? {
        return Err(MembershipRefusal::MalformedState);
    }
    validate_parts(&value.parts)?;
    Ok(())
}

#[derive(Clone, Copy)]
struct Row<'a> {
    part: &'a PartId,
    state: MembershipState,
    current: Option<&'a AuthenticatedHostObservation>,
}
/// Keep the finite replay scratch in this preparation frame, not caller frames.
#[inline(never)]
fn replay_matches(
    body: &BodyId,
    base: &[PartMembership],
    events: &[MembershipEvent],
    expected: &[PartMembership],
) -> Result<bool, MembershipRefusal> {
    let mut rows = [None; MAX_BODY_PARTS];
    let mut len = base.len();
    for (row, part) in rows.iter_mut().zip(base) {
        *row = Some(Row {
            part: &part.part_id,
            state: part.state,
            current: part.current.as_ref(),
        });
    }
    for event in events {
        match &event.kind {
            MembershipEventKind::Admitted { proof_id } => {
                validate_ids(&[proof_id.as_str()])?;
                if rows[..len]
                    .iter()
                    .flatten()
                    .any(|row| row.part == &event.part_id)
                    || len == MAX_BODY_PARTS
                {
                    return Err(MembershipRefusal::MalformedState);
                }
                rows[len] = Some(Row {
                    part: &event.part_id,
                    state: MembershipState::Admitted,
                    current: None,
                });
                len += 1;
            }
            MembershipEventKind::HostAttached { observation } => {
                validate_observation(observation)?;
                let row = part_mut(&mut rows[..len], &event.part_id)?;
                if row.state != MembershipState::Admitted
                    || row.current.is_some_and(|current| {
                        observation.sequence <= current.sequence
                            || (observation.host_id == current.host_id
                                && observation.boot_id == current.boot_id
                                && observation.offer_generation <= current.offer_generation)
                    })
                {
                    return Err(MembershipRefusal::MalformedState);
                }
                row.current = Some(observation);
            }
            MembershipEventKind::HostDetached { prior_boot_id } => {
                validate_ids(&[prior_boot_id.as_str()])?;
                let row = part_mut(&mut rows[..len], &event.part_id)?;
                if row.state != MembershipState::Admitted
                    || row.current.map(|o| &o.boot_id) != Some(prior_boot_id)
                {
                    return Err(MembershipRefusal::MalformedState);
                }
                row.current = None;
            }
            MembershipEventKind::Revoked => {
                let row = part_mut(&mut rows[..len], &event.part_id)?;
                if row.state != MembershipState::Admitted {
                    return Err(MembershipRefusal::MalformedState);
                }
                row.state = MembershipState::Revoked;
                row.current = None;
            }
        }
        if &event.body_id != body {
            return Err(MembershipRefusal::MalformedState);
        }
    }
    Ok(len == expected.len()
        && rows[..len]
            .iter()
            .flatten()
            .zip(expected)
            .all(|(row, part)| {
                row.part == &part.part_id
                    && row.state == part.state
                    && row.current == part.current.as_ref()
            }))
}
fn part_mut<'a, 'b>(
    rows: &'a mut [Option<Row<'b>>],
    part: &PartId,
) -> Result<&'a mut Row<'b>, MembershipRefusal> {
    rows.iter_mut()
        .flatten()
        .find(|row| row.part == part)
        .ok_or(MembershipRefusal::MalformedState)
}
fn validate_parts(parts: &[PartMembership]) -> Result<(), MembershipRefusal> {
    if parts.len() > MAX_BODY_PARTS {
        return Err(MembershipRefusal::MalformedState);
    }
    for (index, part) in parts.iter().enumerate() {
        validate_ids(&[part.part_id.as_str()])?;
        if parts[..index]
            .iter()
            .any(|prior| prior.part_id == part.part_id)
            || (part.state == MembershipState::Revoked && part.current.is_some())
        {
            return Err(MembershipRefusal::MalformedState);
        }
        if let Some(observation) = &part.current {
            validate_observation(observation)?;
        }
    }
    Ok(())
}
#[cfg(test)]
pub(super) fn replay_scratch_bytes() -> usize {
    core::mem::size_of::<[Option<Row<'_>>; MAX_BODY_PARTS]>()
}
