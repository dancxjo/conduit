//! Pre-admitted canonical encoding for provenance-bearing local Vision objects.

use super::{validate_component, validate_identity, MAXIMUM_LOCAL_VISION_OBJECT_OBSERVATIONS};
use crate::{
    image_observation_reference_type, vision_objects_type, ContinuousLocalVisionObservation,
    LocalVisionObservationRefusal, PixelRegion,
};
use alloc::{string::String, vec::Vec};
use conduit_core::{
    PreparedStructuredValueValidator, StructuredInfoRefusal, MAXIMUM_STRUCTURED_CANONICAL_BYTES,
};
use core::fmt::Write;

/// Owns all schema, identity, and output storage required before Play starts.
pub struct PreparedLocalVisionObjectEncoder {
    image_validator: PreparedStructuredValueValidator,
    image_type_prefix: Vec<u8>,
    output_type_prefix: Vec<u8>,
    implementation_id: String,
    provider_instance_id: String,
    artifact_id: String,
    image_width: u16,
    image_height: u16,
    observation_sign: String,
    output: Vec<u8>,
}

impl PreparedLocalVisionObjectEncoder {
    pub fn new(
        implementation_id: impl Into<String>,
        provider_instance_id: impl Into<String>,
        artifact_id: impl Into<String>,
        image_width: u16,
        image_height: u16,
    ) -> Result<Self, LocalVisionObservationRefusal> {
        let implementation_id = implementation_id.into();
        let provider_instance_id = provider_instance_id.into();
        let artifact_id = artifact_id.into();
        for identity in [&implementation_id, &provider_instance_id, &artifact_id] {
            validate_identity(identity)?;
        }
        if image_width == 0 || image_height == 0 {
            return Err(LocalVisionObservationRefusal::InvalidObservation);
        }
        let image_type = image_observation_reference_type();
        Ok(Self {
            image_validator: PreparedStructuredValueValidator::new(
                &image_type,
                MAXIMUM_STRUCTURED_CANONICAL_BYTES,
            )?,
            image_type_prefix: image_type.canonical_bytes()?,
            output_type_prefix: vision_objects_type().canonical_bytes()?,
            implementation_id,
            provider_instance_id,
            artifact_id,
            image_width,
            image_height,
            observation_sign: String::with_capacity(conduit_human::MAXIMUM_VISUAL_IDENTITY_BYTES),
            output: Vec::with_capacity(MAXIMUM_STRUCTURED_CANONICAL_BYTES),
        })
    }

    pub fn encode(
        &mut self,
        source_image: &[u8],
        observation: &ContinuousLocalVisionObservation,
        run_id: &str,
        observed_at_micros: u64,
        clock_basis: &str,
    ) -> Result<&[u8], LocalVisionObservationRefusal> {
        validate_identity(run_id)?;
        validate_identity(clock_basis)?;
        self.image_validator.validate(source_image)?;
        let source_node = source_image
            .strip_prefix(self.image_type_prefix.as_slice())
            .ok_or(LocalVisionObservationRefusal::MalformedImage)?;
        let emitted_count = observation
            .component_count
            .min(MAXIMUM_LOCAL_VISION_OBJECT_OBSERVATIONS as u8);
        preflight_capacity(self.output_type_prefix.len(), source_node.len())?;
        self.output.clear();
        self.output.extend_from_slice(&self.output_type_prefix);
        wire_collection(&mut self.output, u16::from(emitted_count));
        for index in 0..usize::from(emitted_count) {
            let region = observation.components[index]
                .ok_or(LocalVisionObservationRefusal::InvalidObservation)?;
            let area = observation.component_areas[index];
            validate_component(region, area, self.image_width, self.image_height)?;
            let suffix_bytes = "/observation-".len() + decimal_digits(index);
            if run_id.len().saturating_add(suffix_bytes) > self.observation_sign.capacity() {
                return Err(LocalVisionObservationRefusal::InvalidIdentity);
            }
            self.observation_sign.clear();
            write!(self.observation_sign, "{run_id}/observation-{index}")
                .map_err(|_| LocalVisionObservationRefusal::InvalidIdentity)?;
            validate_identity(&self.observation_sign)?;

            wire_record(&mut self.output, 5);
            text_field(&mut self.output, "candidate_label", "bright-component");
            count_field(&mut self.output, "confidence_permille", 1_000);
            wire_text(&mut self.output, "provenance");
            encode_provenance(
                &mut self.output,
                &self.artifact_id,
                &self.implementation_id,
                &self.observation_sign,
                observed_at_micros,
                clock_basis,
                &self.provider_instance_id,
                run_id,
            );
            wire_text(&mut self.output, "region");
            encode_region(&mut self.output, region);
            wire_text(&mut self.output, "source_image");
            self.output.extend_from_slice(source_node);
        }
        Ok(&self.output)
    }
}

fn decimal_digits(mut value: usize) -> usize {
    let mut digits = 1;
    while value >= 10 {
        value /= 10;
        digits += 1;
    }
    digits
}

fn preflight_capacity(
    output_type_bytes: usize,
    source_node_bytes: usize,
) -> Result<(), LocalVisionObservationRefusal> {
    const MAXIMUM_OBJECT_NODE_BYTES: usize = 12_288;
    let required = output_type_bytes
        .checked_add(
            source_node_bytes
                .checked_mul(usize::from(MAXIMUM_LOCAL_VISION_OBJECT_OBSERVATIONS))
                .ok_or(LocalVisionObservationRefusal::InvalidObservation)?,
        )
        .and_then(|length| length.checked_add(MAXIMUM_OBJECT_NODE_BYTES))
        .ok_or(LocalVisionObservationRefusal::Structured(
            StructuredInfoRefusal::CanonicalEncodingTooLarge,
        ))?;
    if required > MAXIMUM_STRUCTURED_CANONICAL_BYTES {
        return Err(LocalVisionObservationRefusal::Structured(
            StructuredInfoRefusal::CanonicalEncodingTooLarge,
        ));
    }
    Ok(())
}

fn encode_region(output: &mut Vec<u8>, region: PixelRegion) {
    wire_record(output, 4);
    count_field(output, "height", u64::from(region.height));
    count_field(output, "width", u64::from(region.width));
    count_field(output, "x", u64::from(region.x));
    count_field(output, "y", u64::from(region.y));
}

#[allow(clippy::too_many_arguments)]
fn encode_provenance(
    output: &mut Vec<u8>,
    artifact: &str,
    implementation: &str,
    observation_sign: &str,
    observed_at_micros: u64,
    clock_basis: &str,
    provider_instance: &str,
    run: &str,
) {
    wire_record(output, 7);
    text_field(output, "artifact", artifact);
    wire_text(output, "evidence_class");
    wire_variant(output, "deterministic_derived");
    wire_leaf(output, &[]);
    text_field(output, "implementation", implementation);
    text_field(output, "observation_sign", observation_sign);
    wire_text(output, "observed_at");
    wire_record(output, 5);
    text_field(output, "clock_basis", clock_basis);
    count_field(output, "resolution_ticks", 1);
    text_field(output, "scale", "microseconds");
    count_field(output, "ticks", observed_at_micros);
    count_field(output, "uncertainty_ticks", 0);
    text_field(output, "provider_instance", provider_instance);
    text_field(output, "run", run);
}

fn wire_collection(output: &mut Vec<u8>, length: u16) {
    output.push(1);
    output.extend_from_slice(&u32::from(length).to_le_bytes());
}

fn wire_record(output: &mut Vec<u8>, length: u32) {
    output.push(2);
    output.extend_from_slice(&length.to_le_bytes());
}

fn wire_variant(output: &mut Vec<u8>, tag: &str) {
    output.push(3);
    wire_text(output, tag);
}

fn wire_leaf(output: &mut Vec<u8>, value: &[u8]) {
    output.push(0);
    wire_bytes(output, value);
}

fn wire_text(output: &mut Vec<u8>, value: &str) {
    wire_bytes(output, value.as_bytes());
}

fn wire_bytes(output: &mut Vec<u8>, value: &[u8]) {
    output.extend_from_slice(&(value.len() as u32).to_le_bytes());
    output.extend_from_slice(value);
}

fn text_field(output: &mut Vec<u8>, name: &str, value: &str) {
    wire_text(output, name);
    wire_leaf(output, value.as_bytes());
}

fn count_field(output: &mut Vec<u8>, name: &str, value: u64) {
    wire_text(output, name);
    wire_leaf(output, &conduit_core::encode_count(value));
}
