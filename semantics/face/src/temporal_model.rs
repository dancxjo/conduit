//! Finite structured temporal Face for model consumers.

use alloc::string::String;
use alloc::vec::Vec;
use conduit_core::SignId;
use serde::{Deserialize, Serialize};

use crate::{
    format_relative_time, Face, FaceError, FaceTemporalRole, TemporalInstant, TemporalReference,
    TemporalRelation,
};

/// A transient model-facing projection, not canonical event evidence.
///
/// Relative time deliberately leads the serialized shape. Exact temporal truth
/// remains alongside it so a consumer can inspect the derivation without
/// reconstructing the decision-time clock.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelTemporalContextFact {
    pub relative_time: String,
    pub relation: TemporalRelation,
    pub source: TemporalInstant,
    pub reference: TemporalReference,
    pub subject: String,
    pub role: FaceTemporalRole,
    pub sign_id: Option<SignId>,
}

/// Project every validated temporal fact without acquiring a clock.
pub fn project_model_temporal_context(
    face: &Face,
) -> Result<Vec<ModelTemporalContextFact>, FaceError> {
    face.validate()?;

    face.temporal_facts
        .iter()
        .map(|fact| {
            let reference = face
                .temporal_references
                .iter()
                .find(|candidate| candidate.identity == fact.reference)
                .ok_or(FaceError::UnknownTemporalReference)?;
            Ok(ModelTemporalContextFact {
                relative_time: format_relative_time(fact),
                relation: fact.relation,
                source: fact.source.clone(),
                reference: reference.clone(),
                subject: fact.subject.clone(),
                role: fact.role,
                sign_id: fact.sign_id.clone(),
            })
        })
        .collect()
}
