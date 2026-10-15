//! A checked reading of one primary control Region, using exact aural provenance.
use super::*;
use crate::{Face, FaceDisclosureLevel, FaceRelationshipKind, FaceRole, FaceUtteranceProvenance};
use alloc::{collections::BTreeSet, vec};

/// Read the primary Region's human text, typed values, actions and temporal facts.
/// Structural roles, containment, disclosure and implementation counters stay inspectable in Face.
/// The original Face, identity and full utterance digest remain authoritative.
pub fn prepare_spoken_control_slots(
    face: &Face,
    selected_text_indices: &[u32],
) -> Result<SpokenTemplateSlots, SpokenTemplateRefusal> {
    let plan = crate::plan_face_utterances(face).map_err(|_| SpokenTemplateRefusal::Selection)?;
    for index in selected_text_indices {
        let word = face
            .text
            .get(*index as usize)
            .ok_or(SpokenTemplateRefusal::Selection)?;
        let level = face
            .disclosures
            .iter()
            .find(|item| item.subject == word.subject)
            .map_or(FaceDisclosureLevel::Primary, |item| item.level);
        if !matches!(
            level,
            FaceDisclosureLevel::Primary | FaceDisclosureLevel::Context
        ) {
            return Err(SpokenTemplateRefusal::Disclosure);
        }
    }
    let roots: Vec<_> = face
        .subjects
        .iter()
        .filter(|subject| {
            subject.role == FaceRole::Region
                && face
                    .disclosures
                    .iter()
                    .find(|item| item.subject == subject.identity)
                    .map_or(FaceDisclosureLevel::Primary, |item| item.level)
                    == FaceDisclosureLevel::Primary
        })
        .collect();
    let [root] = roots.as_slice() else {
        return Err(SpokenTemplateRefusal::Selection);
    };
    let mut scope = BTreeSet::from([root.identity.as_str()]);
    for _ in 0..face.subjects.len() {
        let mut changed = false;
        for relation in &face.relationships {
            if relation.kind == FaceRelationshipKind::Contains
                && scope.contains(relation.source.as_str())
            {
                changed |= scope.insert(relation.target.as_str());
            }
        }
        if !changed {
            break;
        }
    }
    if !face.text.iter().any(|text| {
        scope.contains(text.subject.as_str())
            && face
                .subjects
                .iter()
                .any(|subject| subject.identity == text.subject && subject.role == FaceRole::Status)
    }) {
        return Err(SpokenTemplateRefusal::MissingEssential);
    }
    let action_in_scope = |id: &str| {
        face.actions
            .iter()
            .any(|action| action.identity == id && scope.contains(action.target.as_str()))
    };
    let mut groups: [Vec<(u32, String)>; 6] = core::array::from_fn(|_| vec![]);
    let mut actions = Vec::new();
    for (index, clause) in plan.clauses.iter().enumerate() {
        let included = match &clause.provenance {
            FaceUtteranceProvenance::Subject(source) => source.identity() == &root.identity,
            FaceUtteranceProvenance::Property(source) => face
                .properties
                .get(*source.index() as usize)
                .is_some_and(|item| {
                    scope.contains(item.subject.as_str())
                        && matches!(item.value, crate::FacePropertyValue::TypedValue { .. })
                }),
            FaceUtteranceProvenance::Relationship(_) | FaceUtteranceProvenance::Composition(_) => {
                false
            }
            FaceUtteranceProvenance::Text(source) => face
                .text
                .get(*source.index() as usize)
                .is_some_and(|item| scope.contains(item.subject.as_str())),
            FaceUtteranceProvenance::Action(source) => action_in_scope(source.identity()),
            FaceUtteranceProvenance::ActionArgument(source) => {
                action_in_scope(source.action_identity())
            }
            FaceUtteranceProvenance::Disclosure(_) => false,
            FaceUtteranceProvenance::TemporalFact(source) => face
                .temporal_facts
                .get(*source.index() as usize)
                .is_some_and(|item| scope.contains(item.subject.as_str())),
            FaceUtteranceProvenance::TemporalReference(source) => face
                .temporal_references
                .get(*source.index() as usize)
                .is_some_and(|reference| {
                    face.temporal_facts.iter().any(|fact| {
                        scope.contains(fact.subject.as_str())
                            && fact.reference == reference.identity
                    })
                }),
        };
        if !included {
            continue;
        }
        let slot = match &clause.provenance {
            FaceUtteranceProvenance::Subject(source) if source.identity() == &root.identity => 0,
            FaceUtteranceProvenance::Action(source) => {
                actions.push(source.identity().clone());
                5
            }
            FaceUtteranceProvenance::ActionArgument(_) => 5,
            _ => 1,
        };
        // Labels come from this exact Face, never a second settings reducer.
        let label = match &clause.provenance {
            FaceUtteranceProvenance::Text(source) => face
                .text
                .get(*source.index() as usize)
                .filter(|item| item.subject != root.identity)
                .and_then(|item| {
                    face.subjects
                        .iter()
                        .find(|subject| subject.identity == item.subject)
                })
                .map(|subject| subject.name.as_str()),
            FaceUtteranceProvenance::Action(source) => face
                .actions
                .iter()
                .find(|action| action.identity == *source.identity())
                .and_then(|action| {
                    face.relationships.iter().find(|relation| {
                        relation.kind == FaceRelationshipKind::Contains
                            && relation.target == action.target
                            && relation.source != root.identity
                            && scope.contains(relation.source.as_str())
                    })
                })
                .and_then(|relation| {
                    face.subjects
                        .iter()
                        .find(|subject| subject.identity == relation.source)
                })
                .map(|subject| subject.name.as_str()),
            _ => None,
        };
        let words = label.map_or_else(
            || clause.text.clone(),
            |label| alloc::format!("{label}: {}", clause.text),
        );
        groups[slot].push((index as u32, words));
    }
    let mut values: [String; 6] = core::array::from_fn(|_| String::new());
    let mut indices = Vec::new();
    for (index, group) in groups.iter().enumerate() {
        values[index] = group
            .iter()
            .map(|(_, words)| words.as_str())
            .collect::<Vec<_>>()
            .join(" ");
        if values[index].len() > MAX_SPOKEN_TEMPLATE_SLOT_BYTES {
            return Err(SpokenTemplateRefusal::Bound);
        }
        indices.extend(group.iter().map(|(index, _)| *index));
    }
    if values[0].is_empty() || values[1].is_empty() {
        return Err(SpokenTemplateRefusal::MissingEssential);
    }
    Ok(SpokenTemplateSlots {
        face_id: face.identity.as_str().into(),
        revision: face.revision,
        plan_digest: plan.digest,
        values,
        indices,
        actions,
        control: true,
    })
}
