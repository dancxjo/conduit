use crate::{server::ServerError, RendererSnapshot};
use patchbay_model::{BodyPlanningSession, PatchbayBodyWorkloadSession};

/// Build a candidate projection from the same retained lifecycle as planning.
/// Nothing is committed until the caller has encoded its whole next snapshot.
pub(in crate::server) fn retain(
    prior: &RendererSnapshot,
    session: &PatchbayBodyWorkloadSession,
    planning: &BodyPlanningSession,
) -> Result<(PatchbayBodyWorkloadSession, RendererSnapshot), ServerError> {
    let evidence = session.evidence();
    if (&evidence.body == planning.body()
        || (planning.wake().lifecycle == conduit_body::WakeLifecycle::Lulled
            && evidence.body.events.starts_with(&planning.body().events)))
        && evidence.wakes.iter().any(|wake| wake == planning.wake())
    {
        return Ok((session.clone(), prior.clone()));
    }
    let sequence = evidence
        .records
        .last()
        .and_then(|record| record.sequence.checked_add(1))
        .ok_or_else(|| ServerError::Interaction("BodyBiographySequenceExhausted".into()))?;
    let mut next = session.clone();
    next.retain_wake(planning.body().clone(), planning.wake().clone(), sequence)
        .map_err(|error| ServerError::Interaction(format!("BodyLifecycleRetention{error:?}")))?;
    let workbench = prior
        .body_workbench
        .as_ref()
        .ok_or_else(|| ServerError::Interaction("BodyWorkbenchAbsent".into()))?;
    let revision = workbench
        .evidence_revision
        .checked_add(1)
        .ok_or_else(|| ServerError::Interaction("BodyEvidenceRevisionExhausted".into()))?;
    let mut snapshot = crate::body_workbench::body_workbench_snapshot_with_reviewed(
        revision,
        next.encoded_evidence(),
        workbench.entrance.clone(),
        &workbench.reviewed_forms,
    )
    .map_err(|error| ServerError::Interaction(error.to_string()))?;
    snapshot.mark_available(conduit_core::SignId::from(format!(
        "patchbay-html/body-lifecycle/evidence-{revision}/available"
    )))?;
    snapshot.interaction = prior.interaction.clone();
    snapshot.body_host_offer_evidence = prior.body_host_offer_evidence.clone();
    snapshot.body_host_planning_offer = prior.body_host_planning_offer.clone();
    snapshot.body_planning = prior.body_planning.clone();
    Ok((next, snapshot))
}
