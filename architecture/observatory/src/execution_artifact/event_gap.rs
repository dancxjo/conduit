//! Native kernel history loss is distinct from retained Core Sign loss.
use alloc::string::String;
use conduit_core::{ActivePlayIdentity, Observation, ObservationKind};

pub(super) fn validate(
    sign: &Observation,
    play: Option<&ActivePlayIdentity>,
) -> Result<(), String> {
    let ObservationKind::ExecutionEventGap {
        first_sequence,
        last_sequence,
        entries,
    } = &sign.kind
    else {
        return Ok(());
    };
    let play = play.ok_or("execution event gap lacks its owning Play identity")?;
    if sign.host_id != play.host_id || sign.boot_id != play.boot_id {
        return Err("execution event gap belongs to a different Host or boot".into());
    }
    if sign.plan_id.is_none()
        || sign.active_play_id.is_none()
        || sign.face_id.is_some()
        || sign.placement_id.is_some()
        || sign.connection_id.is_some()
    {
        return Err(
            "execution event gap lacks exact Play context or invents a local subject".into(),
        );
    }
    if first_sequence > last_sequence
        || *entries == 0
        || u64::from(*entries) > u64::from(*last_sequence) - u64::from(*first_sequence) + 1
    {
        return Err("execution event gap has an invalid native range/count".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::validate_sign_artifact;
    #[test]
    fn native_gap_has_exact_context_and_a_count_that_can_exclude_pinned_events() {
        let (_, _, mut artifact) = super::super::tests::exact_artifacts();
        artifact.sign.kind = ObservationKind::ExecutionEventGap {
            first_sequence: 2,
            last_sequence: 7,
            entries: 3,
        };
        validate_sign_artifact(&artifact).unwrap();
        for kind in [
            ObservationKind::ExecutionEventGap {
                first_sequence: 8,
                last_sequence: 7,
                entries: 1,
            },
            ObservationKind::ExecutionEventGap {
                first_sequence: 2,
                last_sequence: 7,
                entries: 0,
            },
            ObservationKind::ExecutionEventGap {
                first_sequence: 2,
                last_sequence: 7,
                entries: 7,
            },
        ] {
            let mut wrong = artifact.clone();
            wrong.sign.kind = kind;
            assert!(validate_sign_artifact(&wrong).is_err());
        }
        let mut foreign = artifact.clone();
        let identity = conduit_core::bind_sign(
            &conduit_core::HostId::from("foreign"),
            &foreign.sign.boot_id,
            foreign.sign.active_play_id.as_ref(),
            foreign.identity.sequence,
        );
        foreign.sign.host_id = identity.host_id.clone();
        foreign.sign.sign_id = identity.sign_id.clone();
        foreign.identity = identity;
        assert!(validate_sign_artifact(&foreign).is_err());
        let mut wrong = artifact;
        wrong.sign.connection_id = Some(conduit_core::ConnectionId::from("invented"));
        assert!(validate_sign_artifact(&wrong).is_err());
    }
}
