//! Exact Patchbay inspection of one bounded current-experience item.
//!
//! This is a read-only projection. It does not reinterpret encoded domain
//! content, resolve contradictions, manufacture presentation prose, or grant
//! action authority.

use conduit_human::{CurrentExperience, ExperienceItem, ExperienceRelation};
use conduit_presentation::{Presentation, PresentationError};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CurrentExperienceTrace {
    pub item: ExperienceItem,
    pub relationships: Vec<ExperienceRelation>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PresentedCurrentExperienceTrace {
    pub presentation_identity: String,
    pub presentation_revision: u64,
    pub subject_identity: String,
    pub experience: CurrentExperienceTrace,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CurrentExperienceInspectionError {
    EmptyItemIdentity,
    UnknownItem,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PresentedCurrentExperienceInspectionError {
    InvalidPresentation(PresentationError),
    Experience(CurrentExperienceInspectionError),
    UnknownPresentationSubject,
    MissingSourceSign,
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

/// Correlate a presented semantic subject to the exact experience item with
/// the same identity. Every retained Sign source must be present in the
/// immutable Presentation basis, so neither deterministic nor generative
/// manifestation can silently detach the statement from its evidence.
pub fn inspect_presented_current_experience(
    experience: &CurrentExperience,
    presentation: &Presentation,
    item_identity: &str,
) -> Result<PresentedCurrentExperienceTrace, PresentedCurrentExperienceInspectionError> {
    presentation
        .validate()
        .map_err(PresentedCurrentExperienceInspectionError::InvalidPresentation)?;
    if !presentation
        .subjects
        .iter()
        .any(|subject| subject.identity == item_identity)
    {
        return Err(PresentedCurrentExperienceInspectionError::UnknownPresentationSubject);
    }
    let trace = inspect_current_experience_item(experience, item_identity)
        .map_err(PresentedCurrentExperienceInspectionError::Experience)?;
    let every_sign_is_in_basis = trace.item.sources.iter().all(|source| match source {
        conduit_human::ExperienceSourceRef::Sign(sign_id) => {
            presentation.basis.sign_ids.contains(sign_id)
        }
        _ => true,
    });
    if !every_sign_is_in_basis {
        return Err(PresentedCurrentExperienceInspectionError::MissingSourceSign);
    }
    Ok(PresentedCurrentExperienceTrace {
        presentation_identity: presentation.identity.as_str().into(),
        presentation_revision: presentation.revision,
        subject_identity: item_identity.into(),
        experience: trace,
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
    use conduit_presentation::{
        BodySurface, BodySurfaceContext, BodySurfaceFocus, GenerativeNarratorRole,
        GenerativePresenterBounds, GenerativePresenterPolicy, GenerativePresenterRequest,
        PresentationBasis, PresentationDisclosure, PresentationDisclosureLevel, PresentationRole,
        PresentationSubject, PresentationText,
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
    fn exact_item_trace_retains_epistemic_frontts_sources_and_contradiction() {
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

    #[test]
    fn one_correlated_experience_subject_feeds_deterministic_and_generative_presenters() {
        let mut experience = experience();
        experience
            .try_admit(item("door-open", "sign/camera/7"))
            .unwrap();
        let presentation = Presentation::new_with_semantics(
            4,
            PresentationBasis {
                body_id: None,
                wake_id: None,
                source_document_id: None,
                checked_form_id: None,
                expanded_form_id: None,
                plan_id: None,
                active_play_id: None,
                sign_ids: vec![SignId::from("sign/camera/7")],
            },
            vec![PresentationSubject {
                identity: "door-open".into(),
                role: PresentationRole::Info,
                label: "Door state".into(),
                accessibility_name: "Door state".into(),
            }],
            vec![],
            vec![],
            vec![PresentationText {
                subject: "door-open".into(),
                text: "The door is open.".into(),
            }],
            vec![],
            vec![PresentationDisclosure {
                subject: "door-open".into(),
                level: PresentationDisclosureLevel::Primary,
            }],
        )
        .unwrap();
        let correlation =
            inspect_presented_current_experience(&experience, &presentation, "door-open").unwrap();
        let deterministic =
            conduit_presentation::render_linear_presentation(&presentation).unwrap();
        let surface = BodySurface {
            context: BodySurfaceContext::Overview,
            focus: BodySurfaceFocus::Body,
            presentation: presentation.clone(),
            application_actions: vec![],
            operator_actions: vec![],
        };
        let generative = GenerativePresenterRequest::from_body_surface(
            "request/experience/4".into(),
            GenerativePresenterPolicy {
                template_contract_revision: "experience-presenter/1".into(),
                narrator_role: GenerativeNarratorRole::TransientFirstPersonBodyNarrator,
                instructions: "Render only the correlated semantic Presentation.".into(),
            },
            &surface,
            None,
            GenerativePresenterBounds::reviewed_default(),
        )
        .unwrap();

        assert_eq!(
            correlation.presentation_identity,
            deterministic.presentation_id.as_str()
        );
        assert_eq!(
            correlation.presentation_identity,
            generative.semantic_data.source_presentation_identity
        );
        assert_eq!(correlation.subject_identity, "door-open");
        assert_eq!(
            correlation.experience.item.sources,
            vec![ExperienceSourceRef::Sign(SignId::from("sign/camera/7"))]
        );

        let ungrounded = Presentation::new_with_semantics(
            4,
            PresentationBasis {
                sign_ids: vec![],
                ..presentation.basis.clone()
            },
            presentation.subjects.clone(),
            presentation.relationships.clone(),
            presentation.properties.clone(),
            presentation.text.clone(),
            presentation.actions.clone(),
            presentation.disclosures.clone(),
        )
        .unwrap();
        assert_eq!(
            inspect_presented_current_experience(&experience, &ungrounded, "door-open"),
            Err(PresentedCurrentExperienceInspectionError::MissingSourceSign)
        );
    }
}
