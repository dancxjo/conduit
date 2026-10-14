//! Parsec-owned semantic shell; action identity comes only from checked policy output.
use conduit_body::{BodyPlan, BodyPlayIdentity};
use conduit_face::{
    ActionInputBindingProposalInfo, Face, FaceAction, FaceActionAvailability, FaceBasis,
    FaceDisclosureLevel, FaceRole, FaceSubject, SourceEvaluatedParsecInputProposal,
};
use conduit_plot::rust_binding::NativeRustBinding;

pub(super) fn project(
    plan: &BodyPlan,
    play: &BodyPlayIdentity,
    proposal: &SourceEvaluatedParsecInputProposal<'_>,
) -> Result<Face, String> {
    if !play.validate_for(plan) || plan.plots.len() != 1 {
        return Err("Parsec Face differs from current Body Play".into());
    }
    let policy = ActionInputBindingProposalInfo::decode(proposal.proposal_bytes())
        .map_err(|e| format!("Parsec candidate policy: {e:?}"))?;
    if *policy.available() {
        return Err("Parsec candidate projection must not claim input availability".into());
    }
    let source = &plan.plots[0].plan;
    let action = policy.offered_action().action_identity();
    Face::new_with_semantics(
        *policy.mapping_revision(),
        FaceBasis {
            body_id: Some(plan.body_id.clone()),
            wake_id: Some(plan.wake_id.clone()),
            source_document_id: Some(source.source_document_id.clone()),
            checked_plot_id: Some(source.checked_plot_id.clone()),
            expanded_plot_id: Some(source.expanded_plot_id.clone()),
            plan_id: Some(plan.plan_id.clone()),
            active_play_id: Some(play.active_play_id.clone()),
            sign_ids: vec![],
        },
        vec![FaceSubject {
            identity: "parsec/game".into(),
            role: FaceRole::Region,
            name: "Parsec".into(),
        }],
        vec![],
        vec![],
        vec![],
        vec![FaceAction {
            identity: action.into(),
            intent: action.into(),
            target: "parsec/game".into(),
            name: action.into(),
            arguments: vec![],
            disclosure: FaceDisclosureLevel::Primary,
            availability: FaceActionAvailability::Available,
        }],
        vec![],
    )
    .map_err(|e| format!("Parsec semantic Face: {e:?}"))
}
