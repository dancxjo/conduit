//! Selected logical Show → HT16K33 RAM → canonical output-Plot Frame.
//! This encoder creates no bus owner, initialization, grant or acknowledgement.
use super::{EncodedFrame, PreparedFrameEncoder, Refusal};
use conduit_ht16k33::{frame::PreparedDisplayFrame, Address, Profile};
use conduit_seven_segment_mask::{
    SegmentPatternForm, SevenSegmentShow, Tm1637Profile, WiringRefusal,
};

pub struct Ht16k33Encoder {
    profile: Profile,
    address: Address,
    frame: PreparedDisplayFrame,
}

impl Ht16k33Encoder {
    /// Address and backpack wiring must come from explicit checked construction.
    /// The independently admitted HT protocol still owns controller setup and
    /// the selected I²C result; canonical bytes alone prove no physical effect.
    pub fn prepare(profile: Profile, address: Address) -> Result<Self, Refusal> {
        Ok(Self {
            profile,
            address,
            frame: PreparedDisplayFrame::prepare().map_err(Refusal::Frame)?,
        })
    }
}

impl PreparedFrameEncoder for Ht16k33Encoder {
    type Registers = [u8; 16];

    fn validate_support(&self, interpretation: &Tm1637Profile) -> Result<(), Refusal> {
        interpretation.validate().map_err(Refusal::Wiring)?;
        // Each selected HT digit has an independent decimal point. There is
        // no selected colon, so a colon-capable interpretation must refuse.
        if interpretation.colon.is_some() {
            return Err(Refusal::Wiring(WiringRefusal::UnwiredAnnunciator));
        }
        Ok(())
    }

    fn maximum_frame_bytes(&self) -> usize {
        conduit_ht16k33::frame::DISPLAY_FRAME_BYTES
    }

    fn retained_backing_bytes(&self) -> usize {
        0
    }

    fn encode(
        &mut self,
        show: &SevenSegmentShow,
        interpretation: &Tm1637Profile,
    ) -> Result<EncodedFrame<'_, Self::Registers>, Refusal> {
        self.validate_support(interpretation)?;
        // Retain the original interpretation's declared annunciator support;
        // its TM register mapping is not used as HT physical wiring.
        interpretation.map(show).map_err(Refusal::Wiring)?;
        let segments = (*show.digits()).map(|pattern| SegmentPatternForm::encode(pattern)[0]);
        let registers = self
            .profile
            .encode_seven_segment(segments, *show.decimal_points(), *show.colon())
            .map_err(|_| Refusal::Wiring(WiringRefusal::UnwiredAnnunciator))?;
        let canonical = self
            .frame
            .encode(self.address, registers)
            .map_err(Refusal::Frame)?;
        Ok(EncodedFrame {
            registers,
            canonical,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_seven_segment_mask::{DigitWiring, WireBit};

    fn interpretation() -> Tm1637Profile {
        Tm1637Profile {
            digits: core::array::from_fn(|n| DigitWiring {
                address: n as u8,
                segment_bits: [0, 1, 2, 3, 4, 5, 6],
            }),
            decimal_points: core::array::from_fn(|n| {
                Some(WireBit {
                    address: n as u8,
                    bit: 7,
                })
            }),
            colon: None,
        }
    }

    #[test]
    fn exact_selected_profile_address_and_decimal_points_reach_canonical_frame() {
        let profile = Profile::new(
            [3, 1, 5, 7],
            [13, 12, 11, 10, 9, 8, 7, 6, 5, 4, 3, 2, 1, 0],
            14,
        )
        .unwrap();
        let address = Address::new(0x73).unwrap();
        let mask = super::super::PreparedDisplayMask::prepare_with_encoder(
            interpretation(),
            conduit_seven_segment_mask::SevenSegmentLook::fahrenheit(),
            10,
            20,
            Ht16k33Encoder::prepare(profile, address).unwrap(),
        )
        .unwrap();
        assert_eq!(mask.retained_backing_bytes(), 0);
        assert_eq!(
            mask.maximum_frame_bytes(),
            conduit_ht16k33::frame::DISPLAY_FRAME_BYTES
        );
        let mut encoder = Ht16k33Encoder::prepare(profile, address).unwrap();
        let mut expected = PreparedDisplayFrame::prepare().unwrap();
        assert_eq!(encoder.retained_backing_bytes(), 0);
        assert_eq!(
            encoder.maximum_frame_bytes(),
            conduit_ht16k33::frame::DISPLAY_FRAME_BYTES
        );
        for bits in 0..128 {
            let dots = [true, false, true, false];
            let show = SevenSegmentShow::from_parts(
                [SegmentPatternForm::decode(&[bits]).unwrap(); 4],
                dots,
                false,
            );
            let ram = profile
                .encode_seven_segment([bits; 4], dots, false)
                .unwrap();
            let frame = encoder.encode(&show, &interpretation()).unwrap();
            assert_eq!(frame.registers, ram);
            assert_eq!(frame.canonical, expected.encode(address, ram).unwrap());
            conduit_ht16k33::frame::validate_display_frame(frame.canonical).unwrap();
        }
    }

    #[test]
    fn selected_or_requested_colon_and_undeclared_decimal_point_refuse() {
        let mut encoder =
            Ht16k33Encoder::prepare(Profile::adafruit_alpha4(), Address::new(0x70).unwrap())
                .unwrap();
        let mut profile = interpretation();
        profile.colon = Some(WireBit { address: 4, bit: 0 });
        assert!(matches!(
            encoder.validate_support(&profile),
            Err(Refusal::Wiring(WiringRefusal::UnwiredAnnunciator))
        ));
        let blank = SegmentPatternForm::decode(&[0]).unwrap();
        assert!(encoder
            .encode(
                &SevenSegmentShow::from_parts([blank; 4], [false; 4], true),
                &interpretation()
            )
            .is_err());
        let mut no_dots = interpretation();
        no_dots.decimal_points = [None; 4];
        assert!(encoder
            .encode(
                &SevenSegmentShow::from_parts([blank; 4], [true, false, false, false], false),
                &no_dots
            )
            .is_err());
    }
}
