//! Lower one admitted application contribution into portable Presentation truth.

use alloc::{format, vec::Vec};

use crate::{
    ApplicationComponent, ApplicationEventKind, ApplicationNodeState, PresentationAction,
    PresentationActionAvailability, PresentationDisclosure, PresentationDisclosureLevel,
    PresentationInput, PresentationProperty, PresentationPropertyValue, PresentationRelationship,
    PresentationRelationshipKind, PresentationRole, PresentationSubject, PresentationText,
    UTF8_TEXT_VALUE_KIND,
};

use super::{BodySurfaceApplicationAction, BodySurfaceContribution, BodySurfaceContributionRole};

#[allow(clippy::too_many_arguments)]
pub(super) fn append_contribution(
    index: usize,
    contribution: &BodySurfaceContribution,
    context_subject: &str,
    subjects: &mut Vec<PresentationSubject>,
    relationships: &mut Vec<PresentationRelationship>,
    properties: &mut Vec<PresentationProperty>,
    text: &mut Vec<PresentationText>,
    actions: &mut Vec<PresentationAction>,
    inputs: &mut Vec<PresentationInput>,
    disclosures: &mut Vec<PresentationDisclosure>,
    application_actions: &mut Vec<BodySurfaceApplicationAction>,
) {
    let prefix = format!("surface/{}/{index}", contribution.role.token());
    let root = format!("{prefix}/application");
    subjects.push(PresentationSubject {
        identity: root.clone(),
        role: PresentationRole::Region,
        label: contribution.role.token().into(),
        accessibility_name: format!("{} application contribution", contribution.role.token()),
    });
    relationships.push(PresentationRelationship {
        source: context_subject.into(),
        target: root.clone(),
        kind: PresentationRelationshipKind::Contains,
    });
    properties.extend([
        identity_property(
            &root,
            "checked-form-id",
            contribution.checked_form_id.as_str(),
        ),
        identity_property(&root, "plan-id", contribution.plan_id.as_str()),
        identity_property(
            &root,
            "active-play-id",
            contribution.active_play_id.as_str(),
        ),
        PresentationProperty {
            subject: root.clone(),
            name: "application-view-revision".into(),
            value: PresentationPropertyValue::Count(u64::from(contribution.view.revision)),
        },
    ]);
    disclosures.push(PresentationDisclosure {
        subject: root.clone(),
        level: contribution_disclosure(contribution.role),
    });

    for (node_index, node) in contribution.view.nodes.iter().enumerate() {
        let identity = format!("{prefix}/node/{}", node.key);
        let label = if node.text.is_empty() {
            node.key.clone()
        } else {
            node.text.clone()
        };
        subjects.push(PresentationSubject {
            identity: identity.clone(),
            role: node_role(node.component),
            label: label.clone(),
            accessibility_name: label.clone(),
        });
        let parent = node.parent.map_or_else(
            || root.clone(),
            |parent| {
                format!(
                    "{prefix}/node/{}",
                    contribution.view.nodes[usize::from(parent)].key
                )
            },
        );
        relationships.push(PresentationRelationship {
            source: parent,
            target: identity.clone(),
            kind: PresentationRelationshipKind::Contains,
        });
        properties.extend([
            PresentationProperty {
                subject: identity.clone(),
                name: "application-component".into(),
                value: PresentationPropertyValue::Text(format!("{:?}", node.component)),
            },
            PresentationProperty {
                subject: identity.clone(),
                name: "application-node-state".into(),
                value: PresentationPropertyValue::Text(format!("{:?}", node.state)),
            },
        ]);
        if !node.value.is_empty() {
            properties.push(PresentationProperty {
                subject: identity.clone(),
                name: "value".into(),
                value: PresentationPropertyValue::Text(node.value.clone()),
            });
        }
        if !node.text.is_empty() {
            text.push(PresentationText {
                subject: identity.clone(),
                text: node.text.clone(),
            });
        }
        if let Some(action_index) = node.action {
            let source = &contribution.view.actions[usize::from(action_index)];
            let action_identity = format!("{prefix}/action/{}/node/{node_index}", source.id);
            properties.push(PresentationProperty {
                subject: identity.clone(),
                name: "application-action-id".into(),
                value: PresentationPropertyValue::Identity(source.id.clone()),
            });
            actions.push(PresentationAction {
                identity: action_identity.clone(),
                intent: event_intent(source.event).into(),
                target: identity.clone(),
                label: label.clone(),
                disclosure: contribution_disclosure(contribution.role),
                availability: action_availability(node.state),
            });
            application_actions.push(BodySurfaceApplicationAction {
                surface_action_id: action_identity.clone(),
                role: contribution.role,
                checked_form_id: contribution.checked_form_id.clone(),
                plan_id: contribution.plan_id.clone(),
                active_play_id: contribution.active_play_id.clone(),
                application_view_revision: contribution.view.revision,
                application_action_id: source.id.clone(),
                event: source.event,
            });
            if matches!(
                node.component,
                ApplicationComponent::TextInput
                    | ApplicationComponent::TextArea
                    | ApplicationComponent::Select
            ) {
                inputs.push(PresentationInput {
                    identity: format!("{prefix}/input/{node_index}"),
                    target: identity,
                    value_kind: UTF8_TEXT_VALUE_KIND.into(),
                    maximum_bytes: node.value_capacity,
                    allow_empty: true,
                    label,
                    accessibility_name: node.key.clone(),
                    submit_action: action_identity,
                });
            }
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

fn action_availability(state: ApplicationNodeState) -> PresentationActionAvailability {
    match state {
        ApplicationNodeState::Ready => PresentationActionAvailability::Available,
        ApplicationNodeState::Busy => PresentationActionAvailability::Unavailable {
            reason_code: "application-busy".into(),
            explanation: "The application action is currently busy.".into(),
        },
        ApplicationNodeState::Unavailable => PresentationActionAvailability::Unavailable {
            reason_code: "application-unavailable".into(),
            explanation: "The application action is not currently available.".into(),
        },
    }
}

fn contribution_disclosure(role: BodySurfaceContributionRole) -> PresentationDisclosureLevel {
    match role {
        BodySurfaceContributionRole::Foreground => PresentationDisclosureLevel::Primary,
        BodySurfaceContributionRole::Tutorial => PresentationDisclosureLevel::CurrentAction,
        BodySurfaceContributionRole::Inspection => PresentationDisclosureLevel::SelectedDetail,
        BodySurfaceContributionRole::Transient => PresentationDisclosureLevel::CurrentAction,
    }
}

fn node_role(component: ApplicationComponent) -> PresentationRole {
    match component {
        ApplicationComponent::Button | ApplicationComponent::NavigationLink => {
            PresentationRole::Action
        }
        ApplicationComponent::TextInput
        | ApplicationComponent::TextArea
        | ApplicationComponent::Select => PresentationRole::TextEntry,
        ApplicationComponent::Status
        | ApplicationComponent::SuccessStatus
        | ApplicationComponent::FailureStatus
        | ApplicationComponent::WarningStatus
        | ApplicationComponent::MissingEvidence
        | ApplicationComponent::StaleEvidence
        | ApplicationComponent::RefusedEvidence
        | ApplicationComponent::FailedEvidence
        | ApplicationComponent::SuccessfulEvidence => PresentationRole::Status,
        ApplicationComponent::Shell
        | ApplicationComponent::Main
        | ApplicationComponent::Panel
        | ApplicationComponent::Navigation
        | ApplicationComponent::Disclosure
        | ApplicationComponent::PatchbayCanvas => PresentationRole::Region,
        _ => PresentationRole::Item,
    }
}

fn event_intent(kind: ApplicationEventKind) -> &'static str {
    match kind {
        ApplicationEventKind::Activate => "application.event/activate@1",
        ApplicationEventKind::Change => "application.event/change@1",
        ApplicationEventKind::Input => "application.event/input@1",
        ApplicationEventKind::Toggle => "application.event/toggle@1",
        ApplicationEventKind::Submit => "application.event/submit@1",
    }
}
