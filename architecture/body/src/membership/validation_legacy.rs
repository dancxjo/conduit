//! Frozen pre-change allocating oracle; compiled only by owning tests.
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
            || event.change_id
                != legacy_change_id(
                    &value.body_id,
                    &event.part_id,
                    &event.sign_id,
                    event.revision,
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
        .map_or_else(Vec::new, |checkpoint| checkpoint.parts.clone());
    validate_parts(&base_parts)?;
    let replayed = replay_events(&value.body_id, base_parts, &value.events)?;
    if replayed != value.parts {
        return Err(MembershipRefusal::MalformedState);
    }
    validate_parts(&value.parts)?;
    Ok(())
}

fn replay_events(
    body_id: &BodyId,
    mut parts: Vec<PartMembership>,
    events: &[MembershipEvent],
) -> Result<Vec<PartMembership>, MembershipRefusal> {
    for event in events {
        match &event.kind {
            MembershipEventKind::Admitted { proof_id } => {
                validate_ids(&[proof_id.as_str()])?;
                if parts.iter().any(|part| part.part_id == event.part_id)
                    || parts.len() == MAX_BODY_PARTS
                {
                    return Err(MembershipRefusal::MalformedState);
                }
                parts.push(PartMembership {
                    part_id: event.part_id.clone(),
                    state: MembershipState::Admitted,
                    current: None,
                });
            }
            MembershipEventKind::HostAttached { observation } => {
                validate_observation(observation)?;
                let part = replay_part_mut(&mut parts, &event.part_id)?;
                if part.state != MembershipState::Admitted
                    || part.current.as_ref().is_some_and(|current| {
                        observation.sequence <= current.sequence
                            || (observation.host_id == current.host_id
                                && observation.boot_id == current.boot_id
                                && observation.offer_generation <= current.offer_generation)
                    })
                {
                    return Err(MembershipRefusal::MalformedState);
                }
                part.current = Some(observation.clone());
            }
            MembershipEventKind::HostDetached { prior_boot_id } => {
                validate_ids(&[prior_boot_id.as_str()])?;
                let part = replay_part_mut(&mut parts, &event.part_id)?;
                if part.state != MembershipState::Admitted
                    || part.current.as_ref().map(|current| &current.boot_id) != Some(prior_boot_id)
                {
                    return Err(MembershipRefusal::MalformedState);
                }
                part.current = None;
            }
            MembershipEventKind::Revoked => {
                let part = replay_part_mut(&mut parts, &event.part_id)?;
                if part.state != MembershipState::Admitted {
                    return Err(MembershipRefusal::MalformedState);
                }
                part.state = MembershipState::Revoked;
                part.current = None;
            }
        }
        if &event.body_id != body_id {
            return Err(MembershipRefusal::MalformedState);
        }
    }
    Ok(parts)
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

fn replay_part_mut<'a>(
    parts: &'a mut [PartMembership],
    part_id: &PartId,
) -> Result<&'a mut PartMembership, MembershipRefusal> {
    parts
        .iter_mut()
        .find(|part| &part.part_id == part_id)
        .ok_or(MembershipRefusal::MalformedState)
}

fn legacy_change_id(
    body: &BodyId,
    part: &PartId,
    sign: &SignId,
    revision: BodyMembershipRevision,
) -> MembershipChangeId {
    use core::fmt::Write;
    use sha2::{Digest, Sha256};
    let mut digest = Sha256::new();
    for text in [
        "membership-change",
        body.as_str(),
        part.as_str(),
        sign.as_str(),
    ] {
        digest.update((text.len() as u32).to_le_bytes());
        digest.update(text.as_bytes());
    }
    digest.update(revision.0.to_le_bytes());
    let mut text = alloc::string::String::with_capacity(64);
    for byte in digest.finalize() {
        let _ = write!(text, "{byte:02x}");
    }
    MembershipChangeId::bound(text)
}
