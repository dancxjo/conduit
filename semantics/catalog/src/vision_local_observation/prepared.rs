//! Pre-admitted canonical encoding for local Vision motion observations.

use super::{
    local_vision_motion_observations_type, LocalVisionObservationRefusal,
    MAXIMUM_LOCAL_VISION_IDENTITY_BYTES, MAXIMUM_LOCAL_VISION_MOTION_OBSERVATIONS,
};
use crate::{image_resource_type, ContinuousLocalVisionObservation};
use alloc::{string::String, vec::Vec};
use conduit_core::{
    PreparedStructuredValueValidator, StructuredInfoRefusal, MAXIMUM_STRUCTURED_CANONICAL_BYTES,
};

/// Owns every schema, identity, and byte buffer needed by the Play-time encoder.
pub struct PreparedLocalVisionMotionEncoder {
    image_validator: PreparedStructuredValueValidator,
    image_type_prefix: Vec<u8>,
    output_type_prefix: Vec<u8>,
    implementation_id: String,
    provider_instance_id: String,
    artifact_id: String,
    output: Vec<u8>,
}

impl PreparedLocalVisionMotionEncoder {
    pub fn new(
        implementation_id: impl Into<String>,
        provider_instance_id: impl Into<String>,
        artifact_id: impl Into<String>,
    ) -> Result<Self, LocalVisionObservationRefusal> {
        let implementation_id = implementation_id.into();
        let provider_instance_id = provider_instance_id.into();
        let artifact_id = artifact_id.into();
        for identity in [&implementation_id, &provider_instance_id, &artifact_id] {
            validate_identity(identity)?;
        }
        let image_type = image_resource_type();
        Ok(Self {
            image_validator: PreparedStructuredValueValidator::new(
                &image_type,
                MAXIMUM_STRUCTURED_CANONICAL_BYTES,
            )?,
            image_type_prefix: image_type.canonical_bytes()?,
            output_type_prefix: local_vision_motion_observations_type().canonical_bytes()?,
            implementation_id,
            provider_instance_id,
            artifact_id,
            output: Vec::with_capacity(MAXIMUM_STRUCTURED_CANONICAL_BYTES),
        })
    }

    /// Rewrites retained storage without allocating after Play starts.
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
        self.output.clear();
        self.output.extend_from_slice(&self.output_type_prefix);
        wire_collection(&mut self.output, MAXIMUM_LOCAL_VISION_MOTION_OBSERVATIONS);
        wire_variant(&mut self.output, "observation");
        wire_record(&mut self.output, 4);
        wire_text(&mut self.output, "motion");
        encode_motion(&mut self.output, observation)?;
        wire_text(&mut self.output, "provenance");
        wire_record(&mut self.output, 5);
        text_field(&mut self.output, "artifact", &self.artifact_id);
        text_field(&mut self.output, "evidence_class", "deterministic-derived");
        text_field(&mut self.output, "implementation", &self.implementation_id);
        text_field(
            &mut self.output,
            "provider_instance",
            &self.provider_instance_id,
        );
        text_field(&mut self.output, "run", run_id);
        wire_text(&mut self.output, "sequence");
        count(&mut self.output, observation.sequence);
        wire_text(&mut self.output, "source_image");
        ensure_remaining_capacity(&self.output, source_node.len())?;
        self.output.extend_from_slice(source_node);
        for _ in 1..MAXIMUM_LOCAL_VISION_MOTION_OBSERVATIONS {
            wire_variant(&mut self.output, "unused");
            wire_leaf(&mut self.output, &[]);
        }
        Ok(&self.output)
    }
}

fn encode_motion(
    output: &mut Vec<u8>,
    observation: &ContinuousLocalVisionObservation,
) -> Result<(), LocalVisionObservationRefusal> {
    let Some(motion) = observation.motion else {
        wire_variant(output, "unchanged");
        wire_leaf(output, &[]);
        return Ok(());
    };
    if motion.region.width == 0 || motion.region.height == 0 || motion.changed_pixels == 0 {
        return Err(LocalVisionObservationRefusal::InvalidObservation);
    }
    wire_variant(output, "changed");
    wire_record(output, 5);
    count_field(output, "changed_pixels", u64::from(motion.changed_pixels));
    count_field(output, "height", u64::from(motion.region.height));
    count_field(output, "width", u64::from(motion.region.width));
    count_field(output, "x", u64::from(motion.region.x));
    count_field(output, "y", u64::from(motion.region.y));
    Ok(())
}

fn ensure_remaining_capacity(
    output: &[u8],
    source_node_bytes: usize,
) -> Result<(), LocalVisionObservationRefusal> {
    const UNUSED_SLOT_BYTES: usize = 16;
    let unused = usize::from(MAXIMUM_LOCAL_VISION_MOTION_OBSERVATIONS - 1) * UNUSED_SLOT_BYTES;
    let required = output
        .len()
        .checked_add(source_node_bytes)
        .and_then(|length| length.checked_add(unused))
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

fn validate_identity(identity: &str) -> Result<(), LocalVisionObservationRefusal> {
    if identity.is_empty() || identity.len() > MAXIMUM_LOCAL_VISION_IDENTITY_BYTES {
        Err(LocalVisionObservationRefusal::InvalidIdentity)
    } else {
        Ok(())
    }
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
    wire_leaf(output, &conduit_core::encode_count(value));
}
