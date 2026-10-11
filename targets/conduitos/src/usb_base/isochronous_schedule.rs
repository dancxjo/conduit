//! Finite USB2 transport geometry and explicit frame deadlines; no PCM cadence.
//! Values are advisory. Root must admit the exact resource and transfer owner.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IsochronousOutGeometry {
    packet_bytes: u16,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IsochronousScheduleRefusal {
    Unsupported,
    Geometry,
    Deadline,
}
impl IsochronousOutGeometry {
    /// First finite transport: full-speed, one packet per 1 ms service interval.
    /// Eight 256-byte buffers and a 64-TRB ring are reserved before Play.
    pub fn usb2(
        speed: u8,
        packet_field: u16,
        interval: u8,
    ) -> Result<Self, IsochronousScheduleRefusal> {
        if speed != 1 || interval != 1 {
            return Err(IsochronousScheduleRefusal::Unsupported);
        }
        if packet_field == 0 || packet_field > 256 {
            return Err(IsochronousScheduleRefusal::Geometry);
        }
        Ok(Self {
            packet_bytes: packet_field,
        })
    }
    pub const fn packet_bytes(self) -> u16 {
        self.packet_bytes
    }
    pub const fn context_interval(self) -> u8 {
        3
    }
}
/// Observed controller-local clock and retained finite endpoint geometry.
/// Snapshotting confers no transfer authority and performs no domain conversion.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IsochronousOutSnapshot {
    pub current_microframe: u16,
    pub scheduling_threshold: u8,
    pub service_interval_microframes: u16,
    pub maximum_packet_bytes: u16,
    pub pending_packets: u8,
    pub maximum_pending_packets: u8,
    pub dma_resource_bytes: u32,
}

/// 11-bit USB frame number, independently derived from an admitted clock domain.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IsochronousFrame(u16);
impl IsochronousFrame {
    pub fn new(frame: u16) -> Result<Self, IsochronousScheduleRefusal> {
        if frame > 2047 {
            return Err(IsochronousScheduleRefusal::Geometry);
        }
        Ok(Self(frame))
    }
    pub const fn number(self) -> u16 {
        self.0
    }
    /// xHCI 1.2b §§4.11.2.5,5.3.4: IST rounded up to a frame plus one;
    /// scheduled frame is at most 895 frames ahead. Never silently use SIA.
    pub fn validate(self, mfindex: u16, ist: u8) -> Result<(), IsochronousScheduleRefusal> {
        if mfindex > 0x3fff || ist > 15 {
            return Err(IsochronousScheduleRefusal::Geometry);
        }
        let threshold_frames = if ist & 8 != 0 {
            u16::from(ist & 7)
        } else {
            u16::from(ist & 7).div_ceil(8)
        };
        let distance = self.0.wrapping_sub(mfindex >> 3) & 2047;
        if distance < threshold_frames + 1 || distance > 895 {
            return Err(IsochronousScheduleRefusal::Deadline);
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IsochronousOutObservation {
    Completed {
        frame: u16,
        requested: u16,
        actual: u16,
    },
    Short {
        frame: u16,
        requested: u16,
        actual: u16,
    },
    MissedService {
        frame: u16,
    },
    Underrun,
    ProviderLost,
    Cancelled,
    Drained,
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn frame_window_wraps_but_does_not_accept_past_or_far_future() {
        let frame = IsochronousFrame::new(0).unwrap();
        assert_eq!(frame.validate(2047 << 3, 0), Ok(()));
        assert_eq!(
            frame.validate(0, 0),
            Err(IsochronousScheduleRefusal::Deadline)
        );
        assert_eq!(
            IsochronousFrame::new(896).unwrap().validate(0, 0),
            Err(IsochronousScheduleRefusal::Deadline)
        );
        assert_eq!(IsochronousFrame::new(895).unwrap().validate(0, 0), Ok(()));
    }
    #[test]
    fn ist_units_and_rounding_are_real_controller_requirements() {
        assert!(IsochronousFrame::new(1).unwrap().validate(0, 1).is_err());
        assert!(IsochronousFrame::new(2).unwrap().validate(0, 7).is_ok());
        assert!(IsochronousFrame::new(7).unwrap().validate(0, 15).is_err());
        assert!(IsochronousFrame::new(8).unwrap().validate(0, 15).is_ok());
    }
    #[test]
    fn unsupported_usb_speeds_and_resource_extents_refuse() {
        assert!(IsochronousOutGeometry::usb2(3, 192, 1).is_err());
        assert!(IsochronousOutGeometry::usb2(1, 192, 2).is_err());
        assert!(IsochronousOutGeometry::usb2(1, 257, 1).is_err());
        assert!(IsochronousOutGeometry::usb2(1, 192, 1).is_ok());
    }
}
