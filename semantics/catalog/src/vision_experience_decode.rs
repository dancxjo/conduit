//! Checked reconstruction of domain-owned visual semantics from exact port values.

use alloc::{boxed::Box, string::String, vec::Vec};
use conduit_core::{
    ArtifactId, BaseImplementationId, BaseInstanceId, KindId, SignId, StructuredFieldValue,
    StructuredInfoValue, StructuredInfoValueShape, TemporalInstant, TemporalScale,
};
use conduit_human::{
    ImageRegion, MotionObservation, ObjectObservation, TrackObservation, VisibleTextObservation,
    VisualEvidenceClass, VisualExperience, VisualExperienceLimits, VisualExperienceObservation,
    VisualExperienceRelation, VisualExperienceRelationKind, VisualImpression,
    VisualImpressionDisposition, VisualObservationProvenance,
};

use crate::{
    image_observation_from_value, vision_texts_type, vision_tracks_type, visual_experience_type,
    visual_impression_type, VisualValueRefusal,
};

pub fn visible_text_observations_from_value(
    value: &StructuredInfoValue,
    image_profile: &KindId,
) -> Result<Vec<VisibleTextObservation>, VisualValueRefusal> {
    require_type(value, &vision_texts_type())?;
    collection(value)?
        .iter()
        .map(|value| {
            let observation = visible_text_from_value(value)?;
            observation
                .validate(image_profile)
                .map_err(|_| VisualValueRefusal::InvalidObservation)?;
            Ok(observation)
        })
        .collect()
}

pub fn track_observations_from_value(
    value: &StructuredInfoValue,
    image_profile: &KindId,
) -> Result<Vec<TrackObservation>, VisualValueRefusal> {
    require_type(value, &vision_tracks_type())?;
    collection(value)?
        .iter()
        .map(|value| {
            let observation = track_from_value(value)?;
            observation
                .validate(image_profile)
                .map_err(|_| VisualValueRefusal::InvalidObservation)?;
            Ok(observation)
        })
        .collect()
}

pub fn object_observations_from_value(
    value: &StructuredInfoValue,
    image_profile: &KindId,
) -> Result<Vec<ObjectObservation>, VisualValueRefusal> {
    require_type(value, &crate::vision_objects_type())?;
    collection(value)?
        .iter()
        .map(|value| {
            let observation = object_from_value(value)?;
            observation
                .validate(image_profile)
                .map_err(|_| VisualValueRefusal::InvalidObservation)?;
            Ok(observation)
        })
        .collect()
}

pub fn visual_impression_from_value(
    value: &StructuredInfoValue,
    image_profile: &KindId,
) -> Result<VisualImpression, VisualValueRefusal> {
    require_type(value, &visual_impression_type())?;
    let impression = visual_impression_from_value_unchecked(value)?;
    impression
        .validate(image_profile)
        .map_err(|_| VisualValueRefusal::InvalidImpression)?;
    Ok(impression)
}

pub fn visual_experience_from_value(
    value: &StructuredInfoValue,
    image_profile: KindId,
    limits: VisualExperienceLimits,
) -> Result<VisualExperience, VisualValueRefusal> {
    require_type(value, &visual_experience_type())?;
    let source_image = image(field(value, "source_image")?)?;
    let mut experience = VisualExperience::new(source_image, image_profile, limits)
        .map_err(|_| VisualValueRefusal::InvalidObservation)?;
    for value in collection(field(value, "observations")?)? {
        let StructuredInfoValueShape::Variant { tag, payload } = value.shape() else {
            return Err(VisualValueRefusal::InvalidObservation);
        };
        let observation = match tag {
            "object" => VisualExperienceObservation::Object(Box::new(object_from_value(payload)?)),
            "visible_text" => VisualExperienceObservation::VisibleText(Box::new(
                visible_text_from_value(payload)?,
            )),
            "motion" => VisualExperienceObservation::Motion(Box::new(motion_from_value(payload)?)),
            "track" => VisualExperienceObservation::Track(Box::new(track_from_value(payload)?)),
            "impression" => VisualExperienceObservation::Impression(Box::new(
                visual_impression_from_value_unchecked(payload)?,
            )),
            _ => return Err(VisualValueRefusal::InvalidObservation),
        };
        experience
            .try_admit(observation)
            .map_err(|_| VisualValueRefusal::InvalidObservation)?;
    }
    for value in collection(field(value, "relations")?)? {
        experience
            .relate(VisualExperienceRelation {
                subject_sign_id: SignId::from(text(field(value, "subject_sign")?)?),
                object_sign_id: SignId::from(text(field(value, "object_sign")?)?),
                kind: match text(field(value, "kind")?)? {
                    "relates" => VisualExperienceRelationKind::Relates,
                    "supports" => VisualExperienceRelationKind::Supports,
                    "contradicts" => VisualExperienceRelationKind::Contradicts,
                    _ => return Err(VisualValueRefusal::InvalidObservation),
                },
            })
            .map_err(|_| VisualValueRefusal::InvalidObservation)?;
    }
    Ok(experience)
}

fn object_from_value(value: &StructuredInfoValue) -> Result<ObjectObservation, VisualValueRefusal> {
    Ok(ObjectObservation {
        source_image: image(field(value, "source_image")?)?,
        candidate_label: text(field(value, "candidate_label")?)?.into(),
        region: region(field(value, "region")?)?,
        confidence_permille: bounded_u16(field(value, "confidence_permille")?)?,
        provenance: provenance(field(value, "provenance")?)?,
    })
}

fn visible_text_from_value(
    value: &StructuredInfoValue,
) -> Result<VisibleTextObservation, VisualValueRefusal> {
    Ok(VisibleTextObservation {
        source_image: image(field(value, "source_image")?)?,
        text: text(field(value, "text")?)?.into(),
        region: optional_region(field(value, "region")?)?,
        confidence_permille: bounded_u16(field(value, "confidence_permille")?)?,
        provenance: provenance(field(value, "provenance")?)?,
    })
}

fn motion_from_value(value: &StructuredInfoValue) -> Result<MotionObservation, VisualValueRefusal> {
    Ok(MotionObservation {
        source_image: image(field(value, "source_image")?)?,
        changed_region: region(field(value, "changed_region")?)?,
        change_permille: bounded_u16(field(value, "change_permille")?)?,
        provenance: provenance(field(value, "provenance")?)?,
    })
}

fn track_from_value(value: &StructuredInfoValue) -> Result<TrackObservation, VisualValueRefusal> {
    Ok(TrackObservation {
        source_image: image(field(value, "source_image")?)?,
        tracking_context_id: text(field(value, "tracking_context")?)?.into(),
        track_id: text(field(value, "track")?)?.into(),
        contributing_observation_sign_ids: text_values(field(
            value,
            "contributing_observation_signs",
        )?)?
        .into_iter()
        .map(SignId::from)
        .collect(),
        current_region: region(field(value, "current_region")?)?,
        continuity_confidence_permille: bounded_u16(field(
            value,
            "continuity_confidence_permille",
        )?)?,
        provenance: provenance(field(value, "provenance")?)?,
    })
}

fn visual_impression_from_value_unchecked(
    value: &StructuredInfoValue,
) -> Result<VisualImpression, VisualValueRefusal> {
    Ok(VisualImpression {
        source_image: image(field(value, "source_image")?)?,
        selected_observation_sign_ids: text_values(field(value, "selected_observation_signs")?)?
            .into_iter()
            .map(SignId::from)
            .collect(),
        text: text(field(value, "text")?)?.into(),
        model_id: text(field(value, "model")?)?.into(),
        prompt_contract_revision: text(field(value, "prompt_contract_revision")?)?.into(),
        disposition: disposition(field(value, "disposition")?)?,
        provenance: provenance(field(value, "provenance")?)?,
    })
}

fn provenance(
    value: &StructuredInfoValue,
) -> Result<VisualObservationProvenance, VisualValueRefusal> {
    let evidence_class = match variant_tag(field(value, "evidence_class")?)? {
        "deterministic_derived" => VisualEvidenceClass::DeterministicDerived,
        "model_derived" => VisualEvidenceClass::ModelDerived,
        "statistical_candidate" => VisualEvidenceClass::StatisticalCandidate,
        _ => return Err(VisualValueRefusal::InvalidObservation),
    };
    let observed = field(value, "observed_at")?;
    let provenance = VisualObservationProvenance {
        evidence_class,
        observation_sign_id: SignId::from(text(field(value, "observation_sign")?)?),
        observed_at: TemporalInstant {
            ticks: count(field(observed, "ticks")?)?,
            scale: match text(field(observed, "scale")?)? {
                "seconds" => TemporalScale::Seconds,
                "milliseconds" => TemporalScale::Milliseconds,
                "microseconds" => TemporalScale::Microseconds,
                "nanoseconds" => TemporalScale::Nanoseconds,
                _ => return Err(VisualValueRefusal::InvalidObservation),
            },
            clock_basis: text(field(observed, "clock_basis")?)?.into(),
            resolution_ticks: count(field(observed, "resolution_ticks")?)?,
            uncertainty_ticks: count(field(observed, "uncertainty_ticks")?)?,
        },
        implementation_id: BaseImplementationId::from(text(field(value, "implementation")?)?),
        provider_instance_id: BaseInstanceId::from(text(field(value, "provider_instance")?)?),
        artifact_id: ArtifactId::from(text(field(value, "artifact")?)?),
        run_id: text(field(value, "run")?)?.into(),
    };
    provenance
        .validate()
        .map_err(|_| VisualValueRefusal::InvalidObservation)?;
    Ok(provenance)
}

fn disposition(
    value: &StructuredInfoValue,
) -> Result<VisualImpressionDisposition, VisualValueRefusal> {
    let StructuredInfoValueShape::Variant { tag, payload } = value.shape() else {
        return Err(VisualValueRefusal::InvalidImpression);
    };
    match tag {
        "complete" => Ok(VisualImpressionDisposition::Complete),
        "truncated" => Ok(VisualImpressionDisposition::Truncated {
            original_bytes: count(payload)?
                .try_into()
                .map_err(|_| VisualValueRefusal::InvalidImpression)?,
        }),
        _ => Err(VisualValueRefusal::InvalidImpression),
    }
}

fn optional_region(value: &StructuredInfoValue) -> Result<Option<ImageRegion>, VisualValueRefusal> {
    let StructuredInfoValueShape::Variant { tag, payload } = value.shape() else {
        return Err(VisualValueRefusal::InvalidObservation);
    };
    match tag {
        "absent" => Ok(None),
        "present" => Ok(Some(region(payload)?)),
        _ => Err(VisualValueRefusal::InvalidObservation),
    }
}

fn region(value: &StructuredInfoValue) -> Result<ImageRegion, VisualValueRefusal> {
    Ok(ImageRegion {
        x: bounded_u16(field(value, "x")?)?,
        y: bounded_u16(field(value, "y")?)?,
        width: bounded_u16(field(value, "width")?)?,
        height: bounded_u16(field(value, "height")?)?,
    })
}

fn image(
    value: &StructuredInfoValue,
) -> Result<conduit_human::ImageObservationReference, VisualValueRefusal> {
    image_observation_from_value(value).map_err(VisualValueRefusal::Image)
}

fn require_type(
    value: &StructuredInfoValue,
    expected: &conduit_core::StructuredInfoType,
) -> Result<(), VisualValueRefusal> {
    if value.value_type() == expected {
        Ok(())
    } else {
        Err(VisualValueRefusal::InvalidObservation)
    }
}

fn field<'a>(
    value: &'a StructuredInfoValue,
    name: &str,
) -> Result<&'a StructuredInfoValue, VisualValueRefusal> {
    let StructuredInfoValueShape::Record(fields) = value.shape() else {
        return Err(VisualValueRefusal::InvalidObservation);
    };
    fields
        .iter()
        .find(|field| field.name() == name)
        .map(StructuredFieldValue::value)
        .ok_or(VisualValueRefusal::InvalidObservation)
}

fn collection(value: &StructuredInfoValue) -> Result<&[StructuredInfoValue], VisualValueRefusal> {
    let StructuredInfoValueShape::Collection(values) = value.shape() else {
        return Err(VisualValueRefusal::InvalidObservation);
    };
    Ok(values)
}

fn text(value: &StructuredInfoValue) -> Result<&str, VisualValueRefusal> {
    let StructuredInfoValueShape::Leaf(bytes) = value.shape() else {
        return Err(VisualValueRefusal::InvalidObservation);
    };
    core::str::from_utf8(bytes).map_err(|_| VisualValueRefusal::InvalidObservation)
}

fn text_values(value: &StructuredInfoValue) -> Result<Vec<String>, VisualValueRefusal> {
    collection(value)?
        .iter()
        .map(|value| Ok(text(value)?.into()))
        .collect()
}

fn count(value: &StructuredInfoValue) -> Result<u64, VisualValueRefusal> {
    let StructuredInfoValueShape::Leaf(bytes) = value.shape() else {
        return Err(VisualValueRefusal::InvalidObservation);
    };
    conduit_core::decode_count(bytes).map_err(|_| VisualValueRefusal::InvalidObservation)
}

fn bounded_u16(value: &StructuredInfoValue) -> Result<u16, VisualValueRefusal> {
    count(value)?
        .try_into()
        .map_err(|_| VisualValueRefusal::InvalidObservation)
}

fn variant_tag(value: &StructuredInfoValue) -> Result<&str, VisualValueRefusal> {
    let StructuredInfoValueShape::Variant { tag, .. } = value.shape() else {
        return Err(VisualValueRefusal::InvalidObservation);
    };
    Ok(tag)
}
