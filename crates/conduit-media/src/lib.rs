//! Host-neutral bounded media value contracts.
//!
//! This crate defines values and exact compatibility only. It does not expose
//! codecs, devices, host discovery, implicit conversion, or another event
//! model.

use sha2::{Digest, Sha256};

pub const MAXIMUM_PLANES: usize = 4;
pub const MAXIMUM_CHANNELS: u16 = 64;
pub const MAXIMUM_METADATA_ENTRIES: u16 = 64;
pub const MAXIMUM_MEDIA_BYTES: usize = 16 * 1024 * 1024;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MediaReason {
    InvalidTimeBase,
    MissingTimestamp,
    InvalidDuration,
    InvalidDimensions,
    InvalidPlaneLayout,
    UnsupportedFormat,
    ChannelLayoutMismatch,
    DescriptorMismatch,
    MetadataOverflow,
    ByteOverflow,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RationalTimeBase {
    pub numerator: u32,
    pub denominator: u32,
}

impl RationalTimeBase {
    pub fn validate(self) -> Result<(), MediaReason> {
        if self.numerator == 0 || self.denominator == 0 {
            return Err(MediaReason::InvalidTimeBase);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MediaTime {
    pub time_base: RationalTimeBase,
    pub timestamp: Option<i64>,
    pub duration: u64,
    pub discontinuity: bool,
    pub conversion_uncertainty_ticks: u64,
}

impl MediaTime {
    pub fn validate(self) -> Result<(), MediaReason> {
        self.time_base.validate()?;
        if self.timestamp.is_none() {
            return Err(MediaReason::MissingTimestamp);
        }
        if self.duration == 0 {
            return Err(MediaReason::InvalidDuration);
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Plane {
    pub offset: usize,
    pub stride: usize,
    pub rows: usize,
}

impl Plane {
    fn end(self) -> Option<usize> {
        self.stride
            .checked_mul(self.rows)
            .and_then(|bytes| self.offset.checked_add(bytes))
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AudioDescriptor<'a> {
    pub sample_format: &'a str,
    pub sample_rate_hz: u32,
    pub channel_layout: &'a str,
    pub channels: u16,
    pub frames: u32,
    pub planes: &'a [Plane],
    pub maximum_bytes: usize,
}

impl AudioDescriptor<'_> {
    pub fn validate(self) -> Result<(), MediaReason> {
        if !matches!(self.sample_format, "s16le" | "s24le" | "f32le") || self.sample_rate_hz == 0 {
            return Err(MediaReason::UnsupportedFormat);
        }
        if self.channels == 0 || self.channels > MAXIMUM_CHANNELS || self.frames == 0 {
            return Err(MediaReason::ChannelLayoutMismatch);
        }
        validate_planes(self.planes, self.maximum_bytes)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct VideoDescriptor<'a> {
    pub width: u32,
    pub height: u32,
    pub pixel_format: &'a str,
    pub color_space: &'a str,
    pub color_range: &'a str,
    pub transfer: &'a str,
    pub orientation_degrees: u16,
    pub alpha: bool,
    pub planes: &'a [Plane],
    pub maximum_bytes: usize,
}

impl VideoDescriptor<'_> {
    pub fn validate(self) -> Result<(), MediaReason> {
        if self.width == 0 || self.height == 0 {
            return Err(MediaReason::InvalidDimensions);
        }
        if !matches!(self.pixel_format, "rgb24" | "rgba32" | "gray8" | "yuv420p") {
            return Err(MediaReason::UnsupportedFormat);
        }
        if !matches!(self.orientation_degrees, 0 | 90 | 180 | 270) {
            return Err(MediaReason::InvalidDimensions);
        }
        validate_planes(self.planes, self.maximum_bytes)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PacketDescriptor<'a> {
    pub codec: &'a str,
    pub profile: &'a str,
    pub extradata_identity: [u8; 32],
    pub key: bool,
    pub discontinuity: bool,
    pub maximum_bytes: usize,
}

impl PacketDescriptor<'_> {
    pub fn validate(self) -> Result<(), MediaReason> {
        if self.codec.is_empty() || self.profile.is_empty() {
            return Err(MediaReason::UnsupportedFormat);
        }
        if self.maximum_bytes == 0 || self.maximum_bytes > MAXIMUM_MEDIA_BYTES {
            return Err(MediaReason::ByteOverflow);
        }
        Ok(())
    }
}

fn validate_planes(planes: &[Plane], maximum_bytes: usize) -> Result<(), MediaReason> {
    if planes.is_empty()
        || planes.len() > MAXIMUM_PLANES
        || maximum_bytes == 0
        || maximum_bytes > MAXIMUM_MEDIA_BYTES
        || planes
            .iter()
            .any(|plane| plane.stride == 0 || plane.rows == 0 || plane.end() > Some(maximum_bytes))
    {
        return Err(MediaReason::InvalidPlaneLayout);
    }
    Ok(())
}

#[must_use]
pub fn descriptor_hash(canonical_descriptor: &str) -> [u8; 32] {
    Sha256::digest(canonical_descriptor.as_bytes()).into()
}

pub fn exact_audio_compatibility(
    producer: AudioDescriptor<'_>,
    consumer: AudioDescriptor<'_>,
) -> Result<(), MediaReason> {
    producer.validate()?;
    consumer.validate()?;
    if producer == consumer {
        Ok(())
    } else if producer.channel_layout != consumer.channel_layout
        || producer.channels != consumer.channels
    {
        Err(MediaReason::ChannelLayoutMismatch)
    } else {
        Err(MediaReason::DescriptorMismatch)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIXTURE: &str = include_str!("../../../conformance/c4/media-values.json");
    const AUDIO_PLANES: [Plane; 1] = [Plane {
        offset: 0,
        stride: 8,
        rows: 48,
    }];

    fn audio() -> AudioDescriptor<'static> {
        AudioDescriptor {
            sample_format: "s16le",
            sample_rate_hz: 48_000,
            channel_layout: "stereo-lr",
            channels: 2,
            frames: 192,
            planes: &AUDIO_PLANES,
            maximum_bytes: 384,
        }
    }

    #[test]
    fn synthetic_pcm_is_finite_and_exact() {
        assert_eq!(audio().validate(), Ok(()));
        let mut different = audio();
        different.channel_layout = "stereo-rl";
        assert_eq!(
            exact_audio_compatibility(audio(), different),
            Err(MediaReason::ChannelLayoutMismatch)
        );
    }

    #[test]
    fn time_and_plane_failures_are_explicit() {
        assert_eq!(
            MediaTime {
                time_base: RationalTimeBase {
                    numerator: 1,
                    denominator: 48_000,
                },
                timestamp: Some(0),
                duration: 192,
                discontinuity: false,
                conversion_uncertainty_ticks: 0,
            }
            .validate(),
            Ok(())
        );
        let mut oversized = audio();
        oversized.maximum_bytes = 128;
        assert_eq!(oversized.validate(), Err(MediaReason::InvalidPlaneLayout));
    }

    #[test]
    fn conformance_fixture_owns_the_complete_first_matrix() {
        let fixture: serde_json::Value = serde_json::from_str(FIXTURE).unwrap();
        assert_eq!(fixture["schema"], "conduit.media-value-conformance");
        assert_eq!(fixture["schema_version"], 0);
        assert_eq!(fixture["types"].as_array().unwrap().len(), 6);
        assert_eq!(fixture["positive"].as_array().unwrap().len(), 5);
        assert_eq!(fixture["negative"].as_array().unwrap().len(), 14);
    }
}
