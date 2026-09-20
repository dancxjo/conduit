//! Exact portable Info emitted by a continuous local-Vision realization.

use alloc::{
    string::{String, ToString},
    vec,
    vec::Vec,
};
use conduit_core::{
    kind_id, StructuredFieldType, StructuredFieldValue, StructuredInfoRefusal, StructuredInfoType,
    StructuredInfoValue, StructuredVariantCase,
};

use crate::{image_resource_type, ContinuousLocalVisionObservation, PixelRegion};

pub const LOCAL_VISION_MOTION_OBSERVATION_TYPE: &str = "VisionLocalMotionObservation";
pub const MAXIMUM_LOCAL_VISION_MOTION_OBSERVATIONS: u16 = 4;
pub const MAXIMUM_LOCAL_VISION_IDENTITY_BYTES: usize = 256;

mod prepared;
pub use prepared::PreparedLocalVisionMotionEncoder;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LocalVisionProvenance {
    pub implementation_id: String,
    pub provider_instance_id: String,
    pub artifact_id: String,
    pub run_id: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LocalVisionObservationRefusal {
    InvalidIdentity,
    MalformedImage,
    InvalidObservation,
    Structured(StructuredInfoRefusal),
}

impl From<StructuredInfoRefusal> for LocalVisionObservationRefusal {
    fn from(value: StructuredInfoRefusal) -> Self {
        Self::Structured(value)
    }
}

pub fn local_vision_motion_observation_type() -> StructuredInfoType {
    record(
        "vision/local-motion-observation@1",
        vec![
            field("motion", local_vision_motion_state_type()),
            field("provenance", local_vision_provenance_type()),
            field("sequence", count_type()),
            field("source_image", image_resource_type()),
        ],
    )
}

pub fn local_vision_motion_observations_type() -> StructuredInfoType {
    let slot = StructuredInfoType::variant(
        kind_id("vision/local-motion-observation-slot@1"),
        vec![
            case("observation", local_vision_motion_observation_type()),
            case("unused", unit_type()),
        ],
    )
    .expect("reviewed local Vision motion slot");
    StructuredInfoType::collection(slot, Some(MAXIMUM_LOCAL_VISION_MOTION_OBSERVATIONS))
        .expect("reviewed bounded local Vision motion observations")
}

fn local_vision_motion_state_type() -> StructuredInfoType {
    StructuredInfoType::variant(
        kind_id("vision/local-motion-state@1"),
        vec![
            case("changed", local_vision_motion_region_type()),
            case("unchanged", unit_type()),
        ],
    )
    .expect("reviewed local Vision motion state")
}

fn local_vision_motion_region_type() -> StructuredInfoType {
    record(
        "vision/local-motion-region@1",
        vec![
            field("changed_pixels", count_type()),
            field("height", count_type()),
            field("width", count_type()),
            field("x", count_type()),
            field("y", count_type()),
        ],
    )
}

fn local_vision_provenance_type() -> StructuredInfoType {
    record(
        "vision/local-provider-provenance@1",
        vec![
            field("artifact", text_type()),
            field("evidence_class", text_type()),
            field("implementation", text_type()),
            field("provider_instance", text_type()),
            field("run", text_type()),
        ],
    )
}

pub fn local_vision_motion_observation_value(
    source_image: StructuredInfoValue,
    observation: &ContinuousLocalVisionObservation,
    provenance: &LocalVisionProvenance,
) -> Result<StructuredInfoValue, LocalVisionObservationRefusal> {
    if source_image.value_type() != &image_resource_type() {
        return Err(LocalVisionObservationRefusal::MalformedImage);
    }
    for identity in [
        &provenance.implementation_id,
        &provenance.provider_instance_id,
        &provenance.artifact_id,
        &provenance.run_id,
    ] {
        if identity.is_empty() || identity.len() > MAXIMUM_LOCAL_VISION_IDENTITY_BYTES {
            return Err(LocalVisionObservationRefusal::InvalidIdentity);
        }
    }
    let motion = match observation.motion {
        Some(motion) => StructuredInfoValue::variant(
            local_vision_motion_state_type(),
            "changed",
            motion_region_value(motion.region, motion.changed_pixels)?,
        )?,
        None => StructuredInfoValue::variant(
            local_vision_motion_state_type(),
            "unchanged",
            unit_value()?,
        )?,
    };
    let value = record_value(
        local_vision_motion_observation_type(),
        vec![
            ("motion", motion),
            ("provenance", provenance_value(provenance)?),
            ("sequence", count_value(observation.sequence)?),
            ("source_image", source_image),
        ],
    )?;
    let slot_type = local_vision_motion_observations_type();
    let conduit_core::StructuredInfoTypeShape::Collection { element, .. } = slot_type.shape()
    else {
        return Err(LocalVisionObservationRefusal::InvalidObservation);
    };
    let element = element.clone();
    let mut slots = vec![StructuredInfoValue::variant(
        element.clone(),
        "observation",
        value,
    )?];
    while slots.len() < usize::from(MAXIMUM_LOCAL_VISION_MOTION_OBSERVATIONS) {
        slots.push(StructuredInfoValue::variant(
            element.clone(),
            "unused",
            unit_value()?,
        )?);
    }
    Ok(StructuredInfoValue::collection(slot_type, slots)?)
}

fn motion_region_value(
    region: PixelRegion,
    changed_pixels: u32,
) -> Result<StructuredInfoValue, LocalVisionObservationRefusal> {
    if region.width == 0 || region.height == 0 || changed_pixels == 0 {
        return Err(LocalVisionObservationRefusal::InvalidObservation);
    }
    Ok(record_value(
        local_vision_motion_region_type(),
        vec![
            ("changed_pixels", count_value(u64::from(changed_pixels))?),
            ("height", count_value(u64::from(region.height))?),
            ("width", count_value(u64::from(region.width))?),
            ("x", count_value(u64::from(region.x))?),
            ("y", count_value(u64::from(region.y))?),
        ],
    )?)
}

fn provenance_value(
    provenance: &LocalVisionProvenance,
) -> Result<StructuredInfoValue, LocalVisionObservationRefusal> {
    Ok(record_value(
        local_vision_provenance_type(),
        vec![
            ("artifact", text_value(&provenance.artifact_id)?),
            ("evidence_class", text_value("deterministic-derived")?),
            ("implementation", text_value(&provenance.implementation_id)?),
            (
                "provider_instance",
                text_value(&provenance.provider_instance_id)?,
            ),
            ("run", text_value(&provenance.run_id)?),
        ],
    )?)
}

fn record(kind: &str, fields: Vec<StructuredFieldType>) -> StructuredInfoType {
    StructuredInfoType::record(kind_id(kind), fields).expect("reviewed local Vision record")
}

fn field(name: &str, value_type: StructuredInfoType) -> StructuredFieldType {
    StructuredFieldType::new(name, value_type).expect("reviewed local Vision field")
}

fn case(name: &str, value_type: StructuredInfoType) -> StructuredVariantCase {
    StructuredVariantCase::new(name, value_type).expect("reviewed local Vision case")
}

fn text_type() -> StructuredInfoType {
    StructuredInfoType::leaf(kind_id("value/text@1")).expect("reviewed text")
}

fn count_type() -> StructuredInfoType {
    StructuredInfoType::leaf(kind_id("value/count@1")).expect("reviewed count")
}

fn unit_type() -> StructuredInfoType {
    StructuredInfoType::leaf(kind_id("value/unit@1")).expect("reviewed unit")
}

fn text_value(value: &str) -> Result<StructuredInfoValue, StructuredInfoRefusal> {
    StructuredInfoValue::leaf(text_type(), value.as_bytes().to_vec())
}

fn count_value(value: u64) -> Result<StructuredInfoValue, StructuredInfoRefusal> {
    StructuredInfoValue::leaf(count_type(), value.to_string().into_bytes())
}

fn unit_value() -> Result<StructuredInfoValue, StructuredInfoRefusal> {
    StructuredInfoValue::leaf(unit_type(), Vec::new())
}

fn record_value(
    value_type: StructuredInfoType,
    fields: Vec<(&str, StructuredInfoValue)>,
) -> Result<StructuredInfoValue, StructuredInfoRefusal> {
    StructuredInfoValue::record(
        value_type,
        fields
            .into_iter()
            .map(|(name, value)| StructuredFieldValue::new(name, value))
            .collect::<Result<Vec<_>, _>>()?,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::deterministic_vision_fixture;
    use conduit_core::{StructuredInfoTypeShape, StructuredInfoValueShape};

    fn provenance() -> LocalVisionProvenance {
        LocalVisionProvenance {
            implementation_id: "std/continuous-local-vision@1".into(),
            provider_instance_id: "finite-image-residence/test-1".into(),
            artifact_id: "conduit-std-host/continuous-local-vision@1".into(),
            run_id: "play/test-1/sequence-2".into(),
        }
    }

    #[test]
    fn value_preserves_exact_source_generation_motion_and_provider_truth() {
        let image = deterministic_vision_fixture().unwrap().image;
        let observation = ContinuousLocalVisionObservation {
            sequence: 2,
            motion: Some(crate::MotionRegion {
                region: PixelRegion {
                    x: 3,
                    y: 4,
                    width: 5,
                    height: 6,
                },
                changed_pixels: 17,
            }),
            components: [None; crate::MAXIMUM_LOCAL_COMPONENTS],
            component_areas: [0; crate::MAXIMUM_LOCAL_COMPONENTS],
            component_count: 0,
            observed_component_count: 0,
            components_truncated: false,
        };
        let value =
            local_vision_motion_observation_value(image.clone(), &observation, &provenance())
                .unwrap();
        assert_eq!(value.value_type(), &local_vision_motion_observations_type());
        let encoded = value.canonical_bytes().unwrap();
        let decoded = StructuredInfoValue::from_canonical_bytes(&encoded).unwrap();
        assert_eq!(decoded, value);
        let StructuredInfoValueShape::Collection(slots) = value.shape() else {
            panic!("expected slots")
        };
        let StructuredInfoValueShape::Variant { payload, .. } = slots[0].shape() else {
            panic!("expected observation")
        };
        let StructuredInfoValueShape::Record(fields) = payload.shape() else {
            panic!("expected observation record")
        };
        assert_eq!(
            fields
                .iter()
                .find(|field| field.name() == "source_image")
                .unwrap()
                .value(),
            &image
        );
    }

    #[test]
    fn contract_is_fixed_four_slots_with_no_text_only_escape_hatch() {
        let value_type = local_vision_motion_observations_type();
        let StructuredInfoTypeShape::Collection { length, element } = value_type.shape() else {
            panic!("expected collection")
        };
        assert_eq!(length, MAXIMUM_LOCAL_VISION_MOTION_OBSERVATIONS);
        assert!(matches!(
            element.shape(),
            StructuredInfoTypeShape::Variant { .. }
        ));
    }

    #[test]
    fn prepared_encoder_matches_reference_canonical_value() {
        let image = deterministic_vision_fixture().unwrap().image;
        let image_bytes = image.canonical_bytes().unwrap();
        let observation = ContinuousLocalVisionObservation {
            sequence: 18446744073709551615,
            motion: Some(crate::MotionRegion {
                region: PixelRegion {
                    x: 3,
                    y: 4,
                    width: 5,
                    height: 6,
                },
                changed_pixels: 17,
            }),
            components: [None; crate::MAXIMUM_LOCAL_COMPONENTS],
            component_areas: [0; crate::MAXIMUM_LOCAL_COMPONENTS],
            component_count: 0,
            observed_component_count: 0,
            components_truncated: false,
        };
        let provenance = provenance();
        let expected = local_vision_motion_observation_value(image, &observation, &provenance)
            .unwrap()
            .canonical_bytes()
            .unwrap();
        let mut encoder = PreparedLocalVisionMotionEncoder::new(
            &provenance.implementation_id,
            &provenance.provider_instance_id,
            &provenance.artifact_id,
        )
        .unwrap();
        let actual = encoder
            .encode(&image_bytes, &observation, &provenance.run_id)
            .unwrap();
        assert_eq!(actual, expected);
    }
}
