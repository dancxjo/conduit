//! Finite timed Mask behavior, independent of acquisition and manifestation.
use super::*;
pub struct TimedSevenSegmentMask {
    look: SevenSegmentLook,
    pages: Alternation,
}
impl TimedSevenSegmentMask {
    pub fn prepare(
        look: SevenSegmentLook,
        period_ms: u64,
        lifetime_ms: u64,
    ) -> Result<Self, TimingRefusal> {
        Ok(Self {
            look,
            pages: Alternation::new(period_ms, lifetime_ms)?,
        })
    }
    pub fn render<'a>(
        &self,
        face: &PreparedEnvironmentFace<'a>,
        elapsed_ms: u64,
        profile: &Tm1637Profile,
    ) -> Result<RenderedShow<'a>, WiringRefusal> {
        match self.pages.select(elapsed_ms) {
            Ok(selection) => render_for_profile(face, selection, &self.look, profile),
            Err(_) => {
                profile.validate()?;
                Ok(RenderedShow {
                    face_content_id: face.content_id,
                    face_revision: face.revision,
                    show: unavailable(),
                    disposition: MaskDisposition::Unavailable(MaskUnavailable::OutsideLifetime),
                })
            }
        }
    }
}
