//! Human-readable browser projection of the exact checked Face.

use super::*;
use conduit_presentation::{PresentationActionAvailability, PresentationPropertyValue};

impl OwnerBrowserMask {
    pub(super) fn view(&self) -> FaceView {
        let face = &self.presentation;
        FaceView {
            schema: "conduit.browser/owner-face-mask@1",
            body_id: self.body_id.as_str().into(),
            face_id: face.identity.as_str().into(),
            face_revision: face.revision.to_string(),
            mask_plot_id: self
                .planned
                .mask
                .plot_identity
                .checked_plot_id
                .as_str()
                .into(),
            mask_plan_id: self.planned.plan.plan_id.as_str().into(),
            mask_play_id: self.play.active_play_id.as_str().into(),
            show_id: self.show.show_id.as_str().into(),
            show_state: if self.show.show.lifecycle == ManifestationLifecycle::Available {
                "available"
            } else {
                "prepared"
            },
            interactions_admitted: self.interactions_admitted,
            subjects: face
                .subjects
                .iter()
                .map(|subject| SubjectView {
                    identity: subject.identity.clone(),
                    name: subject.name.clone(),
                    role: format!("{:?}", subject.role),
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
                        availability: availability.into(),
                        explanation,
                        reason_code,
                        arguments: action
                            .arguments
                            .iter()
                            .map(|argument| ArgumentView {
                                name: argument.name.clone(),
                                value_name: argument.value_name.clone(),
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
                        current_value: face.properties.iter().find_map(|property| {
                            if property.subject == action.target && property.name == "interval-ms" {
                                if let PresentationPropertyValue::Count(value) = &property.value {
                                    return Some(value.to_string());
                                }
                            }
                            None
                        }),
                    }
                })
                .collect(),
        }
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
        PresentationPropertyValue::ValueContract(value) => format!("{value:?}"),
        PresentationPropertyValue::Content(value) => format!("{} content bytes", value.len()),
    }
}
