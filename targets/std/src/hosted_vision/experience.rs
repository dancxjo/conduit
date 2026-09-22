//! Deterministic bounded relation of one image generation's visual observations.

use super::{FiniteHostedVisionBase, HostedVisionRefusal};
use conduit_core::StructuredInfoValue;
use conduit_human::{
    VisualExperience, VisualExperienceLimits, VisualExperienceObservation,
    VisualExperienceRelation, VisualExperienceRelationKind,
};

const INPUT_COUNT: usize = 5;

pub(super) struct VisualExperienceState {
    inputs: [Vec<u8>; INPUT_COUNT],
    present: u8,
    output: Vec<u8>,
}

impl VisualExperienceState {
    pub(super) fn new() -> Self {
        Self {
            inputs: std::array::from_fn(|_| {
                Vec::with_capacity(conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES)
            }),
            present: 0,
            output: Vec::with_capacity(conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES),
        }
    }

    fn clear_inputs(&mut self) {
        self.present = 0;
        for input in &mut self.inputs {
            input.clear();
        }
    }
}

impl FiniteHostedVisionBase {
    pub(crate) fn execute_experience(
        &mut self,
        input: &[u8],
    ) -> Result<Option<&[u8]>, HostedVisionRefusal> {
        let value = StructuredInfoValue::from_canonical_bytes(input)
            .map_err(|_| HostedVisionRefusal::InvalidOutput)?;
        let slot = if value.value_type() == &conduit_semantic_catalog::vision_objects_type() {
            0
        } else if value.value_type() == &conduit_semantic_catalog::visual_impression_type() {
            1
        } else if value.value_type() == &conduit_semantic_catalog::vision_motions_type() {
            2
        } else if value.value_type() == &conduit_semantic_catalog::vision_texts_type() {
            3
        } else if value.value_type() == &conduit_semantic_catalog::vision_tracks_type() {
            4
        } else {
            return Err(HostedVisionRefusal::InvalidOutput);
        };
        let state = &mut self.experience;
        let mask = 1u8 << slot;
        if state.present & mask != 0 || input.len() > state.inputs[slot].capacity() {
            state.clear_inputs();
            return Err(HostedVisionRefusal::InvalidOutput);
        }
        state.inputs[slot].extend_from_slice(input);
        state.present |= mask;
        if state.present != 0b1_1111 {
            return Ok(None);
        }
        match self.finish_experience() {
            Ok(()) => Ok(Some(self.experience.output.as_slice())),
            Err(error) => {
                self.experience.clear_inputs();
                Err(error)
            }
        }
    }

    fn finish_experience(&mut self) -> Result<(), HostedVisionRefusal> {
        let state = &mut self.experience;
        let values = state
            .inputs
            .iter()
            .map(|bytes| {
                StructuredInfoValue::from_canonical_bytes(bytes)
                    .map_err(|_| HostedVisionRefusal::InvalidOutput)
            })
            .collect::<Result<Vec<_>, _>>()?;
        let impression =
            conduit_semantic_catalog::visual_impression_from_value_with_embedded_profile(
                &values[1],
            )
            .map_err(|_| HostedVisionRefusal::InvalidOutput)?;
        let profile = impression.source_image.content.content_profile.clone();
        let objects =
            conduit_semantic_catalog::object_observations_from_value(&values[0], &profile)
                .map_err(|_| HostedVisionRefusal::InvalidOutput)?;
        let motions =
            conduit_semantic_catalog::motion_observations_from_value(&values[2], &profile)
                .map_err(|_| HostedVisionRefusal::InvalidOutput)?;
        let texts =
            conduit_semantic_catalog::visible_text_observations_from_value(&values[3], &profile)
                .map_err(|_| HostedVisionRefusal::InvalidOutput)?;
        let tracks = conduit_semantic_catalog::track_observations_from_value(&values[4], &profile)
            .map_err(|_| HostedVisionRefusal::InvalidOutput)?;
        let mut experience = VisualExperience::new(
            impression.source_image.clone(),
            profile,
            VisualExperienceLimits {
                maximum_observations: 24,
                maximum_objects: 4,
                maximum_visible_texts: 8,
                maximum_motions: 4,
                maximum_tracks: 4,
                maximum_impressions: 1,
                maximum_relations: 24,
                maximum_text_bytes: 8 * 1024,
            },
        )
        .map_err(|_| HostedVisionRefusal::InvalidOutput)?;
        for value in objects {
            experience
                .try_admit(VisualExperienceObservation::Object(Box::new(value)))
                .map_err(|_| HostedVisionRefusal::InvalidOutput)?;
        }
        for value in texts {
            experience
                .try_admit(VisualExperienceObservation::VisibleText(Box::new(value)))
                .map_err(|_| HostedVisionRefusal::InvalidOutput)?;
        }
        for value in motions {
            experience
                .try_admit(VisualExperienceObservation::Motion(Box::new(value)))
                .map_err(|_| HostedVisionRefusal::InvalidOutput)?;
        }
        for value in tracks {
            experience
                .try_admit(VisualExperienceObservation::Track(Box::new(value)))
                .map_err(|_| HostedVisionRefusal::InvalidOutput)?;
        }
        let selected = impression.selected_observation_sign_ids.clone();
        let impression_sign = impression.provenance.observation_sign_id.clone();
        experience
            .try_admit(VisualExperienceObservation::Impression(Box::new(
                impression,
            )))
            .map_err(|_| HostedVisionRefusal::InvalidOutput)?;
        for sign in selected {
            experience
                .relate(VisualExperienceRelation {
                    subject_sign_id: sign,
                    object_sign_id: impression_sign.clone(),
                    kind: VisualExperienceRelationKind::Supports,
                })
                .map_err(|_| HostedVisionRefusal::InvalidOutput)?;
        }
        let encoded = conduit_semantic_catalog::visual_experience_value(&experience)
            .and_then(|value| value.canonical_bytes().map_err(Into::into))
            .map_err(|_| HostedVisionRefusal::InvalidOutput)?;
        if encoded.len() > state.output.capacity() {
            return Err(HostedVisionRefusal::InvalidOutput);
        }
        state.output.clear();
        state.output.extend_from_slice(&encoded);
        state.clear_inputs();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hosted_vision::HostedVisionFrame;
    use conduit_core::{ArtifactId, BaseImplementationId, BaseInstanceId, SignId};
    use conduit_human::{
        VisualEvidenceClass, VisualImpression, VisualImpressionDisposition,
        VisualObservationProvenance,
    };

    #[test]
    fn five_typed_inputs_become_one_checked_experience() {
        let fixture = conduit_semantic_catalog::deterministic_vision_fixture().unwrap();
        let image = fixture.image.canonical_bytes().unwrap();
        let (_, resource, width, height) =
            crate::hosted_vision::decode_image_resource_for_test(&image);
        let profile = resource.content_profile.clone();
        let source_value = conduit_semantic_catalog::image_observation_value(
            &conduit_human::ImageObservationReference::new(
                resource.clone(),
                width,
                height,
                &profile,
            )
            .unwrap(),
        )
        .unwrap();
        let source = conduit_semantic_catalog::image_observation_from_value(&source_value).unwrap();
        let impression = VisualImpression {
            source_image: source,
            selected_observation_sign_ids: vec![],
            text: "A finite scene.".into(),
            model_id: "fixture-model".into(),
            prompt_contract_revision: "prompt/vision@1".into(),
            disposition: VisualImpressionDisposition::Complete,
            provenance: VisualObservationProvenance {
                evidence_class: VisualEvidenceClass::ModelDerived,
                observation_sign_id: SignId::from("sign/impression"),
                observed_at: conduit_core::TemporalInstant {
                    ticks: 41,
                    scale: conduit_core::TemporalScale::Microseconds,
                    clock_basis: "boot/clock".into(),
                    resolution_ticks: 1,
                    uncertainty_ticks: 0,
                },
                implementation_id: BaseImplementationId::from("fixture/vision@1"),
                provider_instance_id: BaseInstanceId::from("fixture/provider"),
                artifact_id: ArtifactId::from("fixture/artifact"),
                run_id: "play/1".into(),
            },
        };
        let inputs = [
            conduit_semantic_catalog::object_observations_value(&[], &profile).unwrap(),
            conduit_semantic_catalog::visual_impression_value(&impression, &profile).unwrap(),
            conduit_semantic_catalog::motion_observations_value(&[], &profile).unwrap(),
            conduit_semantic_catalog::visible_text_observations_value(&[], &profile).unwrap(),
            conduit_semantic_catalog::track_observations_value(&[], &profile).unwrap(),
        ];
        let mut vision = FiniteHostedVisionBase::new(
            vec![HostedVisionFrame {
                canonical_image: image,
                resource,
                width,
                height,
                grayscale_pixels: vec![0; usize::from(width) * usize::from(height)],
            }],
            width,
            height,
            4,
            "finite-image-residence/experience-proof",
        )
        .unwrap();
        for value in &inputs[..4] {
            assert!(vision
                .execute_experience(&value.canonical_bytes().unwrap())
                .unwrap()
                .is_none());
        }
        let output = vision
            .execute_experience(&inputs[4].canonical_bytes().unwrap())
            .unwrap()
            .unwrap();
        let value = StructuredInfoValue::from_canonical_bytes(output).unwrap();
        let experience = conduit_semantic_catalog::visual_experience_from_value(
            &value,
            profile,
            VisualExperienceLimits {
                maximum_observations: 24,
                maximum_objects: 4,
                maximum_visible_texts: 8,
                maximum_motions: 4,
                maximum_tracks: 4,
                maximum_impressions: 1,
                maximum_relations: 24,
                maximum_text_bytes: 8 * 1024,
            },
        )
        .unwrap();
        assert_eq!(experience.observations().len(), 1);
        assert!(matches!(
            experience.observations()[0],
            VisualExperienceObservation::Impression(_)
        ));
    }
}
