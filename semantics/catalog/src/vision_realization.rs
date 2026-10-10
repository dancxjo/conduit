//! Deterministic image metadata and bounded detector fixture.

use alloc::{string::ToString, vec::Vec};
use conduit_core::{
    BoundedResourceRef, KindId, Quantity, ResourceClassId, ResourceExtent, ResourceLifetime,
    ResourceSemanticIdentity, ResourceVersionIdentity, StructuredInfoRefusal, StructuredInfoValue,
    Unit,
};
use conduit_plot::rust_binding::{NativeBindingRefusal, NativeRustBinding};
use conduit_presentation::{
    Extent2, ImageColorProfile, ImageFormat, ImagePixelExtent, ImageResource, Point2, Rect2,
    VisionColorSample, VisionDetectionSlot, VisionDetectionsFour, VisionEvidenceClass,
    VisionLandmarkSlot, VisionLandmarks, VisionProvenance,
};

use crate::{
    validate_confidence, VisionRefusal, MAXIMUM_VISION_LANDMARKS, VISION_IMAGE_ACCESS_CLASS,
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

impl From<NativeBindingRefusal> for VisionInfoRefusal {
    fn from(_: NativeBindingRefusal) -> Self {
        Self::MalformedInfo
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
    let image = ImageResource::new(
        ImageColorProfile::named("srgb".into())?,
        reference,
        ImagePixelExtent::new(48, 64)?,
        ImageFormat::rgba8(),
        IMAGE_FRAME.into(),
    )?
    .into_structured()?;
    Ok(VisionFixture { image })
}

pub fn deterministic_detect_image(
    image: &StructuredInfoValue,
) -> Result<StructuredInfoValue, VisionInfoRefusal> {
    validate_image(image)?;
    VisionDetectionsFour::new([
        detection_slot(
            "square",
            950_000,
            (100_000, 100_000, 400_000, 400_000),
            &[('a', 100_000, 100_000), ('b', 500_000, 500_000)],
            Some((220, 40, 30, 300_000, 300_000)),
        )?,
        detection_slot(
            "circle",
            875_000,
            (600_000, 200_000, 250_000, 250_000),
            &[('c', 725_000, 325_000)],
            None,
        )?,
        VisionDetectionSlot::unused(),
        VisionDetectionSlot::unused(),
    ])?
    .into_structured()
    .map_err(Into::into)
}

fn validate_image(image: &StructuredInfoValue) -> Result<(), VisionInfoRefusal> {
    let image = ImageResource::from_structured(image.clone())?;
    let reference = image.content();
    if reference.content_profile.as_str() != VISION_IMAGE_CONTENT_PROFILE
        || reference.access_class.as_str() != VISION_IMAGE_ACCESS_CLASS
    {
        return Err(VisionInfoRefusal::InvalidImageReference);
    }
    Ok(())
}

fn detection_slot(
    label: &str,
    confidence: i64,
    region: (i64, i64, i64, i64),
    landmarks: &[(char, i64, i64)],
    color: Option<(u64, u64, u64, i64, i64)>,
) -> Result<VisionDetectionSlot, VisionInfoRefusal> {
    let confidence = Quantity::new(confidence, Unit::Millionth);
    validate_confidence(confidence).map_err(VisionInfoRefusal::InvalidConfidence)?;
    VisionDetectionSlot::detection(
        label.into(),
        color_sample(color)?,
        confidence,
        IMAGE_IDENTITY.into(),
        landmark_slots(landmarks)?,
        VisionProvenance::new(
            VisionEvidenceClass::model_derived(),
            "vision/deterministic-shapes@1".into(),
            "fixture-1".into(),
            "fixture/shape-detector".into(),
        )?,
        rect(region)?,
    )
    .map_err(Into::into)
}

fn landmark_slots(landmarks: &[(char, i64, i64)]) -> Result<VisionLandmarks, VisionInfoRefusal> {
    if landmarks.len() > usize::from(MAXIMUM_VISION_LANDMARKS) {
        return Err(VisionInfoRefusal::MalformedInfo);
    }
    let mut slots = landmarks
        .iter()
        .map(|(name, x, y)| {
            VisionLandmarkSlot::keypoint(
                Quantity::new(900_000, Unit::Millionth),
                name.to_string(),
                point(*x, *y)?,
            )
            .map_err(Into::into)
        })
        .collect::<Result<Vec<_>, VisionInfoRefusal>>()?;
    slots.resize_with(
        usize::from(MAXIMUM_VISION_LANDMARKS),
        VisionLandmarkSlot::unused,
    );
    let slots: [VisionLandmarkSlot; 8] = slots
        .try_into()
        .map_err(|_| VisionInfoRefusal::MalformedInfo)?;
    VisionLandmarks::new(slots).map_err(Into::into)
}

fn color_sample(
    color: Option<(u64, u64, u64, i64, i64)>,
) -> Result<VisionColorSample, VisionInfoRefusal> {
    let Some((red, green, blue, x, y)) = color else {
        return Ok(VisionColorSample::absent());
    };
    let red = u8::try_from(red).map_err(|_| VisionInfoRefusal::MalformedInfo)?;
    let green = u8::try_from(green).map_err(|_| VisionInfoRefusal::MalformedInfo)?;
    let blue = u8::try_from(blue).map_err(|_| VisionInfoRefusal::MalformedInfo)?;
    VisionColorSample::rgb(blue, green, point(x, y)?, red).map_err(Into::into)
}

fn rect((x, y, width, height): (i64, i64, i64, i64)) -> Result<Rect2, VisionInfoRefusal> {
    Rect2::new(
        Extent2::new(
            Quantity::new(height, Unit::Millionth),
            Quantity::new(width, Unit::Millionth),
        )?,
        point(x, y)?,
    )
    .map_err(Into::into)
}

fn point(x: i64, y: i64) -> Result<Point2, VisionInfoRefusal> {
    Point2::new(
        IMAGE_FRAME.into(),
        Quantity::new(x, Unit::Millionth),
        Quantity::new(y, Unit::Millionth),
    )
    .map_err(Into::into)
}
