//! Partition the existing exact aural plan, never a second Todo fact/count reducer.
use super::*;
use crate::{FaceUtteranceProvenance, Presentation, PresentationDisclosureLevel, PresentationRole};
use alloc::vec;

pub struct SpokenTemplateSlots {
    pub(super) face_id: String,
    pub(super) revision: u64,
    pub(super) plan_digest: [u8; 32],
    pub(super) values: [String; 6],
    pub(super) indices: Vec<u32>,
    pub(super) actions: Vec<String>,
}

/// `selected_text_indices` are Face.text indices supplied by the existing outline,
/// not utterance-clause indices. They are validated as references, never used to
/// remove essential facts. Both expressions retain every clause of this exact
/// admitted aural projection, including unavailable action and unknown-role truth.
/// Three total Item subjects is this bounded slice; larger Faces honestly refuse.
pub fn prepare_spoken_template_slots(
    face: &Presentation,
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
            .find(|value| value.subject == word.subject)
            .map(|value| value.level)
            .unwrap_or(PresentationDisclosureLevel::Primary);
        if !matches!(
            level,
            PresentationDisclosureLevel::Primary | PresentationDisclosureLevel::Context
        ) {
            return Err(SpokenTemplateRefusal::Disclosure);
        }
    }
    let titles: Vec<_> = face
        .subjects
        .iter()
        .filter(|subject| {
            matches!(
                subject.role,
                PresentationRole::Collection | PresentationRole::Document
            )
        })
        .collect();
    if titles.len() != 1 {
        return Err(SpokenTemplateRefusal::Selection);
    }
    if !face.text.iter().any(|word| {
        face.subjects.iter().any(|subject| {
            subject.identity == word.subject && subject.role == PresentationRole::Status
        })
    }) {
        return Err(SpokenTemplateRefusal::MissingEssential);
    }
    // Item order comes from the semantic aural plan, never incidental vector order.
    let items: Vec<_> = plan
        .clauses
        .iter()
        .filter_map(|clause| match &clause.provenance {
            FaceUtteranceProvenance::Subject(source) => face.subjects.iter().find(|subject| {
                &subject.identity == source.identity() && subject.role == PresentationRole::Item
            }),
            _ => None,
        })
        .collect();
    if items.len() > 3 {
        return Err(SpokenTemplateRefusal::Bound);
    }
    let mut groups: [Vec<(u32, &str)>; 6] = core::array::from_fn(|_| vec![]);
    let mut actions = Vec::new();
    for (index, clause) in plan.clauses.iter().enumerate() {
        let subject = match &clause.provenance {
            FaceUtteranceProvenance::Subject(source) => Some(source.identity().as_str()),
            FaceUtteranceProvenance::Property(source) => face
                .properties
                .get(*source.index() as usize)
                .map(|value| value.subject.as_str()),
            FaceUtteranceProvenance::Text(source) => face
                .text
                .get(*source.index() as usize)
                .map(|value| value.subject.as_str()),
            _ => None,
        };
        let slot = match &clause.provenance {
            FaceUtteranceProvenance::Action(source) => {
                actions.push(source.identity().clone());
                5
            }
            FaceUtteranceProvenance::ActionArgument(_) => 5,
            FaceUtteranceProvenance::Subject(source)
                if source.identity() == &titles[0].identity =>
            {
                0
            }
            _ => subject
                .and_then(|subject| items.iter().position(|item| item.identity == subject))
                .map_or(1, |item| item + 2),
        };
        groups[slot].push((index as u32, clause.text.as_str()));
    }
    let mut values: [String; 6] = core::array::from_fn(|_| String::new());
    let mut indices = Vec::new();
    for (index, group) in groups.iter().enumerate() {
        values[index] = group
            .iter()
            .map(|(_, words)| *words)
            .collect::<Vec<_>>()
            .join(" ");
        if values[index].len() > MAX_SPOKEN_TEMPLATE_SLOT_BYTES {
            return Err(SpokenTemplateRefusal::Bound);
        }
        indices.extend(group.iter().map(|(index, _)| *index));
    }
    if indices.len() != plan.clauses.len() || values[0].is_empty() || values[1].is_empty() {
        return Err(SpokenTemplateRefusal::MissingEssential);
    }
    Ok(SpokenTemplateSlots {
        face_id: face.identity.as_str().into(),
        revision: face.revision,
        plan_digest: plan.digest,
        values,
        indices,
        actions,
    })
}
