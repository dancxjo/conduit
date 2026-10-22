#![no_std]
//! Finite seven-segment interpretation of an immutable semantic Face revision.
//! Rendering has no clock, storage, GPIO, transport or manifestation side effects.

#[allow(dead_code)]
mod generated {
    include!(concat!(env!("OUT_DIR"), "/types.rs"));

    impl SevenSegmentShow {
        pub const fn from_parts(
            digits: [SegmentPattern; 4],
            decimal_points: [bool; 4],
            colon: bool,
        ) -> Self {
            Self {
                digits,
                decimal_points,
                colon,
            }
        }
    }
    impl SevenSegmentLook {
        pub const fn fahrenheit() -> Self {
            Self {
                temperature_unit: SevenSegmentTemperatureUnit::Fahrenheit,
            }
        }
        pub const fn celsius() -> Self {
            Self {
                temperature_unit: SevenSegmentTemperatureUnit::Celsius,
            }
        }
    }
}
pub use generated::{
    SegmentPattern, SegmentPatternForm, SevenSegmentLook, SevenSegmentSelection, SevenSegmentShow,
    SevenSegmentTemperatureUnit,
};
mod format;
mod projection;
mod rounding;
mod wiring;
pub use format::{
    render, render_for_profile, Alternation, MaskDisposition, MaskUnavailable, RenderedShow,
    TimingRefusal,
};
pub use projection::{FaceProjectionRefusal, PreparedEnvironmentFace};
pub use wiring::{DigitWiring, Tm1637Profile, WireBit, WiringRefusal};

pub(crate) fn pattern(bits: u8) -> SegmentPattern {
    SegmentPatternForm::decode(&[bits]).expect("reviewed seven-bit glyph")
}
pub(crate) fn unavailable() -> SevenSegmentShow {
    SevenSegmentShow::from_parts([pattern(0x40); 4], [false; 4], false)
}
pub(crate) const NUMERALS: [u8; 10] = [0x3f, 0x06, 0x5b, 0x4f, 0x66, 0x6d, 0x7d, 0x07, 0x7f, 0x6f];
