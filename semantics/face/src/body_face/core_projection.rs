//! Current execution and operator-action truth for the face core.

use alloc::{format, vec, vec::Vec};
use conduit_body::{Body, BodyState, Wake};

use crate::{
    FaceAction, FaceActionAvailability, FaceDisclosure, FaceDisclosureLevel, FaceProperty,
    FacePropertyValue, FaceRelationship, FaceRelationshipKind, FaceRole, FaceSubject,
};

use super::{FaceContribution, FaceOperatorAction, FaceOperatorActionKind};
use crate::FaceContributionRole;

pub(super) fn append_execution_truth(
    wake: Option<&Wake>,
    body_subject: &str,
    subjects: &mut Vec<FaceSubject>,
    relationships: &mut Vec<FaceRelationship>,
    properties: &mut Vec<FaceProperty>,
    disclosures: &mut Vec<FaceDisclosure>,
) {
    let Some(wake) = wake else { return };
    for plan in &wake.plans {
        let plan_subject = format!("plan/{}", plan.plan_id.as_str());
        subjects.push(FaceSubject {
            identity: plan_subject.clone(),
            role: FaceRole::Plan,
            name: format!("Current wake Plan {}", plan.plan_id.as_str()),
        });
        relationships.push(FaceRelationship {
            source: body_subject.into(),
            target: plan_subject.clone(),
            kind: FaceRelationshipKind::Contains,
        });
        properties.extend([
            identity_property(&plan_subject, "plan-id", plan.plan_id.as_str()),
            FaceProperty {
                subject: plan_subject.clone(),
                name: "wake-plan-state".into(),
                value: FacePropertyValue::Text(format!("{:?}", plan.state)),
            },
        ]);
        disclosures.push(FaceDisclosure {
            subject: plan_subject.clone(),
            level: FaceDisclosureLevel::ExactProvenance,
        });
        if let Some(active_play_id) = &plan.active_play_id {
            let play_subject = format!("play/{}", active_play_id.as_str());
            subjects.push(FaceSubject {
                identity: play_subject.clone(),
                role: FaceRole::Play,
                name: format!("Active play {}", active_play_id.as_str()),
            });
            relationships.push(FaceRelationship {
                source: plan_subject,
                target: play_subject.clone(),
                kind: FaceRelationshipKind::Realizes,
            });
            properties.push(identity_property(
                &play_subject,
                "active-play-id",
                active_play_id.as_str(),
            ));
            disclosures.push(FaceDisclosure {
                subject: play_subject,
                level: FaceDisclosureLevel::ExactProvenance,
            });
        }
    }
}

pub(super) fn append_operator_actions(
    body: &Body,
    contributions: &[FaceContribution],
    body_subject: &str,
    actions: &mut Vec<FaceAction>,
    routing: &mut Vec<FaceOperatorAction>,
) {
    match body.state {
        BodyState::Lulled => push_action(
            body,
            body_subject,
            "wake",
            "conduit.intent/wake@1",
            "Wake",
            FaceActionAvailability::Available,
            FaceOperatorActionKind::Wake,
            actions,
            routing,
        ),
        BodyState::Awake { .. } => push_action(
            body,
            body_subject,
            "lull",
            "conduit.intent/lull@1",
            "Lull",
            FaceActionAvailability::Available,
            FaceOperatorActionKind::Lull,
            actions,
            routing,
        ),
        BodyState::Fulfilled { .. } => {}
    }
    for (token, intent, label, kind) in [
        (
            "open-overview",
            "conduit.intent/open-body-overview@1",
            "Open Body overview",
            FaceOperatorActionKind::OpenOverview,
        ),
        (
            "open-library",
            "conduit.intent/open-plot-library@1",
            "Open Plots library",
            FaceOperatorActionKind::OpenLibrary,
        ),
    ] {
        push_action(
            body,
            body_subject,
            token,
            intent,
            label,
            FaceActionAvailability::Available,
            kind,
            actions,
            routing,
        );
    }
    for plot in body.workset.plots() {
        let checked = &plot.checked_plot_id;
        let plot_target = format!("plot/{}", checked.as_str());
        let foreground = contributions.iter().any(|item| {
            item.role == FaceContributionRole::Foreground && item.checked_plot_id == *checked
        });
        let inspection = contributions.iter().any(|item| {
            item.role == FaceContributionRole::Inspection && item.checked_plot_id == *checked
        });
        push_action(
            body,
            &plot_target,
            &format!("open-plot/{}", checked.as_str()),
            "conduit.intent/open-resident-plot@1",
            "Open resident Plot",
            availability(
                foreground,
                "plot-not-playing",
                "The resident Plot is not currently presenting.",
            ),
            FaceOperatorActionKind::OpenResidentPlot(checked.clone()),
            actions,
            routing,
        );
        push_action(
            body,
            &plot_target,
            &format!("open-inspection/{}", checked.as_str()),
            "conduit.intent/open-inspection@1",
            "Open inspection",
            availability(
                inspection,
                "inspection-not-playing",
                "No admitted inspection contribution is currently playing.",
            ),
            FaceOperatorActionKind::OpenInspection(checked.clone()),
            actions,
            routing,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn push_action(
    body: &Body,
    target: &str,
    token: &str,
    intent: &str,
    label: &str,
    availability: FaceActionAvailability,
    kind: FaceOperatorActionKind,
    actions: &mut Vec<FaceAction>,
    routing: &mut Vec<FaceOperatorAction>,
) {
    let identity = format!("body/action/{token}/{}", body.workload_revision);
    actions.push(FaceAction {
        identity: identity.clone(),
        intent: intent.into(),
        target: target.into(),
        name: label.into(),
        arguments: vec![],
        disclosure: FaceDisclosureLevel::CurrentAction,
        availability,
    });
    routing.push(FaceOperatorAction {
        surface_action_id: identity,
        kind,
    });
}

fn availability(available: bool, reason_code: &str, explanation: &str) -> FaceActionAvailability {
    if available {
        FaceActionAvailability::Available
    } else {
        FaceActionAvailability::Unavailable {
            reason_code: reason_code.into(),
            explanation: explanation.into(),
        }
    }
}

fn identity_property(subject: &str, name: &str, value: &str) -> FaceProperty {
    FaceProperty {
        subject: subject.into(),
        name: name.into(),
        value: FacePropertyValue::Identity(value.into()),
    }
}
