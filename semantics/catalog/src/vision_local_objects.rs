//! Bounded local-CV adapter validation.
//!
//! Portable object and motion values are owned by the Vision experience family.
//! This module retains only preparation-time validation for the hosted adapter.

use conduit_core::StructuredInfoRefusal;

use crate::PixelRegion;

mod prepared;
pub use prepared::PreparedLocalVisionObjectEncoder;

pub const MAXIMUM_LOCAL_VISION_OBJECT_OBSERVATIONS: u16 = 4;
pub const MAXIMUM_LOCAL_VISION_IDENTITY_BYTES: usize = 256;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LocalVisionObservationRefusal {
    InvalidIdentity,
    MalformedImage,
    InvalidObservation,
    Structured(StructuredInfoRefusal),
}

impl From<StructuredInfoRefusal> for LocalVisionObservationRefusal {
    fn from(value: StructuredInfoRefusal) -> Self {
        Self::Structured(value)
    }
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

pub(super) fn validate_identity(identity: &str) -> Result<(), LocalVisionObservationRefusal> {
    if identity.is_empty() || identity.len() > MAXIMUM_LOCAL_VISION_IDENTITY_BYTES {
        Err(LocalVisionObservationRefusal::InvalidIdentity)
    } else {
        Ok(())
    }
}
