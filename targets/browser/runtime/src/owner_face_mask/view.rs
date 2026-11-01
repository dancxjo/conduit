//! Human-readable browser projection of the exact checked Face.

use super::*;
use conduit_face::{FaceActionAvailability, FaceProperty, FacePropertyValue, FaceSubject};

impl OwnerBrowserMask {
    pub(super) fn view(&self) -> FaceView {
        let mut view = project_view(
            &self.body_id,
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
    local_view_admitted(body_id, face, show, play, true)
}

pub(crate) fn local_view_admitted(
    body_id: &BodyId,
    face: &Face,
    show: &MaskShow,
    play: &conduit_core::ActivePlayIdentity,
    admitted: bool,
) -> FaceView {
    project_view(
        body_id,
        face,
        &show.planned_mask,
        play,
        show,
        admitted && show.show.lifecycle == ManifestationLifecycle::Available,
        None,
    )
}

fn project_view(
    body_id: &BodyId,
    face: &Face,
    planned: &conduit_face::PlannedMaskPlot,
    play: &conduit_core::ActivePlayIdentity,
    show: &MaskShow,
    interactions_admitted: bool,
    route: Option<RouteView>,
) -> FaceView {
    FaceView {
        schema: "conduit.browser/owner-face-mask@2",
        component: None,
        body_id: body_id.as_str().into(),
        face_id: face.identity.as_str().into(),
        face_revision: face.revision.to_string(),
        mask_plot_id: planned.mask.plot_identity.checked_plot_id.as_str().into(),
        mask_expanded_plot_id: planned.mask.plot_identity.expanded_plot_id.as_str().into(),
        mask_plan_id: planned.plan.plan_id.as_str().into(),
        mask_play_id: play.active_play_id.as_str().into(),
        mask_renderer_artifact_id: planned.show_placement().artifact_id.as_str().into(),
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
                            current_value: current_text_value(face, action),
                            name: argument.name.clone(),
                            value_name: argument.value_name.clone(),
                            value_kind: argument.contract.value_kind.as_str().into(),
                            maximum_bytes: argument.contract.maximum_bytes,
                            minimum_bytes: argument
                                .contract
                                .constraints
                                .iter()
                                .filter_map(|constraint| match constraint {
                                    ValueConstraint::ByteLength { minimum, .. } => Some(*minimum),
                                    _ => None,
                                })
                                .max()
                                .unwrap_or(0),
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

// Only an exact typed Source value/action association supplies initial input.
// Human-readable Face text is never interpreted as editable state.
fn current_text_value(face: &Face, action: &conduit_face::FaceAction) -> Option<String> {
    let [argument] = action.arguments.as_slice() else {
        return None;
    };
    if argument.contract.value_kind.as_str() != conduit_face::UTF8_TEXT_VALUE_KIND
        || !face.subjects.iter().any(|subject| {
            subject.identity == action.target && subject.role == conduit_face::FaceRole::TextEntry
        })
    {
        return None;
    }
    let unique = |name: &str| {
        let mut properties = face
            .properties
            .iter()
            .filter(|p| p.subject == action.target && p.name == name);
        let value = &properties.next()?.value;
        properties.next().is_none().then_some(value)
    };
    let FacePropertyValue::Identity(binding) = unique("value-change-action")? else {
        return None;
    };
    if binding != &action.identity {
        return None;
    }
    let FacePropertyValue::TypedValue { contract, bytes } = unique("current-value")? else {
        return None;
    };
    if contract != &argument.contract || contract.validate(bytes).is_err() {
        return None;
    }
    String::from_utf8(bytes.clone()).ok()
}

#[cfg(test)]
mod current_value_tests {
    use super::*;

    #[test]
    fn editable_text_requires_unique_exact_typed_source_association() {
        let mut face: Face = serde_json::from_str(include_str!(
            "../../../../../proof/conformance/face-waist/domain-derived/open-domain-face.json"
        ))
        .unwrap();
        let action = conduit_face::FaceAction {
            identity: "edit".into(),
            intent: "edit@1".into(),
            target: "document".into(),
            name: "Edit".into(),
            arguments: vec![conduit_face::FaceActionArgument::text(
                "text".into(),
                "Text".into(),
                0,
                256,
            )
            .unwrap()],
            disclosure: conduit_face::FaceDisclosureLevel::CurrentAction,
            availability: FaceActionAvailability::Available,
        };
        face.subjects.push(FaceSubject {
            identity: "document".into(),
            role: conduit_face::FaceRole::TextEntry,
            name: "Document".into(),
        });
        let value = FaceProperty {
            subject: "document".into(),
            name: "current-value".into(),
            value: FacePropertyValue::TypedValue {
                contract: action.arguments[0].contract.clone(),
                bytes: b"I was here.".to_vec(),
            },
        };
        face.properties.push(value.clone());
        assert_eq!(current_text_value(&face, &action), None);
        face.properties.push(FaceProperty {
            subject: "document".into(),
            name: "value-change-action".into(),
            value: FacePropertyValue::Identity("edit".into()),
        });
        assert_eq!(
            current_text_value(&face, &action).as_deref(),
            Some("I was here.")
        );
        let mut foreign = action.clone();
        foreign.identity = "other-edit".into();
        assert_eq!(current_text_value(&face, &foreign), None);
        let mut different = action.clone();
        different.arguments[0].contract.maximum_bytes = 128;
        assert_eq!(current_text_value(&face, &different), None);
        face.properties.push(value);
        assert_eq!(current_text_value(&face, &action), None);
    }
}
