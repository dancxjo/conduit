//! Exact typed object observations emitted by the bounded local-CV realization.

use alloc::{vec, vec::Vec};
use conduit_core::{
    kind_id, StructuredFieldType, StructuredFieldValue, StructuredInfoRefusal, StructuredInfoType,
    StructuredInfoValue, StructuredVariantCase, QUANTITY_INFO_ID,
};

use crate::{
    image_resource_type, ContinuousLocalVisionObservation, LocalVisionObservationRefusal,
    LocalVisionProvenance, PixelRegion, MAXIMUM_LOCAL_VISION_IDENTITY_BYTES,
};

mod prepared;
pub use prepared::PreparedLocalVisionObjectEncoder;

pub const LOCAL_VISION_OBJECT_OBSERVATIONS_TYPE: &str = "VisionLocalObjectObservationsFour";
pub const MAXIMUM_LOCAL_VISION_OBJECT_OBSERVATIONS: u16 = 4;

pub fn local_vision_object_observations_type() -> StructuredInfoType {
    record(
        "vision/local-object-observations@1",
        vec![
            field("emitted_count", count_type()),
            field("items", local_vision_object_items_type()),
            field("observed_count", count_type()),
            field("provenance", local_vision_object_provenance_type()),
            field("sequence", count_type()),
            field("source_image", image_resource_type()),
            field("truncation", local_vision_object_truncation_type()),
        ],
    )
}

fn local_vision_object_items_type() -> StructuredInfoType {
    let slot = StructuredInfoType::variant(
        kind_id("vision/local-object-observation-slot@1"),
        vec![
            case("observation", local_vision_object_observation_type()),
            case("unused", unit_type()),
        ],
    )
    .expect("reviewed local object slot");
    StructuredInfoType::collection(slot, Some(MAXIMUM_LOCAL_VISION_OBJECT_OBSERVATIONS))
        .expect("reviewed bounded local object observations")
}

fn local_vision_object_observation_type() -> StructuredInfoType {
    record(
        "vision/local-object-observation@1",
        vec![
            field("area_pixels", count_type()),
            field("classification", text_type()),
            field("confidence", local_vision_object_confidence_type()),
            field("region", local_vision_object_region_type()),
        ],
    )
}

fn local_vision_object_confidence_type() -> StructuredInfoType {
    StructuredInfoType::variant(
        kind_id("vision/local-object-confidence@1"),
        vec![
            case("not_estimated", unit_type()),
            case(
                "ratio",
                StructuredInfoType::leaf(kind_id(QUANTITY_INFO_ID)).expect("reviewed quantity"),
            ),
        ],
    )
    .expect("reviewed optional local object confidence")
}

fn local_vision_object_region_type() -> StructuredInfoType {
    record(
        "vision/local-object-pixel-region@1",
        vec![
            field("height", count_type()),
            field("width", count_type()),
            field("x", count_type()),
            field("y", count_type()),
        ],
    )
}

fn local_vision_object_provenance_type() -> StructuredInfoType {
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

fn local_vision_object_truncation_type() -> StructuredInfoType {
    StructuredInfoType::variant(
        kind_id("vision/local-object-truncation@1"),
        vec![
            case("complete", unit_type()),
            case("truncated", unit_type()),
        ],
    )
    .expect("reviewed local object truncation")
}

pub fn local_vision_object_observations_value(
    source_image: StructuredInfoValue,
    observation: &ContinuousLocalVisionObservation,
    image_width: u16,
    image_height: u16,
    provenance: &LocalVisionProvenance,
) -> Result<StructuredInfoValue, LocalVisionObservationRefusal> {
    if source_image.value_type() != &image_resource_type() {
        return Err(LocalVisionObservationRefusal::MalformedImage);
    }
    validate_provenance(provenance)?;
    let slot_type = local_vision_object_items_type();
    let conduit_core::StructuredInfoTypeShape::Collection { element, .. } = slot_type.shape()
    else {
        return Err(LocalVisionObservationRefusal::InvalidObservation);
    };
    let emitted_count = observation
        .component_count
        .min(MAXIMUM_LOCAL_VISION_OBJECT_OBSERVATIONS as u8);
    let mut items = Vec::with_capacity(usize::from(MAXIMUM_LOCAL_VISION_OBJECT_OBSERVATIONS));
    for index in 0..usize::from(emitted_count) {
        let region = observation.components[index]
            .ok_or(LocalVisionObservationRefusal::InvalidObservation)?;
        let area = observation.component_areas[index];
        validate_component(region, area, image_width, image_height)?;
        let value = record_value(
            local_vision_object_observation_type(),
            vec![
                ("area_pixels", count_value(u64::from(area))?),
                ("classification", text_value("bright-component")?),
                (
                    "confidence",
                    StructuredInfoValue::variant(
                        local_vision_object_confidence_type(),
                        "not_estimated",
                        unit_value()?,
                    )?,
                ),
                ("region", region_value(region)?),
            ],
        )?;
        items.push(StructuredInfoValue::variant(
            element.clone(),
            "observation",
            value,
        )?);
    }
    while items.len() < usize::from(MAXIMUM_LOCAL_VISION_OBJECT_OBSERVATIONS) {
        items.push(StructuredInfoValue::variant(
            element.clone(),
            "unused",
            unit_value()?,
        )?);
    }
    let items = StructuredInfoValue::collection(slot_type, items)?;
    Ok(record_value(
        local_vision_object_observations_type(),
        vec![
            ("emitted_count", count_value(u64::from(emitted_count))?),
            ("items", items),
            (
                "observed_count",
                count_value(u64::from(observation.observed_component_count))?,
            ),
            ("provenance", provenance_value(provenance)?),
            ("sequence", count_value(observation.sequence)?),
            ("source_image", source_image),
            (
                "truncation",
                StructuredInfoValue::variant(
                    local_vision_object_truncation_type(),
                    if observation.components_truncated
                        || observation.component_count > emitted_count
                        || observation.observed_component_count > u32::from(emitted_count)
                    {
                        "truncated"
                    } else {
                        "complete"
                    },
                    unit_value()?,
                )?,
            ),
        ],
    )?)
}

pub(super) fn validate_component(
    region: PixelRegion,
    area: u32,
    image_width: u16,
    image_height: u16,
) -> Result<(), LocalVisionObservationRefusal> {
    let right = region.x.checked_add(region.width);
    let bottom = region.y.checked_add(region.height);
    if region.width == 0
        || region.height == 0
        || area == 0
        || right.is_none_or(|right| right > image_width)
        || bottom.is_none_or(|bottom| bottom > image_height)
    {
        Err(LocalVisionObservationRefusal::InvalidObservation)
    } else {
        Ok(())
    }
}

fn validate_provenance(
    provenance: &LocalVisionProvenance,
) -> Result<(), LocalVisionObservationRefusal> {
    for identity in [
        &provenance.implementation_id,
        &provenance.provider_instance_id,
        &provenance.artifact_id,
        &provenance.run_id,
    ] {
        validate_identity(identity)?;
    }
    Ok(())
}

pub(super) fn validate_identity(identity: &str) -> Result<(), LocalVisionObservationRefusal> {
    if identity.is_empty() || identity.len() > MAXIMUM_LOCAL_VISION_IDENTITY_BYTES {
        Err(LocalVisionObservationRefusal::InvalidIdentity)
    } else {
        Ok(())
    }
}

fn region_value(region: PixelRegion) -> Result<StructuredInfoValue, StructuredInfoRefusal> {
    record_value(
        local_vision_object_region_type(),
        vec![
            ("height", count_value(u64::from(region.height))?),
            ("width", count_value(u64::from(region.width))?),
            ("x", count_value(u64::from(region.x))?),
            ("y", count_value(u64::from(region.y))?),
        ],
    )
}

fn provenance_value(
    provenance: &LocalVisionProvenance,
) -> Result<StructuredInfoValue, StructuredInfoRefusal> {
    record_value(
        local_vision_object_provenance_type(),
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
    )
}

fn record(kind: &str, fields: Vec<StructuredFieldType>) -> StructuredInfoType {
    StructuredInfoType::record(kind_id(kind), fields).expect("reviewed local object record")
}

fn field(name: &str, value_type: StructuredInfoType) -> StructuredFieldType {
    StructuredFieldType::new(name, value_type).expect("reviewed local object field")
}

fn case(name: &str, value_type: StructuredInfoType) -> StructuredVariantCase {
    StructuredVariantCase::new(name, value_type).expect("reviewed local object case")
}

fn text_type() -> StructuredInfoType {
    StructuredInfoType::leaf(kind_id("value/text")).expect("reviewed text")
}

fn count_type() -> StructuredInfoType {
    StructuredInfoType::leaf(kind_id("value/count")).expect("reviewed count")
}

fn unit_type() -> StructuredInfoType {
    StructuredInfoType::leaf(kind_id("value/unit")).expect("reviewed unit")
}

fn text_value(value: &str) -> Result<StructuredInfoValue, StructuredInfoRefusal> {
    StructuredInfoValue::leaf(text_type(), value.as_bytes().to_vec())
}

fn count_value(value: u64) -> Result<StructuredInfoValue, StructuredInfoRefusal> {
    StructuredInfoValue::leaf(count_type(), conduit_core::encode_count(value).to_vec())
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
    use crate::{deterministic_vision_fixture, MAXIMUM_LOCAL_COMPONENTS};
    use conduit_core::StructuredInfoValueShape;

    fn provenance() -> LocalVisionProvenance {
        LocalVisionProvenance {
            implementation_id: "std/continuous-local-vision@1".into(),
            provider_instance_id: "finite-image-residence/test-1".into(),
            artifact_id: "conduit-std-host/continuous-local-vision@1".into(),
            run_id: "play/test-1/request-2".into(),
        }
    }

    #[test]
    fn prepared_object_encoder_matches_reference_value() {
        let image = deterministic_vision_fixture().unwrap().image;
        let image_bytes = image.canonical_bytes().unwrap();
        let mut components = [None; MAXIMUM_LOCAL_COMPONENTS];
        components[0] = Some(PixelRegion {
            x: 3,
            y: 4,
            width: 5,
            height: 6,
        });
        let mut component_areas = [0; MAXIMUM_LOCAL_COMPONENTS];
        component_areas[0] = 17;
        let observation = ContinuousLocalVisionObservation {
            sequence: u64::MAX,
            motion: None,
            components,
            component_areas,
            component_count: 1,
            observed_component_count: 5,
            components_truncated: true,
        };
        let provenance = provenance();
        let expected =
            local_vision_object_observations_value(image, &observation, 64, 48, &provenance)
                .unwrap()
                .canonical_bytes()
                .unwrap();
        let mut encoder = PreparedLocalVisionObjectEncoder::new(
            &provenance.implementation_id,
            &provenance.provider_instance_id,
            &provenance.artifact_id,
            64,
            48,
        )
        .unwrap();
        let actual = encoder
            .encode(&image_bytes, &observation, &provenance.run_id)
            .unwrap();
        assert_eq!(actual, expected);
        let decoded = StructuredInfoValue::from_canonical_bytes(actual).unwrap();
        assert_eq!(
            decoded.value_type(),
            &local_vision_object_observations_type()
        );
        assert_eq!(leaf_count(record_field(&decoded, "emitted_count")), 1);
        assert_eq!(leaf_count(record_field(&decoded, "observed_count")), 5);
        assert_eq!(
            variant_tag(record_field(&decoded, "truncation")),
            "truncated"
        );
        let encoded_provenance = record_field(&decoded, "provenance");
        assert_eq!(
            leaf_text(record_field(encoded_provenance, "provider_instance")),
            provenance.provider_instance_id
        );
        assert_eq!(
            leaf_text(record_field(encoded_provenance, "run")),
            provenance.run_id
        );
    }

    #[test]
    fn object_geometry_cannot_escape_the_exact_source_extent() {
        let image = deterministic_vision_fixture().unwrap().image;
        let mut components = [None; MAXIMUM_LOCAL_COMPONENTS];
        components[0] = Some(PixelRegion {
            x: 63,
            y: 47,
            width: 2,
            height: 1,
        });
        let mut component_areas = [0; MAXIMUM_LOCAL_COMPONENTS];
        component_areas[0] = 1;
        let observation = ContinuousLocalVisionObservation {
            sequence: 1,
            motion: None,
            components,
            component_areas,
            component_count: 1,
            observed_component_count: 1,
            components_truncated: false,
        };
        assert_eq!(
            local_vision_object_observations_value(image, &observation, 64, 48, &provenance()),
            Err(LocalVisionObservationRefusal::InvalidObservation)
        );
    }

    fn record_field<'a>(value: &'a StructuredInfoValue, name: &str) -> &'a StructuredInfoValue {
        let StructuredInfoValueShape::Record(fields) = value.shape() else {
            panic!("expected record")
        };
        fields
            .iter()
            .find(|field| field.name() == name)
            .expect("expected field")
            .value()
    }

    fn leaf_text(value: &StructuredInfoValue) -> &str {
        let StructuredInfoValueShape::Leaf(value) = value.shape() else {
            panic!("expected leaf")
        };
        core::str::from_utf8(value).unwrap()
    }

    fn leaf_count(value: &StructuredInfoValue) -> u64 {
        let StructuredInfoValueShape::Leaf(value) = value.shape() else {
            panic!("expected leaf")
        };
        conduit_core::decode_count(value).unwrap()
    }

    fn variant_tag(value: &StructuredInfoValue) -> &str {
        let StructuredInfoValueShape::Variant { tag, .. } = value.shape() else {
            panic!("expected variant")
        };
        tag
    }
}
