//! Exact Patchbay inspection of one bounded current-experience item.
//!
//! This is a read-only projection. It does not reinterpret encoded domain
//! content, resolve contradictions, manufacture presentation prose, or grant
//! action authority.

use conduit_human::{CurrentExperience, ExperienceItem, ExperienceRelation};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CurrentExperienceTrace {
    pub item: ExperienceItem,
    pub relationships: Vec<ExperienceRelation>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CurrentExperienceInspectionError {
    EmptyItemIdentity,
    UnknownItem,
}

/// Trace one exact semantic experience item to its retained source references
/// and every retained relationship that names it.
pub fn inspect_current_experience_item(
    experience: &CurrentExperience,
    item_identity: &str,
) -> Result<CurrentExperienceTrace, CurrentExperienceInspectionError> {
    if item_identity.is_empty() {
        return Err(CurrentExperienceInspectionError::EmptyItemIdentity);
    }
    let item = experience
        .items()
        .iter()
        .find(|item| item.id == item_identity)
        .cloned()
        .ok_or(CurrentExperienceInspectionError::UnknownItem)?;
    let relationships = experience
        .relationships()
        .iter()
        .filter(|relation| {
            relation.subject_id == item_identity || relation.object_id == item_identity
        })
        .cloned()
        .collect();
    Ok(CurrentExperienceTrace {
        item,
        relationships,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::{kind_id, SignId, TemporalInstant, TemporalScale};
    use conduit_human::{
        ExperienceAvailability, ExperienceCertainty, ExperienceDomain, ExperienceLimits,
        ExperienceOrigin, ExperienceRelationKind, ExperienceSourceRef, ExperienceTemporalPolicy,
        ExperienceTemporalRole,
    };

    fn at(ticks: u64) -> TemporalInstant {
        TemporalInstant {
            ticks,
            scale: TemporalScale::Milliseconds,
            clock_basis: "clock/patchbay-experience".into(),
            resolution_ticks: 1,
            uncertainty_ticks: 0,
        }
    }

    fn experience() -> CurrentExperience {
        CurrentExperience::new(
            ExperienceLimits {
                maximum_items: 2,
                maximum_current_items: 2,
                maximum_recent_items: 2,
                maximum_stale_items: 2,
                maximum_historical_items: 2,
                maximum_items_per_domain: 2,
                maximum_model_derived_items: 2,
                maximum_selected_memory_items: 2,
                maximum_source_refs: 2,
                maximum_relationships: 2,
                maximum_item_bytes: 32,
                maximum_encoded_bytes: 64,
                maximum_conflict_alternatives: 2,
                maximum_identity_bytes: 64,
            },
            at(10),
            ExperienceTemporalPolicy {
                maximum_current_age_ticks: 2,
                maximum_recent_age_ticks: 5,
            },
        )
        .unwrap()
    }

    fn item(identity: &str, sign: &str) -> ExperienceItem {
        ExperienceItem {
            id: identity.into(),
            domain: ExperienceDomain::Visual,
            content_kind: kind_id("experience/door-state@1"),
            encoded_content: identity.as_bytes().to_vec(),
            origin: ExperienceOrigin::Observation,
            temporal_role: ExperienceTemporalRole::Current,
            availability: ExperienceAvailability::Present,
            certainty: ExperienceCertainty::Certain,
            observed_at: Some(at(10)),
            recorded_at: None,
            sources: vec![ExperienceSourceRef::Sign(SignId::from(sign))],
        }
    }

    #[test]
    fn exact_item_trace_retains_epistemic_facets_sources_and_contradiction() {
        let mut experience = experience();
        experience
            .try_admit(item("door-open", "sign/camera/7"))
            .unwrap();
        experience
            .try_admit(item("door-closed", "sign/human/2"))
            .unwrap();
        experience
            .relate(ExperienceRelation {
                subject_id: "door-open".into(),
                object_id: "door-closed".into(),
                kind: ExperienceRelationKind::Contradicts,
            })
            .unwrap();

        let trace = inspect_current_experience_item(&experience, "door-open").unwrap();
        assert_eq!(trace.item.origin, ExperienceOrigin::Observation);
        assert_eq!(trace.item.temporal_role, ExperienceTemporalRole::Current);
        assert_eq!(
            trace.item.sources,
            vec![ExperienceSourceRef::Sign(SignId::from("sign/camera/7"))]
        );
        assert_eq!(trace.relationships, experience.relationships());
        assert_eq!(
            inspect_current_experience_item(&experience, "invented"),
            Err(CurrentExperienceInspectionError::UnknownItem)
        );
    }
}
