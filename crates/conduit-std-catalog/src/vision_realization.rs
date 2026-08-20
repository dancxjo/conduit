//! Deterministic image metadata and bounded detector fixture.

use alloc::{string::ToString, vec, vec::Vec};
use conduit_core::{
    BoundedResourceRef, KindId, Quantity, QuantityUnit, ResourceClassId, ResourceExtent,
    ResourceLifetime, ResourceSemanticIdentity, ResourceVersionIdentity, StructuredFieldValue,
    StructuredInfoRefusal, StructuredInfoType, StructuredInfoTypeShape, StructuredInfoValue,
    StructuredInfoValueShape,
};

use crate::{
    extent2_type, image_color_profile_type, image_format_type, image_resource_type, point2_type,
    rect2_type, validate_confidence, vision_color_sample_type, vision_detection_slot_type,
    vision_detection_type, vision_detections_type, vision_evidence_class_type,
    vision_keypoint_type, vision_landmark_slot_type, vision_landmarks_type, vision_provenance_type,
    VisionRefusal, MAXIMUM_VISION_DETECTIONS, MAXIMUM_VISION_LANDMARKS, VISION_IMAGE_ACCESS_CLASS,
    VISION_IMAGE_CONTENT_PROFILE,
};

const IMAGE_IDENTITY: &str = "image/checkerboard-v1";
const IMAGE_FRAME: &str = "image/checkerboard-v1/normalized";

pub struct VisionFixture {
    pub image: StructuredInfoValue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VisionInfoRefusal {
    MalformedInfo,
    InvalidImageReference,
    InvalidConfidence(VisionRefusal),
    Structured(StructuredInfoRefusal),
}

impl From<StructuredInfoRefusal> for VisionInfoRefusal {
    fn from(value: StructuredInfoRefusal) -> Self {
        Self::Structured(value)
    }
}

pub fn deterministic_vision_fixture() -> Result<VisionFixture, VisionInfoRefusal> {
    let reference = BoundedResourceRef {
        identity: ResourceSemanticIdentity::from_digest([0x11; 32]),
        content_profile: KindId::from(VISION_IMAGE_CONTENT_PROFILE),
        access_class: ResourceClassId::from(VISION_IMAGE_ACCESS_CLASS),
        extent: ResourceExtent {
            bytes: 12_288,
            items: Some(3_072),
        },
        lifetime: ResourceLifetime {
            version: ResourceVersionIdentity::from_digest([0x22; 32]),
            expires_at: None,
        },
    };
    reference
        .validate()
        .map_err(|_| VisionInfoRefusal::InvalidImageReference)?;
    let image = record_value(
        image_resource_type(),
        vec![
            (
                "color_profile",
                StructuredInfoValue::variant(
                    image_color_profile_type(),
                    "named",
                    text_value("srgb"),
                )?,
            ),
            (
                "content",
                leaf_value(
                    conduit_core::RESOURCE_REFERENCE_INFO_ID,
                    reference
                        .encode()
                        .map_err(|_| VisionInfoRefusal::InvalidImageReference)?,
                )?,
            ),
            (
                "extent",
                record_value(
                    crate::image_pixel_extent_type(),
                    vec![("height", count_value(48)), ("width", count_value(64))],
                )?,
            ),
            ("format", unit_variant(image_format_type(), "rgba8")?),
            ("image_frame", text_value(IMAGE_FRAME)),
        ],
    )?;
    Ok(VisionFixture { image })
}

pub fn deterministic_detect_image(
    image: &StructuredInfoValue,
) -> Result<StructuredInfoValue, VisionInfoRefusal> {
    validate_image(image)?;
    let first = detection_value(
        "square",
        950_000,
        (100_000, 100_000, 400_000, 400_000),
        &[('a', 100_000, 100_000), ('b', 500_000, 500_000)],
        Some((220, 40, 30, 300_000, 300_000)),
    )?;
    let second = detection_value(
        "circle",
        875_000,
        (600_000, 200_000, 250_000, 250_000),
        &[('c', 725_000, 325_000)],
        None,
    )?;
    let slot_type = vision_detection_slot_type();
    let mut detections = vec![
        StructuredInfoValue::variant(slot_type.clone(), "detection", first)?,
        StructuredInfoValue::variant(slot_type.clone(), "detection", second)?,
    ];
    while detections.len() < usize::from(MAXIMUM_VISION_DETECTIONS) {
        detections.push(unit_variant(slot_type.clone(), "unused")?);
    }
    Ok(StructuredInfoValue::collection(
        vision_detections_type(),
        detections,
    )?)
}

fn validate_image(image: &StructuredInfoValue) -> Result<(), VisionInfoRefusal> {
    if image.value_type() != &image_resource_type() {
        return Err(VisionInfoRefusal::MalformedInfo);
    }
    let reference = BoundedResourceRef::decode(leaf_bytes(record_field(image, "content")?)?)
        .map_err(|_| VisionInfoRefusal::InvalidImageReference)?;
    if reference.content_profile.as_str() != VISION_IMAGE_CONTENT_PROFILE
        || reference.access_class.as_str() != VISION_IMAGE_ACCESS_CLASS
    {
        return Err(VisionInfoRefusal::InvalidImageReference);
    }
    Ok(())
}

fn detection_value(
    label: &str,
    confidence: i64,
    region: (i64, i64, i64, i64),
    landmarks: &[(char, i64, i64)],
    color: Option<(u64, u64, u64, i64, i64)>,
) -> Result<StructuredInfoValue, VisionInfoRefusal> {
    let confidence = Quantity::new(confidence, QuantityUnit::Millionth);
    validate_confidence(confidence).map_err(VisionInfoRefusal::InvalidConfidence)?;
    record_value(
        vision_detection_type(),
        vec![
            ("classification", text_value(label)),
            ("color_sample", color_sample_value(color)?),
            ("confidence", quantity_value(confidence)?),
            ("image_identity", text_value(IMAGE_IDENTITY)),
            ("landmarks", landmarks_value(landmarks)?),
            ("provenance", provenance_value()?),
            ("region", rect_value(region)?),
        ],
    )
}

fn provenance_value() -> Result<StructuredInfoValue, VisionInfoRefusal> {
    record_value(
        vision_provenance_type(),
        vec![
            (
                "evidence_class",
                unit_variant(vision_evidence_class_type(), "model_derived")?,
            ),
            ("profile", text_value("vision/deterministic-shapes@1")),
            ("revision", text_value("fixture-1")),
            ("source", text_value("fixture/shape-detector")),
        ],
    )
}

fn landmarks_value(
    landmarks: &[(char, i64, i64)],
) -> Result<StructuredInfoValue, VisionInfoRefusal> {
    if landmarks.len() > usize::from(MAXIMUM_VISION_LANDMARKS) {
        return Err(VisionInfoRefusal::MalformedInfo);
    }
    let slot_type = vision_landmark_slot_type();
    let mut slots = landmarks
        .iter()
        .map(|(name, x, y)| {
            let confidence = Quantity::new(900_000, QuantityUnit::Millionth);
            let keypoint = record_value(
                vision_keypoint_type(),
                vec![
                    ("confidence", quantity_value(confidence)?),
                    ("name", text_value(&name.to_string())),
                    ("point", point_value(*x, *y)?),
                ],
            )?;
            Ok(StructuredInfoValue::variant(
                slot_type.clone(),
                "keypoint",
                keypoint,
            )?)
        })
        .collect::<Result<Vec<_>, VisionInfoRefusal>>()?;
    while slots.len() < usize::from(MAXIMUM_VISION_LANDMARKS) {
        slots.push(unit_variant(slot_type.clone(), "unused")?);
    }
    Ok(StructuredInfoValue::collection(
        vision_landmarks_type(),
        slots,
    )?)
}

fn color_sample_value(
    color: Option<(u64, u64, u64, i64, i64)>,
) -> Result<StructuredInfoValue, VisionInfoRefusal> {
    let value_type = vision_color_sample_type();
    let Some((red, green, blue, x, y)) = color else {
        return unit_variant(value_type, "absent");
    };
    for channel in [red, green, blue] {
        if channel > 255 {
            return Err(VisionInfoRefusal::MalformedInfo);
        }
    }
    let payload_type = variant_payload_type(&value_type, "rgb")?;
    Ok(StructuredInfoValue::variant(
        value_type,
        "rgb",
        record_value(
            payload_type,
            vec![
                ("blue", count_value(blue)),
                ("green", count_value(green)),
                ("point", point_value(x, y)?),
                ("red", count_value(red)),
            ],
        )?,
    )?)
}

fn rect_value(
    (x, y, width, height): (i64, i64, i64, i64),
) -> Result<StructuredInfoValue, VisionInfoRefusal> {
    record_value(
        rect2_type(),
        vec![
            (
                "extent",
                record_value(
                    extent2_type(),
                    vec![
                        (
                            "height",
                            quantity_value(Quantity::new(height, QuantityUnit::Millionth))?,
                        ),
                        (
                            "width",
                            quantity_value(Quantity::new(width, QuantityUnit::Millionth))?,
                        ),
                    ],
                )?,
            ),
            ("origin", point_value(x, y)?),
        ],
    )
}

fn point_value(x: i64, y: i64) -> Result<StructuredInfoValue, VisionInfoRefusal> {
    record_value(
        point2_type(),
        vec![
            ("frame", text_value(IMAGE_FRAME)),
            (
                "x",
                quantity_value(Quantity::new(x, QuantityUnit::Millionth))?,
            ),
            (
                "y",
                quantity_value(Quantity::new(y, QuantityUnit::Millionth))?,
            ),
        ],
    )
}

fn quantity_value(value: Quantity) -> Result<StructuredInfoValue, VisionInfoRefusal> {
    leaf_value(conduit_core::QUANTITY_INFO_ID, value.encode().to_vec())
}

fn unit_variant(
    value_type: StructuredInfoType,
    tag: &str,
) -> Result<StructuredInfoValue, VisionInfoRefusal> {
    Ok(StructuredInfoValue::variant(
        value_type,
        tag,
        leaf_value("value/unit@1", Vec::new())?,
    )?)
}

fn text_value(value: &str) -> StructuredInfoValue {
    StructuredInfoValue::leaf(
        StructuredInfoType::leaf(conduit_core::kind_id("value/text@1")).unwrap(),
        value.as_bytes().to_vec(),
    )
    .expect("bounded deterministic vision text")
}

fn count_value(value: u64) -> StructuredInfoValue {
    StructuredInfoValue::leaf(
        StructuredInfoType::leaf(conduit_core::kind_id("value/count@1")).unwrap(),
        value.to_string().into_bytes(),
    )
    .expect("bounded deterministic vision count")
}

fn leaf_value(kind: &str, bytes: Vec<u8>) -> Result<StructuredInfoValue, VisionInfoRefusal> {
    Ok(StructuredInfoValue::leaf(
        StructuredInfoType::leaf(conduit_core::kind_id(kind))?,
        bytes,
    )?)
}

fn record_value(
    value_type: StructuredInfoType,
    fields: Vec<(&str, StructuredInfoValue)>,
) -> Result<StructuredInfoValue, VisionInfoRefusal> {
    Ok(StructuredInfoValue::record(
        value_type,
        fields
            .into_iter()
            .map(|(name, value)| StructuredFieldValue::new(name, value))
            .collect::<Result<Vec<_>, _>>()?,
    )?)
}

fn record_field<'a>(
    value: &'a StructuredInfoValue,
    name: &str,
) -> Result<&'a StructuredInfoValue, VisionInfoRefusal> {
    let StructuredInfoValueShape::Record(fields) = value.shape() else {
        return Err(VisionInfoRefusal::MalformedInfo);
    };
    fields
        .iter()
        .find(|field| field.name() == name)
        .map(StructuredFieldValue::value)
        .ok_or(VisionInfoRefusal::MalformedInfo)
}

fn variant_payload_type(
    value_type: &StructuredInfoType,
    tag: &str,
) -> Result<StructuredInfoType, VisionInfoRefusal> {
    let StructuredInfoTypeShape::Variant { cases, .. } = value_type.shape() else {
        return Err(VisionInfoRefusal::MalformedInfo);
    };
    cases
        .iter()
        .find(|case| case.tag() == tag)
        .map(|case| case.payload_type().clone())
        .ok_or(VisionInfoRefusal::MalformedInfo)
}

fn leaf_bytes(value: &StructuredInfoValue) -> Result<&[u8], VisionInfoRefusal> {
    let StructuredInfoValueShape::Leaf(bytes) = value.shape() else {
        return Err(VisionInfoRefusal::MalformedInfo);
    };
    Ok(bytes)
}
