//! Bounded, versioned interchange encoding for image-and-text records.

use alloc::vec::Vec;
use conduit_core::{BoundedResourceRef, KindId, MAXIMUM_RESOURCE_REFERENCE_ENCODED_BYTES};

use crate::{
    ImageTextMetadata, ImageTextRecord, ImageTextRefusal, MAXIMUM_IMAGE_TEXT_CAPTION_BYTES,
    MAXIMUM_IMAGE_TEXT_METADATA_ENTRIES, MAXIMUM_IMAGE_TEXT_METADATA_KEY_BYTES,
    MAXIMUM_IMAGE_TEXT_METADATA_VALUE_BYTES,
};

pub const IMAGE_TEXT_ENCODING_VERSION: u8 = 1;
pub const MAXIMUM_IMAGE_TEXT_ENCODED_BYTES: usize = 1
    + 2
    + MAXIMUM_RESOURCE_REFERENCE_ENCODED_BYTES
    + 2
    + MAXIMUM_IMAGE_TEXT_CAPTION_BYTES
    + 1
    + MAXIMUM_IMAGE_TEXT_METADATA_ENTRIES
        * (1 + MAXIMUM_IMAGE_TEXT_METADATA_KEY_BYTES + 2 + MAXIMUM_IMAGE_TEXT_METADATA_VALUE_BYTES)
    + 32;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ImageTextCodecRefusal {
    InvalidRecord(ImageTextRefusal),
    OutputTooSmall,
    EncodingTooLarge,
    Malformed,
    UnsupportedVersion,
}

impl ImageTextRecord {
    pub fn encode_into(
        &self,
        expected_image_profile: &KindId,
        output: &mut [u8],
    ) -> Result<usize, ImageTextCodecRefusal> {
        self.validate(expected_image_profile)
            .map_err(ImageTextCodecRefusal::InvalidRecord)?;
        let image = self
            .image
            .encode()
            .map_err(|_| ImageTextCodecRefusal::InvalidRecord(ImageTextRefusal::InvalidImage))?;
        let required = 1
            + 2
            + image.len()
            + 2
            + self.caption.len()
            + 1
            + self
                .metadata
                .iter()
                .map(|entry| 1 + entry.key.len() + 2 + entry.value.len())
                .sum::<usize>()
            + self.content_digest.len();
        if required > MAXIMUM_IMAGE_TEXT_ENCODED_BYTES {
            return Err(ImageTextCodecRefusal::EncodingTooLarge);
        }
        if output.len() < required {
            return Err(ImageTextCodecRefusal::OutputTooSmall);
        }

        let mut writer = Writer::new(output);
        writer.u8(IMAGE_TEXT_ENCODING_VERSION);
        writer.bytes_u16(&image);
        writer.bytes_u16(self.caption.as_bytes());
        writer.u8(self.metadata.len() as u8);
        for entry in &self.metadata {
            writer.bytes_u8(entry.key.as_bytes());
            writer.bytes_u16(entry.value.as_bytes());
        }
        writer.bytes(&self.content_digest);
        Ok(writer.written())
    }

    pub fn decode(
        expected_image_profile: &KindId,
        encoded: &[u8],
    ) -> Result<Self, ImageTextCodecRefusal> {
        if encoded.len() > MAXIMUM_IMAGE_TEXT_ENCODED_BYTES {
            return Err(ImageTextCodecRefusal::EncodingTooLarge);
        }
        let mut cursor = Cursor::new(encoded);
        if cursor.u8()? != IMAGE_TEXT_ENCODING_VERSION {
            return Err(ImageTextCodecRefusal::UnsupportedVersion);
        }
        let image = BoundedResourceRef::decode(cursor.bytes_u16()?)
            .map_err(|_| ImageTextCodecRefusal::InvalidRecord(ImageTextRefusal::InvalidImage))?;
        let caption = cursor.text_u16()?.into();
        let count = usize::from(cursor.u8()?);
        if count > MAXIMUM_IMAGE_TEXT_METADATA_ENTRIES {
            return Err(ImageTextCodecRefusal::InvalidRecord(
                ImageTextRefusal::TooManyMetadataEntries,
            ));
        }
        let mut metadata = Vec::with_capacity(count);
        for _ in 0..count {
            metadata.push(ImageTextMetadata {
                key: cursor.text_u8()?.into(),
                value: cursor.text_u16()?.into(),
            });
        }
        let mut content_digest = [0; 32];
        content_digest.copy_from_slice(cursor.bytes(32)?);
        if !cursor.finished() {
            return Err(ImageTextCodecRefusal::Malformed);
        }
        let record = Self {
            image,
            caption,
            metadata,
            content_digest,
        };
        record
            .validate(expected_image_profile)
            .map_err(ImageTextCodecRefusal::InvalidRecord)?;
        Ok(record)
    }
}

struct Writer<'a> {
    output: &'a mut [u8],
    offset: usize,
}

impl<'a> Writer<'a> {
    fn new(output: &'a mut [u8]) -> Self {
        Self { output, offset: 0 }
    }

    fn u8(&mut self, value: u8) {
        self.output[self.offset] = value;
        self.offset += 1;
    }

    fn bytes(&mut self, value: &[u8]) {
        let end = self.offset + value.len();
        self.output[self.offset..end].copy_from_slice(value);
        self.offset = end;
    }

    fn bytes_u8(&mut self, value: &[u8]) {
        self.u8(value.len() as u8);
        self.bytes(value);
    }

    fn bytes_u16(&mut self, value: &[u8]) {
        self.bytes(&(value.len() as u16).to_le_bytes());
        self.bytes(value);
    }

    fn written(&self) -> usize {
        self.offset
    }
}

struct Cursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}

impl<'a> Cursor<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self { bytes, offset: 0 }
    }

    fn u8(&mut self) -> Result<u8, ImageTextCodecRefusal> {
        Ok(self.bytes(1)?[0])
    }

    fn bytes(&mut self, length: usize) -> Result<&'a [u8], ImageTextCodecRefusal> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(ImageTextCodecRefusal::Malformed)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(ImageTextCodecRefusal::Malformed)?;
        self.offset = end;
        Ok(value)
    }

    fn bytes_u8(&mut self) -> Result<&'a [u8], ImageTextCodecRefusal> {
        let length = usize::from(self.u8()?);
        self.bytes(length)
    }

    fn bytes_u16(&mut self) -> Result<&'a [u8], ImageTextCodecRefusal> {
        let raw = self.bytes(2)?;
        self.bytes(usize::from(u16::from_le_bytes([raw[0], raw[1]])))
    }

    fn text_u8(&mut self) -> Result<&'a str, ImageTextCodecRefusal> {
        core::str::from_utf8(self.bytes_u8()?).map_err(|_| ImageTextCodecRefusal::Malformed)
    }

    fn text_u16(&mut self) -> Result<&'a str, ImageTextCodecRefusal> {
        core::str::from_utf8(self.bytes_u16()?).map_err(|_| ImageTextCodecRefusal::Malformed)
    }

    fn finished(&self) -> bool {
        self.offset == self.bytes.len()
    }
}
