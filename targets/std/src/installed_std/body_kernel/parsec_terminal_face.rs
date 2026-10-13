//! Parsec-owned semantic shell; action identity comes only from checked policy output.
use conduit_body::{BodyPlan, BodyPlayIdentity};
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_presentation::{
    ActionInputBindingProposalInfo, Presentation, PresentationAction,
    PresentationActionAvailability, PresentationBasis, PresentationDisclosureLevel,
    PresentationRole, PresentationSubject, SourceEvaluatedParsecInputProposal,
};

pub(super) fn project(
    plan: &BodyPlan,
    play: &BodyPlayIdentity,
    proposal: &SourceEvaluatedParsecInputProposal<'_>,
) -> Result<Presentation, String> {
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
    Presentation::new_with_semantics(
        *policy.mapping_revision(),
        PresentationBasis {
            body_id: Some(plan.body_id.clone()),
            wake_id: Some(plan.wake_id.clone()),
            source_document_id: Some(source.source_document_id.clone()),
            checked_plot_id: Some(source.checked_plot_id.clone()),
            expanded_plot_id: Some(source.expanded_plot_id.clone()),
            plan_id: Some(plan.plan_id.clone()),
            active_play_id: Some(play.active_play_id.clone()),
            sign_ids: vec![],
        },
        vec![PresentationSubject {
            identity: "parsec/game".into(),
            role: PresentationRole::Region,
            name: "Parsec".into(),
        }],
        vec![],
        vec![],
        vec![],
        vec![PresentationAction {
            identity: action.into(),
            intent: action.into(),
            target: "parsec/game".into(),
            name: action.into(),
            arguments: vec![],
            disclosure: PresentationDisclosureLevel::Primary,
            availability: PresentationActionAvailability::Available,
        }],
        vec![],
    )
    .map_err(|e| format!("Parsec semantic Face: {e:?}"))
}
