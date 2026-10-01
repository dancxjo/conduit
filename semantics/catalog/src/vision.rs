//! Portable bounded image metadata and model-derived vision Info.
//!
//! Conduitese owns the family. This module keeps the catalog's stable names and
//! the contextual confidence rule used by vision realizations.

use alloc::{vec, vec::Vec};
use conduit_core::{Quantity, QuantityDimension, StructuredInfoType};
use conduit_form::rust_binding::NativeRustBinding;
use conduit_presentation::{
    ImageColorProfile, ImageFormat, ImagePixelExtent, ImageResource, VisionColorSample,
    VisionDetection, VisionDetectionSlot, VisionDetectionsFour, VisionEvidenceClass,
    VisionKeypoint, VisionLandmarkSlot, VisionLandmarks, VisionProvenance, VisionRgbSample,
};

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

fn native_type<T: NativeRustBinding>() -> StructuredInfoType {
    T::semantic_type().expect("checked native vision Type")
}

pub fn image_pixel_extent_type() -> StructuredInfoType {
    native_type::<ImagePixelExtent>()
}
pub fn image_format_type() -> StructuredInfoType {
    native_type::<ImageFormat>()
}
pub fn image_color_profile_type() -> StructuredInfoType {
    native_type::<ImageColorProfile>()
}
pub fn image_resource_type() -> StructuredInfoType {
    native_type::<ImageResource>()
}
pub fn vision_evidence_class_type() -> StructuredInfoType {
    native_type::<VisionEvidenceClass>()
}
pub fn vision_provenance_type() -> StructuredInfoType {
    native_type::<VisionProvenance>()
}
pub fn vision_keypoint_type() -> StructuredInfoType {
    native_type::<VisionKeypoint>()
}
pub fn vision_landmark_slot_type() -> StructuredInfoType {
    native_type::<VisionLandmarkSlot>()
}
pub fn vision_landmarks_type() -> StructuredInfoType {
    native_type::<VisionLandmarks>()
}
pub fn vision_color_sample_type() -> StructuredInfoType {
    native_type::<VisionColorSample>()
}
pub fn vision_detection_type() -> StructuredInfoType {
    native_type::<VisionDetection>()
}
pub fn vision_detection_slot_type() -> StructuredInfoType {
    native_type::<VisionDetectionSlot>()
}
pub fn vision_detections_type() -> StructuredInfoType {
    native_type::<VisionDetectionsFour>()
}

#[allow(dead_code)]
fn rgb_sample_type() -> StructuredInfoType {
    native_type::<VisionRgbSample>()
}

pub fn vision_registered_types() -> Vec<(&'static str, StructuredInfoType)> {
    vec![
        (IMAGE_RESOURCE_TYPE, image_resource_type()),
        (VISION_DETECTION_TYPE, vision_detection_type()),
        (VISION_DETECTIONS_TYPE, vision_detections_type()),
    ]
}
