//! Portable contracts for composing continuous Vision from replaceable branches.

use alloc::{vec, vec::Vec};
use conduit_core::{
    kind_id, port_id, KindId, PortDescriptor, PortDirection, PortTemporal, StructuredFieldType,
    StructuredInfoType,
};

use crate::{image_resource_type, local_vision_object_observations_type};

pub const VISION_NORMALIZE_KIND: &str = "vision/normalize";
pub const VISION_MOTION_KIND: &str = "vision/local-motion";
pub const VISION_OBJECTS_KIND: &str = "vision/local-objects";
pub const VISION_OCR_KIND: &str = "vision/local-ocr";
pub const VISION_TRACK_KIND: &str = "vision/local-track";
pub const VISION_DESCRIBE_KIND: &str = "vision/model-describe";
pub const VISION_EXPERIENCE_KIND: &str = "vision/relate-experience";

pub const VISION_MOTIONS_TYPE: &str = "VisionMotionsFour";
pub const VISION_OBJECTS_TYPE: &str = "VisionLocalObjectObservationsFour";
pub const VISION_TEXTS_TYPE: &str = "VisionTextsEight";
pub const VISION_TRACKS_TYPE: &str = "VisionTracksFour";
pub const VISUAL_IMPRESSION_TYPE: &str = "VisualImpression";
pub const VISUAL_EXPERIENCE_TYPE: &str = "VisualExperience";

fn text() -> StructuredInfoType {
    StructuredInfoType::leaf(kind_id("value/text@1")).expect("reviewed text")
}

fn field(name: &str, value_type: StructuredInfoType) -> StructuredFieldType {
    StructuredFieldType::new(name, value_type).expect("reviewed vision field")
}

fn record(kind: &str, fields: Vec<StructuredFieldType>) -> StructuredInfoType {
    StructuredInfoType::record(kind_id(kind), fields).expect("reviewed vision record")
}

fn bounded_text_collection(kind: &str, maximum: u16) -> StructuredInfoType {
    record(
        kind,
        vec![field(
            "items",
            StructuredInfoType::collection(text(), Some(maximum))
                .expect("bounded vision collection"),
        )],
    )
}

pub fn vision_motions_type() -> StructuredInfoType {
    crate::local_vision_motion_observations_type()
}

pub fn vision_texts_type() -> StructuredInfoType {
    bounded_text_collection("vision/visible-texts@1", 8)
}

pub fn vision_tracks_type() -> StructuredInfoType {
    bounded_text_collection("vision/tracks@1", 4)
}

pub fn visual_impression_type() -> StructuredInfoType {
    record(
        "vision/visual-impression@1",
        vec![
            field("model", text()),
            field("source_image_identity", text()),
            field("text", text()),
        ],
    )
}

pub fn visual_experience_type() -> StructuredInfoType {
    record(
        "vision/visual-experience@1",
        vec![
            field("source_image_identity", text()),
            field(
                "observation_sign_ids",
                StructuredInfoType::collection(text(), Some(24))
                    .expect("bounded observation identities"),
            ),
            field("relation_revision", text()),
        ],
    )
}

pub fn vision_experience_registered_types() -> Vec<(&'static str, StructuredInfoType)> {
    vec![
        (VISION_MOTIONS_TYPE, vision_motions_type()),
        (
            crate::LOCAL_VISION_MOTION_OBSERVATION_TYPE,
            crate::local_vision_motion_observation_type(),
        ),
        (VISION_OBJECTS_TYPE, local_vision_object_observations_type()),
        (VISION_TEXTS_TYPE, vision_texts_type()),
        (VISION_TRACKS_TYPE, vision_tracks_type()),
        (VISUAL_IMPRESSION_TYPE, visual_impression_type()),
        (VISUAL_EXPERIENCE_TYPE, visual_experience_type()),
    ]
}

pub fn vision_experience_kind_contracts() -> Vec<(KindId, Vec<PortDescriptor>, Vec<PortDescriptor>)>
{
    let image = image_resource_type();
    let detections = local_vision_object_observations_type();
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
    }
}
