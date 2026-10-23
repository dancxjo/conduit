#![no_std]
//! Class-neutral electrical pulse sampling. No peripheral recognition or policy.
extern crate alloc;
mod codec;
mod contract;
#[cfg(feature = "host-call")]
pub mod installation;
#[cfg(feature = "host-call")]
pub mod owner;
pub use codec::{PreparedPulseCodec, PulseCodecRefusal};
pub use contract::{PulseReadContract, PULSE_READ_CALL, PULSE_READ_KIND, PULSE_READ_MAXIMUM_BYTES};

pub const MAXIMUM_OCTETS: usize = 8;

/// Wire geometry supplied by the requesting Plot. The selected Base owns the pin.
/// After the low drive, release the line, observe high then low, skip the given
/// number of complete high pulses, and sample each following high pulse after
/// the requested offset. Samples pack MSB first, eight samples per octet.
/// All time coordinates here are integer microseconds of the Base clock.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PulseReadRequest {
    pub hold_low_us: u32,
    pub skip_high_pulses: u8,
    pub sample_after_rise_us: u16,
    pub octets: u8,
    pub capture_timeout_us: u32,
}

/// Finite implementation envelope, supplied by an actual electrical provider.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PulseReadLimits {
    pub maximum_hold_low_us: u32,
    pub maximum_skipped_pulses: u8,
    pub minimum_sample_offset_us: u16,
    pub maximum_sample_offset_us: u16,
    pub maximum_octets: u8,
    pub maximum_capture_timeout_us: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PulseReadRefusal {
    InvalidLimits,
    DriveExtent,
    SkipExtent,
    SampleOffset,
    OctetExtent,
    DeadlineExtent,
}

/// Checked geometry is not resource possession or permission to touch a pin.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CheckedPulseRead(PulseReadRequest);
impl CheckedPulseRead {
    pub fn admit(
        request: PulseReadRequest,
        limits: PulseReadLimits,
    ) -> Result<Self, PulseReadRefusal> {
        use PulseReadRefusal as R;
        if limits.maximum_hold_low_us == 0
            || limits.maximum_octets == 0
            || usize::from(limits.maximum_octets) > MAXIMUM_OCTETS
            || limits.minimum_sample_offset_us == 0
            || limits.minimum_sample_offset_us > limits.maximum_sample_offset_us
            || limits.maximum_capture_timeout_us == 0
        {
            return Err(R::InvalidLimits);
        }
        if request.hold_low_us == 0 || request.hold_low_us > limits.maximum_hold_low_us {
            return Err(R::DriveExtent);
        }
        if request.skip_high_pulses > limits.maximum_skipped_pulses {
            return Err(R::SkipExtent);
        }
        if !(limits.minimum_sample_offset_us..=limits.maximum_sample_offset_us)
            .contains(&request.sample_after_rise_us)
        {
            return Err(R::SampleOffset);
        }
        if request.octets == 0 || request.octets > limits.maximum_octets {
            return Err(R::OctetExtent);
        }
        let minimum = u32::from(request.octets) * 8 * u32::from(request.sample_after_rise_us);
        if request.capture_timeout_us < minimum
            || request.capture_timeout_us > limits.maximum_capture_timeout_us
        {
            return Err(R::DeadlineExtent);
        }
        Ok(Self(request))
    }
    pub const fn request(self) -> PulseReadRequest {
        self.0
    }
    pub const fn deadline_span_us(self) -> u64 {
        self.0.hold_low_us as u64 + self.0.capture_timeout_us as u64
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PulseReadDisposition {
    Complete,
    TimedOut,
    ProviderFailed,
}

/// Only complete octets are observable. A timed-out partial octet is discarded;
/// unused storage is zero, and the disposition remains separate from length.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PulseReadCapture {
    pub disposition: PulseReadDisposition,
    pub length: u8,
    pub wire: [u8; MAXIMUM_OCTETS],
}
impl PulseReadCapture {
    pub fn from_complete_octets(disposition: PulseReadDisposition, octets: &[u8]) -> Option<Self> {
        if octets.len() > MAXIMUM_OCTETS
            || (disposition == PulseReadDisposition::Complete && octets.is_empty())
        {
            return None;
        }
        let mut wire = [0; MAXIMUM_OCTETS];
        wire[..octets.len()].copy_from_slice(octets);
        Some(Self {
            disposition,
            length: octets.len() as u8,
            wire,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn limits() -> PulseReadLimits {
        PulseReadLimits {
            maximum_hold_low_us: 50_000,
            maximum_skipped_pulses: 4,
            minimum_sample_offset_us: 10,
            maximum_sample_offset_us: 200,
            maximum_octets: 8,
            maximum_capture_timeout_us: 50_000,
        }
    }
    fn request() -> PulseReadRequest {
        PulseReadRequest {
            hold_low_us: 20_000,
            skip_high_pulses: 1,
            sample_after_rise_us: 40,
            octets: 5,
            capture_timeout_us: 10_000,
        }
    }
    #[test]
    fn geometry_refuses_before_any_electrical_provider_is_needed() {
        let checked = CheckedPulseRead::admit(request(), limits()).unwrap();
        assert_eq!(checked.deadline_span_us(), 30_000);
        for (changed, expected) in [
            (
                PulseReadRequest {
                    hold_low_us: 50_001,
                    ..request()
                },
                PulseReadRefusal::DriveExtent,
            ),
            (
                PulseReadRequest {
                    skip_high_pulses: 5,
                    ..request()
                },
                PulseReadRefusal::SkipExtent,
            ),
            (
                PulseReadRequest {
                    sample_after_rise_us: 0,
                    ..request()
                },
                PulseReadRefusal::SampleOffset,
            ),
            (
                PulseReadRequest {
                    octets: 0,
                    ..request()
                },
                PulseReadRefusal::OctetExtent,
            ),
            (
                PulseReadRequest {
                    octets: 9,
                    ..request()
                },
                PulseReadRefusal::OctetExtent,
            ),
            (
                PulseReadRequest {
                    capture_timeout_us: 1599,
                    ..request()
                },
                PulseReadRefusal::DeadlineExtent,
            ),
            (
                PulseReadRequest {
                    capture_timeout_us: 50_001,
                    ..request()
                },
                PulseReadRefusal::DeadlineExtent,
            ),
        ] {
            assert_eq!(CheckedPulseRead::admit(changed, limits()), Err(expected));
        }
        assert_eq!(
            CheckedPulseRead::admit(
                request(),
                PulseReadLimits {
                    maximum_octets: 9,
                    ..limits()
                }
            ),
            Err(PulseReadRefusal::InvalidLimits)
        );
    }
    #[test]
    fn partial_capture_has_explicit_extent_and_cannot_publish_padding() {
        let capture =
            PulseReadCapture::from_complete_octets(PulseReadDisposition::TimedOut, &[0x81, 0x01])
                .unwrap();
        assert_eq!(capture.length, 2);
        assert_eq!(capture.wire, [0x81, 1, 0, 0, 0, 0, 0, 0]);
        assert!(
            PulseReadCapture::from_complete_octets(PulseReadDisposition::Complete, &[0; 9])
                .is_none()
        );
    }
}
