use super::rounding::rounded;
use super::{
    pattern, unavailable, PreparedEnvironmentFace, SevenSegmentSelection as Selection,
    SevenSegmentShow, NUMERALS,
};
use super::{SevenSegmentLook, SevenSegmentTemperatureUnit};
use conduit_core::{Quantity, Unit};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaskUnavailable {
    Sensor,
    Stale,
    MissingValue,
    OutsideLifetime,
    Overflow,
    NumericExtent,
    Wiring(super::WiringRefusal),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaskDisposition {
    Temperature { decimal_places: u8 },
    Humidity,
    Clock,
    PressureHectopascal,
    Unavailable(MaskUnavailable),
}
pub struct RenderedShow<'a> {
    pub face_content_id: &'a str,
    pub face_revision: u64,
    pub show: SevenSegmentShow,
    pub disposition: MaskDisposition,
}

/// Pure interpretation of one exact Face revision. Any timed selection is input.
pub fn render<'a>(
    face: &PreparedEnvironmentFace<'a>,
    selection: Selection,
    look: &SevenSegmentLook,
) -> RenderedShow<'a> {
    let value = match selection {
        Selection::Clock => clock(face),
        _ if !face.available => Err(MaskUnavailable::Sensor),
        _ if !face.fresh => Err(MaskUnavailable::Stale),
        Selection::Temperature => face
            .temperature
            .ok_or(MaskUnavailable::MissingValue)
            .and_then(|value| temperature(value, look, true)),
        Selection::Humidity => face
            .humidity
            .ok_or(MaskUnavailable::MissingValue)
            .and_then(|value| humidity(value, look)),
    };
    let (mut show, disposition) =
        value.unwrap_or_else(|why| (unavailable(), MaskDisposition::Unavailable(why)));
    if matches!(
        disposition,
        MaskDisposition::Temperature { decimal_places: 1 }
    ) && matches!(
        look.fraction_marker(),
        super::SevenSegmentFractionMarker::Colon
    ) {
        // Tenths precede the final numeral and unit, so the separator is
        // between positions 1 and 2. The Look explicitly selects the colon;
        // the hardware profile still declares it as a colon, never as a dot.
        show = SevenSegmentShow::from_parts(*show.digits(), [false; 4], true);
    }
    RenderedShow {
        face_content_id: face.content_id,
        face_revision: face.revision,
        show,
        disposition,
    }
}

/// Fit against the selected profile before publishing the Show. At most two
/// temperature candidates are considered; no transport operation is retried.
pub fn render_for_profile<'a>(
    face: &PreparedEnvironmentFace<'a>,
    selection: Selection,
    look: &SevenSegmentLook,
    profile: &super::Tm1637Profile,
) -> Result<RenderedShow<'a>, super::WiringRefusal> {
    profile.validate()?;
    let mut result = render(face, selection, look);
    if profile.map(&result.show).is_ok() {
        return Ok(result);
    }
    if matches!(
        result.disposition,
        MaskDisposition::Temperature { decimal_places: 1 }
    ) {
        if let Some(value) = face.temperature {
            if let Ok((show, disposition)) = temperature(value, look, false) {
                result.show = show;
                result.disposition = disposition;
            }
        }
    }
    if let Err(why) = profile.map(&result.show) {
        result.show = unavailable();
        result.disposition = MaskDisposition::Unavailable(MaskUnavailable::Wiring(why));
    }
    Ok(result)
}

fn number(value: i128, places: usize, suffix: u8) -> Result<SevenSegmentShow, MaskUnavailable> {
    let mut digits = [pattern(0); 4];
    let mut decimal_points = [false; 4];
    digits[3] = pattern(suffix);
    let mut position = 3usize;
    let mut magnitude = value.unsigned_abs();
    let mut used = 0;
    loop {
        position = position.checked_sub(1).ok_or(MaskUnavailable::Overflow)?;
        digits[position] = pattern(NUMERALS[(magnitude % 10) as usize]);
        magnitude /= 10;
        used += 1;
        if places != 0 && used == places + 1 {
            decimal_points[position] = true;
        }
        if magnitude == 0 && used > places {
            break;
        }
    }
    if value < 0 {
        position = position.checked_sub(1).ok_or(MaskUnavailable::Overflow)?;
        digits[position] = pattern(0x40);
    }
    Ok(SevenSegmentShow::from_parts(digits, decimal_points, false))
}
// Two numeric positions keep the explicit unit: 72°F. A three-position
// number (including its sign) uses the final position for °: 100° or -10°.
fn degree_number(value: i128, unit: u8) -> Result<SevenSegmentShow, MaskUnavailable> {
    const DEGREE: u8 = 0x63; // upper square: a, b, f, g
    if (-9..=99).contains(&value) {
        let numeric = number(value, 0, DEGREE)?;
        let mut digits = [pattern(0); 4];
        digits[..3].copy_from_slice(&numeric.digits()[1..]);
        digits[3] = pattern(unit);
        Ok(SevenSegmentShow::from_parts(digits, [false; 4], false))
    } else {
        number(value, 0, DEGREE)
    }
}
fn temperature(
    value: Quantity,
    look: &SevenSegmentLook,
    decimal_allowed: bool,
) -> Result<(SevenSegmentShow, MaskDisposition), MaskUnavailable> {
    let (unit, glyph) = match look.temperature_unit() {
        SevenSegmentTemperatureUnit::Fahrenheit => (Unit::Fahrenheit, 0x71),
        SevenSegmentTemperatureUnit::Celsius => (Unit::Celsius, 0x39),
    };
    if matches!(
        look.temperature_format(),
        super::SevenSegmentTemperatureFormat::WholeDegrees
    ) {
        let whole = rounded(value, unit, 0)?;
        let show = degree_number(whole, glyph)?;
        return Ok((show, MaskDisposition::Temperature { decimal_places: 0 }));
    }
    if decimal_allowed {
        let tenths = rounded(value, unit, 1)?;
        if let Ok(show) = number(tenths, 1, glyph) {
            return Ok((show, MaskDisposition::Temperature { decimal_places: 1 }));
        }
    }
    let whole = rounded(value, unit, 0)?;
    Ok((
        number(whole, 0, glyph)?,
        MaskDisposition::Temperature { decimal_places: 0 },
    ))
}
fn humidity(
    value: Quantity,
    _look: &SevenSegmentLook,
) -> Result<(SevenSegmentShow, MaskDisposition), MaskUnavailable> {
    let whole = rounded(value, Unit::Percent, 0)?;
    if !(0..=100).contains(&whole) {
        return Err(MaskUnavailable::Overflow);
    }
    Ok((
        SevenSegmentShow::from_parts(
            [
                pattern(0x76), // H
                pattern(if whole == 100 { NUMERALS[1] } else { 0 }),
                pattern(NUMERALS[((whole / 10) % 10) as usize]),
                pattern(NUMERALS[(whole % 10) as usize]),
            ],
            [false; 4],
            false,
        ),
        MaskDisposition::Humidity,
    ))
}

fn clock(
    face: &PreparedEnvironmentFace<'_>,
) -> Result<(SevenSegmentShow, MaskDisposition), MaskUnavailable> {
    let minute = face.minute_of_day.ok_or(MaskUnavailable::MissingValue)?;
    if !face.clock_fresh {
        return Err(MaskUnavailable::Stale);
    }
    let hour = minute / 60;
    let minute = minute % 60;
    let digits = [hour / 10, hour % 10, minute / 10, minute % 10]
        .map(|digit| pattern(NUMERALS[digit as usize]));
    Ok((
        SevenSegmentShow::from_parts(digits, [false; 4], true),
        MaskDisposition::Clock,
    ))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimingRefusal {
    InvalidBounds,
    OutsideLifetime,
}
#[derive(Clone, Copy)]
pub struct Alternation {
    period_ms: u64,
    lifetime_ms: u64,
}
impl Alternation {
    /// Admitted finite policy; the caller supplies elapsed time from its selected
    /// clock. This object creates neither a timer nor a private scheduling loop.
    pub fn new(period_ms: u64, lifetime_ms: u64) -> Result<Self, TimingRefusal> {
        if period_ms == 0 || lifetime_ms == 0 || period_ms > lifetime_ms {
            return Err(TimingRefusal::InvalidBounds);
        }
        Ok(Self {
            period_ms,
            lifetime_ms,
        })
    }
    pub fn select(&self, elapsed_ms: u64) -> Result<Selection, TimingRefusal> {
        if elapsed_ms >= self.lifetime_ms {
            return Err(TimingRefusal::OutsideLifetime);
        }
        Ok(if (elapsed_ms / self.period_ms).is_multiple_of(2) {
            Selection::Temperature
        } else {
            Selection::Humidity
        })
    }
}
