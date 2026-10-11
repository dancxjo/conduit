//! Human-readable browser projection of the exact checked Face.

use super::*;
use conduit_presentation::{PresentationActionAvailability, PresentationPropertyValue};

impl OwnerBrowserMask {
    pub(super) fn view(&self) -> FaceView {
        let mut view = project_view(
            &self.body_id,
            &self.presentation,
            &self.planned,
            &self.play,
            &self.show,
            self.interactions_admitted,
            self.route.as_ref().map(|route| RouteView {
                plan_id: route.plan_id.clone(),
                owner_host_id: route.owner_host_id.clone(),
                owner_boot_id: route.owner_boot_id.clone(),
                mask_host_id: route.mask_host_id.clone(),
                mask_boot_id: route.mask_boot_id.clone(),
            }),
        );
        view.component = self.component.as_ref().map(|component| component.clone());
        view
    }
}

pub(crate) fn local_view(
    body_id: &BodyId,
    face: &Presentation,
    show: &MaskShow,
    play: &conduit_core::ActivePlayIdentity,
) -> FaceView {
    project_view(
        body_id,
        face,
        &show.planned_mask,
        play,
        show,
        show.show.lifecycle == ManifestationLifecycle::Available,
        None,
    )
}

fn project_view(
    body_id: &BodyId,
    face: &Presentation,
    planned: &conduit_presentation::PlannedMaskPlot,
    play: &conduit_core::ActivePlayIdentity,
    show: &MaskShow,
    interactions_admitted: bool,
    route: Option<RouteView>,
) -> FaceView {
    FaceView {
        schema: "conduit.browser/owner-face-mask@1",
        component: None,
        body_id: body_id.as_str().into(),
        face_id: face.identity.as_str().into(),
        face_revision: face.revision.to_string(),
        mask_plot_id: planned.mask.plot_identity.checked_plot_id.as_str().into(),
        mask_expanded_plot_id: planned
            .mask
            .plot_identity
            .expanded_plot_id
            .as_str()
            .into(),
        mask_plan_id: planned.plan.plan_id.as_str().into(),
        mask_play_id: play.active_play_id.as_str().into(),
        route,
        show_id: show.show_id.as_str().into(),
        show_state: if show.show.lifecycle == ManifestationLifecycle::Available {
            "available"
        } else {
            "prepared"
        },
        interactions_admitted,
        subjects: face
            .subjects
            .iter()
            .map(|subject| SubjectView {
                identity: subject.identity.clone(),
                name: subject.name.clone(),
                role: format!("{:?}", subject.role),
                semantic_role: match &subject.role {
                    conduit_presentation::PresentationRole::Semantic(id) => {
                        Some(id.as_str().into())
                    }
                    _ => None,
                },
                choice_multiplicity: face.properties.iter().find_map(|property| {
                    if property.subject == subject.identity
                        && property.name == "choice-multiplicity"
                    {
                        if let PresentationPropertyValue::Text(value) = &property.value {
                            return Some(value.clone());
                        }
                    }
                    None
                }),
                values: face
                    .properties
                    .iter()
                    .filter(|property| property.subject == subject.identity)
                    .filter_map(|property| match &property.value {
                        PresentationPropertyValue::TypedValue { contract, bytes } => {
                            Some(ValueView {
                                name: property.name.clone(),
                                kind: contract.value_kind.as_str().into(),
                                text: conduit_presentation::display_typed_value(contract, bytes),
                            })
                        }
                        _ => None,
                    })
                    .collect(),
                disclosure: face
                    .disclosures
                    .iter()
                    .find(|disclosure| disclosure.subject == subject.identity)
                    .map(|disclosure| format!("{:?}", disclosure.level))
                    .unwrap_or_else(|| "ExactProvenance".into()),
                text: face
                    .text
                    .iter()
                    .filter(|text| text.subject == subject.identity)
                    .map(|text| text.text.clone())
                    .collect(),
                properties: face
                    .properties
                    .iter()
                    .filter(|property| property.subject == subject.identity)
                    .map(|property| {
                        format!("{}: {}", property.name, property_value(&property.value))
                    })
                    .collect(),
                flags: face
                    .properties
                    .iter()
                    .filter(|property| property.subject == subject.identity)
                    .filter_map(|property| match &property.value {
                        PresentationPropertyValue::Flag(value) => Some(FlagView {
                            name: property.name.clone(),
                            value: *value,
                        }),
                        _ => None,
                    })
                    .collect(),
            })
            .collect(),
        relationships: face
            .relationships
            .iter()
            .map(|relationship| RelationshipView {
                source: relationship.source.clone(),
                target: relationship.target.clone(),
                kind: format!("{:?}", relationship.kind),
            })
            .collect(),
        actions: face
            .actions
            .iter()
            .map(|action| {
                let (availability, explanation, reason_code) = match &action.availability {
                    PresentationActionAvailability::Available => ("available", None, None),
                    PresentationActionAvailability::Unavailable {
                        explanation,
                        reason_code,
                    } => (
                        "unavailable",
                        Some(explanation.clone()),
                        Some(reason_code.clone()),
                    ),
                    PresentationActionAvailability::Refused {
                        explanation,
                        reason_code,
                    } => (
                        "refused",
                        Some(explanation.clone()),
                        Some(reason_code.clone()),
                    ),
                };
                ActionView {
                    identity: action.identity.clone(),
                    name: action.name.clone(),
                    intent: action.intent.clone(),
                    target: action.target.clone(),
                    disclosure: format!("{:?}", action.disclosure),
                    availability: availability.into(),
                    explanation,
                    reason_code,
                    arguments: action
                        .arguments
                        .iter()
                        .map(|argument| ArgumentView {
                            name: argument.name.clone(),
                            value_name: argument.value_name.clone(),
                            value_kind: argument.contract.value_kind.as_str().into(),
                            maximum_bytes: argument.contract.maximum_bytes,
                            choices: argument
                                .contract
                                .constraints
                                .iter()
                                .find_map(|constraint| match constraint {
                                    ValueConstraint::CanonicalMembership {
                                        members,
                                        negated: false,
                                    } => Some(
                                        members
                                            .iter()
                                            .filter_map(|value| {
                                                String::from_utf8(value.clone()).ok()
                                            })
                                            .collect(),
                                    ),
                                    _ => None,
                                })
                                .unwrap_or_default(),
                        })
                        .collect(),
                }
            })
            .collect(),
    }
}

fn property_value(value: &PresentationPropertyValue) -> String {
    match value {
        PresentationPropertyValue::Identity(value) | PresentationPropertyValue::Text(value) => {
            value.clone()
        }
        PresentationPropertyValue::BaseImplementationId(value) => value.as_str().into(),
        PresentationPropertyValue::Count(value) => value.to_string(),
        PresentationPropertyValue::Signed(value) => value.to_string(),
        PresentationPropertyValue::Flag(value) => value.to_string(),
        PresentationPropertyValue::TypedValue { contract, bytes } => {
            conduit_presentation::display_typed_value(contract, bytes)
        }
        PresentationPropertyValue::ValueContract(value) => format!("{value:?}"),
        PresentationPropertyValue::Content(value) => format!("{} content bytes", value.len()),
    }
}
