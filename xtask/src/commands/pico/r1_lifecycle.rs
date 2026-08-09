//! Typed R1 Lull and later-Wake evidence after physical session quiescence.

use conduit_body::{Body, BodyState, Wake, WakeLifecycle};
use conduit_core::ClueId;
use serde::Serialize;

use super::PicoResult;

#[derive(Serialize)]
pub struct R1LullSign {
    schema: &'static str,
    proof_class: &'static str,
    body_id: String,
    completed_wake_id: String,
    later_wake_id: String,
    active_play_quiescence: &'static str,
    body_retained: bool,
    completed_wake_lulled: bool,
    later_wake_new: bool,
    later_wake_state: &'static str,
}

pub struct R1LullClues {
    pub wake_lulled: ClueId,
    pub body_retained: ClueId,
    pub later_wake: ClueId,
}

pub fn lull_and_wake(
    body: &Body,
    wake: &Wake,
    session_terminal: bool,
    later_wake_sequence: u64,
    clues: R1LullClues,
) -> PicoResult<R1LullSign> {
    if !session_terminal {
        return Err("R1 active Play is not quiescent at Lull".into());
    }
    let lulled_wake = wake.lull(clues.wake_lulled).map_err(lifecycle_error)?;
    let retained = body
        .retain_after_lull(&lulled_wake, clues.body_retained)
        .map_err(lifecycle_error)?;
    let (_, later_wake) = retained
        .wake(later_wake_sequence, clues.later_wake)
        .map_err(lifecycle_error)?;
    let body_retained = retained.body_id == body.body_id && later_wake.body_id == body.body_id;
    let later_wake_new = later_wake.wake_id != lulled_wake.wake_id;
    if !body_retained
        || !later_wake_new
        || retained.state != BodyState::Lulled
        || lulled_wake.lifecycle != WakeLifecycle::Lulled
        || later_wake.lifecycle != WakeLifecycle::AwaitingPlan
    {
        return Err("R1 Lull/later-Wake identities or states changed unexpectedly".into());
    }
    Ok(R1LullSign {
        schema: "conduit.r1/lull-later-wake@1",
        proof_class: "physical-cross-host",
        body_id: body.body_id.as_str().to_string(),
        completed_wake_id: lulled_wake.wake_id.as_str().to_string(),
        later_wake_id: later_wake.wake_id.as_str().to_string(),
        active_play_quiescence: "reciprocal-session-terminal",
        body_retained,
        completed_wake_lulled: true,
        later_wake_new,
        later_wake_state: "awaiting-plan",
    })
}

fn lifecycle_error(error: conduit_body::BodyLifecycleError) -> Box<dyn std::error::Error> {
    format!("R1 lifecycle transition rejected: {error:?}").into()
}

#[cfg(test)]
mod tests {
    use conduit_core::{bind_active_play, BootId, HostId};

    use super::*;

    fn playing() -> (Body, Wake) {
        let plan = conduit_system_continuity::exact_r1_control_plan(
            BootId::from(conduit_net::R1_PICO_BOOT_ID),
            conduit_system_continuity::R1SignalRouteSet::UsbOnly,
        )
        .unwrap()
        .plan;
        let body = Body::born(
            plan.source_document_id.clone(),
            plan.checked_form_id.clone(),
            1,
            ClueId::from("r1/test/born"),
        )
        .unwrap();
        let (body, wake) = body.wake(1, ClueId::from("r1/test/woke")).unwrap();
        let wake = wake
            .plan_ready(&plan, ClueId::from("r1/test/planned"))
            .unwrap();
        let play = bind_active_play(
            &plan.plan_id,
            &HostId::from(conduit_net::R1_STD_HOST_ID),
            &BootId::from(conduit_net::R1_STD_BOOT_ID),
            0,
        );
        let wake = wake
            .play_started(&play, ClueId::from("r1/test/playing"))
            .unwrap();
        (body, wake)
    }

    fn clues() -> R1LullClues {
        R1LullClues {
            wake_lulled: ClueId::from("r1/test/lulled"),
            body_retained: ClueId::from("r1/test/retained"),
            later_wake: ClueId::from("r1/test/later-wake"),
        }
    }

    #[test]
    fn terminal_play_lulls_same_body_and_later_wake_is_new() {
        let (body, wake) = playing();
        let sign = lull_and_wake(&body, &wake, true, 2, clues()).unwrap();
        assert_eq!(sign.body_id, body.body_id.as_str());
        assert_eq!(sign.completed_wake_id, wake.wake_id.as_str());
        assert_ne!(sign.later_wake_id, sign.completed_wake_id);
        assert!(sign.body_retained && sign.later_wake_new);
    }

    #[test]
    fn active_play_cannot_be_reported_as_lulled() {
        let (body, wake) = playing();
        assert!(lull_and_wake(&body, &wake, false, 2, clues()).is_err());
    }
}
