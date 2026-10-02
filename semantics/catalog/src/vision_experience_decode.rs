//! Checked reconstruction of domain-owned visual semantics from native Types.

use alloc::{boxed::Box, vec::Vec};
use conduit_core::{
    ArtifactId, BaseImplementationId, BaseInstanceId, KindId, SignId, StructuredInfoValue,
    TemporalInstant, TemporalScale,
};
use conduit_human::{
    ImageRegion, MotionObservation, ObjectObservation, TrackObservation, VisibleTextObservation,
    VisualEvidenceClass, VisualExperience, VisualExperienceLimits, VisualExperienceObservation,
    VisualExperienceRelation, VisualExperienceRelationKind, VisualImpression,
    VisualImpressionDisposition, VisualObservationProvenance,
};
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_presentation::{
    VisionMotionObservation, VisionMotionsFour, VisionObjectObservation, VisionObjectObservations,
    VisionObservationEvidenceClass, VisionObservationProvenance, VisionOptionalPixelRegion,
    VisionPixelRegion, VisionTextsEight, VisionTrackObservation, VisionTracksFour,
    VisionVisibleTextObservation, VisualExperience as NativeVisualExperience,
    VisualExperienceObservation as NativeExperienceObservation,
    VisualImpression as NativeVisualImpression,
    VisualImpressionDisposition as NativeImpressionDisposition,
};

use crate::VisualValueRefusal;

pub fn visible_text_observations_from_value(
    value: &StructuredInfoValue,
    image_profile: &KindId,
) -> Result<Vec<VisibleTextObservation>, VisualValueRefusal> {
    let native = VisionTextsEight::from_structured(value.clone())?;
    validate_all(
        native.get().iter().map(visible_text_from_native),
        image_profile,
    )
}

pub fn motion_observations_from_value(
    value: &StructuredInfoValue,
    image_profile: &KindId,
) -> Result<Vec<MotionObservation>, VisualValueRefusal> {
    let native = VisionMotionsFour::from_structured(value.clone())?;
    validate_all(native.get().iter().map(motion_from_native), image_profile)
}

pub fn track_observations_from_value(
    value: &StructuredInfoValue,
    image_profile: &KindId,
) -> Result<Vec<TrackObservation>, VisualValueRefusal> {
    let native = VisionTracksFour::from_structured(value.clone())?;
    validate_all(native.get().iter().map(track_from_native), image_profile)
}

pub fn object_observations_from_value(
    value: &StructuredInfoValue,
    image_profile: &KindId,
) -> Result<Vec<ObjectObservation>, VisualValueRefusal> {
    let native = VisionObjectObservations::from_structured(value.clone())?;
    validate_all(native.get().iter().map(object_from_native), image_profile)
}

fn validate_all<T>(
    values: impl Iterator<Item = Result<T, VisualValueRefusal>>,
    image_profile: &KindId,
) -> Result<Vec<T>, VisualValueRefusal>
where
    T: ValidateVisual,
{
    values
        .map(|value| {
            let value = value?;
            value.validate_visual(image_profile)?;
            Ok(value)
        })
        .collect()
}

trait ValidateVisual {
    fn validate_visual(&self, image_profile: &KindId) -> Result<(), VisualValueRefusal>;
}

macro_rules! validate_visual {
    ($($kind:ty),+ $(,)?) => {$(
        impl ValidateVisual for $kind {
            fn validate_visual(&self, image_profile: &KindId) -> Result<(), VisualValueRefusal> {
                self.validate(image_profile).map_err(|_| VisualValueRefusal::InvalidObservation)
            }
        }
    )+};
}
validate_visual!(
    VisibleTextObservation,
    MotionObservation,
    TrackObservation,
    ObjectObservation
);

pub fn visual_impression_from_value(
    value: &StructuredInfoValue,
    image_profile: &KindId,
) -> Result<VisualImpression, VisualValueRefusal> {
    let native = NativeVisualImpression::from_structured(value.clone())?;
    let impression = impression_from_native(&native)?;
    impression
        .validate(image_profile)
        .map_err(|_| VisualValueRefusal::InvalidImpression)?;
    Ok(impression)
}

pub fn visual_impression_from_value_with_embedded_profile(
    value: &StructuredInfoValue,
) -> Result<VisualImpression, VisualValueRefusal> {
    let native = NativeVisualImpression::from_structured(value.clone())?;
    let impression = impression_from_native(&native)?;
    let profile = impression.source_image.content.content_profile.clone();
    impression
        .validate(&profile)
        .map_err(|_| VisualValueRefusal::InvalidImpression)?;
    Ok(impression)
}

pub fn visual_experience_from_value(
    value: &StructuredInfoValue,
    image_profile: KindId,
    limits: VisualExperienceLimits,
) -> Result<VisualExperience, VisualValueRefusal> {
    let native = NativeVisualExperience::from_structured(value.clone())?;
    let mut experience =
        VisualExperience::new(native.source_image().clone(), image_profile, limits)
            .map_err(|_| VisualValueRefusal::InvalidObservation)?;
    for value in native.observations().get().iter() {
        let observation = match value {
            NativeExperienceObservation::Object(value) => {
                VisualExperienceObservation::Object(Box::new(object_from_payload(value)?))
            }
            NativeExperienceObservation::VisibleText(value) => {
                VisualExperienceObservation::VisibleText(Box::new(visible_text_from_payload(
                    value,
                )?))
            }
            NativeExperienceObservation::Motion(value) => {
                VisualExperienceObservation::Motion(Box::new(motion_from_payload(value)?))
            }
            NativeExperienceObservation::Track(value) => {
                VisualExperienceObservation::Track(Box::new(track_from_payload(value)?))
            }
            NativeExperienceObservation::Impression(value) => {
                VisualExperienceObservation::Impression(Box::new(impression_from_payload(value)?))
            }
        };
        experience
            .try_admit(observation)
            .map_err(|_| VisualValueRefusal::InvalidObservation)?;
    }
    for value in native.relations().get().iter() {
        let kind = match value.kind().as_str() {
            "relates" => VisualExperienceRelationKind::Relates,
            "supports" => VisualExperienceRelationKind::Supports,
            "contradicts" => VisualExperienceRelationKind::Contradicts,
            _ => return Err(VisualValueRefusal::InvalidObservation),
        };
        experience
            .relate(VisualExperienceRelation {
                subject_sign_id: SignId::from(value.subject_sign().as_str()),
                object_sign_id: SignId::from(value.object_sign().as_str()),
                kind,
            })
            .map_err(|_| VisualValueRefusal::InvalidObservation)?;
    }
    Ok(experience)
}

fn object_from_native(
    value: &VisionObjectObservation,
) -> Result<ObjectObservation, VisualValueRefusal> {
    Ok(ObjectObservation {
        source_image: value.source_image().clone(),
        candidate_label: value.candidate_label().clone(),
        region: region(value.region())?,
        confidence_permille: bounded_u16(*value.confidence_permille())?,
        provenance: provenance(value.provenance())?,
    })
}

fn motion_from_native(
    value: &VisionMotionObservation,
) -> Result<MotionObservation, VisualValueRefusal> {
    Ok(MotionObservation {
        source_image: value.source_image().clone(),
        changed_region: region(value.changed_region())?,
        change_permille: bounded_u16(*value.change_permille())?,
        provenance: provenance(value.provenance())?,
    })
}

fn visible_text_from_native(
    value: &VisionVisibleTextObservation,
) -> Result<VisibleTextObservation, VisualValueRefusal> {
    Ok(VisibleTextObservation {
        source_image: value.source_image().clone(),
        text: value.text().clone(),
        region: optional_region(value.region())?,
        confidence_permille: bounded_u16(*value.confidence_permille())?,
        provenance: provenance(value.provenance())?,
    })
}

fn track_from_native(
    value: &VisionTrackObservation,
) -> Result<TrackObservation, VisualValueRefusal> {
    Ok(TrackObservation {
        source_image: value.source_image().clone(),
        tracking_context_id: value.tracking_context().clone(),
        track_id: value.track().clone(),
        contributing_observation_sign_ids: value
            .contributing_observation_signs()
            .get()
            .iter()
            .map(|value| SignId::from(value.as_str()))
            .collect(),
        current_region: region(value.current_region())?,
        continuity_confidence_permille: bounded_u16(*value.continuity_confidence_permille())?,
        provenance: provenance(value.provenance())?,
    })
}

fn impression_from_native(
    value: &NativeVisualImpression,
) -> Result<VisualImpression, VisualValueRefusal> {
    Ok(VisualImpression {
        source_image: value.source_image().clone(),
        selected_observation_sign_ids: value
            .selected_observation_signs()
            .get()
            .iter()
            .map(|value| SignId::from(value.as_str()))
            .collect(),
        text: value.text().clone(),
        model_id: value.model().clone(),
        prompt_contract_revision: value.prompt_contract_revision().clone(),
        disposition: disposition(value.disposition())?,
        provenance: provenance(value.provenance())?,
    })
}

fn provenance(
    value: &VisionObservationProvenance,
) -> Result<VisualObservationProvenance, VisualValueRefusal> {
    let evidence_class = match value.evidence_class() {
        VisionObservationEvidenceClass::DeterministicDerived => {
            VisualEvidenceClass::DeterministicDerived
        }
        VisionObservationEvidenceClass::ModelDerived => VisualEvidenceClass::ModelDerived,
        VisionObservationEvidenceClass::StatisticalCandidate => {
            VisualEvidenceClass::StatisticalCandidate
        }
    };
    let observed = value.observed_at();
    let scale = match observed.scale() {
        conduit_time::NativeTemporalScale::Seconds => TemporalScale::Seconds,
        conduit_time::NativeTemporalScale::Milliseconds => TemporalScale::Milliseconds,
        conduit_time::NativeTemporalScale::Microseconds => TemporalScale::Microseconds,
        conduit_time::NativeTemporalScale::Nanoseconds => TemporalScale::Nanoseconds,
    };
    let result = VisualObservationProvenance {
        evidence_class,
        observation_sign_id: SignId::from(value.observation_sign().as_str()),
        observed_at: TemporalInstant {
            ticks: *observed.ticks(),
            scale,
            clock_basis: observed.clock_basis().clone(),
            resolution_ticks: *observed.resolution_ticks(),
            uncertainty_ticks: *observed.uncertainty_ticks(),
        },
        implementation_id: BaseImplementationId::from(value.implementation().as_str()),
        provider_instance_id: BaseInstanceId::from(value.provider_instance().as_str()),
        artifact_id: ArtifactId::from(value.artifact().as_str()),
        run_id: value.run().clone(),
    };
    result
        .validate()
        .map_err(|_| VisualValueRefusal::InvalidObservation)?;
    Ok(result)
}

fn region(value: &VisionPixelRegion) -> Result<ImageRegion, VisualValueRefusal> {
    ImageRegion::from_xywh(
        bounded_u16(*value.x())?,
        bounded_u16(*value.y())?,
        bounded_u16(*value.width())?,
        bounded_u16(*value.height())?,
    )
    .map_err(|_| VisualValueRefusal::InvalidObservation)
}

fn optional_region(
    value: &VisionOptionalPixelRegion,
) -> Result<Option<ImageRegion>, VisualValueRefusal> {
    match value {
        VisionOptionalPixelRegion::Absent => Ok(None),
        VisionOptionalPixelRegion::Present(value) => Ok(Some(
            ImageRegion::from_xywh(
                bounded_u16(*value.x())?,
                bounded_u16(*value.y())?,
                bounded_u16(*value.width())?,
                bounded_u16(*value.height())?,
            )
            .map_err(|_| VisualValueRefusal::InvalidObservation)?,
        )),
    }
}

fn disposition(
    value: &NativeImpressionDisposition,
) -> Result<VisualImpressionDisposition, VisualValueRefusal> {
    match value {
        NativeImpressionDisposition::Complete => Ok(VisualImpressionDisposition::Complete),
        NativeImpressionDisposition::Truncated(value) => VisualImpressionDisposition::truncated(
            (*value)
                .try_into()
                .map_err(|_| VisualValueRefusal::InvalidImpression)?,
        )
        .map_err(|_| VisualValueRefusal::InvalidImpression),
    }
}

fn bounded_u16(value: u64) -> Result<u16, VisualValueRefusal> {
    value
        .try_into()
        .map_err(|_| VisualValueRefusal::InvalidObservation)
}

fn object_from_payload(
    value: &conduit_presentation::VisualExperienceObservationObject,
) -> Result<ObjectObservation, VisualValueRefusal> {
    Ok(ObjectObservation {
        source_image: value.source_image().clone(),
        candidate_label: value.candidate_label().clone(),
        region: region(value.region())?,
        confidence_permille: bounded_u16(*value.confidence_permille())?,
        provenance: provenance(value.provenance())?,
    })
}
fn motion_from_payload(
    value: &conduit_presentation::VisualExperienceObservationMotion,
) -> Result<MotionObservation, VisualValueRefusal> {
    Ok(MotionObservation {
        source_image: value.source_image().clone(),
        changed_region: region(value.changed_region())?,
        change_permille: bounded_u16(*value.change_permille())?,
        provenance: provenance(value.provenance())?,
    })
}
fn visible_text_from_payload(
    value: &conduit_presentation::VisualExperienceObservationVisibleText,
) -> Result<VisibleTextObservation, VisualValueRefusal> {
    Ok(VisibleTextObservation {
        source_image: value.source_image().clone(),
        text: value.text().clone(),
        region: optional_region(value.region())?,
        confidence_permille: bounded_u16(*value.confidence_permille())?,
        provenance: provenance(value.provenance())?,
    })
}
fn track_from_payload(
    value: &conduit_presentation::VisualExperienceObservationTrack,
) -> Result<TrackObservation, VisualValueRefusal> {
    Ok(TrackObservation {
        source_image: value.source_image().clone(),
        tracking_context_id: value.tracking_context().clone(),
        track_id: value.track().clone(),
        contributing_observation_sign_ids: value
            .contributing_observation_signs()
            .get()
            .iter()
            .map(|v| SignId::from(v.as_str()))
            .collect(),
        current_region: region(value.current_region())?,
        continuity_confidence_permille: bounded_u16(*value.continuity_confidence_permille())?,
        provenance: provenance(value.provenance())?,
    })
}
fn impression_from_payload(
    value: &conduit_presentation::VisualExperienceObservationImpression,
) -> Result<VisualImpression, VisualValueRefusal> {
    Ok(VisualImpression {
        source_image: value.source_image().clone(),
        selected_observation_sign_ids: value
            .selected_observation_signs()
            .get()
            .iter()
            .map(|v| SignId::from(v.as_str()))
            .collect(),
        text: value.text().clone(),
        model_id: value.model().clone(),
        prompt_contract_revision: value.prompt_contract_revision().clone(),
        disposition: disposition(value.disposition())?,
        provenance: provenance(value.provenance())?,
    })
}
