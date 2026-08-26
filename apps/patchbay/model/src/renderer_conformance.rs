//! Renderer-neutral equivalence oracle for Patchbay entrance manifestations.

use conduit_presentation::{
    Presentation, PresentationAction, PresentationDisclosure, PresentationProperty,
    PresentationRelationship, PresentationSubject, PresentationTemporalFact, PresentationText,
    TemporalReference,
};
use serde::{Deserialize, Serialize};

use crate::{EntranceAction, EntranceLayer, EntranceRefusal, PatchbayEntranceState};

pub const ENTRANCE_EQUIVALENCE_SCHEMA: &str = "conduit.patchbay.entrance-equivalence@2";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntranceEquivalenceReport {
    pub schema: String,
    pub presentation_id: String,
    pub presentation_revision: u64,
    pub subjects: Vec<PresentationSubject>,
    pub relationships: Vec<PresentationRelationship>,
    pub properties: Vec<PresentationProperty>,
    pub text: Vec<PresentationText>,
    pub semantic_actions: Vec<PresentationAction>,
    pub disclosures: Vec<PresentationDisclosure>,
    pub temporal_references: Vec<TemporalReference>,
    pub temporal_facts: Vec<PresentationTemporalFact>,
    pub selected_subject: Option<String>,
    pub actions: Vec<EntranceAction>,
    pub layer: EntranceLayer,
    pub refusal: Option<EntranceRefusal>,
    pub equivalent: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntranceEquivalenceError {
    InvalidPresentation,
    StaleEntrance,
    SemanticDrift,
}

pub fn compare_entrances(
    presentation: &Presentation,
    native: &PatchbayEntranceState,
    browser: &PatchbayEntranceState,
) -> Result<EntranceEquivalenceReport, EntranceEquivalenceError> {
    presentation
        .validate()
        .map_err(|_| EntranceEquivalenceError::InvalidPresentation)?;
    let current = |state: &PatchbayEntranceState| {
        state.body_id == presentation.basis.body_id
            && state.presentation_id == presentation.identity.as_str()
            && state.presentation_revision == presentation.revision
    };
    if !current(native) || !current(browser) {
        return Err(EntranceEquivalenceError::StaleEntrance);
    }
    if native.selected_subject != browser.selected_subject
        || native.available_actions != browser.available_actions
        || native.layer != browser.layer
        || native.last_refusal != browser.last_refusal
    {
        return Err(EntranceEquivalenceError::SemanticDrift);
    }
    let mut subjects = presentation.subjects.clone();
    subjects.sort_by(|left, right| left.identity.cmp(&right.identity));
    let mut relationships = presentation.relationships.clone();
    relationships.sort_by(|left, right| {
        (&left.source, &left.target, left.kind as u8).cmp(&(
            &right.source,
            &right.target,
            right.kind as u8,
        ))
    });
    let mut properties = presentation.properties.clone();
    properties
        .sort_by(|left, right| (&left.subject, &left.name).cmp(&(&right.subject, &right.name)));
    let mut content_text = presentation.text.clone();
    content_text
        .sort_by(|left, right| (&left.subject, &left.text).cmp(&(&right.subject, &right.text)));
    let mut semantic_actions = presentation.actions.clone();
    semantic_actions.sort_by(|left, right| left.identity.cmp(&right.identity));
    let mut disclosures = presentation.disclosures.clone();
    disclosures.sort_by(|left, right| left.subject.cmp(&right.subject));
    let mut temporal_references = presentation.temporal_references.clone();
    temporal_references.sort_by(|left, right| left.identity.cmp(&right.identity));
    let mut temporal_facts = presentation.temporal_facts.clone();
    temporal_facts.sort_by(|left, right| {
        (&left.subject, left.role as u8, &left.reference).cmp(&(
            &right.subject,
            right.role as u8,
            &right.reference,
        ))
    });
    Ok(EntranceEquivalenceReport {
        schema: ENTRANCE_EQUIVALENCE_SCHEMA.into(),
        presentation_id: presentation.identity.as_str().into(),
        presentation_revision: presentation.revision,
        subjects,
        relationships,
        properties,
        text: content_text,
        semantic_actions,
        disclosures,
        temporal_references,
        temporal_facts,
        selected_subject: native.selected_subject.clone(),
        actions: native.available_actions.clone(),
        layer: native.layer,
        refusal: native.last_refusal.clone(),
        equivalent: true,
    })
}
