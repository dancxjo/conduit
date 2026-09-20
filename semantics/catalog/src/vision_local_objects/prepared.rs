//! Pre-admitted canonical encoding for local Vision object observations.

use super::{
    local_vision_object_observations_type, validate_component, validate_identity,
    MAXIMUM_LOCAL_VISION_OBJECT_OBSERVATIONS,
};
use crate::{
    image_resource_type, ContinuousLocalVisionObservation, LocalVisionObservationRefusal,
    PixelRegion,
};
use alloc::{string::String, vec::Vec};
use conduit_core::{
    PreparedStructuredValueValidator, StructuredInfoRefusal, MAXIMUM_STRUCTURED_CANONICAL_BYTES,
};

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
        let image_type = image_resource_type();
        Ok(Self {
            image_validator: PreparedStructuredValueValidator::new(
                &image_type,
                MAXIMUM_STRUCTURED_CANONICAL_BYTES,
            )?,
            image_type_prefix: image_type.canonical_bytes()?,
            output_type_prefix: local_vision_object_observations_type().canonical_bytes()?,
            implementation_id,
            provider_instance_id,
            artifact_id,
            image_width,
            image_height,
            output: Vec::with_capacity(MAXIMUM_STRUCTURED_CANONICAL_BYTES),
        })
    }

    pub fn encode(
        &mut self,
        source_image: &[u8],
        observation: &ContinuousLocalVisionObservation,
        run_id: &str,
    ) -> Result<&[u8], LocalVisionObservationRefusal> {
        validate_identity(run_id)?;
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
        wire_record(&mut self.output, 7);
        count_field(&mut self.output, "emitted_count", u64::from(emitted_count));
        wire_text(&mut self.output, "items");
        wire_collection(&mut self.output, MAXIMUM_LOCAL_VISION_OBJECT_OBSERVATIONS);
        for index in 0..usize::from(emitted_count) {
            let region = observation.components[index]
                .ok_or(LocalVisionObservationRefusal::InvalidObservation)?;
            let area = observation.component_areas[index];
            validate_component(region, area, self.image_width, self.image_height)?;
            wire_variant(&mut self.output, "observation");
            wire_record(&mut self.output, 4);
            count_field(&mut self.output, "area_pixels", u64::from(area));
            text_field(&mut self.output, "classification", "bright-component");
            wire_text(&mut self.output, "confidence");
            wire_variant(&mut self.output, "not_estimated");
            wire_leaf(&mut self.output, &[]);
            wire_text(&mut self.output, "region");
            encode_region(&mut self.output, region);
        }
        for _ in emitted_count..MAXIMUM_LOCAL_VISION_OBJECT_OBSERVATIONS as u8 {
            wire_variant(&mut self.output, "unused");
            wire_leaf(&mut self.output, &[]);
        }
        count_field(
            &mut self.output,
            "observed_count",
            u64::from(observation.observed_component_count),
        );
        wire_text(&mut self.output, "provenance");
        encode_provenance(
            &mut self.output,
            &self.artifact_id,
            &self.implementation_id,
            &self.provider_instance_id,
            run_id,
        );
        count_field(&mut self.output, "sequence", observation.sequence);
        wire_text(&mut self.output, "source_image");
        self.output.extend_from_slice(source_node);
        wire_text(&mut self.output, "truncation");
        wire_variant(
            &mut self.output,
            if observation.components_truncated
                || observation.component_count > emitted_count
                || observation.observed_component_count > u32::from(emitted_count)
            {
                "truncated"
            } else {
                "complete"
            },
        );
        wire_leaf(&mut self.output, &[]);
        Ok(&self.output)
    }
}

fn preflight_capacity(
    output_type_bytes: usize,
    source_node_bytes: usize,
) -> Result<(), LocalVisionObservationRefusal> {
    const MAXIMUM_OBJECT_NODE_BYTES: usize = 8_192;
    let required = output_type_bytes
        .checked_add(source_node_bytes)
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

fn encode_provenance(
    output: &mut Vec<u8>,
    artifact: &str,
    implementation: &str,
    provider_instance: &str,
    run: &str,
) {
    wire_record(output, 5);
    text_field(output, "artifact", artifact);
    text_field(output, "evidence_class", "deterministic-derived");
    text_field(output, "implementation", implementation);
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
    count(output, value);
}

fn count(output: &mut Vec<u8>, value: u64) {
    let mut digits = [0_u8; 20];
    let mut cursor = digits.len();
    let mut remaining = value;
    loop {
        cursor -= 1;
        digits[cursor] = b'0' + (remaining % 10) as u8;
        remaining /= 10;
        if remaining == 0 {
            break;
        }
    }
    wire_leaf(output, &digits[cursor..]);
}
