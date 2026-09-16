//! Current execution and operator-action truth for the Body Surface core.

use alloc::{format, vec::Vec};
use conduit_body::{Body, BodyState, Wake};

use crate::{
    PresentationAction, PresentationActionAvailability, PresentationDisclosure,
    PresentationDisclosureLevel, PresentationProperty, PresentationPropertyValue,
    PresentationRelationship, PresentationRelationshipKind, PresentationRole, PresentationSubject,
};

use super::{
    BodySurfaceContribution, BodySurfaceContributionRole, BodySurfaceOperatorAction,
    BodySurfaceOperatorActionKind,
};

pub(super) fn append_execution_truth(
    wake: Option<&Wake>,
    body_subject: &str,
    subjects: &mut Vec<PresentationSubject>,
    relationships: &mut Vec<PresentationRelationship>,
    properties: &mut Vec<PresentationProperty>,
    disclosures: &mut Vec<PresentationDisclosure>,
) {
    let Some(wake) = wake else { return };
    for plan in &wake.plans {
        let plan_subject = format!("plan/{}", plan.plan_id.as_str());
        subjects.push(PresentationSubject {
            identity: plan_subject.clone(),
            role: PresentationRole::Plan,
            label: plan.plan_id.as_str().into(),
            accessibility_name: format!("Current Wake Plan {}", plan.plan_id.as_str()),
        });
        relationships.push(PresentationRelationship {
            source: body_subject.into(),
            target: plan_subject.clone(),
            kind: PresentationRelationshipKind::Contains,
        });
        properties.extend([
            identity_property(&plan_subject, "plan-id", plan.plan_id.as_str()),
            PresentationProperty {
                subject: plan_subject.clone(),
                name: "wake-plan-state".into(),
                value: PresentationPropertyValue::Text(format!("{:?}", plan.state)),
            },
        ]);
        disclosures.push(PresentationDisclosure {
            subject: plan_subject.clone(),
            level: PresentationDisclosureLevel::ExactProvenance,
        });
        if let Some(active_play_id) = &plan.active_play_id {
            let play_subject = format!("play/{}", active_play_id.as_str());
            subjects.push(PresentationSubject {
                identity: play_subject.clone(),
                role: PresentationRole::Play,
                label: active_play_id.as_str().into(),
                accessibility_name: format!("Active Play {}", active_play_id.as_str()),
            });
            relationships.push(PresentationRelationship {
                source: plan_subject,
                target: play_subject.clone(),
                kind: PresentationRelationshipKind::Realizes,
            });
            properties.push(identity_property(
                &play_subject,
                "active-play-id",
                active_play_id.as_str(),
            ));
            disclosures.push(PresentationDisclosure {
                subject: play_subject,
                level: PresentationDisclosureLevel::ExactProvenance,
            });
        }
    }
}

pub(super) fn append_operator_actions(
    body: &Body,
    contributions: &[BodySurfaceContribution],
    body_subject: &str,
    actions: &mut Vec<PresentationAction>,
    routing: &mut Vec<BodySurfaceOperatorAction>,
) {
    match body.state {
        BodyState::Lulled => push_action(
            body,
            body_subject,
            "wake",
            "conduit.intent/wake@1",
            "Wake",
            PresentationActionAvailability::Available,
            BodySurfaceOperatorActionKind::Wake,
            actions,
            routing,
        ),
        BodyState::Awake { .. } => push_action(
            body,
            body_subject,
            "lull",
            "conduit.intent/lull@1",
            "Lull",
            PresentationActionAvailability::Available,
            BodySurfaceOperatorActionKind::Lull,
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
            BodySurfaceOperatorActionKind::OpenOverview,
        ),
        (
            "open-library",
            "conduit.intent/open-form-library@1",
            "Open Forms library",
            BodySurfaceOperatorActionKind::OpenLibrary,
        ),
    ] {
        push_action(
            body,
            body_subject,
            token,
            intent,
            label,
            PresentationActionAvailability::Available,
            kind,
            actions,
            routing,
        );
    }
    for form in body.workset.forms() {
        let checked = &form.checked_form_id;
        let form_target = format!("form/{}", checked.as_str());
        let foreground = contributions.iter().any(|item| {
            item.role == BodySurfaceContributionRole::Foreground && item.checked_form_id == *checked
        });
        let inspection = contributions.iter().any(|item| {
            item.role == BodySurfaceContributionRole::Inspection && item.checked_form_id == *checked
        });
        push_action(
            body,
            &form_target,
            &format!("open-form/{}", checked.as_str()),
            "conduit.intent/open-resident-form@1",
            "Open resident Form",
            availability(
                foreground,
                "form-not-playing",
                "The resident Form is not currently presenting.",
            ),
            BodySurfaceOperatorActionKind::OpenResidentForm(checked.clone()),
            actions,
            routing,
        );
        push_action(
            body,
            &form_target,
            &format!("open-inspection/{}", checked.as_str()),
            "conduit.intent/open-inspection@1",
            "Open inspection",
            availability(
                inspection,
                "inspection-not-playing",
                "No admitted inspection contribution is currently playing.",
            ),
            BodySurfaceOperatorActionKind::OpenInspection(checked.clone()),
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
    availability: PresentationActionAvailability,
    kind: BodySurfaceOperatorActionKind,
    actions: &mut Vec<PresentationAction>,
    routing: &mut Vec<BodySurfaceOperatorAction>,
) {
    let identity = format!("body/action/{token}/{}", body.workload_revision);
    actions.push(PresentationAction {
        identity: identity.clone(),
        intent: intent.into(),
        target: target.into(),
        label: label.into(),
        disclosure: PresentationDisclosureLevel::CurrentAction,
        availability,
    });
    routing.push(BodySurfaceOperatorAction {
        surface_action_id: identity,
        kind,
    });
}

fn availability(
    available: bool,
    reason_code: &str,
    explanation: &str,
) -> PresentationActionAvailability {
    if available {
        PresentationActionAvailability::Available
    } else {
        PresentationActionAvailability::Unavailable {
            reason_code: reason_code.into(),
            explanation: explanation.into(),
        }
    }
}

fn identity_property(subject: &str, name: &str, value: &str) -> PresentationProperty {
    PresentationProperty {
        subject: subject.into(),
        name: name.into(),
        value: PresentationPropertyValue::Identity(value.into()),
    }
}
