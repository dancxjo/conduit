//! Native Conduitese projection of provenance-bearing visual semantics.

use alloc::{string::String, vec::Vec};
use conduit_core::{KindId, StructuredInfoRefusal, StructuredInfoValue, TemporalScale};
use conduit_human::{
    ImageRegion, MotionObservation, ObjectObservation, TrackObservation, VisibleTextObservation,
    VisualEvidenceClass, VisualExperience as HumanVisualExperience,
    VisualExperienceObservation as HumanExperienceObservation, VisualExperienceRelationKind,
    VisualImpression as HumanVisualImpression,
    VisualImpressionDisposition as HumanImpressionDisposition, VisualObservationProvenance,
};
use conduit_plot::rust_binding::{BoundedSequence, NativeBindingRefusal, NativeRustBinding};
use conduit_presentation::{
    VisionMotionObservation, VisionMotionsFour, VisionObjectObservation, VisionObjectObservations,
    VisionObservationEvidenceClass, VisionObservationProvenance, VisionObservationSigns,
    VisionOptionalPixelRegion, VisionPixelRegion, VisionTextsEight, VisionTrackObservation,
    VisionTracksFour, VisionVisibleTextObservation, VisualExperience, VisualExperienceObservation,
    VisualExperienceObservations, VisualExperienceRelation, VisualExperienceRelations,
    VisualImpression, VisualImpressionDisposition, VisualSelectedObservationSigns,
};

use crate::ImageTextValueRefusal;

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

impl From<NativeBindingRefusal> for VisualValueRefusal {
    fn from(_: NativeBindingRefusal) -> Self {
        Self::InvalidObservation
    }
}

pub fn visible_text_observations_value(
    observations: &[VisibleTextObservation],
    image_profile: &KindId,
) -> Result<StructuredInfoValue, VisualValueRefusal> {
    for observation in observations {
        observation
            .validate(image_profile)
            .map_err(|_| VisualValueRefusal::InvalidObservation)?;
    }
    VisionTextsEight::new(bounded(observations.iter().map(visible_text_native))?)?
        .into_structured()
        .map_err(Into::into)
}

pub fn motion_observations_value(
    observations: &[MotionObservation],
    image_profile: &KindId,
) -> Result<StructuredInfoValue, VisualValueRefusal> {
    for observation in observations {
        observation
            .validate(image_profile)
            .map_err(|_| VisualValueRefusal::InvalidObservation)?;
    }
    VisionMotionsFour::new(bounded(observations.iter().map(motion_native))?)?
        .into_structured()
        .map_err(Into::into)
}

pub fn track_observations_value(
    observations: &[TrackObservation],
    image_profile: &KindId,
) -> Result<StructuredInfoValue, VisualValueRefusal> {
    for observation in observations {
        observation
            .validate(image_profile)
            .map_err(|_| VisualValueRefusal::InvalidObservation)?;
    }
    VisionTracksFour::new(bounded(observations.iter().map(track_native))?)?
        .into_structured()
        .map_err(Into::into)
}

pub fn object_observations_value(
    observations: &[ObjectObservation],
    image_profile: &KindId,
) -> Result<StructuredInfoValue, VisualValueRefusal> {
    for observation in observations {
        observation
            .validate(image_profile)
            .map_err(|_| VisualValueRefusal::InvalidObservation)?;
    }
    VisionObjectObservations::new(bounded(observations.iter().map(object_native))?)?
        .into_structured()
        .map_err(Into::into)
}

pub fn visual_impression_value(
    impression: &HumanVisualImpression,
    image_profile: &KindId,
) -> Result<StructuredInfoValue, VisualValueRefusal> {
    impression
        .validate(image_profile)
        .map_err(|_| VisualValueRefusal::InvalidImpression)?;
    impression_native(impression)?
        .into_structured()
        .map_err(Into::into)
}

pub fn visual_experience_value(
    experience: &HumanVisualExperience,
) -> Result<StructuredInfoValue, VisualValueRefusal> {
    let observations = bounded(
        experience
            .observations()
            .iter()
            .map(experience_observation_native),
    )?;
    let relations = bounded(experience.relations().iter().map(|relation| {
        VisualExperienceRelation::new(
            relation_kind(relation.kind).into(),
            relation.object_sign_id.as_str().into(),
            relation.subject_sign_id.as_str().into(),
        )
        .map_err(Into::into)
    }))?;
    VisualExperience::new(
        VisualExperienceObservations::new(observations)?,
        VisualExperienceRelations::new(relations)?,
        experience.source_image().clone(),
    )?
    .into_structured()
    .map_err(Into::into)
}

fn object_native(value: &ObjectObservation) -> Result<VisionObjectObservation, VisualValueRefusal> {
    VisionObjectObservation::new(
        value.candidate_label.clone(),
        u64::from(value.confidence_permille),
        provenance_native(&value.provenance)?,
        region_native(value.region)?,
        value.source_image.clone(),
    )
    .map_err(Into::into)
}

fn motion_native(value: &MotionObservation) -> Result<VisionMotionObservation, VisualValueRefusal> {
    VisionMotionObservation::new(
        u64::from(value.change_permille),
        region_native(value.changed_region)?,
        provenance_native(&value.provenance)?,
        value.source_image.clone(),
    )
    .map_err(Into::into)
}

fn visible_text_native(
    value: &VisibleTextObservation,
) -> Result<VisionVisibleTextObservation, VisualValueRefusal> {
    VisionVisibleTextObservation::new(
        u64::from(value.confidence_permille),
        provenance_native(&value.provenance)?,
        optional_region_native(value.region)?,
        value.source_image.clone(),
        value.text.clone(),
    )
    .map_err(Into::into)
}

fn track_native(value: &TrackObservation) -> Result<VisionTrackObservation, VisualValueRefusal> {
    let signs = bounded(
        value
            .contributing_observation_sign_ids
            .iter()
            .map(|id| Ok::<String, VisualValueRefusal>(id.as_str().into())),
    )?;
    VisionTrackObservation::new(
        u64::from(value.continuity_confidence_permille),
        VisionObservationSigns::new(signs)?,
        region_native(value.current_region)?,
        provenance_native(&value.provenance)?,
        value.source_image.clone(),
        value.track_id.clone(),
        value.tracking_context_id.clone(),
    )
    .map_err(Into::into)
}

fn impression_native(
    value: &HumanVisualImpression,
) -> Result<VisualImpression, VisualValueRefusal> {
    let disposition = match &value.disposition {
        HumanImpressionDisposition::Complete => VisualImpressionDisposition::complete(),
        HumanImpressionDisposition::Truncated(truncated) => {
            VisualImpressionDisposition::truncated(u64::from(*truncated.original_bytes()))?
        }
    };
    let signs = bounded(
        value
            .selected_observation_sign_ids
            .iter()
            .map(|id| Ok::<String, VisualValueRefusal>(id.as_str().into())),
    )?;
    VisualImpression::new(
        disposition,
        value.model_id.clone(),
        value.prompt_contract_revision.clone(),
        provenance_native(&value.provenance)?,
        VisualSelectedObservationSigns::new(signs)?,
        value.source_image.clone(),
        value.text.clone(),
    )
    .map_err(Into::into)
}

fn experience_observation_native(
    value: &HumanExperienceObservation,
) -> Result<VisualExperienceObservation, VisualValueRefusal> {
    match value {
        HumanExperienceObservation::Object(value) => VisualExperienceObservation::object(
            value.candidate_label.clone(),
            u64::from(value.confidence_permille),
            provenance_native(&value.provenance)?,
            region_native(value.region)?,
            value.source_image.clone(),
        ),
        HumanExperienceObservation::VisibleText(value) => {
            VisualExperienceObservation::visible_text(
                u64::from(value.confidence_permille),
                provenance_native(&value.provenance)?,
                optional_region_native(value.region)?,
                value.source_image.clone(),
                value.text.clone(),
            )
        }
        HumanExperienceObservation::Motion(value) => VisualExperienceObservation::motion(
            u64::from(value.change_permille),
            region_native(value.changed_region)?,
            provenance_native(&value.provenance)?,
            value.source_image.clone(),
        ),
        HumanExperienceObservation::Track(value) => {
            let signs = bounded(
                value
                    .contributing_observation_sign_ids
                    .iter()
                    .map(|id| Ok::<String, VisualValueRefusal>(id.as_str().into())),
            )?;
            VisualExperienceObservation::track(
                u64::from(value.continuity_confidence_permille),
                VisionObservationSigns::new(signs)?,
                region_native(value.current_region)?,
                provenance_native(&value.provenance)?,
                value.source_image.clone(),
                value.track_id.clone(),
                value.tracking_context_id.clone(),
            )
        }
        HumanExperienceObservation::Impression(value) => {
            let disposition = match &value.disposition {
                HumanImpressionDisposition::Complete => VisualImpressionDisposition::complete(),
                HumanImpressionDisposition::Truncated(truncated) => {
                    VisualImpressionDisposition::truncated(u64::from(*truncated.original_bytes()))?
                }
            };
            let signs = bounded(
                value
                    .selected_observation_sign_ids
                    .iter()
                    .map(|id| Ok::<String, VisualValueRefusal>(id.as_str().into())),
            )?;
            VisualExperienceObservation::impression(
                disposition,
                value.model_id.clone(),
                value.prompt_contract_revision.clone(),
                provenance_native(&value.provenance)?,
                VisualSelectedObservationSigns::new(signs)?,
                value.source_image.clone(),
                value.text.clone(),
            )
        }
    }
    .map_err(Into::into)
}

fn provenance_native(
    value: &VisualObservationProvenance,
) -> Result<VisionObservationProvenance, VisualValueRefusal> {
    value
        .validate()
        .map_err(|_| VisualValueRefusal::InvalidObservation)?;
    let evidence = match value.evidence_class {
        VisualEvidenceClass::DeterministicDerived => {
            VisionObservationEvidenceClass::deterministic_derived()
        }
        VisualEvidenceClass::ModelDerived => VisionObservationEvidenceClass::model_derived(),
        VisualEvidenceClass::StatisticalCandidate => {
            VisionObservationEvidenceClass::statistical_candidate()
        }
    };
    let scale = match value.observed_at.scale {
        TemporalScale::Seconds => conduit_time::NativeTemporalScale::seconds(),
        TemporalScale::Milliseconds => conduit_time::NativeTemporalScale::milliseconds(),
        TemporalScale::Microseconds => conduit_time::NativeTemporalScale::microseconds(),
        TemporalScale::Nanoseconds => conduit_time::NativeTemporalScale::nanoseconds(),
    };
    let observed_at = conduit_time::NativeTemporalInstant::new(
        value.observed_at.clock_basis.clone(),
        value.observed_at.resolution_ticks,
        scale,
        value.observed_at.ticks,
        value.observed_at.uncertainty_ticks,
    )?;
    VisionObservationProvenance::new(
        value.artifact_id.as_str().into(),
        evidence,
        value.implementation_id.as_str().into(),
        value.observation_sign_id.as_str().into(),
        observed_at,
        value.provider_instance_id.as_str().into(),
        value.run_id.clone(),
    )
    .map_err(Into::into)
}

fn region_native(value: ImageRegion) -> Result<VisionPixelRegion, VisualValueRefusal> {
    VisionPixelRegion::new(
        u64::from(*value.height()),
        u64::from(*value.width()),
        u64::from(*value.x()),
        u64::from(*value.y()),
    )
    .map_err(Into::into)
}

fn optional_region_native(
    value: Option<ImageRegion>,
) -> Result<VisionOptionalPixelRegion, VisualValueRefusal> {
    match value {
        Some(value) => {
            let value = region_native(value)?;
            VisionOptionalPixelRegion::present(
                *value.height(),
                *value.width(),
                *value.x(),
                *value.y(),
            )
            .map_err(Into::into)
        }
        None => Ok(VisionOptionalPixelRegion::absent()),
    }
}

fn bounded<T, const N: usize>(
    values: impl IntoIterator<Item = Result<T, VisualValueRefusal>>,
) -> Result<BoundedSequence<T, N>, VisualValueRefusal> {
    let values = values.into_iter().collect::<Result<Vec<_>, _>>()?;
    BoundedSequence::try_from_iter(values).map_err(|_| VisualValueRefusal::InvalidObservation)
}

const fn relation_kind(kind: VisualExperienceRelationKind) -> &'static str {
    match kind {
        VisualExperienceRelationKind::Relates => "relates",
        VisualExperienceRelationKind::Supports => "supports",
        VisualExperienceRelationKind::Contradicts => "contradicts",
    }
}
