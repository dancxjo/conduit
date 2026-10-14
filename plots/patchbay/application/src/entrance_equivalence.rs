//! Renderer-neutral equivalence oracle for Patchbay entrance manifestations.

use alloc::{string::String, vec::Vec};
use conduit_face::{
    Face, FaceAction, FaceDisclosure, FaceProperty, FaceRelationship, FaceSubject,
    FaceTemporalFact, FaceText, TemporalReference,
};
use serde::{Deserialize, Serialize};

use crate::{EntranceAction, EntranceLayer, EntranceRefusal, PatchbayEntranceState};

pub const ENTRANCE_EQUIVALENCE_SCHEMA: &str = "conduit.patchbay.entrance-equivalence@2";
pub const ONE_PLOT_TWO_FACES_HOST_ID: &str = "journey/one-plot-two-fronts/host";
pub const ONE_PLOT_TWO_FACES_BOOT_ID: &str = "journey/one-plot-two-fronts/boot-1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntranceEquivalenceReport {
    pub schema: String,
    pub face_id: String,
    pub face_revision: u64,
    pub subjects: Vec<FaceSubject>,
    pub relationships: Vec<FaceRelationship>,
    pub properties: Vec<FaceProperty>,
    pub text: Vec<FaceText>,
    pub semantic_actions: Vec<FaceAction>,
    pub disclosures: Vec<FaceDisclosure>,
    pub temporal_references: Vec<TemporalReference>,
    pub temporal_facts: Vec<FaceTemporalFact>,
    pub selected_subject: Option<String>,
    pub actions: Vec<EntranceAction>,
    pub layer: EntranceLayer,
    pub refusal: Option<EntranceRefusal>,
    pub equivalent: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntranceEquivalenceError {
    InvalidFace,
    StaleEntrance,
    SemanticDrift,
}

pub fn compare_entrances(
    face: &Face,
    native: &PatchbayEntranceState,
    browser: &PatchbayEntranceState,
) -> Result<EntranceEquivalenceReport, EntranceEquivalenceError> {
    face.validate()
        .map_err(|_| EntranceEquivalenceError::InvalidFace)?;
    let current = |state: &PatchbayEntranceState| {
        state.body_id == face.basis.body_id
            && state.face_id == face.identity.as_str()
            && state.face_revision == face.revision
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
    let mut subjects = face.subjects.clone();
    subjects.sort_by(|left, right| left.identity.cmp(&right.identity));
    let mut relationships = face.relationships.clone();
    relationships.sort_by(|left, right| {
        (&left.source, &left.target, &left.kind).cmp(&(&right.source, &right.target, &right.kind))
    });
    let mut properties = face.properties.clone();
    properties
        .sort_by(|left, right| (&left.subject, &left.name).cmp(&(&right.subject, &right.name)));
    let mut content_text = face.text.clone();
    content_text
        .sort_by(|left, right| (&left.subject, &left.text).cmp(&(&right.subject, &right.text)));
    let mut semantic_actions = face.actions.clone();
    semantic_actions.sort_by(|left, right| left.identity.cmp(&right.identity));
    let mut disclosures = face.disclosures.clone();
    disclosures.sort_by(|left, right| left.subject.cmp(&right.subject));
    let mut temporal_references = face.temporal_references.clone();
    temporal_references.sort_by(|left, right| left.identity.cmp(&right.identity));
    let mut temporal_facts = face.temporal_facts.clone();
    temporal_facts.sort_by(|left, right| {
        (&left.subject, left.role as u8, &left.reference).cmp(&(
            &right.subject,
            right.role as u8,
            &right.reference,
        ))
    });
    Ok(EntranceEquivalenceReport {
        schema: ENTRANCE_EQUIVALENCE_SCHEMA.into(),
        face_id: face.identity.as_str().into(),
        face_revision: face.revision,
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
