//! Deterministic speech selection from one canonical Face; never application state.
use super::*;
use conduit_presentation::{
    GeneratedWordingClause, GeneratedWordingProposal, GeneratedWordingRefusal,
    PresentationDisclosureLevel,
};

/// Inspectable selection identity and references; the canonical Face retains all detail.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct SpokenOutline {
    pub face_id: String,
    pub face_revision: u64,
    pub clauses: Vec<String>,
    pub text_indices: Vec<u32>,
    pub action_ids: Vec<String>,
}

impl SpokenOutline {
    /// Accept finite model wording only from the selected facts and offered actions.
    /// Exact quotation/value checking remains owned by the existing semantic validator.
    pub fn render_model_wording(
        &self,
        face: &Presentation,
        proposal: &GeneratedWordingProposal,
    ) -> Result<String, GeneratedWordingRefusal> {
        if self.face_id != face.identity.as_str() || self.face_revision != face.revision {
            return Err(GeneratedWordingRefusal::StaleFace);
        }
        if select_spoken_outline(face).ok().as_ref() != Some(self) {
            return Err(GeneratedWordingRefusal::InventedClaim);
        }
        let rendered = proposal.render_exact(face)?;
        for clause in &proposal.clauses {
            let selected = match clause {
                GeneratedWordingClause::Text { index, .. } => self.text_indices.contains(index),
                GeneratedWordingClause::Action { identity, .. } => {
                    self.action_ids.contains(identity)
                }
                GeneratedWordingClause::Property { .. } => false,
            };
            if !selected {
                return Err(GeneratedWordingRefusal::InventedClaim);
            }
        }
        Ok(rendered)
    }
}

/// A cursor retains correlation and an ordinal, never a copied Todo state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpokenItemCursor {
    face_id: String,
    revision: u64,
    offset: usize,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpokenItemPage {
    pub subject_ids: Vec<String>,
    pub clauses: Vec<String>,
    pub more: bool,
}
impl SpokenItemCursor {
    pub fn new(face: &Presentation) -> Result<Self, SpokenFaceRefusal> {
        FaceReadingCursor::new(face).map_err(reading_refusal)?;
        Ok(Self {
            face_id: face.identity.as_str().into(),
            revision: face.revision,
            offset: 0,
        })
    }
    /// Three items per explicit request. A rejected page never advances the cursor.
    pub fn next_page(&mut self, face: &Presentation) -> Result<SpokenItemPage, SpokenFaceRefusal> {
        if self.face_id != face.identity.as_str() || self.revision != face.revision {
            return Err(SpokenFaceRefusal::StaleFace);
        }
        FaceReadingCursor::new(face).map_err(reading_refusal)?;
        let items = face
            .subjects
            .iter()
            .filter(|subject| {
                subject.role == PresentationRole::Item
                    && face
                        .disclosures
                        .iter()
                        .find(|item| item.subject == subject.identity)
                        .is_none_or(|item| item.level == PresentationDisclosureLevel::Primary)
            })
            .skip(self.offset)
            .take(4)
            .collect::<Vec<_>>();
        let selected = &items[..items.len().min(3)];
        let clauses = selected
            .iter()
            .map(|item| format!("{}, item.", item.name))
            .collect::<Vec<_>>();
        if clauses.iter().map(|clause| clause.len()).sum::<usize>() > 4_096 {
            return Err(SpokenFaceRefusal::VoiceBound);
        }
        self.offset += selected.len();
        Ok(SpokenItemPage {
            subject_ids: selected.iter().map(|item| item.identity.clone()).collect(),
            clauses,
            more: items.len() > 3,
        })
    }
}

/// Select a bounded first utterance from Face wording and explicitly primary
/// subjects. This is a Mask reading policy, not a replacement for Face truth.
pub fn select_spoken_outline(face: &Presentation) -> Result<SpokenOutline, SpokenFaceRefusal> {
    FaceReadingCursor::new(face).map_err(reading_refusal)?;
    let mut text_indices = Vec::new();
    let mut action_ids = Vec::new();
    let level = |identity: &str| {
        face.disclosures
            .iter()
            .find(|disclosure| disclosure.subject == identity)
            .map(|disclosure| disclosure.level)
    };
    let primary = |identity: &str| {
        matches!(
            level(identity),
            None | Some(PresentationDisclosureLevel::Primary)
        )
    };
    let subject_role = |identity: &str| {
        face.subjects
            .iter()
            .find(|subject| subject.identity == identity)
            .map(|subject| &subject.role)
    };
    let has_application_wording = face.text.iter().any(|wording| {
        (primary(&wording.subject)
            || level(&wording.subject) == Some(PresentationDisclosureLevel::Context))
            && !matches!(subject_role(&wording.subject), Some(PresentationRole::Body))
    });
    let mut result = Vec::new();
    let title = face.subjects.iter().find(|subject| {
        matches!(
            level(&subject.identity),
            Some(PresentationDisclosureLevel::Context | PresentationDisclosureLevel::Primary)
        ) && matches!(
            subject.role,
            PresentationRole::Collection | PresentationRole::Document
        )
    });
    if let Some(subject) = title {
        result.push(format!("{}.", subject.name));
    }
    let mut omitted = false;
    // Face Context is the encounter's orientation, so it precedes the result
    // even if it was serialized after primary wording in the Face.
    for expected in [
        Some(PresentationDisclosureLevel::Context),
        None,
        Some(PresentationDisclosureLevel::Primary),
    ] {
        for (index, wording) in face.text.iter().enumerate().filter(|(_, wording)| {
            level(&wording.subject) == expected
                && !title.is_some_and(|subject| {
                    wording.subject == subject.identity && wording.text == subject.name
                })
                && !(has_application_wording
                    && matches!(subject_role(&wording.subject), Some(PresentationRole::Body)))
        }) {
            if result.len() >= 4 {
                omitted = true;
                break;
            }
            result.push(wording.text.clone());
            text_indices.push(index as u32);
        }
    }
    let primary_items = face
        .subjects
        .iter()
        .filter(|subject| primary(&subject.identity) && subject.role == PresentationRole::Item)
        .collect::<Vec<_>>();
    for subject in primary_items.iter().take(3) {
        result.push(format!("{}.", subject.name));
    }
    if primary_items.len() > 3 {
        omitted = true;
    }
    if result.is_empty() {
        if let Some(subject) = face.subjects.iter().find(|subject| {
            primary(&subject.identity)
                && matches!(
                    subject.role,
                    PresentationRole::Body | PresentationRole::Region
                )
        }) {
            result.push(format!("{}.", subject.name));
        }
    }
    if result.is_empty() {
        return Err(SpokenFaceRefusal::VoiceBound);
    }
    if omitted {
        if primary_items.len() > 3 {
            result.push("Type read current items to hear what remains.".into());
        } else {
            result.push("Type read all for more detail.".into());
        }
    } else if result.len() < 7 {
        // Application wording should lead to a content action, not a generic
        // Body or Plot navigation prompt from the same Face.
        let mut offered = face.actions.iter().filter(|action| {
            action.availability.is_available()
                && action.disclosure == PresentationDisclosureLevel::CurrentAction
                && (!has_application_wording
                    || matches!(
                        subject_role(&action.target),
                        Some(
                            PresentationRole::Collection
                                | PresentationRole::Item
                                | PresentationRole::Document
                                | PresentationRole::TextEntry
                        )
                    ))
                && matches!(
                    level(&action.target),
                    None | Some(PresentationDisclosureLevel::Primary)
                        | Some(PresentationDisclosureLevel::Context)
                )
        });
        if let Some(action) = offered
            .clone()
            .find(|action| level(&action.target) == Some(PresentationDisclosureLevel::Context))
            .or_else(|| offered.next())
        {
            result.push(format!("You can {}.", action.name));
            action_ids.push(action.identity.clone());
        }
    }
    if result.iter().map(|clause| clause.len()).sum::<usize>() > 4_096 {
        return Err(SpokenFaceRefusal::VoiceBound);
    }
    Ok(SpokenOutline {
        face_id: face.identity.as_str().into(),
        face_revision: face.revision,
        clauses: result,
        text_indices,
        action_ids,
    })
}
