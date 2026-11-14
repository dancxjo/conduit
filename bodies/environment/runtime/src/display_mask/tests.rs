use super::*;
use conduit_seven_segment_mask::{DigitWiring, SegmentPatternForm, WireBit, WiringRefusal};

fn profile() -> Tm1637Profile {
    Tm1637Profile {
        digits: core::array::from_fn(|n| DigitWiring {
            address: n as u8,
            segment_bits: [0, 1, 2, 3, 4, 5, 6],
        }),
        decimal_points: [None; 4],
        colon: Some(WireBit { address: 1, bit: 7 }),
    }
}
fn show(bits: u8) -> SevenSegmentShow {
    SevenSegmentShow::from_parts(
        [SegmentPatternForm::decode(&[bits]).unwrap(); 4],
        [false; 4],
        true,
    )
}

#[test]
fn default_encoder_preserves_all_logical_patterns_and_exact_canonical_controls() {
    let mut mask =
        PreparedDisplayMask::prepare(profile(), SevenSegmentLook::fahrenheit(), 10, 20, 3).unwrap();
    let mut original = PreparedDisplayFrame::prepare().unwrap();
    assert_eq!(mask.retained_backing_bytes(), 0);
    assert_eq!(
        mask.maximum_frame_bytes(),
        conduit_tm1637_protocol::DISPLAY_FRAME_BYTES
    );
    for bits in 0..128 {
        let logical = show(bits);
        let expected = profile().map(&logical).unwrap();
        let encoded = mask.encode_frame(&logical).unwrap();
        assert_eq!(encoded.registers, expected);
        assert_eq!(
            encoded.canonical,
            original.encode(expected, 3, true).unwrap()
        );
    }
}

struct FiniteEncoder {
    maximum: usize,
    colon: bool,
    bytes: alloc::boxed::Box<[u8]>,
}
fn finite(maximum: usize, colon: bool) -> FiniteEncoder {
    FiniteEncoder {
        maximum,
        colon,
        bytes: alloc::vec![1, 2].into_boxed_slice(),
    }
}
impl PreparedFrameEncoder for FiniteEncoder {
    type Registers = ();
    fn validate_support(&self, profile: &Tm1637Profile) -> Result<(), Refusal> {
        if profile.colon.is_some() && !self.colon {
            return Err(Refusal::Wiring(WiringRefusal::UnwiredAnnunciator));
        }
        Ok(())
    }
    fn maximum_frame_bytes(&self) -> usize {
        self.maximum
    }
    fn retained_backing_bytes(&self) -> usize {
        self.bytes.len()
    }
    fn encode(
        &mut self,
        _: &SevenSegmentShow,
        _: &Tm1637Profile,
    ) -> Result<EncodedFrame<'_, ()>, Refusal> {
        Ok(EncodedFrame {
            registers: (),
            canonical: &self.bytes,
        })
    }
}

#[test]
fn selected_encoder_must_support_profile_and_nonzero_exact_output_extent() {
    let prepare = |encoder| {
        PreparedDisplayMask::prepare_with_encoder(
            profile(),
            SevenSegmentLook::fahrenheit(),
            10,
            20,
            encoder,
        )
    };
    assert!(matches!(
        prepare(finite(2, false)),
        Err(Refusal::Wiring(WiringRefusal::UnwiredAnnunciator))
    ));
    assert!(matches!(
        prepare(finite(0, true)),
        Err(Refusal::FrameExtent)
    ));
    let mut short = prepare(finite(1, true)).unwrap();
    assert!(matches!(
        short.encode_frame(&show(0)),
        Err(Refusal::FrameExtent)
    ));
    let mut exact = prepare(finite(2, true)).unwrap();
    assert_eq!(exact.retained_backing_bytes(), 2);
    assert_eq!(exact.encode_frame(&show(0)).unwrap().canonical, &[1, 2]);
}
