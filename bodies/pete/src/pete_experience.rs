//! Typed preparation adapters. These functions do no scheduling or model work.

use conduit_core::SignId;
use conduit_human::{
    CurrentExperience, ExperienceAvailability, ExperienceCertainty, ExperienceDomain,
    ExperienceItem, ExperienceLimits, ExperienceOrigin, ExperienceRefusal, ExperienceRelation,
    ExperienceRelationKind, ExperienceSourceRef, ExperienceTemporalPolicy,
};
use serde::Serialize;

use crate::{
    pete_retained_experience_input, PeteSituation, PeteSituationFact, RetainedExperience,
    SituationFactState,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PeteExperienceRefusal {
    InvalidFact,
    IndeterminateTime,
    Encoding,
    ContentBound,
    Admission(ExperienceRefusal),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeteExperienceInputs {
    pub items: Vec<ExperienceItem>,
    pub relationships: Vec<ExperienceRelation>,
}

/// Assemble one Experience revision using the Situation's reference clock and
/// records already selected by the caller. Admission is all-or-nothing.
pub fn integrate_pete_experience(
    situation: &PeteSituation,
    policy: ExperienceTemporalPolicy,
    selected_memory: &[RetainedExperience],
) -> Result<CurrentExperience, PeteExperienceRefusal> {
    use conduit_semantic_catalog::{
        EXPERIENCE_EVIDENCE_MAX_BYTES, EXPERIENCE_EVIDENCE_MAX_CONTENT_BYTES,
        EXPERIENCE_EVIDENCE_MAX_IDENTITY_BYTES, EXPERIENCE_EVIDENCE_MAX_ITEMS,
        EXPERIENCE_EVIDENCE_MAX_RELATIONSHIPS, EXPERIENCE_EVIDENCE_MAX_SOURCES,
    };
    let inputs = pete_situation_experiences(situation, policy)?;
    let maximum_items = EXPERIENCE_EVIDENCE_MAX_ITEMS;
    if inputs.items.len().saturating_add(selected_memory.len()) > maximum_items {
        return Err(PeteExperienceRefusal::Admission(
            ExperienceRefusal::ItemCapacity,
        ));
    }
    let mut current = CurrentExperience::new(
        ExperienceLimits {
            maximum_items,
            maximum_current_items: maximum_items,
            maximum_recent_items: maximum_items,
            maximum_stale_items: maximum_items,
            maximum_historical_items: maximum_items,
            maximum_items_per_domain: maximum_items,
            maximum_model_derived_items: maximum_items,
            maximum_selected_memory_items: maximum_items,
            maximum_source_refs: EXPERIENCE_EVIDENCE_MAX_SOURCES,
            maximum_relationships: EXPERIENCE_EVIDENCE_MAX_RELATIONSHIPS,
            maximum_item_bytes: EXPERIENCE_EVIDENCE_MAX_CONTENT_BYTES,
            maximum_encoded_bytes: EXPERIENCE_EVIDENCE_MAX_BYTES,
            maximum_conflict_alternatives: 2,
            maximum_identity_bytes: EXPERIENCE_EVIDENCE_MAX_IDENTITY_BYTES,
        },
        situation.reference.clone(),
        policy,
    )
    .map_err(PeteExperienceRefusal::Admission)?;

    let memories = selected_memory.iter().map(pete_retained_experience_input);
    for item in inputs.items.into_iter().map(Ok).chain(memories) {
        current
            .try_admit(item?)
            .map_err(|error| PeteExperienceRefusal::Admission(error.refusal))?;
    }
    for relation in inputs.relationships {
        current
            .relate(relation)
            .map_err(PeteExperienceRefusal::Admission)?;
    }
    Ok(current)
}

/// Preserve each alternative as its own uncertain item with its own source.
/// Missing sources produce no fabricated observation. Unavailable sources are
/// represented by empty unavailable items, rather than their previous value.
pub fn pete_situation_experiences(
    situation: &PeteSituation,
    policy: ExperienceTemporalPolicy,
) -> Result<PeteExperienceInputs, PeteExperienceRefusal> {
    if situation.battery.value_kind != conduit_robotics::ROBOTICS_BATTERY_INFO_ID
        || situation.utterance.value_kind != crate::PETE_UTTERANCE_INFO_KIND
        || situation.body.value_kind != crate::PETE_BODY_STATE_INFO_KIND
        || situation.reference.validate().is_err()
        || situation.maximum_age_ticks == 0
    {
        return Err(PeteExperienceRefusal::InvalidFact);
    }
    let mut items = Vec::with_capacity(6);
    append_fact(
        &mut items,
        "battery",
        &situation.battery,
        situation,
        policy,
        (ExperienceDomain::BodyState, ExperienceOrigin::Observation),
    )?;
    append_fact(
        &mut items,
        "utterance",
        &situation.utterance,
        situation,
        policy,
        (
            ExperienceDomain::HumanUtterance,
            ExperienceOrigin::HumanReported,
        ),
    )?;
    append_fact(
        &mut items,
        "body",
        &situation.body,
        situation,
        policy,
        (ExperienceDomain::BodyState, ExperienceOrigin::Observation),
    )?;
    let mut relationships = Vec::with_capacity(3);
    for name in ["battery", "utterance", "body"] {
        let prefix = format!("pete/{name}/");
        let mut alternatives = items.iter().filter(|item| item.id.starts_with(&prefix));
        if let (Some(first), Some(second)) = (alternatives.next(), alternatives.next()) {
            relationships.push(ExperienceRelation {
                subject_id: first.id.clone(),
                object_id: second.id.clone(),
                kind: ExperienceRelationKind::Contradicts,
            });
        }
    }
    Ok(PeteExperienceInputs {
        items,
        relationships,
    })
}

fn append_fact<T: Serialize>(
    items: &mut Vec<ExperienceItem>,
    name: &str,
    fact: &PeteSituationFact<T>,
    situation: &PeteSituation,
    policy: ExperienceTemporalPolicy,
    classification: (ExperienceDomain, ExperienceOrigin),
) -> Result<(), PeteExperienceRefusal> {
    let (domain, origin) = classification;
    if (fact.state != SituationFactState::Unavailable && fact.unavailable_source.is_some())
        || (fact.state == SituationFactState::Contradictory
            && fact.provenance.len() == 2
            && fact.provenance[0].sign_id == fact.provenance[1].sign_id)
    {
        return Err(PeteExperienceRefusal::InvalidFact);
    }
    let values: Vec<&T> = match fact.state {
        SituationFactState::Missing
            if fact.value.is_none()
                && fact.provenance.is_empty()
                && fact.alternatives.is_empty() =>
        {
            return Ok(())
        }
        SituationFactState::Unavailable
            if fact.value.is_none()
                && fact.provenance.is_empty()
                && fact.alternatives.is_empty() =>
        {
            let source = fact
                .unavailable_source
                .as_ref()
                .filter(|s| !s.is_empty())
                .ok_or(PeteExperienceRefusal::InvalidFact)?;
            items.push(ExperienceItem {
                id: format!("pete/{name}/unavailable"),
                domain,
                content_kind: fact.value_kind.clone().into(),
                encoded_content: vec![],
                origin,
                temporal_role: conduit_human::ExperienceTemporalRole::Current,
                availability: ExperienceAvailability::SourceUnavailable,
                certainty: ExperienceCertainty::Uncertain,
                observed_at: None,
                recorded_at: None,
                sources: vec![ExperienceSourceRef::Source {
                    source_id: source.clone(),
                }],
            });
            return Ok(());
        }
        SituationFactState::Contradictory
            if fact.value.is_none()
                && fact.alternatives.len() == 2
                && fact.provenance.len() == 2 =>
        {
            fact.alternatives.iter().collect()
        }
        SituationFactState::Current
        | SituationFactState::Stale
        | SituationFactState::Indeterminate
            if fact.alternatives.is_empty() && fact.provenance.len() == 1 =>
        {
            vec![fact
                .value
                .as_ref()
                .ok_or(PeteExperienceRefusal::InvalidFact)?]
        }
        _ => return Err(PeteExperienceRefusal::InvalidFact),
    };
    for (value, source) in values.into_iter().zip(&fact.provenance) {
        if source.sign_id.is_empty()
            || fact.value_kind.is_empty()
            || source.observed_at.relation_to(&situation.reference).ok() != Some(source.relation)
        {
            return Err(PeteExperienceRefusal::InvalidFact);
        }
        let temporal_role = policy
            .classify(&source.observed_at, &situation.reference)
            .map_err(|_| PeteExperienceRefusal::IndeterminateTime)?;
        let encoded_content =
            serde_json::to_vec(value).map_err(|_| PeteExperienceRefusal::Encoding)?;
        if encoded_content.len() > conduit_human::MAXIMUM_BODY_SELF_STATE_BYTES {
            return Err(PeteExperienceRefusal::ContentBound);
        }
        items.push(ExperienceItem {
            id: format!("pete/{name}/{}", source.sign_id),
            domain,
            content_kind: fact.value_kind.clone().into(),
            encoded_content,
            origin,
            temporal_role,
            availability: ExperienceAvailability::Present,
            certainty: ExperienceCertainty::Uncertain,
            observed_at: Some(source.observed_at.clone()),
            recorded_at: None,
            sources: vec![ExperienceSourceRef::Sign(SignId::from(
                source.sign_id.clone(),
            ))],
        });
    }
    Ok(())
}

#[cfg(test)]
#[path = "pete_experience_tests.rs"]
mod tests;
