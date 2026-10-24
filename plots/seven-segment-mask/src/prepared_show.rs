//! Finite resolved presentation material for one exact Face revision.
use super::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShowPreparationRefusal {
    IdentityExtent,
    Wiring(WiringRefusal),
}

struct Page {
    digits: [u8; 4],
    decimal_points: [bool; 4],
    colon: bool,
    disposition: MaskDisposition,
}
impl Page {
    fn prepare(rendered: RenderedShow<'_>) -> Self {
        Self {
            digits: (*rendered.show.digits()).map(|pattern| SegmentPatternForm::encode(pattern)[0]),
            decimal_points: *rendered.show.decimal_points(),
            colon: *rendered.show.colon(),
            disposition: rendered.disposition,
        }
    }
    fn show(&self) -> (SevenSegmentShow, MaskDisposition) {
        // Only the generated checked Form encoder writes these private bytes.
        let digits = self
            .digits
            .map(|byte| SegmentPatternForm::decode(&[byte]).expect("prepared segment Form"));
        (
            SevenSegmentShow::from_parts(digits, self.decimal_points, self.colon),
            self.disposition,
        )
    }
}

/// Two resolved pages and one finite timing policy. This owns exact correlation
/// bytes, so it can outlive a synchronous Face read without retaining that Face
/// or its quantities. It is inert preparation material, not an active Show,
/// timer, output grant or substitute Face. Its owner must separately admit all
/// invocation/effect bounds and revoke manifestation on basis or provider loss.
/// The current canonical Face identity profile is 64 UTF-8 bytes; another
/// profile refuses explicitly rather than truncating its identity.
pub struct PreparedAlternatingShow {
    identity: [u8; 64],
    revision: u64,
    pages: [Page; 2],
    timing: Alternation,
}
impl PreparedAlternatingShow {
    pub(crate) fn prepare(
        face: &PreparedEnvironmentFace<'_>,
        look: &SevenSegmentLook,
        timing: Alternation,
        profile: &Tm1637Profile,
    ) -> Result<Self, ShowPreparationRefusal> {
        let identity = face
            .content_id
            .as_bytes()
            .try_into()
            .map_err(|_| ShowPreparationRefusal::IdentityExtent)?;
        let temperature =
            render_for_profile(face, SevenSegmentSelection::Temperature, look, profile)
                .map_err(ShowPreparationRefusal::Wiring)?;
        let humidity = render_for_profile(face, SevenSegmentSelection::Humidity, look, profile)
            .map_err(ShowPreparationRefusal::Wiring)?;
        Ok(Self {
            identity,
            revision: face.revision,
            pages: [Page::prepare(temperature), Page::prepare(humidity)],
            timing,
        })
    }
    pub fn face_content_id(&self) -> &str {
        // Copied exclusively from a validated Face's str; never mutated.
        core::str::from_utf8(&self.identity).expect("prepared Face identity")
    }
    pub fn face_revision(&self) -> u64 {
        self.revision
    }
    /// Select already resolved patterns from an explicitly supplied elapsed
    /// clock value. Repeated calls are pure reads, not admitted frame writes.
    /// Expiry preserves correlation while producing the existing unavailable
    /// presentation. This does not advance or refresh semantic Face time.
    pub fn at(&self, elapsed_ms: u64) -> RenderedShow<'_> {
        let (show, disposition) = match self.timing.select(elapsed_ms) {
            Ok(SevenSegmentSelection::Temperature) => self.pages[0].show(),
            Ok(SevenSegmentSelection::Humidity) => self.pages[1].show(),
            Ok(SevenSegmentSelection::Clock) => {
                unreachable!("Alternation selects only temperature or humidity")
            }
            Err(_) => (
                unavailable(),
                MaskDisposition::Unavailable(MaskUnavailable::OutsideLifetime),
            ),
        };
        RenderedShow {
            face_content_id: self.face_content_id(),
            face_revision: self.revision,
            show,
            disposition,
        }
    }
}
