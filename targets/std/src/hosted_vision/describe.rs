//! Bounded four-input model-derived visual impression realization.

use super::{FiniteHostedVisionBase, HostedVisionRefusal};
use conduit_core::{SignId, StructuredInfoValue, TemporalInstant, TemporalScale};
use conduit_human::{
    VisualEvidenceClass, VisualImpression, VisualImpressionDisposition,
    VisualObservationProvenance, MAXIMUM_IMPRESSION_OBSERVATION_REFS,
    MAXIMUM_VISUAL_IMPRESSION_BYTES,
};
use std::fmt::Write;

const INPUT_COUNT: usize = 4;
const MAXIMUM_CONTEXT_BYTES: usize = 16 * 1024;

pub(super) struct VisualDescriptionState {
    model: Box<dyn crate::hosted_local_model::HostedVisualModelAdapter>,
    inputs: [Vec<u8>; INPUT_COUNT],
    present: u8,
    graymap: Vec<u8>,
    context: String,
    output: Vec<u8>,
}

impl VisualDescriptionState {
    pub(super) fn new(model: Box<dyn crate::hosted_local_model::HostedVisualModelAdapter>) -> Self {
        Self {
            model,
            inputs: std::array::from_fn(|_| {
                Vec::with_capacity(conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES)
            }),
            present: 0,
            graymap: Vec::with_capacity(conduit_semantic_catalog::MAXIMUM_LOCAL_CV_PIXELS + 32),
            context: String::with_capacity(MAXIMUM_CONTEXT_BYTES),
            output: Vec::with_capacity(conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES),
        }
    }

    fn clear(&mut self) {
        self.present = 0;
        for input in &mut self.inputs {
            input.clear();
        }
        self.context.clear();
        self.output.clear();
    }
}

impl FiniteHostedVisionBase {
    pub(crate) fn execute_describe(
        &mut self,
        input: &[u8],
        run_id: &str,
        observed_at_micros: u64,
        clock_basis: &str,
    ) -> Result<Option<&[u8]>, HostedVisionRefusal> {
        let value = StructuredInfoValue::from_canonical_bytes(input)
            .map_err(|_| HostedVisionRefusal::InvalidOutput)?;
        let slot = if value.value_type() == &conduit_semantic_catalog::image_resource_type() {
            0
        } else if value.value_type() == &conduit_semantic_catalog::vision_objects_type() {
            1
        } else if value.value_type() == &conduit_semantic_catalog::vision_texts_type() {
            2
        } else if value.value_type() == &conduit_semantic_catalog::vision_tracks_type() {
            3
        } else {
            return Err(HostedVisionRefusal::InvalidOutput);
        };
        let state = self
            .describe
            .as_mut()
            .ok_or(HostedVisionRefusal::VisualModel)?;
        let mask = 1u8 << slot;
        if state.present & mask != 0 || input.len() > state.inputs[slot].capacity() {
            state.clear();
            return Err(HostedVisionRefusal::InvalidOutput);
        }
        state.inputs[slot].extend_from_slice(input);
        state.present |= mask;
        if state.present != 0b1111 {
            return Ok(None);
        }
        match self.finish_describe(run_id, observed_at_micros, clock_basis) {
            Ok(()) => Ok(self.describe.as_ref().map(|state| state.output.as_slice())),
            Err(error) => {
                if let Some(state) = self.describe.as_mut() {
                    state.clear();
                }
                Err(error)
            }
        }
    }

    fn finish_describe(
        &mut self,
        run_id: &str,
        observed_at_micros: u64,
        clock_basis: &str,
    ) -> Result<(), HostedVisionRefusal> {
        let state = self
            .describe
            .as_mut()
            .ok_or(HostedVisionRefusal::VisualModel)?;
        let image_value = StructuredInfoValue::from_canonical_bytes(&state.inputs[0])
            .map_err(|_| HostedVisionRefusal::InvalidOutput)?;
        let (frame_index, pixels) = self.provider.resolve_exact_canonical(&state.inputs[0])?;
        let source_value = StructuredInfoValue::from_canonical_bytes(
            self.observation_images
                .get(frame_index)
                .ok_or(HostedVisionRefusal::InvalidOutput)?,
        )
        .map_err(|_| HostedVisionRefusal::InvalidOutput)?;
        let source_image = conduit_semantic_catalog::image_observation_from_value(&source_value)
            .map_err(|_| HostedVisionRefusal::InvalidOutput)?;
        if image_value.value_type() != &conduit_semantic_catalog::image_resource_type() {
            return Err(HostedVisionRefusal::InvalidOutput);
        }
        let objects_value = StructuredInfoValue::from_canonical_bytes(&state.inputs[1])
            .map_err(|_| HostedVisionRefusal::InvalidOutput)?;
        let texts_value = StructuredInfoValue::from_canonical_bytes(&state.inputs[2])
            .map_err(|_| HostedVisionRefusal::InvalidOutput)?;
        let tracks_value = StructuredInfoValue::from_canonical_bytes(&state.inputs[3])
            .map_err(|_| HostedVisionRefusal::InvalidOutput)?;
        let objects = conduit_semantic_catalog::object_observations_from_value(
            &objects_value,
            &source_image.content.content_profile,
        )
        .map_err(|_| HostedVisionRefusal::InvalidOutput)?;
        let texts = conduit_semantic_catalog::visible_text_observations_from_value(
            &texts_value,
            &source_image.content.content_profile,
        )
        .map_err(|_| HostedVisionRefusal::InvalidOutput)?;
        let tracks = conduit_semantic_catalog::track_observations_from_value(
            &tracks_value,
            &source_image.content.content_profile,
        )
        .map_err(|_| HostedVisionRefusal::InvalidOutput)?;
        if objects
            .iter()
            .any(|value| value.source_image != source_image)
            || texts.iter().any(|value| value.source_image != source_image)
            || tracks
                .iter()
                .any(|value| value.source_image != source_image)
        {
            return Err(HostedVisionRefusal::InvalidOutput);
        }

        state.context.clear();
        let mut signs = Vec::with_capacity(MAXIMUM_IMPRESSION_OBSERVATION_REFS);
        for object in &objects {
            append_context(
                &mut state.context,
                format_args!(
                    "object candidate {:?} confidence {} region {},{},{},{}; ",
                    object.candidate_label,
                    object.confidence_permille,
                    object.region.x,
                    object.region.y,
                    object.region.width,
                    object.region.height,
                ),
            )?;
            push_sign(&mut signs, &object.provenance.observation_sign_id)?;
        }
        for text in &texts {
            append_context(
                &mut state.context,
                format_args!(
                    "visible text candidate {:?} confidence {}; ",
                    text.text, text.confidence_permille,
                ),
            )?;
            push_sign(&mut signs, &text.provenance.observation_sign_id)?;
        }
        for track in &tracks {
            append_context(
                &mut state.context,
                format_args!(
                    "track {:?} confidence {} region {},{},{},{}; ",
                    track.track_id,
                    track.continuity_confidence_permille,
                    track.current_region.x,
                    track.current_region.y,
                    track.current_region.width,
                    track.current_region.height,
                ),
            )?;
            push_sign(&mut signs, &track.provenance.observation_sign_id)?;
        }
        crate::encode_graymap(
            pixels,
            source_image.width,
            source_image.height,
            &mut state.graymap,
        )
        .map_err(|_| HostedVisionRefusal::InvalidOutput)?;
        let generated = state
            .model
            .describe(&state.graymap, &state.context)
            .map_err(|_| HostedVisionRefusal::VisualModel)?;
        let original_bytes = generated.text.len();
        let text_end = generated
            .text
            .floor_char_boundary(MAXIMUM_VISUAL_IMPRESSION_BYTES);
        let text = generated.text[..text_end].to_owned();
        let disposition = if text_end == original_bytes {
            VisualImpressionDisposition::Complete
        } else {
            VisualImpressionDisposition::Truncated {
                original_bytes: u32::try_from(original_bytes)
                    .map_err(|_| HostedVisionRefusal::InvalidOutput)?,
            }
        };
        let impression = VisualImpression {
            source_image: source_image.clone(),
            selected_observation_sign_ids: signs,
            text,
            model_id: generated.model_id,
            prompt_contract_revision: generated.prompt_contract_revision.to_owned(),
            disposition,
            provenance: VisualObservationProvenance {
                evidence_class: VisualEvidenceClass::ModelDerived,
                observation_sign_id: SignId::from(format!("{run_id}/impression")),
                observed_at: TemporalInstant {
                    ticks: observed_at_micros,
                    scale: TemporalScale::Microseconds,
                    clock_basis: clock_basis.to_owned(),
                    resolution_ticks: 1,
                    uncertainty_ticks: 0,
                },
                implementation_id: conduit_core::BaseImplementationId::from(
                    conduit_std_offers::LOCAL_VISION_DESCRIBE_IMPLEMENTATION,
                ),
                provider_instance_id: conduit_core::BaseInstanceId::from(
                    generated.provider_instance_id,
                ),
                artifact_id: conduit_core::ArtifactId::from(generated.artifact_id),
                run_id: run_id.to_owned(),
            },
        };
        let encoded = conduit_semantic_catalog::visual_impression_value(
            &impression,
            &source_image.content.content_profile,
        )
        .and_then(|value| value.canonical_bytes().map_err(Into::into))
        .map_err(|_| HostedVisionRefusal::InvalidOutput)?;
        if encoded.len() > state.output.capacity() {
            return Err(HostedVisionRefusal::InvalidOutput);
        }
        state.output.clear();
        state.output.extend_from_slice(&encoded);
        state.present = 0;
        for input in &mut state.inputs {
            input.clear();
        }
        Ok(())
    }
}

fn append_context(
    target: &mut String,
    args: std::fmt::Arguments<'_>,
) -> Result<(), HostedVisionRefusal> {
    struct BoundedWriter<'a>(&'a mut String);
    impl std::fmt::Write for BoundedWriter<'_> {
        fn write_str(&mut self, value: &str) -> std::fmt::Result {
            if self.0.len().saturating_add(value.len()) > MAXIMUM_CONTEXT_BYTES {
                return Err(std::fmt::Error);
            }
            self.0.push_str(value);
            Ok(())
        }
    }
    BoundedWriter(target)
        .write_fmt(args)
        .map_err(|_| HostedVisionRefusal::InvalidOutput)
}

fn push_sign(target: &mut Vec<SignId>, sign: &SignId) -> Result<(), HostedVisionRefusal> {
    if target.len() == target.capacity() || target.contains(sign) {
        return Err(HostedVisionRefusal::InvalidOutput);
    }
    target.push(sign.clone());
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hosted_local_model::{HostedVisualModelAdapter, VisualModelOutput};
    use crate::hosted_vision::HostedVisionFrame;

    struct Model;

    impl HostedVisualModelAdapter for Model {
        fn describe(&mut self, image: &[u8], context: &str) -> Result<VisualModelOutput, String> {
            assert!(image.starts_with(b"P5\n64 48\n255\n"));
            assert!(context.is_empty());
            Ok(VisualModelOutput {
                text: "A bounded grayscale scene.".into(),
                model_id: "fixture-vision-model".into(),
                provider_instance_id: "fixture-provider/1".into(),
                artifact_id: "fixture-artifact/sha256-1".into(),
                prompt_contract_revision: "conduit.prompt/visual-description@1",
                truncated: false,
                work_units: 7,
            })
        }
    }

    #[test]
    fn four_exact_inputs_emit_one_provenance_bearing_impression() {
        let fixture = conduit_semantic_catalog::deterministic_vision_fixture().unwrap();
        let image = fixture.image.canonical_bytes().unwrap();
        let (_, resource, width, height) =
            crate::hosted_vision::decode_image_resource_for_test(&image);
        let profile = resource.content_profile.clone();
        let objects = conduit_semantic_catalog::object_observations_value(&[], &profile)
            .unwrap()
            .canonical_bytes()
            .unwrap();
        let texts = conduit_semantic_catalog::visible_text_observations_value(&[], &profile)
            .unwrap()
            .canonical_bytes()
            .unwrap();
        let tracks = conduit_semantic_catalog::track_observations_value(&[], &profile)
            .unwrap()
            .canonical_bytes()
            .unwrap();
        let mut vision = FiniteHostedVisionBase::new(
            vec![HostedVisionFrame {
                canonical_image: image.clone(),
                resource,
                width,
                height,
                grayscale_pixels: vec![0; usize::from(width) * usize::from(height)],
            }],
            width,
            height,
            4,
            "finite-image-residence/describe-proof",
        )
        .unwrap()
        .with_visual_model(Model);
        assert!(vision.describe_offer().is_some());

        for input in [&image, &objects, &texts] {
            assert_eq!(
                vision
                    .execute_describe(input, "play/describe/request-1", 41, "boot/clock")
                    .unwrap(),
                None
            );
        }
        let encoded = vision
            .execute_describe(&tracks, "play/describe/request-1", 41, "boot/clock")
            .unwrap()
            .unwrap();
        let value = StructuredInfoValue::from_canonical_bytes(encoded).unwrap();
        let impression =
            conduit_semantic_catalog::visual_impression_from_value(&value, &profile).unwrap();
        assert_eq!(impression.text, "A bounded grayscale scene.");
        assert_eq!(impression.model_id, "fixture-vision-model");
        assert_eq!(
            impression.provenance.evidence_class,
            VisualEvidenceClass::ModelDerived
        );
        assert_eq!(
            impression.provenance.implementation_id.as_str(),
            conduit_std_offers::LOCAL_VISION_DESCRIBE_IMPLEMENTATION
        );
        assert_eq!(impression.provenance.run_id, "play/describe/request-1");
    }
}
