//! Portable human-media contracts.

use crate::human_media_catalog::install_camera_catalogs;
use alloc::{string::String, string::ToString, vec, vec::Vec};
use conduit_core::{
    kind_id, port_id, BoundedResourceRef, KindContractRevision, KindId, PortDescriptor,
    PortDirection, PortTemporal, StructuredFieldType, StructuredFieldValue, StructuredInfoType,
    StructuredInfoTypeShape, StructuredInfoValue, StructuredInfoValueShape, StructuredVariantCase,
    RESOURCE_REFERENCE_INFO_ID,
};

pub const IMAGE_TEXT_COMPOSE_KIND: &str = "media/compose-image-text";
pub const IMAGE_TEXT_COMPOSE_REVISION: &str = "conduit.human/image-text-compose@1";
pub const IMAGE_TEXT_TYPED_RECORD_KIND: &str = "media/image-text-to-typed-record";
pub const IMAGE_TEXT_TYPED_RECORD_REVISION: &str = "conduit.human/image-text-typed-record@1";
pub const IMAGE_REFERENCE_TYPE: &str = "ImageObservationReference";
pub const IMAGE_TEXT_RECORD_TYPE: &str = "ImageTextRecord";

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ImageTextValueRefusal {
    InvalidRecord(conduit_human::ImageTextRefusal),
    TypedRecord(conduit_net::TypedRecordFrameRefusal),
    Malformed,
}

pub fn image_observation_reference_type() -> StructuredInfoType {
    StructuredInfoType::record(
        kind_id("human/image-observation-reference@1"),
        vec![
            StructuredFieldType::new(
                "content",
                StructuredInfoType::leaf(kind_id(RESOURCE_REFERENCE_INFO_ID)).unwrap(),
            )
            .unwrap(),
            StructuredFieldType::new(
                "height",
                StructuredInfoType::leaf(kind_id("value/count@1")).unwrap(),
            )
            .unwrap(),
            StructuredFieldType::new(
                "width",
                StructuredInfoType::leaf(kind_id("value/count@1")).unwrap(),
            )
            .unwrap(),
        ],
    )
    .expect("image observation reference type")
}

pub fn image_text_record_type() -> StructuredInfoType {
    let text = || StructuredInfoType::leaf(kind_id("value/text@1")).expect("text type");
    let unit = || StructuredInfoType::leaf(kind_id("value/unit@1")).expect("unit type");
    let metadata = StructuredInfoType::record(
        kind_id("human/image-text-metadata@1"),
        vec![
            StructuredFieldType::new("key", text()).unwrap(),
            StructuredFieldType::new("value", text()).unwrap(),
        ],
    )
    .unwrap();
    let slot = StructuredInfoType::variant(
        kind_id("human/optional-image-text-metadata@1"),
        vec![
            StructuredVariantCase::new("absent", unit()).unwrap(),
            StructuredVariantCase::new("present", metadata).unwrap(),
        ],
    )
    .unwrap();
    StructuredInfoType::record(
        kind_id("human/image-text-record@1"),
        vec![
            StructuredFieldType::new("caption", text()).unwrap(),
            StructuredFieldType::new(
                "content_digest",
                StructuredInfoType::leaf(kind_id("value/bytes@1")).unwrap(),
            )
            .unwrap(),
            StructuredFieldType::new("image", image_observation_reference_type()).unwrap(),
            StructuredFieldType::new(
                "metadata",
                StructuredInfoType::collection(
                    slot,
                    Some(conduit_human::MAXIMUM_IMAGE_TEXT_METADATA_ENTRIES as u16),
                )
                .unwrap(),
            )
            .unwrap(),
        ],
    )
    .unwrap()
}

pub fn image_text_record_value(
    record: &conduit_human::ImageTextRecord,
    expected_image_profile: &KindId,
) -> Result<StructuredInfoValue, ImageTextValueRefusal> {
    record
        .validate(expected_image_profile)
        .map_err(ImageTextValueRefusal::InvalidRecord)?;
    let text = |value: &str| leaf_value("value/text@1", value.as_bytes().to_vec());
    let mut slots = Vec::with_capacity(conduit_human::MAXIMUM_IMAGE_TEXT_METADATA_ENTRIES);
    for entry in &record.metadata {
        let metadata = StructuredInfoValue::record(
            metadata_type(),
            vec![
                field_value("key", text(&entry.key)?),
                field_value("value", text(&entry.value)?),
            ],
        )
        .map_err(|_| ImageTextValueRefusal::Malformed)?;
        slots.push(
            StructuredInfoValue::variant(metadata_slot_type(), "present", metadata)
                .map_err(|_| ImageTextValueRefusal::Malformed)?,
        );
    }
    while slots.len() < conduit_human::MAXIMUM_IMAGE_TEXT_METADATA_ENTRIES {
        slots.push(
            StructuredInfoValue::variant(
                metadata_slot_type(),
                "absent",
                leaf_value("value/unit@1", Vec::new())?,
            )
            .map_err(|_| ImageTextValueRefusal::Malformed)?,
        );
    }
    let metadata = StructuredInfoValue::collection(metadata_collection_type(), slots)
        .map_err(|_| ImageTextValueRefusal::Malformed)?;
    StructuredInfoValue::record(
        image_text_record_type(),
        vec![
            field_value("caption", text(&record.caption)?),
            field_value(
                "content_digest",
                leaf_value("value/bytes@1", record.content_digest.to_vec())?,
            ),
            field_value("image", image_observation_value(&record.image)?),
            field_value("metadata", metadata),
        ],
    )
    .map_err(|_| ImageTextValueRefusal::Malformed)
}

pub fn image_text_record_from_value(
    value: &StructuredInfoValue,
    expected_image_profile: &KindId,
) -> Result<conduit_human::ImageTextRecord, ImageTextValueRefusal> {
    if value.value_type() != &image_text_record_type() {
        return Err(ImageTextValueRefusal::Malformed);
    }
    let fields = record_fields(value)?;
    let image = image_observation_from_value(field(fields, "image")?)?;
    let caption = text_from(field(fields, "caption")?)?;
    let digest: [u8; 32] = leaf_bytes(field(fields, "content_digest")?)?
        .try_into()
        .map_err(|_| ImageTextValueRefusal::Malformed)?;
    let StructuredInfoValueShape::Collection(slots) = field(fields, "metadata")?.shape() else {
        return Err(ImageTextValueRefusal::Malformed);
    };
    let mut metadata = Vec::new();
    let mut absent_seen = false;
    for slot in slots {
        let StructuredInfoValueShape::Variant { tag, payload } = slot.shape() else {
            return Err(ImageTextValueRefusal::Malformed);
        };
        match tag {
            "absent" => absent_seen = true,
            "present" if !absent_seen => {
                let fields = record_fields(payload)?;
                metadata.push(conduit_human::ImageTextMetadata {
                    key: text_from(field(fields, "key")?)?,
                    value: text_from(field(fields, "value")?)?,
                });
            }
            _ => return Err(ImageTextValueRefusal::Malformed),
        }
    }
    let record = conduit_human::ImageTextRecord {
        image,
        caption,
        metadata,
        content_digest: digest,
    };
    record
        .validate(expected_image_profile)
        .map_err(ImageTextValueRefusal::InvalidRecord)?;
    Ok(record)
}

pub fn image_text_typed_record_value(
    record: &conduit_human::ImageTextRecord,
    expected_image_profile: &KindId,
) -> Result<StructuredInfoValue, ImageTextValueRefusal> {
    let value = image_text_record_value(record, expected_image_profile)?;
    conduit_net::typed_record_value(&value).map_err(ImageTextValueRefusal::TypedRecord)
}

fn metadata_type() -> StructuredInfoType {
    let collection = metadata_collection_type();
    let StructuredInfoTypeShape::Collection { element, .. } = collection.shape() else {
        unreachable!()
    };
    let StructuredInfoTypeShape::Variant { cases, .. } = element.shape() else {
        unreachable!()
    };
    cases
        .iter()
        .find(|case| case.tag() == "present")
        .unwrap()
        .payload_type()
        .clone()
}

fn image_observation_value(
    image: &conduit_human::ImageObservationReference,
) -> Result<StructuredInfoValue, ImageTextValueRefusal> {
    StructuredInfoValue::record(
        image_observation_reference_type(),
        vec![
            field_value(
                "content",
                leaf_value(
                    RESOURCE_REFERENCE_INFO_ID,
                    image
                        .content
                        .encode()
                        .map_err(|_| ImageTextValueRefusal::Malformed)?,
                )?,
            ),
            field_value("height", count_value(image.height)?),
            field_value("width", count_value(image.width)?),
        ],
    )
    .map_err(|_| ImageTextValueRefusal::Malformed)
}

fn image_observation_from_value(
    value: &StructuredInfoValue,
) -> Result<conduit_human::ImageObservationReference, ImageTextValueRefusal> {
    if value.value_type() != &image_observation_reference_type() {
        return Err(ImageTextValueRefusal::Malformed);
    }
    let fields = record_fields(value)?;
    Ok(conduit_human::ImageObservationReference {
        content: BoundedResourceRef::decode(leaf_bytes(field(fields, "content")?)?)
            .map_err(|_| ImageTextValueRefusal::Malformed)?,
        height: u16::try_from(count_from(field(fields, "height")?)?)
            .map_err(|_| ImageTextValueRefusal::Malformed)?,
        width: u16::try_from(count_from(field(fields, "width")?)?)
            .map_err(|_| ImageTextValueRefusal::Malformed)?,
    })
}

fn metadata_slot_type() -> StructuredInfoType {
    let collection = metadata_collection_type();
    let StructuredInfoTypeShape::Collection { element, .. } = collection.shape() else {
        unreachable!()
    };
    element.clone()
}

fn metadata_collection_type() -> StructuredInfoType {
    let record = image_text_record_type();
    let StructuredInfoTypeShape::Record { fields, .. } = record.shape() else {
        unreachable!()
    };
    fields
        .iter()
        .find(|field| field.name() == "metadata")
        .unwrap()
        .value_type()
        .clone()
}

#[cfg(feature = "form-catalog")]
pub fn install_human_media_catalogs(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    use conduit_form::{KindDefinition, KindSignature};

    install_camera_catalogs(startup, profile)?;

    startup
        .insert_structured_type(IMAGE_REFERENCE_TYPE, image_observation_reference_type())
        .map_err(|error| error.to_string())?;
    startup
        .insert_structured_type(IMAGE_TEXT_RECORD_TYPE, image_text_record_type())
        .map_err(|error| error.to_string())?;

    for (kind, revision, inputs, outputs) in [
        (
            IMAGE_TEXT_COMPOSE_KIND,
            IMAGE_TEXT_COMPOSE_REVISION,
            vec![
                structured_port(
                    "image",
                    &image_observation_reference_type(),
                    PortDirection::Input,
                ),
                PortDescriptor {
                    port_id: port_id("caption"),
                    value_kind: kind_id("value/text@1"),
                    direction: PortDirection::Input,
                    temporal: PortTemporal::Value,
                },
            ],
            vec![structured_port(
                "record",
                &image_text_record_type(),
                PortDirection::Output,
            )],
        ),
        (
            IMAGE_TEXT_TYPED_RECORD_KIND,
            IMAGE_TEXT_TYPED_RECORD_REVISION,
            vec![structured_port(
                "record",
                &image_text_record_type(),
                PortDirection::Input,
            )],
            vec![structured_port(
                "typed",
                &conduit_net::typed_record_type(),
                PortDirection::Output,
            )],
        ),
    ] {
        startup.insert(KindSignature {
            kind: kind.into(),
            startup_parameters: vec![],
        })?;
        profile
            .insert(KindDefinition {
                kind_id: kind_id(kind),
                kind_contract_revision: KindContractRevision::from(revision),
                inputs,
                outputs,
                configuration: vec![],
            })
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

fn structured_port(
    name: &str,
    value_type: &StructuredInfoType,
    direction: PortDirection,
) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: value_type.profile().unwrap().value_kind().clone(),
        direction,
        temporal: PortTemporal::Value,
    }
}

fn leaf_value(
    identity: &str,
    bytes: Vec<u8>,
) -> Result<StructuredInfoValue, ImageTextValueRefusal> {
    StructuredInfoValue::leaf(
        StructuredInfoType::leaf(kind_id(identity))
            .map_err(|_| ImageTextValueRefusal::Malformed)?,
        bytes,
    )
    .map_err(|_| ImageTextValueRefusal::Malformed)
}

fn count_value(value: u16) -> Result<StructuredInfoValue, ImageTextValueRefusal> {
    leaf_value("value/count@1", u64::from(value).to_le_bytes().to_vec())
}

fn field_value(name: &str, value: StructuredInfoValue) -> StructuredFieldValue {
    StructuredFieldValue::new(name, value).expect("reviewed image-text field name is finite")
}

fn record_fields(
    value: &StructuredInfoValue,
) -> Result<&[StructuredFieldValue], ImageTextValueRefusal> {
    match value.shape() {
        StructuredInfoValueShape::Record(fields) => Ok(fields),
        _ => Err(ImageTextValueRefusal::Malformed),
    }
}

fn field<'a>(
    fields: &'a [StructuredFieldValue],
    name: &str,
) -> Result<&'a StructuredInfoValue, ImageTextValueRefusal> {
    fields
        .iter()
        .find(|field| field.name() == name)
        .map(StructuredFieldValue::value)
        .ok_or(ImageTextValueRefusal::Malformed)
}

fn leaf_bytes(value: &StructuredInfoValue) -> Result<&[u8], ImageTextValueRefusal> {
    match value.shape() {
        StructuredInfoValueShape::Leaf(bytes) => Ok(bytes),
        _ => Err(ImageTextValueRefusal::Malformed),
    }
}

fn count_from(value: &StructuredInfoValue) -> Result<u64, ImageTextValueRefusal> {
    leaf_bytes(value)?
        .try_into()
        .map(u64::from_le_bytes)
        .map_err(|_| ImageTextValueRefusal::Malformed)
}

fn text_from(value: &StructuredInfoValue) -> Result<String, ImageTextValueRefusal> {
    String::from_utf8(leaf_bytes(value)?.to_vec()).map_err(|_| ImageTextValueRefusal::Malformed)
}
