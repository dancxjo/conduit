//! Owner-side interpretation of its actual Body Face. This produces finite
//! Frame input only; the checked output Plot owns packet encoding and delivery.
use conduit_environment_runtime::display_packets::PreparedDisplayFrame;
use conduit_face::Face;
use conduit_seven_segment_mask::{
    PreparedEnvironmentFace, RenderedShow, SevenSegmentLook, TimedSevenSegmentMask, Tm1637Profile,
};

pub struct Display {
    profile: Tm1637Profile,
    mask: TimedSevenSegmentMask,
    frame: PreparedDisplayFrame,
    brightness: u8,
}
pub struct Frame<'a> {
    pub rendered: RenderedShow<'a>,
    pub registers: [u8; 6],
    pub canonical: &'a [u8],
}
#[derive(Debug)]
pub enum Refusal {
    Wiring(conduit_seven_segment_mask::WiringRefusal),
    Timing(conduit_seven_segment_mask::TimingRefusal),
    Face(conduit_seven_segment_mask::FaceProjectionRefusal),
    Frame(conduit_core::StructuredInfoRefusal),
    Brightness,
}
impl Display {
    pub fn prepare(
        profile: Tm1637Profile,
        look: SevenSegmentLook,
        period_ms: u64,
        lifetime_ms: u64,
        brightness: u8,
    ) -> Result<Self, Refusal> {
        profile.validate().map_err(Refusal::Wiring)?;
        if brightness > 7 {
            return Err(Refusal::Brightness);
        }
        Ok(Self {
            profile,
            mask: TimedSevenSegmentMask::prepare(look, period_ms, lifetime_ms)
                .map_err(Refusal::Timing)?,
            frame: PreparedDisplayFrame::prepare().map_err(Refusal::Frame)?,
            brightness,
        })
    }
    pub fn render<'a>(&'a mut self, face: &'a Face, elapsed_ms: u64) -> Result<Frame<'a>, Refusal> {
        let selected =
            PreparedEnvironmentFace::prepare(face, "environment").map_err(Refusal::Face)?;
        let rendered = self
            .mask
            .render(&selected, elapsed_ms, &self.profile)
            .map_err(Refusal::Wiring)?;
        let registers = self.profile.map(&rendered.show).map_err(Refusal::Wiring)?;
        let canonical = self
            .frame
            .encode(registers, self.brightness, true)
            .map_err(Refusal::Frame)?;
        Ok(Frame {
            rendered,
            registers,
            canonical,
        })
    }
}

impl core::fmt::Display for Refusal {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Wiring(reason) => write!(f, "wiring: {reason:?}"),
            Self::Timing(reason) => write!(f, "timing: {reason:?}"),
            Self::Face(reason) => write!(f, "Face: {reason:?}"),
            Self::Frame(reason) => write!(f, "Frame: {reason:?}"),
            Self::Brightness => f.write_str("brightness outside 0..=7"),
        }
    }
}
