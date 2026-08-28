use conduit_core::{bind_active_play, Plan, SignId};
use conduit_planner::{PolicyScope, PolicySourceId, PolicySourceRevision, ReviewedObservation};

pub fn active_wake(plan: &Plan, local_host_id: &str) -> conduit_body::Wake {
    let body = conduit_body::Body::born(
        plan.source_document_id.clone(),
        plan.checked_form_id.clone(),
        1,
        SignId::from("human-locality/body-born"),
    )
    .unwrap();
    let (_body, wake) = body.wake(1, SignId::from("human-locality/wake")).unwrap();
    let wake = wake
        .plan_ready(plan, SignId::from("human-locality/plan-ready"))
        .unwrap();
    let local_fragment = plan
        .fragments
        .iter()
        .find(|fragment| fragment.host_id.as_str() == local_host_id)
        .unwrap();
    let play = bind_active_play(
        &plan.plan_id,
        &local_fragment.host_id,
        &local_fragment.boot_id,
        1,
    );
    wake.play_started(&play, SignId::from("human-locality/play-started"))
        .unwrap()
}

pub fn policy_source(id: &str, revision: u64, scope: PolicyScope) -> PolicySourceRevision {
    PolicySourceRevision {
        source_id: PolicySourceId::from(id),
        revision,
        scope,
    }
}

pub fn reviewed_observations(
    observations: &[conduit_core::ResourceObservation],
    source: &PolicySourceRevision,
) -> Vec<ReviewedObservation> {
    observations
        .iter()
        .cloned()
        .map(|observation| ReviewedObservation {
            observation,
            source: source.clone(),
            observed_epoch: 10,
            valid_through_epoch: 12,
        })
        .collect()
}
