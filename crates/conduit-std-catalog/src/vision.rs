//! Portable bounded image metadata and model-derived vision Info.
//!
//! Pixel content remains one bounded resource reference. Regions and landmarks
//! reuse the geometry catalog in a named normalized image frame.

use alloc::{vec, vec::Vec};
use conduit_core::{
    kind_id, Quantity, QuantityDimension, StructuredFieldType, StructuredInfoType,
    StructuredVariantCase, QUANTITY_INFO_ID, RESOURCE_REFERENCE_INFO_ID,
};

use crate::{point2_type, rect2_type};

pub const IMAGE_RESOURCE_TYPE: &str = "ImageResource";
pub const VISION_DETECTION_TYPE: &str = "VisionDetection";
pub const VISION_DETECTIONS_TYPE: &str = "VisionDetectionsFour";
pub const VISION_IMAGE_CONTENT_PROFILE: &str = "vision/image-pixels@1";
pub const VISION_IMAGE_ACCESS_CLASS: &str = "conduit.resource/image-content@1";
pub const MAXIMUM_VISION_DETECTIONS: u16 = 4;
pub const MAXIMUM_VISION_LANDMARKS: u16 = 8;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum VisionRefusal {
    NonRatioConfidence,
    ConfidenceOutOfRange,
    MalformedInfo,
    InvalidImageReference,
}

pub fn validate_confidence(confidence: Quantity) -> Result<(), VisionRefusal> {
    if confidence.dimension() != QuantityDimension::Ratio {
        return Err(VisionRefusal::NonRatioConfidence);
    }
    let normalized = confidence
        .convert(conduit_core::QuantityUnit::Millionth)
        .map_err(|_| VisionRefusal::ConfidenceOutOfRange)?;
    if !(0..=1_000_000).contains(&normalized.value()) {
        return Err(VisionRefusal::ConfidenceOutOfRange);
    }
    Ok(())
}

fn leaf(kind: &str) -> StructuredInfoType {
    StructuredInfoType::leaf(kind_id(kind)).expect("reviewed vision leaf")
}

fn field(name: &str, value_type: StructuredInfoType) -> StructuredFieldType {
    StructuredFieldType::new(name, value_type).expect("reviewed vision field")
}

fn case(name: &str, payload_type: StructuredInfoType) -> StructuredVariantCase {
    StructuredVariantCase::new(name, payload_type).expect("reviewed vision case")
}

fn record(kind: &str, fields: Vec<StructuredFieldType>) -> StructuredInfoType {
    StructuredInfoType::record(kind_id(kind), fields).expect("reviewed vision record")
}

fn unit_type() -> StructuredInfoType {
    leaf("value/unit@1")
}

fn text_type() -> StructuredInfoType {
    leaf("value/text@1")
}

fn count_type() -> StructuredInfoType {
    leaf("value/count@1")
}

pub fn image_pixel_extent_type() -> StructuredInfoType {
    record(
        "vision/pixel-extent@1",
        vec![field("height", count_type()), field("width", count_type())],
    )
}

pub fn image_format_type() -> StructuredInfoType {
    StructuredInfoType::variant(
        kind_id("vision/image-format@1"),
        vec![
            case("gray8", unit_type()),
            case("jpeg", unit_type()),
            case("png", unit_type()),
            case("rgba8", unit_type()),
        ],
    )
    .expect("reviewed image formats")
}

pub fn image_color_profile_type() -> StructuredInfoType {
    StructuredInfoType::variant(
        kind_id("vision/color-profile@1"),
        vec![case("absent", unit_type()), case("named", text_type())],
    )
    .expect("reviewed optional color profile")
}

pub fn image_resource_type() -> StructuredInfoType {
    record(
        "vision/image-resource@1",
        vec![
            field("color_profile", image_color_profile_type()),
            field("content", leaf(RESOURCE_REFERENCE_INFO_ID)),
            field("extent", image_pixel_extent_type()),
            field("format", image_format_type()),
            field("image_frame", text_type()),
        ],
    )
}

pub fn vision_evidence_class_type() -> StructuredInfoType {
    StructuredInfoType::variant(
        kind_id("vision/evidence-class@1"),
        vec![
            case("heuristic", unit_type()),
            case("model_derived", unit_type()),
        ],
    )
    .expect("reviewed evidence class")
}

pub fn vision_provenance_type() -> StructuredInfoType {
    record(
        "vision/detection-provenance@1",
        vec![
            field("evidence_class", vision_evidence_class_type()),
            field("profile", text_type()),
            field("revision", text_type()),
            field("source", text_type()),
        ],
    )
}

pub fn vision_keypoint_type() -> StructuredInfoType {
    record(
        "vision/keypoint@1",
        vec![
            field("confidence", leaf(QUANTITY_INFO_ID)),
            field("name", text_type()),
            field("point", point2_type()),
        ],
    )
}

pub fn vision_landmark_slot_type() -> StructuredInfoType {
    StructuredInfoType::variant(
        kind_id("vision/landmark-slot@1"),
        vec![
            case("keypoint", vision_keypoint_type()),
            case("unused", unit_type()),
        ],
    )
    .expect("reviewed landmark slot")
}

pub fn vision_landmarks_type() -> StructuredInfoType {
    StructuredInfoType::collection(vision_landmark_slot_type(), Some(MAXIMUM_VISION_LANDMARKS))
        .expect("bounded landmark slots")
}

fn rgb_sample_type() -> StructuredInfoType {
    record(
        "vision/rgb-sample@1",
        vec![
            field("blue", count_type()),
            field("green", count_type()),
            field("point", point2_type()),
            field("red", count_type()),
        ],
    )
}

pub fn vision_color_sample_type() -> StructuredInfoType {
    StructuredInfoType::variant(
        kind_id("vision/optional-color-sample@1"),
        vec![case("absent", unit_type()), case("rgb", rgb_sample_type())],
    )
    .expect("reviewed optional color sample")
}

pub fn vision_detection_type() -> StructuredInfoType {
    record(
        "vision/detection@1",
        vec![
            field("classification", text_type()),
            field("color_sample", vision_color_sample_type()),
            field("confidence", leaf(QUANTITY_INFO_ID)),
            field("image_identity", text_type()),
            field("landmarks", vision_landmarks_type()),
            field("provenance", vision_provenance_type()),
            field("region", rect2_type()),
        ],
    )
}

pub fn vision_detection_slot_type() -> StructuredInfoType {
    StructuredInfoType::variant(
        kind_id("vision/detection-slot@1"),
        vec![
            case("detection", vision_detection_type()),
            case("unused", unit_type()),
        ],
    )
    .expect("reviewed detection slot")
}

pub fn vision_detections_type() -> StructuredInfoType {
    StructuredInfoType::collection(
        vision_detection_slot_type(),
        Some(MAXIMUM_VISION_DETECTIONS),
    )
    .expect("bounded detection slots")
}

pub fn vision_registered_types() -> Vec<(&'static str, StructuredInfoType)> {
    vec![
        (IMAGE_RESOURCE_TYPE, image_resource_type()),
        (VISION_DETECTION_TYPE, vision_detection_type()),
        (VISION_DETECTIONS_TYPE, vision_detections_type()),
    ]
}
