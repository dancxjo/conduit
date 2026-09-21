//! Canonical StructuredInfo projection of provenance-bearing visual semantics.

use alloc::{vec, vec::Vec};
use conduit_core::{
    KindId, StructuredFieldValue, StructuredInfoRefusal, StructuredInfoType, StructuredInfoValue,
    TemporalScale,
};
use conduit_human::{
    ImageRegion, MotionObservation, ObjectObservation, TrackObservation, VisibleTextObservation,
    VisualEvidenceClass, VisualExperience, VisualExperienceObservation,
    VisualExperienceRelationKind, VisualImpression, VisualImpressionDisposition,
    VisualObservationProvenance,
};

use crate::{
    evidence_class_type, experience_observation_type, experience_relation_type,
    image_observation_value, impression_disposition_type, motion_observation_type,
    object_observation_type, observation_provenance_type, optional_pixel_region_type,
    pixel_region_type, temporal_instant_type, track_observation_type,
    visible_text_observation_type, vision_texts_type, vision_tracks_type, visual_experience_type,
    visual_impression_type, ImageTextValueRefusal,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VisualValueRefusal {
    InvalidObservation,
    InvalidImpression,
    Image(ImageTextValueRefusal),
    Structured(StructuredInfoRefusal),
}

impl From<StructuredInfoRefusal> for VisualValueRefusal {
    fn from(value: StructuredInfoRefusal) -> Self {
        Self::Structured(value)
    }
}

pub fn visible_text_observations_value(
    observations: &[VisibleTextObservation],
    image_profile: &KindId,
) -> Result<StructuredInfoValue, VisualValueRefusal> {
    if observations.len() > 8 {
        return Err(VisualValueRefusal::InvalidObservation);
    }
    let values = observations
        .iter()
        .map(|observation| {
            observation
                .validate(image_profile)
                .map_err(|_| VisualValueRefusal::InvalidObservation)?;
            visible_text_value(observation)
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(StructuredInfoValue::sequence(vision_texts_type(), values)?)
}

pub fn track_observations_value(
    observations: &[TrackObservation],
    image_profile: &KindId,
) -> Result<StructuredInfoValue, VisualValueRefusal> {
    if observations.len() > 4 {
        return Err(VisualValueRefusal::InvalidObservation);
    }
    let values = observations
        .iter()
        .map(|observation| {
            observation
                .validate(image_profile)
                .map_err(|_| VisualValueRefusal::InvalidObservation)?;
            track_value(observation)
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(StructuredInfoValue::sequence(vision_tracks_type(), values)?)
}

pub fn object_observations_value(
    observations: &[ObjectObservation],
    image_profile: &KindId,
) -> Result<StructuredInfoValue, VisualValueRefusal> {
    if observations.len() > 4 {
        return Err(VisualValueRefusal::InvalidObservation);
    }
    let values = observations
        .iter()
        .map(|observation| {
            observation
                .validate(image_profile)
                .map_err(|_| VisualValueRefusal::InvalidObservation)?;
            object_value(observation)
        })
        .collect::<Result<Vec<_>, _>>()?;
    Ok(StructuredInfoValue::sequence(
        crate::vision_objects_type(),
        values,
    )?)
}

pub fn visual_impression_value(
    impression: &VisualImpression,
    image_profile: &KindId,
) -> Result<StructuredInfoValue, VisualValueRefusal> {
    impression
        .validate(image_profile)
        .map_err(|_| VisualValueRefusal::InvalidImpression)?;
    visual_impression_value_unchecked(impression)
}

fn visual_impression_value_unchecked(
    impression: &VisualImpression,
) -> Result<StructuredInfoValue, VisualValueRefusal> {
    record_value(
        visual_impression_type(),
        vec![
            (
                "disposition",
                impression_disposition_value(impression.disposition)?,
            ),
            ("model", text_value(&impression.model_id)?),
            (
                "prompt_contract_revision",
                text_value(&impression.prompt_contract_revision)?,
            ),
            ("provenance", provenance_value(&impression.provenance)?),
            (
                "selected_observation_signs",
                text_sequence(
                    24,
                    impression
                        .selected_observation_sign_ids
                        .iter()
                        .map(|identity| identity.as_str()),
                )?,
            ),
            ("source_image", image_value(&impression.source_image)?),
            ("text", text_value(&impression.text)?),
        ],
    )
}

pub fn visual_experience_value(
    experience: &VisualExperience,
) -> Result<StructuredInfoValue, VisualValueRefusal> {
    let observations = experience
        .observations()
        .iter()
        .map(|observation| {
            let (tag, payload) = match observation {
                VisualExperienceObservation::Object(value) => ("object", object_value(value)?),
                VisualExperienceObservation::VisibleText(value) => {
                    ("visible_text", visible_text_value(value)?)
                }
                VisualExperienceObservation::Motion(value) => ("motion", motion_value(value)?),
                VisualExperienceObservation::Track(value) => ("track", track_value(value)?),
                VisualExperienceObservation::Impression(value) => {
                    ("impression", visual_impression_value_unchecked(value)?)
                }
            };
            Ok(StructuredInfoValue::variant(
                experience_observation_type(),
                tag,
                payload,
            )?)
        })
        .collect::<Result<Vec<_>, VisualValueRefusal>>()?;
    let relations = experience
        .relations()
        .iter()
        .map(|relation| {
            record_value(
                experience_relation_type(),
                vec![
                    ("kind", text_value(relation_kind(relation.kind))?),
                    ("object_sign", text_value(relation.object_sign_id.as_str())?),
                    (
                        "subject_sign",
                        text_value(relation.subject_sign_id.as_str())?,
                    ),
                ],
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    record_value(
        visual_experience_type(),
        vec![
            (
                "observations",
                StructuredInfoValue::sequence(
                    StructuredInfoType::sequence(experience_observation_type(), 24)?,
                    observations,
                )?,
            ),
            (
                "relations",
                StructuredInfoValue::sequence(
                    StructuredInfoType::sequence(experience_relation_type(), 32)?,
                    relations,
                )?,
            ),
            ("source_image", image_value(experience.source_image())?),
        ],
    )
}

fn object_value(
    observation: &ObjectObservation,
) -> Result<StructuredInfoValue, VisualValueRefusal> {
    record_value(
        object_observation_type(),
        vec![
            ("candidate_label", text_value(&observation.candidate_label)?),
            (
                "confidence_permille",
                count_value(observation.confidence_permille)?,
            ),
            ("provenance", provenance_value(&observation.provenance)?),
            ("region", region_value(observation.region)?),
            ("source_image", image_value(&observation.source_image)?),
        ],
    )
}

fn motion_value(
    observation: &MotionObservation,
) -> Result<StructuredInfoValue, VisualValueRefusal> {
    record_value(
        motion_observation_type(),
        vec![
            ("change_permille", count_value(observation.change_permille)?),
            ("changed_region", region_value(observation.changed_region)?),
            ("provenance", provenance_value(&observation.provenance)?),
            ("source_image", image_value(&observation.source_image)?),
        ],
    )
}

fn visible_text_value(
    observation: &VisibleTextObservation,
) -> Result<StructuredInfoValue, VisualValueRefusal> {
    record_value(
        visible_text_observation_type(),
        vec![
            (
                "confidence_permille",
                count_value(observation.confidence_permille)?,
            ),
            ("provenance", provenance_value(&observation.provenance)?),
            ("region", optional_region_value(observation.region)?),
            ("source_image", image_value(&observation.source_image)?),
            ("text", text_value(&observation.text)?),
        ],
    )
}

fn track_value(observation: &TrackObservation) -> Result<StructuredInfoValue, VisualValueRefusal> {
    record_value(
        track_observation_type(),
        vec![
            (
                "continuity_confidence_permille",
                count_value(observation.continuity_confidence_permille)?,
            ),
            (
                "contributing_observation_signs",
                text_sequence(
                    16,
                    observation
                        .contributing_observation_sign_ids
                        .iter()
                        .map(|identity| identity.as_str()),
                )?,
            ),
            ("current_region", region_value(observation.current_region)?),
            ("provenance", provenance_value(&observation.provenance)?),
            ("source_image", image_value(&observation.source_image)?),
            ("track", text_value(&observation.track_id)?),
            (
                "tracking_context",
                text_value(&observation.tracking_context_id)?,
            ),
        ],
    )
}

fn image_value(
    image: &conduit_human::ImageObservationReference,
) -> Result<StructuredInfoValue, VisualValueRefusal> {
    image_observation_value(image).map_err(VisualValueRefusal::Image)
}

fn provenance_value(
    provenance: &VisualObservationProvenance,
) -> Result<StructuredInfoValue, VisualValueRefusal> {
    provenance
        .validate()
        .map_err(|_| VisualValueRefusal::InvalidObservation)?;
    record_value(
        observation_provenance_type(),
        vec![
            ("artifact", text_value(provenance.artifact_id.as_str())?),
            (
                "evidence_class",
                evidence_class_value(provenance.evidence_class)?,
            ),
            (
                "implementation",
                text_value(provenance.implementation_id.as_str())?,
            ),
            (
                "observation_sign",
                text_value(provenance.observation_sign_id.as_str())?,
            ),
            ("observed_at", temporal_value(&provenance.observed_at)?),
            (
                "provider_instance",
                text_value(provenance.provider_instance_id.as_str())?,
            ),
            ("run", text_value(&provenance.run_id)?),
        ],
    )
}

fn temporal_value(
    instant: &conduit_core::TemporalInstant,
) -> Result<StructuredInfoValue, VisualValueRefusal> {
    instant
        .validate()
        .map_err(|_| VisualValueRefusal::InvalidObservation)?;
    record_value(
        temporal_instant_type(),
        vec![
            ("clock_basis", text_value(&instant.clock_basis)?),
            ("resolution_ticks", count_value(instant.resolution_ticks)?),
            (
                "scale",
                text_value(match instant.scale {
                    TemporalScale::Seconds => "seconds",
                    TemporalScale::Milliseconds => "milliseconds",
                    TemporalScale::Microseconds => "microseconds",
                    TemporalScale::Nanoseconds => "nanoseconds",
                })?,
            ),
            ("ticks", count_value(instant.ticks)?),
            ("uncertainty_ticks", count_value(instant.uncertainty_ticks)?),
        ],
    )
}

fn region_value(region: ImageRegion) -> Result<StructuredInfoValue, VisualValueRefusal> {
    record_value(
        pixel_region_type(),
        vec![
            ("height", count_value(region.height)?),
            ("width", count_value(region.width)?),
            ("x", count_value(region.x)?),
            ("y", count_value(region.y)?),
        ],
    )
}

fn optional_region_value(
    region: Option<ImageRegion>,
) -> Result<StructuredInfoValue, VisualValueRefusal> {
    let (tag, payload) = match region {
        Some(region) => ("present", region_value(region)?),
        None => (
            "absent",
            StructuredInfoValue::leaf(unit_type(), Vec::new())?,
        ),
    };
    Ok(StructuredInfoValue::variant(
        optional_pixel_region_type(),
        tag,
        payload,
    )?)
}

fn evidence_class_value(
    evidence: VisualEvidenceClass,
) -> Result<StructuredInfoValue, VisualValueRefusal> {
    let tag = match evidence {
        VisualEvidenceClass::DeterministicDerived => "deterministic_derived",
        VisualEvidenceClass::ModelDerived => "model_derived",
        VisualEvidenceClass::StatisticalCandidate => "statistical_candidate",
    };
    Ok(StructuredInfoValue::variant(
        evidence_class_type(),
        tag,
        StructuredInfoValue::leaf(unit_type(), Vec::new())?,
    )?)
}

fn impression_disposition_value(
    disposition: VisualImpressionDisposition,
) -> Result<StructuredInfoValue, VisualValueRefusal> {
    let (tag, payload) = match disposition {
        VisualImpressionDisposition::Complete => (
            "complete",
            StructuredInfoValue::leaf(unit_type(), Vec::new())?,
        ),
        VisualImpressionDisposition::Truncated { original_bytes } => {
            ("truncated", count_value(original_bytes)?)
        }
    };
    Ok(StructuredInfoValue::variant(
        impression_disposition_type(),
        tag,
        payload,
    )?)
}

fn text_sequence<'a>(
    capacity: u16,
    values: impl Iterator<Item = &'a str>,
) -> Result<StructuredInfoValue, VisualValueRefusal> {
    let value_type = StructuredInfoType::sequence(text_type(), capacity)?;
    let values = values.map(text_value).collect::<Result<Vec<_>, _>>()?;
    Ok(StructuredInfoValue::sequence(value_type, values)?)
}

const fn relation_kind(kind: VisualExperienceRelationKind) -> &'static str {
    match kind {
        VisualExperienceRelationKind::Relates => "relates",
        VisualExperienceRelationKind::Supports => "supports",
        VisualExperienceRelationKind::Contradicts => "contradicts",
    }
}

fn text_type() -> StructuredInfoType {
    StructuredInfoType::leaf(conduit_core::kind_id("value/text")).expect("reviewed text")
}

fn unit_type() -> StructuredInfoType {
    StructuredInfoType::leaf(conduit_core::kind_id("value/unit")).expect("reviewed unit")
}

fn text_value(value: &str) -> Result<StructuredInfoValue, VisualValueRefusal> {
    Ok(StructuredInfoValue::leaf(
        text_type(),
        value.as_bytes().to_vec(),
    )?)
}

fn count_value(value: impl Into<u64>) -> Result<StructuredInfoValue, VisualValueRefusal> {
    Ok(StructuredInfoValue::leaf(
        StructuredInfoType::leaf(conduit_core::kind_id("value/count"))?,
        conduit_core::encode_count(value.into()).to_vec(),
    )?)
}

fn record_value(
    value_type: StructuredInfoType,
    fields: Vec<(&str, StructuredInfoValue)>,
) -> Result<StructuredInfoValue, VisualValueRefusal> {
    Ok(StructuredInfoValue::record(
        value_type,
        fields
            .into_iter()
            .map(|(name, value)| StructuredFieldValue::new(name, value))
            .collect::<Result<Vec<_>, _>>()?,
    )?)
}
