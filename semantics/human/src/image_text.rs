//! Portable finite composition of one semantic image reference and text.

use alloc::{string::String, vec::Vec};
use conduit_core::{semantic_digest, BoundedResourceRef, KindId};

pub const MAXIMUM_IMAGE_TEXT_CAPTION_BYTES: usize = 512;
pub const MAXIMUM_IMAGE_OBSERVATION_WIDTH: u16 = 4_096;
pub const MAXIMUM_IMAGE_OBSERVATION_HEIGHT: u16 = 4_096;
pub const MAXIMUM_IMAGE_OBSERVATION_BYTES: u64 = 16 * 1024 * 1024;
pub const MAXIMUM_IMAGE_TEXT_METADATA_ENTRIES: usize = 8;
pub const MAXIMUM_IMAGE_TEXT_METADATA_KEY_BYTES: usize = 64;
pub const MAXIMUM_IMAGE_TEXT_METADATA_VALUE_BYTES: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageTextMetadata {
    pub key: String,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageObservationReference {
    pub content: BoundedResourceRef,
    pub width: u16,
    pub height: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImageTextRecord {
    pub image: ImageObservationReference,
    pub caption: String,
    pub metadata: Vec<ImageTextMetadata>,
    pub content_digest: [u8; 32],
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ImageTextRefusal {
    InvalidImage,
    WrongImageProfile,
    InvalidImageDimensions,
    ImageTooLarge,
    EmptyCaption,
    CaptionTooLarge,
    TooManyMetadataEntries,
    EmptyMetadataKey,
    MetadataKeyTooLarge,
    MetadataValueTooLarge,
    DuplicateMetadataKey,
    IntegrityMismatch,
}

pub fn compose_image_text(
    expected_image_profile: &KindId,
    image: ImageObservationReference,
    caption: String,
    metadata: Vec<ImageTextMetadata>,
) -> Result<ImageTextRecord, ImageTextRefusal> {
    image
        .content
        .validate()
        .map_err(|_| ImageTextRefusal::InvalidImage)?;
    if &image.content.content_profile != expected_image_profile {
        return Err(ImageTextRefusal::WrongImageProfile);
    }
    if image.width == 0
        || image.height == 0
        || image.width > MAXIMUM_IMAGE_OBSERVATION_WIDTH
        || image.height > MAXIMUM_IMAGE_OBSERVATION_HEIGHT
    {
        return Err(ImageTextRefusal::InvalidImageDimensions);
    }
    if image.content.extent.bytes > MAXIMUM_IMAGE_OBSERVATION_BYTES {
        return Err(ImageTextRefusal::ImageTooLarge);
    }
    if caption.is_empty() {
        return Err(ImageTextRefusal::EmptyCaption);
    }
    if caption.len() > MAXIMUM_IMAGE_TEXT_CAPTION_BYTES {
        return Err(ImageTextRefusal::CaptionTooLarge);
    }
    if metadata.len() > MAXIMUM_IMAGE_TEXT_METADATA_ENTRIES {
        return Err(ImageTextRefusal::TooManyMetadataEntries);
    }
    for (index, entry) in metadata.iter().enumerate() {
        if entry.key.is_empty() {
            return Err(ImageTextRefusal::EmptyMetadataKey);
        }
        if entry.key.len() > MAXIMUM_IMAGE_TEXT_METADATA_KEY_BYTES {
            return Err(ImageTextRefusal::MetadataKeyTooLarge);
        }
        if entry.value.len() > MAXIMUM_IMAGE_TEXT_METADATA_VALUE_BYTES {
            return Err(ImageTextRefusal::MetadataValueTooLarge);
        }
        if metadata[..index].iter().any(|prior| prior.key == entry.key) {
            return Err(ImageTextRefusal::DuplicateMetadataKey);
        }
    }
    let content_digest = digest(&image, &caption, &metadata);
    Ok(ImageTextRecord {
        image,
        caption,
        metadata,
        content_digest,
    })
}

impl ImageTextRecord {
    pub fn validate(&self, expected_image_profile: &KindId) -> Result<(), ImageTextRefusal> {
        let recomposed = compose_image_text(
            expected_image_profile,
            self.image.clone(),
            self.caption.clone(),
            self.metadata.clone(),
        )?;
        if recomposed.content_digest != self.content_digest {
            return Err(ImageTextRefusal::IntegrityMismatch);
        }
        Ok(())
    }
}

fn digest(
    image: &ImageObservationReference,
    caption: &str,
    metadata: &[ImageTextMetadata],
) -> [u8; 32] {
    let mut bytes = image
        .content
        .encode()
        .expect("the validated image reference remains encodable");
    bytes.extend_from_slice(&image.width.to_le_bytes());
    bytes.extend_from_slice(&image.height.to_le_bytes());
    bytes.extend_from_slice(&(caption.len() as u64).to_le_bytes());
    bytes.extend_from_slice(caption.as_bytes());
    bytes.extend_from_slice(&(metadata.len() as u64).to_le_bytes());
    for entry in metadata {
        bytes.extend_from_slice(&(entry.key.len() as u64).to_le_bytes());
        bytes.extend_from_slice(entry.key.as_bytes());
        bytes.extend_from_slice(&(entry.value.len() as u64).to_le_bytes());
        bytes.extend_from_slice(entry.value.as_bytes());
    }
    semantic_digest("human/image-text-record@1", &bytes)
}
