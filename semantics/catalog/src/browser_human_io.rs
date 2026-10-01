//! Portable human-media contracts.

#[cfg(feature = "form-catalog")]
use crate::human_media_catalog::install_camera_catalogs;
#[cfg(feature = "form-catalog")]
use alloc::string::ToString;
use alloc::vec;
use conduit_core::{
    kind_id, port_id, CapabilityLimits, Kind, KindId, KindIdentity, PortDescriptor, PortDirection,
    PortTemporal, StructuredInfoType, StructuredInfoValue, MAXIMUM_STRUCTURED_CANONICAL_BYTES,
};
use conduit_form::rust_binding::NativeRustBinding;

pub const IMAGE_TEXT_COMPOSE_KIND: &str = "media/compose-image-text";
pub const IMAGE_TEXT_COMPOSE_REVISION: &str = "conduit.human/image-text-compose@1";
pub const IMAGE_TEXT_TYPED_RECORD_KIND: &str = "media/image-text-to-typed-record";
pub const IMAGE_TEXT_TYPED_RECORD_REVISION: &str = "conduit.human/image-text-typed-record@1";
pub const IMAGE_REFERENCE_TYPE: &str = "ImageObservationReference";
pub const IMAGE_TEXT_RECORD_TYPE: &str = "ImageTextRecord";

pub fn image_text_compose_semantic_contract() -> Kind {
    Kind {
        startup_parameters: vec![],
        shorthand: None,
        kind_id: kind_id(IMAGE_TEXT_COMPOSE_KIND),
        kind_contract_revision: KindIdentity::from(IMAGE_TEXT_COMPOSE_REVISION),
        inputs: vec![
            structured_port(
                "image",
                &image_observation_reference_type(),
                PortDirection::Input,
            ),
            PortDescriptor {
                port_id: port_id("caption"),
                value_kind: kind_id("value/text"),
                direction: PortDirection::Input,
                temporal: PortTemporal::Value,
                abnormal_kind: None,
            },
        ],
        outputs: vec![structured_port(
            "record",
            &image_text_record_type(),
            PortDirection::Output,
        )],
        configuration: Default::default(),
        semantic_laws: Default::default(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 2,
            max_queue_bytes: (MAXIMUM_STRUCTURED_CANONICAL_BYTES
                + conduit_human::MAXIMUM_IMAGE_TEXT_CAPTION_BYTES)
                as u32,
        },
    }
}

pub fn image_text_typed_record_semantic_contract() -> Kind {
    Kind {
        startup_parameters: vec![],
        shorthand: None,
        kind_id: kind_id(IMAGE_TEXT_TYPED_RECORD_KIND),
        kind_contract_revision: KindIdentity::from(IMAGE_TEXT_TYPED_RECORD_REVISION),
        inputs: vec![structured_port(
            "record",
            &image_text_record_type(),
            PortDirection::Input,
        )],
        outputs: vec![structured_port(
            "typed",
            &conduit_net::typed_record_type(),
            PortDirection::Output,
        )],
        configuration: Default::default(),
        semantic_laws: Default::default(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32,
        },
    }
}

#[cfg(feature = "form-catalog")]
pub fn install_image_text_inspection_catalog(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    use conduit_form::{KindProjection, KindSignature};

    let contract =
        crate::structured_presentation_contract(IMAGE_TEXT_RECORD_TYPE, &image_text_record_type());
    startup.insert(KindSignature {
        kind: contract.kind_id.as_str().to_string(),
        startup_parameters: vec![],
    })?;
    profile
        .insert(KindProjection {
            kind_id: contract.kind_id,
            kind_contract_revision: contract.kind_contract_revision,
            inputs: contract.inputs,
            outputs: contract.outputs,
            configuration: vec![],
        })
        .map_err(|error| error.to_string())
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ImageTextValueRefusal {
    InvalidRecord(conduit_human::ImageTextRefusal),
    TypedRecord(conduit_net::TypedRecordFrameRefusal),
    Malformed,
}

pub fn image_observation_reference_type() -> StructuredInfoType {
    conduit_human::ImageObservationReference::semantic_type()
        .expect("checked human image reference Type remains decodable")
}

pub fn image_text_record_type() -> StructuredInfoType {
    conduit_human::ImageTextRecord::semantic_type()
        .expect("checked human image-text record Type remains decodable")
}

pub fn image_text_record_value(
    record: &conduit_human::ImageTextRecord,
    expected_image_profile: &KindId,
) -> Result<StructuredInfoValue, ImageTextValueRefusal> {
    record
        .validate(expected_image_profile)
        .map_err(ImageTextValueRefusal::InvalidRecord)?;
    record
        .clone()
        .into_structured()
        .map_err(|_| ImageTextValueRefusal::Malformed)
}

pub fn image_text_record_from_value(
    value: &StructuredInfoValue,
    expected_image_profile: &KindId,
) -> Result<conduit_human::ImageTextRecord, ImageTextValueRefusal> {
    let record = conduit_human::ImageTextRecord::from_structured(value.clone())
        .map_err(|_| ImageTextValueRefusal::Malformed)?;
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

pub fn image_observation_value(
    image: &conduit_human::ImageObservationReference,
) -> Result<StructuredInfoValue, ImageTextValueRefusal> {
    image
        .clone()
        .into_structured()
        .map_err(|_| ImageTextValueRefusal::Malformed)
}

pub fn image_observation_from_value(
    value: &StructuredInfoValue,
) -> Result<conduit_human::ImageObservationReference, ImageTextValueRefusal> {
    conduit_human::ImageObservationReference::from_structured(value.clone())
        .map_err(|_| ImageTextValueRefusal::Malformed)
}

#[cfg(feature = "form-catalog")]
pub fn install_human_media_catalogs(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    use conduit_form::{KindProjection, KindSignature};

    install_camera_catalogs(startup, profile)?;

    startup
        .insert_structured_type(IMAGE_REFERENCE_TYPE, image_observation_reference_type())
        .map_err(|error| error.to_string())?;
    startup
        .insert_structured_type(IMAGE_TEXT_RECORD_TYPE, image_text_record_type())
        .map_err(|error| error.to_string())?;

    for contract in [
        image_text_compose_semantic_contract(),
        image_text_typed_record_semantic_contract(),
    ] {
        startup.insert(KindSignature {
            kind: contract.kind_id.as_str().into(),
            startup_parameters: vec![],
        })?;
        profile
            .insert(KindProjection {
                kind_id: contract.kind_id,
                kind_contract_revision: contract.kind_contract_revision,
                inputs: contract.inputs,
                outputs: contract.outputs,
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
        abnormal_kind: None,
    }
}
