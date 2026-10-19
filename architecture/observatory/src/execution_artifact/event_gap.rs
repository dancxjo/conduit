//! Native kernel history loss is distinct from retained Core Sign loss.
use alloc::string::String;
use conduit_core::{Observation, ObservationKind};

pub(super) fn validate(sign: &Observation) -> Result<(), String> {
    let ObservationKind::ExecutionEventGap {
        first_sequence,
        last_sequence,
        entries,
    } = &sign.kind
    else {
        return Ok(());
    };
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
        let mut wrong = artifact;
        wrong.sign.connection_id = Some(conduit_core::ConnectionId::from("invented"));
        assert!(validate_sign_artifact(&wrong).is_err());
    }
}
