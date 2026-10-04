use super::*;
use conduit_core::ValueConstraint;
use conduit_presentation::readable_finite_text_choices;

/// Keep the common Face's reading order and exact provenance; only the spoken
/// phrasing changes. Unknown semantic roles remain readable as their human
/// leaf name, while the full identity stays inspectable in the Face.
pub(super) fn voice_clauses(
    face: &Presentation,
    plan: &FaceUtterancePlan,
) -> Result<Vec<String>, SpokenFaceRefusal> {
    const MAX_VOICED_CLAUSE_BYTES: usize = 4_096;
    const MAX_VOICED_VIEW_BYTES: usize = 2 * 1024 * 1024;
    let mut voiced = Vec::with_capacity(plan.clauses.len());
    let mut bytes = 0usize;
    for clause in &plan.clauses {
        let text = voice_clause(face, clause);
        bytes = bytes
            .checked_add(text.len() + 1)
            .ok_or(SpokenFaceRefusal::VoiceBound)?;
        if text.is_empty() || text.len() > MAX_VOICED_CLAUSE_BYTES || bytes > MAX_VOICED_VIEW_BYTES
        {
            return Err(SpokenFaceRefusal::VoiceBound);
        }
        voiced.push(text);
    }
    Ok(voiced)
}

fn voice_clause(face: &Presentation, clause: &FaceUtteranceClause) -> String {
    match &clause.provenance {
        FaceUtteranceProvenance::Subject(source) => {
            let Some(subject) = face
                .subjects
                .iter()
                .find(|item| item.identity == *source.identity())
            else {
                return clause.text.clone();
            };
            format!("{}, {}.", subject.name, spoken_role(&subject.role))
        }
        FaceUtteranceProvenance::Relationship(source) => {
            let Some(relationship) = face.relationships.get(*source.index() as usize) else {
                return clause.text.clone();
            };
            let name = |id: &str| {
                face.subjects
                    .iter()
                    .find(|item| item.identity == id)
                    .map(|item| item.name.as_str())
            };
            let (Some(from), Some(to)) = (name(&relationship.source), name(&relationship.target))
            else {
                return clause.text.clone();
            };
            match &relationship.kind {
                PresentationRelationshipKind::Contains => format!("{from} contains {to}."),
                PresentationRelationshipKind::Connects => format!("{from} connects to {to}."),
                PresentationRelationshipKind::Describes => format!("{from} describes {to}."),
                PresentationRelationshipKind::Realizes => format!("{from} realizes {to}."),
                PresentationRelationshipKind::Observes => format!("{from} observes {to}."),
                PresentationRelationshipKind::Semantic(id) => {
                    format!("{from}: {}: {to}.", human_leaf(id.as_str()))
                }
            }
        }
        FaceUtteranceProvenance::Property(source) => {
            let Some(property) = face.properties.get(*source.index() as usize) else {
                return clause.text.clone();
            };
            let name = face
                .subjects
                .iter()
                .find(|item| item.identity == property.subject)
                .map(|item| item.name.as_str())
                .unwrap_or("This item");
            match &property.value {
                PresentationPropertyValue::Text(value) => {
                    format!("{name}, {}: {value}.", property.name)
                }
                PresentationPropertyValue::Count(value) => {
                    format!("{name}, {}: {value}.", property.name)
                }
                PresentationPropertyValue::Signed(value) => {
                    format!("{name}, {}: {value}.", property.name)
                }
                PresentationPropertyValue::Flag(value) => format!(
                    "{name}, {}: {}.",
                    property.name,
                    if *value { "yes" } else { "no" }
                ),
                _ => clause.text.clone(),
            }
        }
        FaceUtteranceProvenance::Action(source) => {
            let Some(action) = face
                .actions
                .iter()
                .find(|item| item.identity == *source.identity())
            else {
                return clause.text.clone();
            };
            let target = face
                .subjects
                .iter()
                .find(|item| item.identity == action.target)
                .map(|item| item.name.as_str())
                .unwrap_or("this item");
            match &action.availability {
                PresentationActionAvailability::Available => {
                    format!("{}. For {target}. Available.", action.name)
                }
                PresentationActionAvailability::Unavailable { explanation, .. } => {
                    format!("{}. For {target}. Unavailable: {explanation}", action.name)
                }
                PresentationActionAvailability::Refused { explanation, .. } => {
                    format!("{}. For {target}. Refused: {explanation}", action.name)
                }
            }
        }
        FaceUtteranceProvenance::ActionArgument(source) => {
            let Some(action) = face
                .actions
                .iter()
                .find(|item| item.identity == *source.action_identity())
            else {
                return clause.text.clone();
            };
            let Some(argument) = action
                .arguments
                .iter()
                .find(|item| item.name == *source.argument_name())
            else {
                return clause.text.clone();
            };
            match (
                argument.contract.value_kind.as_str(),
                argument.contract.constraints.as_slice(),
            ) {
                (UTF8_TEXT_VALUE_KIND, [ValueConstraint::ByteLength { minimum, maximum }]) => {
                    format!(
                        "{}. Enter {minimum} to {maximum} UTF-8 bytes with edit {}, then activate {}.",
                        argument.value_name, argument.name, action.name
                    )
                }
                ("value/bool", _) => format!(
                    "{}. Choose true or false with edit {}, then activate {}.",
                    argument.value_name, argument.name, action.name
                ),
                (UTF8_TEXT_VALUE_KIND, _) => readable_finite_text_choices(&argument.contract)
                    .map_or_else(
                        || {
                            format!(
                                "{} Use edit {} to enter a valid value, then activate {}.",
                                clause.text, argument.name, action.name
                            )
                        },
                        |choices| {
                            format!(
                                "{}. Choose one of: {}. Enter it with edit {}, then activate {}.",
                                argument.value_name,
                                choices.join(", "),
                                argument.name,
                                action.name
                            )
                        },
                    ),
                _ => clause.text.clone(),
            }
        }
        FaceUtteranceProvenance::Text(_) => clause.text.clone(),
        _ => clause.text.clone(),
    }
}

fn spoken_role(role: &PresentationRole) -> String {
    match role {
        PresentationRole::Semantic(id) => human_leaf(id.as_str()),
        PresentationRole::TextEntry => "text entry".into(),
        PresentationRole::Info => "information".into(),
        PresentationRole::Status => "status".into(),
        PresentationRole::Region => "region".into(),
        PresentationRole::Collection => "collection".into(),
        PresentationRole::Item => "item".into(),
        PresentationRole::Action => "action".into(),
        other => format!("{other:?}").to_lowercase(),
    }
}

fn human_leaf(identity: &str) -> String {
    identity
        .rsplit('/')
        .next()
        .unwrap_or(identity)
        .replace(['-', '_'], " ")
}
