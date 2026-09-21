//! Portable contracts for composing continuous Vision from replaceable branches.

use alloc::{vec, vec::Vec};
use conduit_core::{
    kind_id, port_id, KindId, PortDescriptor, PortDirection, PortTemporal, StructuredFieldType,
    StructuredInfoType, StructuredVariantCase,
};

use crate::{
    image_observation_reference_type, image_resource_type, local_vision_object_observations_type,
};

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
    StructuredInfoType::leaf(kind_id("value/text")).expect("reviewed text")
}

fn field(name: &str, value_type: StructuredInfoType) -> StructuredFieldType {
    StructuredFieldType::new(name, value_type).expect("reviewed vision field")
}

fn record(kind: &str, fields: Vec<StructuredFieldType>) -> StructuredInfoType {
    StructuredInfoType::record(kind_id(kind), fields).expect("reviewed vision record")
}

fn count() -> StructuredInfoType {
    StructuredInfoType::leaf(kind_id("value/count")).expect("reviewed count")
}

fn unit() -> StructuredInfoType {
    StructuredInfoType::leaf(kind_id("value/unit")).expect("reviewed unit")
}

fn case(name: &str, value_type: StructuredInfoType) -> StructuredVariantCase {
    StructuredVariantCase::new(name, value_type).expect("reviewed vision case")
}

fn sequence(value_type: StructuredInfoType, capacity: u16) -> StructuredInfoType {
    StructuredInfoType::sequence(value_type, capacity).expect("bounded vision sequence")
}

pub(crate) fn pixel_region_type() -> StructuredInfoType {
    record(
        "vision/pixel-region@1",
        vec![
            field("height", count()),
            field("width", count()),
            field("x", count()),
            field("y", count()),
        ],
    )
}

pub(crate) fn optional_pixel_region_type() -> StructuredInfoType {
    StructuredInfoType::variant(
        kind_id("vision/optional-pixel-region@1"),
        vec![case("absent", unit()), case("present", pixel_region_type())],
    )
    .expect("reviewed optional pixel region")
}

pub(crate) fn temporal_instant_type() -> StructuredInfoType {
    record(
        "time/temporal-instant@1",
        vec![
            field("clock_basis", text()),
            field("resolution_ticks", count()),
            field("scale", text()),
            field("ticks", count()),
            field("uncertainty_ticks", count()),
        ],
    )
}

pub(crate) fn evidence_class_type() -> StructuredInfoType {
    StructuredInfoType::variant(
        kind_id("vision/observation-evidence-class@1"),
        vec![
            case("deterministic_derived", unit()),
            case("model_derived", unit()),
            case("statistical_candidate", unit()),
        ],
    )
    .expect("reviewed visual evidence class")
}

pub(crate) fn observation_provenance_type() -> StructuredInfoType {
    record(
        "vision/observation-provenance@1",
        vec![
            field("artifact", text()),
            field("evidence_class", evidence_class_type()),
            field("implementation", text()),
            field("observation_sign", text()),
            field("observed_at", temporal_instant_type()),
            field("provider_instance", text()),
            field("run", text()),
        ],
    )
}

pub fn vision_motions_type() -> StructuredInfoType {
    crate::local_vision_motion_observations_type()
}

pub fn vision_texts_type() -> StructuredInfoType {
    sequence(visible_text_observation_type(), 8)
}

pub fn vision_tracks_type() -> StructuredInfoType {
    sequence(track_observation_type(), 4)
}

pub(crate) fn object_observation_type() -> StructuredInfoType {
    record(
        "vision/object-observation@1",
        vec![
            field("candidate_label", text()),
            field("confidence_permille", count()),
            field("provenance", observation_provenance_type()),
            field("region", pixel_region_type()),
            field("source_image", image_observation_reference_type()),
        ],
    )
}

pub(crate) fn motion_observation_type() -> StructuredInfoType {
    record(
        "vision/motion-observation@1",
        vec![
            field("change_permille", count()),
            field("changed_region", pixel_region_type()),
            field("provenance", observation_provenance_type()),
            field("source_image", image_observation_reference_type()),
        ],
    )
}

pub(crate) fn visible_text_observation_type() -> StructuredInfoType {
    record(
        "vision/visible-text-observation@1",
        vec![
            field("confidence_permille", count()),
            field("provenance", observation_provenance_type()),
            field("region", optional_pixel_region_type()),
            field("source_image", image_observation_reference_type()),
            field("text", text()),
        ],
    )
}

pub(crate) fn track_observation_type() -> StructuredInfoType {
    record(
        "vision/track-observation@1",
        vec![
            field("continuity_confidence_permille", count()),
            field("contributing_observation_signs", sequence(text(), 16)),
            field("current_region", pixel_region_type()),
            field("provenance", observation_provenance_type()),
            field("source_image", image_observation_reference_type()),
            field("track", text()),
            field("tracking_context", text()),
        ],
    )
}

pub fn visual_impression_type() -> StructuredInfoType {
    record(
        "vision/visual-impression@1",
        vec![
            field("disposition", impression_disposition_type()),
            field("model", text()),
            field("prompt_contract_revision", text()),
            field("provenance", observation_provenance_type()),
            field("selected_observation_signs", sequence(text(), 24)),
            field("source_image", image_observation_reference_type()),
            field("text", text()),
        ],
    )
}

pub(crate) fn impression_disposition_type() -> StructuredInfoType {
    StructuredInfoType::variant(
        kind_id("vision/visual-impression-disposition@1"),
        vec![case("complete", unit()), case("truncated", count())],
    )
    .expect("reviewed visual impression disposition")
}

pub fn visual_experience_type() -> StructuredInfoType {
    record(
        "vision/visual-experience@1",
        vec![
            field("observations", sequence(experience_observation_type(), 24)),
            field("relations", sequence(experience_relation_type(), 32)),
            field("source_image", image_observation_reference_type()),
        ],
    )
}

pub(crate) fn experience_observation_type() -> StructuredInfoType {
    StructuredInfoType::variant(
        kind_id("vision/experience-observation@1"),
        vec![
            case("impression", visual_impression_type()),
            case("motion", motion_observation_type()),
            case("object", object_observation_type()),
            case("track", track_observation_type()),
            case("visible_text", visible_text_observation_type()),
        ],
    )
    .expect("reviewed visual experience observation")
}

pub(crate) fn experience_relation_type() -> StructuredInfoType {
    record(
        "vision/experience-relation@1",
        vec![
            field("kind", text()),
            field("object_sign", text()),
            field("subject_sign", text()),
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
