//! Local diagnostic pages use the reusable quantity Mask formatter.
//! No Body Face, admitted Plan, Play or hardware grant is fabricated here.
use conduit_bosch_env::{Humidity, Reading};
use conduit_core::{Quantity, Unit};
use conduit_seven_segment_mask::{prepare_quantity_show, SegmentPatternForm, SevenSegmentShow};
fn glyph(bits: u8) -> conduit_seven_segment_mask::SegmentPattern {
    SegmentPatternForm::decode(&[bits]).expect("reviewed glyph")
}
pub fn prepare(reading: Reading, _revision: u64) -> Result<PreparedReading, ()> {
    let temperature = Quantity::from_decimal(
        i128::from(reading.temperature_centi_celsius),
        -2,
        Unit::Celsius,
    )
    .map_err(|_| ())?;
    let show_temperature = prepare_quantity_show(
        temperature,
        Unit::Fahrenheit,
        0,
        &[],
        &[glyph(0x63), glyph(0x71)],
    )
    .or_else(|_| prepare_quantity_show(temperature, Unit::Fahrenheit, 0, &[], &[glyph(0x71)]))
    .map_err(|_| ())?;
    let missing = SevenSegmentShow::from_parts([glyph(0x40); 4], [false; 4], false);
    let humidity = match reading.humidity {
        Humidity::PercentQ10(h) => {
            let value = Quantity::from_decimal(i128::from(h) * 9_765_625, -10, Unit::Percent)
                .map_err(|_| ())?;
            prepare_quantity_show(value, Unit::Percent, 0, &[glyph(0x76)], &[]).map_err(|_| ())?
        }
        Humidity::Unavailable(_) => missing,
    };
    let pressure =
        Quantity::from_decimal(i128::from(reading.pressure_centi_pascal), -2, Unit::Pascal)
            .map_err(|_| ())?;
    let pressure =
        prepare_quantity_show(pressure, Unit::resolve("hPa").map_err(|_| ())?, 0, &[], &[])
            .map_err(|_| ())?;
    Ok(PreparedReading {
        pages: [show_temperature, humidity, pressure],
    })
}
/// Execution retains only three resolved four-cell pages; no quantity or Face.
pub struct PreparedReading {
    pages: [SevenSegmentShow; 3],
}
impl PreparedReading {
    pub fn render(&self, elapsed: u64) -> [u8; 4] {
        (*self.pages[((elapsed / 2000) % 3) as usize].digits())
            .map(|g| SegmentPatternForm::encode(g)[0])
    }
}
/// Map the selected logical Show through the checked physical wiring.
/// No character recognition, quantity formatting or redraw effects occur here.
pub fn frame(reading: &PreparedReading, elapsed: u64) -> Result<[u8; 16], ()> {
    let show = &reading.pages[((elapsed / 2000) % 3) as usize];
    crate::config::display_profile()
        .encode_seven_segment(
            (*show.digits()).map(|g| SegmentPatternForm::encode(g)[0]),
            *show.decimal_points(),
            *show.colon(),
        )
        .map_err(|_| ())
}
