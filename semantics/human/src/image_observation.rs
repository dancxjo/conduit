//! Finite semantic image observations over separately realized content.

use conduit_core::{BoundedResourceRef, KindId};

pub const MAXIMUM_IMAGE_OBSERVATION_WIDTH: u16 = 4_096;
pub const MAXIMUM_IMAGE_OBSERVATION_HEIGHT: u16 = 4_096;
pub const MAXIMUM_IMAGE_OBSERVATION_BYTES: u64 = 16 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageObservationReference {
    pub content: BoundedResourceRef,
    pub width: u16,
    pub height: u16,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ImageObservationRefusal {
    InvalidResource,
    WrongProfile,
    InvalidDimensions,
    ContentTooLarge,
}

impl ImageObservationReference {
    pub fn new(
        content: BoundedResourceRef,
        width: u16,
        height: u16,
        expected_profile: &KindId,
    ) -> Result<Self, ImageObservationRefusal> {
        let observation = Self {
            content,
            width,
            height,
        };
        observation.validate(expected_profile)?;
        Ok(observation)
    }

    pub fn validate(&self, expected_profile: &KindId) -> Result<(), ImageObservationRefusal> {
        self.content
            .validate()
            .map_err(|_| ImageObservationRefusal::InvalidResource)?;
        if &self.content.content_profile != expected_profile {
            return Err(ImageObservationRefusal::WrongProfile);
        }
        if self.width == 0
            || self.height == 0
            || self.width > MAXIMUM_IMAGE_OBSERVATION_WIDTH
            || self.height > MAXIMUM_IMAGE_OBSERVATION_HEIGHT
        {
            return Err(ImageObservationRefusal::InvalidDimensions);
        }
        if self.content.extent.bytes > MAXIMUM_IMAGE_OBSERVATION_BYTES {
            return Err(ImageObservationRefusal::ContentTooLarge);
        }
        Ok(())
    }
}
