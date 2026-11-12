//! Prepared Mask/Look interpretation and hardware-profile Frame composition.
//! Clock input and Face custody remain caller-owned; this is not a scheduler,
//! caller permit, Body membership, output Line or electrical implementation.
use conduit_face::Face;
use conduit_seven_segment_mask::{
    PreparedEnvironmentFace, RenderedShow, SevenSegmentLook, SevenSegmentShow,
    TimedSevenSegmentMask, Tm1637Profile,
};
use conduit_tm1637_protocol::PreparedDisplayFrame;

#[cfg(feature = "display-mask-ht16k33")]
pub mod ht16k33;

/// Prepared physical encoding only: implementations perform no IO, quantity
/// formatting, time sampling, allocation or authority creation during encode.
/// Bounds and backing must describe the exact retained encoder and stay fixed.
pub trait PreparedFrameEncoder {
    type Registers;
    /// Validate that every annunciator admitted by the interpretation profile
    /// has selected physical support. Missing support must refuse, never vanish.
    fn validate_support(&self, profile: &Tm1637Profile) -> Result<(), Refusal>;
    fn maximum_frame_bytes(&self) -> usize;
    fn retained_backing_bytes(&self) -> usize;
    fn encode(
        &mut self,
        show: &SevenSegmentShow,
        interpretation: &Tm1637Profile,
    ) -> Result<EncodedFrame<'_, Self::Registers>, Refusal>;
}
pub struct EncodedFrame<'a, R> {
    pub registers: R,
    pub canonical: &'a [u8],
}
/// The default encoder retains the original selected TM wiring and controls.
pub struct Tm1637Encoder {
    frame: PreparedDisplayFrame,
    brightness: u8,
}
pub struct PreparedDisplayMask<E = Tm1637Encoder> {
    profile: Tm1637Profile,
    mask: TimedSevenSegmentMask,
    encoder: E,
    maximum_frame_bytes: usize,
}
pub struct Frame<'a, R = [u8; 6]> {
    pub rendered: RenderedShow<'a>,
    pub registers: R,
    pub canonical: &'a [u8],
}
#[derive(Debug)]
pub enum Refusal {
    Wiring(conduit_seven_segment_mask::WiringRefusal),
    Timing(conduit_seven_segment_mask::TimingRefusal),
    Face(conduit_seven_segment_mask::FaceProjectionRefusal),
    Frame(conduit_core::StructuredInfoRefusal),
    Brightness,
    FrameExtent,
    Show(conduit_seven_segment_mask::ShowPreparationRefusal),
}
impl PreparedDisplayMask {
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
        let encoder = Tm1637Encoder {
            frame: PreparedDisplayFrame::prepare().map_err(Refusal::Frame)?,
            brightness,
        };
        Self::prepare_with_encoder(profile, look, period_ms, lifetime_ms, encoder)
    }
}
impl PreparedFrameEncoder for Tm1637Encoder {
    type Registers = [u8; 6];
    fn validate_support(&self, profile: &Tm1637Profile) -> Result<(), Refusal> {
        profile.validate().map_err(Refusal::Wiring)
    }
    fn maximum_frame_bytes(&self) -> usize {
        conduit_tm1637_protocol::DISPLAY_FRAME_BYTES
    }
    fn retained_backing_bytes(&self) -> usize {
        0
    }
    fn encode(
        &mut self,
        show: &SevenSegmentShow,
        interpretation: &Tm1637Profile,
    ) -> Result<EncodedFrame<'_, Self::Registers>, Refusal> {
        let registers = interpretation.map(show).map_err(Refusal::Wiring)?;
        let canonical = self
            .frame
            .encode(registers, self.brightness, true)
            .map_err(Refusal::Frame)?;
        Ok(EncodedFrame {
            registers,
            canonical,
        })
    }
}
impl<E: PreparedFrameEncoder> PreparedDisplayMask<E> {
    /// The interpretation profile still belongs to the shared seven-segment
    /// Mask, whose current preparation API still uses Tm1637Profile to express
    /// digit/annunciator support. This is the temporary interpretation seam,
    /// not a claim that another encoder owns or drives TM electrical wiring.
    /// A physical encoder must explicitly validate its annunciator support
    /// against that profile; the profile creates no physical bus ownership.
    pub fn prepare_with_encoder(
        profile: Tm1637Profile,
        look: SevenSegmentLook,
        period_ms: u64,
        lifetime_ms: u64,
        encoder: E,
    ) -> Result<Self, Refusal> {
        profile.validate().map_err(Refusal::Wiring)?;
        encoder.validate_support(&profile)?;
        let maximum_frame_bytes = encoder.maximum_frame_bytes();
        if maximum_frame_bytes == 0 {
            return Err(Refusal::FrameExtent);
        }
        Ok(Self {
            profile,
            mask: TimedSevenSegmentMask::prepare(look, period_ms, lifetime_ms)
                .map_err(Refusal::Timing)?,
            encoder,
            maximum_frame_bytes,
        })
    }
    pub fn maximum_frame_bytes(&self) -> usize {
        self.maximum_frame_bytes
    }
    /// Reachable external physical encoder backing; inline Self is funded by
    /// its owner. No schema decoding or evaluator is introduced by this seam.
    pub fn retained_backing_bytes(&self) -> usize {
        self.encoder.retained_backing_bytes()
    }
    fn encode_frame(
        &mut self,
        show: &SevenSegmentShow,
    ) -> Result<EncodedFrame<'_, E::Registers>, Refusal> {
        let encoded = self.encoder.encode(show, &self.profile)?;
        if encoded.canonical.len() > self.maximum_frame_bytes {
            return Err(Refusal::FrameExtent);
        }
        Ok(encoded)
    }
    /// Compile one immutable pair of pages while the caller holds its exact
    /// Face borrow. The result owns correlation and can leave that read scope.
    pub fn prepare_show(
        &self,
        face: &Face,
    ) -> Result<conduit_seven_segment_mask::PreparedAlternatingShow, Refusal> {
        let selected =
            PreparedEnvironmentFace::prepare(face, "environment").map_err(Refusal::Face)?;
        self.mask
            .prepare_show(&selected, &self.profile)
            .map_err(Refusal::Show)
    }
    pub fn render_prepared<'a>(
        &'a mut self,
        show: &'a conduit_seven_segment_mask::PreparedAlternatingShow,
        elapsed_ms: u64,
    ) -> Result<Frame<'a, E::Registers>, Refusal> {
        let rendered = show.at(elapsed_ms);
        let encoded = self.encode_frame(&rendered.show)?;
        let EncodedFrame {
            registers,
            canonical,
        } = encoded;
        Ok(Frame {
            rendered,
            registers,
            canonical,
        })
    }
    pub fn render<'a>(
        &'a mut self,
        face: &'a Face,
        elapsed_ms: u64,
    ) -> Result<Frame<'a, E::Registers>, Refusal> {
        let selected =
            PreparedEnvironmentFace::prepare(face, "environment").map_err(Refusal::Face)?;
        let rendered = self
            .mask
            .render(&selected, elapsed_ms, &self.profile)
            .map_err(Refusal::Wiring)?;
        let encoded = self.encode_frame(&rendered.show)?;
        let EncodedFrame {
            registers,
            canonical,
        } = encoded;
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
            Self::Show(reason) => write!(f, "Show: {reason:?}"),
            Self::FrameExtent => f.write_str("physical frame exceeds prepared extent"),
            Self::Brightness => f.write_str("brightness outside 0..=7"),
        }
    }
}

#[cfg(test)]
#[path = "display_mask/tests.rs"]
mod tests;
