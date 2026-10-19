//! Human-readable browser projection of the exact checked Face.

use super::*;
use conduit_face::{FaceActionAvailability, FaceProperty, FacePropertyValue, FaceSubject};

impl OwnerBrowserMask {
    pub(super) fn view(&self) -> FaceView {
        let mut view = project_view(
            self.body_id.as_ref(),
            &self.face,
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
        view.component = self.component.clone();
        view
    }
}

pub(crate) fn local_view(
    body_id: &BodyId,
    face: &Face,
    show: &MaskShow,
    play: &conduit_core::ActivePlayIdentity,
) -> FaceView {
    project_view(
        Some(body_id),
        face,
        &show.planned_mask,
        play,
        show,
        show.show.lifecycle == ManifestationLifecycle::Available,
        None,
    )
}

fn project_view(
    body_id: Option<&BodyId>,
    face: &Face,
    planned: &conduit_face::PlannedMaskPlot,
    play: &conduit_core::ActivePlayIdentity,
    show: &MaskShow,
    interactions_admitted: bool,
    route: Option<RouteView>,
) -> FaceView {
    FaceView {
        schema: "conduit.browser/owner-face-mask@1",
        component: None,
        body_id: body_id.map(|id| id.as_str().into()),
        face_id: face.identity.as_str().into(),
        face_revision: face.revision.to_string(),
        mask_plot_id: planned.mask.plot_identity.checked_plot_id.as_str().into(),
        mask_expanded_plot_id: planned.mask.plot_identity.expanded_plot_id.as_str().into(),
        mask_plan_id: planned.plan.plan_id.as_str().into(),
        mask_play_id: play.active_play_id.as_str().into(),
        route,
        show_id: show.show_id.as_str().into(),
        show_state: match show.show.lifecycle {
            ManifestationLifecycle::Prepared => "prepared",
            ManifestationLifecycle::Available => "available",
            ManifestationLifecycle::Replaced => "replaced",
            ManifestationLifecycle::Closed => "closed",
            ManifestationLifecycle::Failed => "failed",
        },
        show_failure: show.show.failure,
        interactions_admitted: interactions_admitted
            && show.show.lifecycle == ManifestationLifecycle::Available,
        subjects: face
            .subjects
            .iter()
            .map(|subject| SubjectView {
                identity: subject.identity.clone(),
                name: subject.name.clone(),
                role: format!("{:?}", subject.role),
                semantic_role: match &subject.role {
                    conduit_face::FaceRole::Semantic(id) => Some(id.as_str().into()),
                    _ => None,
                },
                choice_multiplicity: face.properties.iter().find_map(|property| {
                    if property.subject == subject.identity
                        && property.name == "choice-multiplicity"
                    {
                        if let FacePropertyValue::Text(value) = &property.value {
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
                        FacePropertyValue::TypedValue { contract, bytes } => Some(ValueView {
                            name: property.name.clone(),
                            kind: contract.value_kind.as_str().into(),
                            text: conduit_face::display_typed_value(contract, bytes),
                        }),
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
                unmodeled_property_count: unmodeled_properties(&face.properties, subject),
                flags: face
                    .properties
                    .iter()
                    .filter(|property| property.subject == subject.identity)
                    .filter_map(|property| match &property.value {
                        FacePropertyValue::Flag(value) => Some(FlagView {
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
                    FaceActionAvailability::Available => ("available", None, None),
                    FaceActionAvailability::Unavailable {
                        explanation,
                        reason_code,
                    } => (
                        "unavailable",
                        Some(explanation.clone()),
                        Some(reason_code.clone()),
                    ),
                    FaceActionAvailability::Refused {
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

fn property_value(value: &FacePropertyValue) -> String {
    match value {
        FacePropertyValue::Identity(value) | FacePropertyValue::Text(value) => value.clone(),
        FacePropertyValue::BaseImplementationId(value) => value.as_str().into(),
        FacePropertyValue::Count(value) => value.to_string(),
        FacePropertyValue::Signed(value) => value.to_string(),
        FacePropertyValue::Flag(value) => value.to_string(),
        FacePropertyValue::TypedValue { contract, bytes } => {
            conduit_face::display_typed_value(contract, bytes)
        }
        FacePropertyValue::ValueContract(value) => format!("{value:?}"),
        FacePropertyValue::Content(value) => format!("{} content bytes", value.len()),
    }
}

// Keep every property in the generic facts list. This count describes only
// facts not also represented by the typed values, flags or choice metadata.
// A renderer can recognize that exact closed profile without parsing text.
fn unmodeled_properties(properties: &[FaceProperty], subject: &FaceSubject) -> usize {
    let choice_group = matches!(&subject.role, conduit_face::FaceRole::Semantic(id)
        if id.as_str() == "conduit.face/choice-group@1");
    let choice_properties = properties
        .iter()
        .filter(|property| {
            property.subject == subject.identity && property.name == "choice-multiplicity"
        })
        .count();
    properties
        .iter()
        .filter(|property| property.subject == subject.identity)
        .filter(|property| match &property.value {
            FacePropertyValue::Flag(_) | FacePropertyValue::TypedValue { .. } => false,
            FacePropertyValue::Text(_)
                if choice_group
                    && choice_properties == 1
                    && property.name == "choice-multiplicity" =>
            {
                false
            }
            _ => true,
        })
        .count()
}

#[cfg(test)]
mod choice_projection_tests {
    use super::*;
    #[test]
    fn real_thermostat_properties_keep_exact_facts_and_recognized_choice_metadata() {
        let mut fragment = conduit_thermostat_face::fragment(
            &conduit_thermostat_plot::ThermostatState::default(),
            conduit_face::FaceContributionBasis {
                checked_plot_id: "checked/choice-projection".into(),
                plan_id: "plan/choice-projection".into(),
                active_play_id: "play/choice-projection".into(),
                required_interaction_context: None,
            },
            true,
        )
        .unwrap();
        for id in [
            "thermostat/mode",
            "thermostat.mode.heat",
            "thermostat.mode.off",
        ] {
            let subject = fragment.subjects.iter().find(|s| s.identity == id).unwrap();
            assert_eq!(unmodeled_properties(&fragment.properties, subject), 0);
            assert!(fragment.properties.iter().any(|p| p.subject == id));
        }
        let heat = fragment
            .subjects
            .iter()
            .find(|s| s.identity == "thermostat.mode.heat")
            .unwrap();
        fragment.properties.push(FaceProperty {
            subject: heat.identity.clone(),
            name: "authored-fact".into(),
            value: FacePropertyValue::Text("retain me".into()),
        });
        assert_eq!(unmodeled_properties(&fragment.properties, heat), 1);
        // Choice metadata on an Action is not a recognized group contract.
        fragment.properties.push(FaceProperty {
            subject: heat.identity.clone(),
            name: "choice-multiplicity".into(),
            value: FacePropertyValue::Text("exclusive".into()),
        });
        assert_eq!(unmodeled_properties(&fragment.properties, heat), 2);
        let group = fragment
            .subjects
            .iter()
            .find(|s| s.identity == "thermostat/mode")
            .unwrap();
        // Duplicate or contradictory metadata must retain the generic fallback.
        fragment.properties.push(FaceProperty {
            subject: group.identity.clone(),
            name: "choice-multiplicity".into(),
            value: FacePropertyValue::Text("multiple".into()),
        });
        assert_eq!(unmodeled_properties(&fragment.properties, group), 2);
    }
}
