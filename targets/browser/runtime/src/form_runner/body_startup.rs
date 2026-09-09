//! Pure startup projection during Body admission, before any effects dispatch.
use conduit_body::{BodyBiographyEvidence, BodyPlan, BodyPlayIdentity, BodyStartup, Wake};

pub(super) fn prepare(
    evidence: Option<&BodyBiographyEvidence>,
    wake: &Wake,
    plan: &BodyPlan,
    play: &BodyPlayIdentity,
    started: &Wake,
) -> Result<Option<BodyStartup>, String> {
    let Some(evidence) = evidence else {
        return Ok(None);
    };
    evidence
        .validate()
        .map_err(|error| format!("startup history: {error:?}"))?;
    if evidence.body_id != plan.body_id || !evidence.wakes.iter().any(|current| current == wake) {
        return Err("startup history does not contain the exact proposed Body Wake".into());
    }
    let sequence = evidence
        .records
        .last()
        .and_then(|record| record.sequence.checked_add(1))
        .ok_or("startup history sequence exhausted")?;
    let mut projected = evidence.clone();
    projected
        .append_wake(projected.body.clone(), started.clone(), sequence)
        .map_err(|error| format!("startup history projection: {error:?}"))?;
    projected
        .startup_for_play(plan, play)
        .map(Some)
        .map_err(|error| format!("startup lifecycle: {error:?}"))
}
