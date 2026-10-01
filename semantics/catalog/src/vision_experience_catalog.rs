//! Portable contracts for composing continuous Vision from replaceable branches.

use alloc::{vec, vec::Vec};
use conduit_core::{
    kind_id, port_id, KindId, PortDescriptor, PortDirection, PortTemporal, StructuredInfoType,
};
use conduit_form::rust_binding::NativeRustBinding;
use conduit_presentation::{
    VisionMotionsFour, VisionObjectObservations, VisionTextsEight, VisionTracksFour,
    VisualExperience, VisualImpression,
};

use crate::image_resource_type;

pub const VISION_NORMALIZE_KIND: &str = "vision/normalize";
pub const VISION_MOTION_KIND: &str = "vision/local-motion";
pub const VISION_OBJECTS_KIND: &str = "vision/local-objects";
pub const VISION_OCR_KIND: &str = "vision/local-ocr";
pub const VISION_TRACK_KIND: &str = "vision/local-track";
pub const VISION_DESCRIBE_KIND: &str = "vision/model-describe";
pub const VISION_EXPERIENCE_KIND: &str = "vision/relate-experience";

pub const VISION_MOTIONS_TYPE: &str = "VisionMotionsFour";
pub const VISION_OBJECTS_TYPE: &str = "VisionObjectObservations";
pub const VISION_TEXTS_TYPE: &str = "VisionTextsEight";
pub const VISION_TRACKS_TYPE: &str = "VisionTracksFour";
pub const VISUAL_IMPRESSION_TYPE: &str = "VisualImpression";
pub const VISUAL_EXPERIENCE_TYPE: &str = "VisualExperience";

fn native_type<T: NativeRustBinding>() -> StructuredInfoType {
    T::semantic_type().expect("checked native Vision Type")
}

pub fn vision_motions_type() -> StructuredInfoType {
    native_type::<VisionMotionsFour>()
}

pub fn vision_objects_type() -> StructuredInfoType {
    native_type::<VisionObjectObservations>()
}

pub fn vision_texts_type() -> StructuredInfoType {
    native_type::<VisionTextsEight>()
}

pub fn vision_tracks_type() -> StructuredInfoType {
    native_type::<VisionTracksFour>()
}

pub fn visual_impression_type() -> StructuredInfoType {
    native_type::<VisualImpression>()
}

pub fn visual_experience_type() -> StructuredInfoType {
    native_type::<VisualExperience>()
}

pub fn vision_experience_registered_types() -> Vec<(&'static str, StructuredInfoType)> {
    vec![
        (VISION_MOTIONS_TYPE, vision_motions_type()),
        (VISION_OBJECTS_TYPE, vision_objects_type()),
        (VISION_TEXTS_TYPE, vision_texts_type()),
        (VISION_TRACKS_TYPE, vision_tracks_type()),
        (VISUAL_IMPRESSION_TYPE, visual_impression_type()),
        (VISUAL_EXPERIENCE_TYPE, visual_experience_type()),
    ]
}

pub fn vision_experience_kind_contracts() -> Vec<(KindId, Vec<PortDescriptor>, Vec<PortDescriptor>)>
{
    let image = image_resource_type();
    let detections = vision_objects_type();
    let motions = vision_motions_type();
    let texts = vision_texts_type();
    let tracks = vision_tracks_type();
    let impression = visual_impression_type();
    vec![
        contract(
            VISION_NORMALIZE_KIND,
            vec![port("image", &image, PortDirection::Input)],
            vec![port("image", &image, PortDirection::Output)],
        ),
        contract(
            VISION_MOTION_KIND,
            vec![port("image", &image, PortDirection::Input)],
            vec![port("motions", &motions, PortDirection::Output)],
        ),
        contract(
            VISION_OBJECTS_KIND,
            vec![port("image", &image, PortDirection::Input)],
            vec![port("detections", &detections, PortDirection::Output)],
        ),
        contract(
            VISION_OCR_KIND,
            vec![port("image", &image, PortDirection::Input)],
            vec![port("texts", &texts, PortDirection::Output)],
        ),
        contract(
            VISION_TRACK_KIND,
            vec![port("detections", &detections, PortDirection::Input)],
            vec![port("tracks", &tracks, PortDirection::Output)],
        ),
        contract(
            VISION_DESCRIBE_KIND,
            vec![
                port("image", &image, PortDirection::Input),
                port("detections", &detections, PortDirection::Input),
                port("texts", &texts, PortDirection::Input),
                port("tracks", &tracks, PortDirection::Input),
            ],
            vec![port("impression", &impression, PortDirection::Output)],
        ),
        contract(
            VISION_EXPERIENCE_KIND,
            vec![
                port("detections", &detections, PortDirection::Input),
                port("impression", &impression, PortDirection::Input),
                port("motions", &motions, PortDirection::Input),
                port("texts", &texts, PortDirection::Input),
                port("tracks", &tracks, PortDirection::Input),
            ],
            vec![port(
                "experience",
                &visual_experience_type(),
                PortDirection::Output,
            )],
        ),
    ]
}

fn contract(
    kind: &str,
    inputs: Vec<PortDescriptor>,
    outputs: Vec<PortDescriptor>,
) -> (KindId, Vec<PortDescriptor>, Vec<PortDescriptor>) {
    (kind_id(kind), inputs, outputs)
}

fn port(name: &str, value_type: &StructuredInfoType, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: value_type
            .profile()
            .expect("reviewed vision profile")
            .value_kind()
            .clone(),
        direction,
        temporal: PortTemporal::Current,
        abnormal_kind: None,
    }
}
