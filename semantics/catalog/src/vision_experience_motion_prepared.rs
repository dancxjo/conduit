//! Pre-admitted canonical encoding for enriched hosted motion observations.

use crate::{
    image_observation_reference_type, vision_motions_type, LocalVisionObservationRefusal,
    MotionRegion, MAXIMUM_LOCAL_VISION_IDENTITY_BYTES,
};
use alloc::{string::String, vec::Vec};
use conduit_core::{PreparedStructuredValueValidator, MAXIMUM_STRUCTURED_CANONICAL_BYTES};
use core::fmt::Write;

pub struct PreparedVisualMotionEncoder {
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

impl PreparedVisualMotionEncoder {
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
        for value in [&implementation_id, &provider_instance_id, &artifact_id] {
            validate_identity(value)?;
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
            output_type_prefix: vision_motions_type().canonical_bytes()?,
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
        motion: Option<MotionRegion>,
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
        self.output.clear();
        self.output.extend_from_slice(&self.output_type_prefix);
        let Some(motion) = motion else {
            wire_collection(&mut self.output, 0);
            return Ok(&self.output);
        };
        let right = motion.region.x.checked_add(motion.region.width);
        let bottom = motion.region.y.checked_add(motion.region.height);
        if motion.region.width == 0
            || motion.region.height == 0
            || motion.changed_pixels == 0
            || right.is_none_or(|value| value > self.image_width)
            || bottom.is_none_or(|value| value > self.image_height)
        {
            return Err(LocalVisionObservationRefusal::InvalidObservation);
        }
        if run_id.len().saturating_add("/motion".len()) > self.observation_sign.capacity() {
            return Err(LocalVisionObservationRefusal::InvalidIdentity);
        }
        self.observation_sign.clear();
        write!(self.observation_sign, "{run_id}/motion")
            .map_err(|_| LocalVisionObservationRefusal::InvalidIdentity)?;
        let total_pixels = u64::from(self.image_width) * u64::from(self.image_height);
        let change_permille = u64::from(motion.changed_pixels)
            .saturating_mul(1_000)
            .checked_div(total_pixels)
            .ok_or(LocalVisionObservationRefusal::InvalidObservation)?;

        wire_collection(&mut self.output, 1);
        wire_record(&mut self.output, 4);
        count_field(&mut self.output, "change_permille", change_permille);
        wire_text(&mut self.output, "changed_region");
        wire_record(&mut self.output, 4);
        count_field(&mut self.output, "height", u64::from(motion.region.height));
        count_field(&mut self.output, "width", u64::from(motion.region.width));
        count_field(&mut self.output, "x", u64::from(motion.region.x));
        count_field(&mut self.output, "y", u64::from(motion.region.y));
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
        wire_text(&mut self.output, "source_image");
        self.output.extend_from_slice(source_node);
        Ok(&self.output)
    }
}

fn validate_identity(value: &str) -> Result<(), LocalVisionObservationRefusal> {
    if value.is_empty() || value.len() > MAXIMUM_LOCAL_VISION_IDENTITY_BYTES {
        Err(LocalVisionObservationRefusal::InvalidIdentity)
    } else {
        Ok(())
    }
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

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::{
        kind_id, ArtifactId, BaseImplementationId, BaseInstanceId, BoundedResourceRef,
        ResourceClassId, ResourceExtent, ResourceLifetime, ResourceSemanticIdentity,
        ResourceVersionIdentity, SignId, TemporalInstant, TemporalScale,
    };
    use conduit_human::{
        ImageRegion, MotionObservation, VisualEvidenceClass, VisualObservationProvenance,
    };

    #[test]
    fn prepared_encoding_matches_checked_rich_motion_value() {
        let resource = BoundedResourceRef {
            identity: ResourceSemanticIdentity::from_digest([7; 32]),
            content_profile: kind_id("media/image-rgb8@1"),
            access_class: ResourceClassId::from("conduit.resource/image-content@1"),
            extent: ResourceExtent {
                bytes: 64 * 48,
                items: Some(1),
            },
            lifetime: ResourceLifetime {
                version: ResourceVersionIdentity::from_digest([8; 32]),
                expires_at: None,
            },
        };
        let source = conduit_human::ImageObservationReference::new(
            resource.clone(),
            64,
            48,
            &resource.content_profile,
        )
        .unwrap();
        let source_bytes = crate::image_observation_value(&source)
            .unwrap()
            .canonical_bytes()
            .unwrap();
        let motion = MotionRegion {
            region: crate::PixelRegion {
                x: 1,
                y: 2,
                width: 3,
                height: 4,
            },
            changed_pixels: 12,
        };
        let expected = crate::motion_observations_value(
            &[MotionObservation {
                source_image: source,
                changed_region: ImageRegion {
                    x: 1,
                    y: 2,
                    width: 3,
                    height: 4,
                },
                change_permille: 3,
                provenance: VisualObservationProvenance {
                    evidence_class: VisualEvidenceClass::DeterministicDerived,
                    observation_sign_id: SignId::from("run/1/motion"),
                    observed_at: TemporalInstant {
                        ticks: 41,
                        scale: TemporalScale::Microseconds,
                        clock_basis: "boot/clock".into(),
                        resolution_ticks: 1,
                        uncertainty_ticks: 0,
                    },
                    implementation_id: BaseImplementationId::from("implementation/vision@1"),
                    provider_instance_id: BaseInstanceId::from("provider/vision"),
                    artifact_id: ArtifactId::from("artifact/vision@1"),
                    run_id: "run/1".into(),
                },
            }],
            &resource.content_profile,
        )
        .unwrap()
        .canonical_bytes()
        .unwrap();
        let mut encoder = PreparedVisualMotionEncoder::new(
            "implementation/vision@1",
            "provider/vision",
            "artifact/vision@1",
            64,
            48,
        )
        .unwrap();
        let actual = encoder
            .encode(&source_bytes, Some(motion), "run/1", 41, "boot/clock")
            .unwrap();
        assert_eq!(actual, expected);
    }
}
