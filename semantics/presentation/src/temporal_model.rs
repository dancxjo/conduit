//! Finite structured temporal Presentation for model consumers.

use alloc::string::String;
use alloc::vec::Vec;
use conduit_core::SignId;
use serde::{Deserialize, Serialize};

use crate::{
    format_relative_time, Presentation, PresentationError, PresentationTemporalRole,
    TemporalInstant, TemporalReference, TemporalRelation,
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
    pub role: PresentationTemporalRole,
    pub sign_id: Option<SignId>,
}

/// Project every validated temporal fact without acquiring a clock.
pub fn project_model_temporal_context(
    presentation: &Presentation,
) -> Result<Vec<ModelTemporalContextFact>, PresentationError> {
    presentation.validate()?;

    presentation
        .temporal_facts
        .iter()
        .map(|fact| {
            let reference = presentation
                .temporal_references
                .iter()
                .find(|candidate| candidate.identity == fact.reference)
                .ok_or(PresentationError::UnknownTemporalReference)?;
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
