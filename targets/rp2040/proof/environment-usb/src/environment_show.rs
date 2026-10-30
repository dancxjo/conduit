//! Hardware-independent composition using the common environmental Face and Look.
//! Diagnostic FaceBasis has no Body, Wake, Source, Plan, or active Play identity.
extern crate alloc;
use alloc::vec;
#[cfg(not(environment_dht11_tm1637))]
use conduit_bosch_env::{Humidity, Reading};
use conduit_environment_face::{contribution_reading, EnvironmentalReading, SensorStatus};
use conduit_face::{Face, FaceBasis, FaceContributionBasis};
use conduit_seven_segment_mask::{
    DigitWiring, PreparedEnvironmentalFace, PreparedEnvironmentalShow, SegmentPatternForm,
    SevenSegmentLook, TimedEnvironmentalMask, Tm1637Profile,
};
fn profile() -> Tm1637Profile {
    Tm1637Profile {
        digits: core::array::from_fn(|i| DigitWiring {
            address: i as u8,
            segment_bits: [0, 1, 2, 3, 4, 5, 6],
        }),
        decimal_points: [None; 4],
        colon: Some(conduit_seven_segment_mask::WireBit { address: 1, bit: 7 }),
    }
}
#[cfg(not(environment_dht11_tm1637))]
pub fn prepare(reading: Reading, revision: u64) -> Result<PreparedReading, ()> {
    let humidity = match reading.humidity {
        Humidity::PercentQ10(v) => Some(v),
        Humidity::Unavailable(_) => None,
    };
    let common = EnvironmentalReading::centi_celsius_centi_pascal(
        reading.temperature_centi_celsius,
        reading.pressure_centi_pascal,
        humidity,
    )
    .map_err(|_| ())?;
    prepare_common(common, revision)
}
pub fn prepare_common(common: EnvironmentalReading, revision: u64) -> Result<PreparedReading, ()> {
    // Explicit diagnostic correlation labels, never a claim of checked/admitted IDs.
    let basis = FaceContributionBasis {
        checked_plot_id: "diagnostic/environment-reading".into(),
        plan_id: "diagnostic/no-admitted-plan".into(),
        active_play_id: "diagnostic/no-active-play".into(),
        required_interaction_context: None,
    };
    let fragment =
        contribution_reading(common, true, SensorStatus::Available, basis).map_err(|_| ())?;
    let face = Face::new(
        revision,
        FaceBasis {
            body_id: None,
            wake_id: None,
            source_document_id: None,
            checked_plot_id: None,
            expanded_plot_id: None,
            plan_id: None,
            active_play_id: None,
            sign_ids: vec![],
        },
        fragment.subjects,
        fragment.relationships,
        fragment.properties,
        fragment.text,
    )
    .map_err(|_| ())?;
    let projected = PreparedEnvironmentalFace::prepare(&face, "environment").map_err(|_| ())?;
    let mask = TimedEnvironmentalMask::prepare(
        SevenSegmentLook::fahrenheit().with_whole_degrees(),
        2000,
        30_000,
    )
    .map_err(|_| ())?;
    let profile = profile();
    let show = mask.prepare_show(&projected, &profile).map_err(|_| ())?;
    Ok(PreparedReading { show })
}
/// Own only resolved finite pages after sample preparation. No Face, Quantity,
/// allocation, clock or hardware owner is retained for timed execution.
pub struct PreparedReading {
    show: PreparedEnvironmentalShow,
}
impl PreparedReading {
    pub fn render(&self, elapsed: u64) -> [u8; 4] {
        (*self.show.at(elapsed).show.digits()).map(|pattern| SegmentPatternForm::encode(pattern)[0])
    }
}
/// Finite environmental seven-segment glyph compatibility adapter to HT wiring.
/// This is not a general fourteen-segment renderer; it consumes logical Show
/// segment Forms, independently of the TM1637 physical register mapping.
/// This adapter performs no quantity formatting, rounding or page selection.
#[cfg(not(any(environment_dht11_tm1637, environment_bosch_tm1637)))]
pub fn frame(reading: &PreparedReading, elapsed: u64) -> Result<[u8; 16], ()> {
    let digits = reading.render(elapsed);
    let mut glyphs = [conduit_ht16k33::Glyph::from_char(' ').map_err(|_| ())?; 4];
    const NUMERALS: [u8; 10] = [0x3f, 0x06, 0x5b, 0x4f, 0x66, 0x6d, 0x7d, 0x07, 0x7f, 0x6f];
    for (out, bits) in glyphs.iter_mut().zip(digits) {
        let character = if let Some(n) = NUMERALS.iter().position(|b| *b == bits) {
            char::from(b'0' + n as u8)
        } else {
            match bits {
                0 => ' ',
                0x40 => '-',
                0x63 => '°',
                0x71 => 'F',
                0x39 => 'C',
                0x76 => 'H',
                _ => return Err(()),
            }
        };
        *out = conduit_ht16k33::Glyph::from_char(character).map_err(|_| ())?;
    }
    Ok(crate::config::display_profile().encode(glyphs, [false; 4]))
}
