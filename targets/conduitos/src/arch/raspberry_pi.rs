//! Physical board facts shared by CPU-specific Raspberry Pi mechanisms.
//! These facts neither select a protocol nor grant access to a device.
//!
//! Wiring provenance: raspberrypi/linux rpi-6.12.y board descriptions
//! bcm2835-rpi-b-plus.dts and bcm2837-rpi-3-b-plus.dts.

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Board {
    BPlus,
    ThreeBPlus,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum HubReset {
    Gpio(u8),
    FirmwareGpio(u8),
}

impl Board {
    pub(crate) const fn peripheral_base(self) -> usize {
        match self {
            Self::BPlus => 0x2000_0000,
            Self::ThreeBPlus => 0x3f00_0000,
        }
    }

    pub(crate) const fn audio_pins(self) -> [u8; 2] {
        match self {
            Self::BPlus => [40, 45],
            Self::ThreeBPlus => [40, 41],
        }
    }

    pub(crate) const fn hub_reset(self) -> HubReset {
        match self {
            Self::BPlus => HubReset::Gpio(31),
            Self::ThreeBPlus => HubReset::FirmwareGpio(3),
        }
    }

    pub(crate) const fn accepts_revision(self, revision: u32) -> bool {
        match self {
            Self::BPlus => matches!(revision, 0x10 | 0x13 | 0x900032),
            // Firmware warranty/overvoltage flags are outside the revision.
            Self::ThreeBPlus => revision & 0x00ff_ffff == 0xa020d3,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn original_and_three_b_plus_have_distinct_register_and_wiring_facts() {
        assert_eq!(Board::BPlus.peripheral_base() + 0x98_0000, 0x2098_0000);
        assert_eq!(Board::ThreeBPlus.peripheral_base() + 0x98_0000, 0x3f98_0000);
        assert_eq!(Board::BPlus.audio_pins(), [40, 45]);
        assert_eq!(Board::ThreeBPlus.audio_pins(), [40, 41]);
        assert_eq!(Board::BPlus.hub_reset(), HubReset::Gpio(31));
        assert_eq!(Board::ThreeBPlus.hub_reset(), HubReset::FirmwareGpio(3));
    }

    #[test]
    fn revision_requires_the_exact_board_even_when_cpu_generation_matches() {
        for revision in [0xa020d3, 0x10a020d3, 0x80a020d3] {
            assert!(Board::ThreeBPlus.accepts_revision(revision));
            assert!(!Board::BPlus.accepts_revision(revision));
        }
        for revision in [0x10, 0x13, 0x900032] {
            assert!(Board::BPlus.accepts_revision(revision));
            assert!(!Board::ThreeBPlus.accepts_revision(revision));
        }
        // Pi 3B, 3A+, Pi 4 and an unobserved manufacturer variant.
        for revision in [0xa02082, 0x9020e0, 0xc03111, 0xa220d3, 0] {
            assert!(!Board::ThreeBPlus.accepts_revision(revision));
        }
    }
}
