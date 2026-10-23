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
    Overflow,
    NumericExtent,
    Wiring(super::WiringRefusal),
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MaskDisposition {
    Temperature { decimal_places: u8 },
    Humidity,
    Clock,
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
            .and_then(humidity),
    };
    let (show, disposition) =
        value.unwrap_or_else(|why| (unavailable(), MaskDisposition::Unavailable(why)));
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
fn temperature(
    value: Quantity,
    look: &SevenSegmentLook,
    decimal_allowed: bool,
) -> Result<(SevenSegmentShow, MaskDisposition), MaskUnavailable> {
    let (unit, glyph) = match look.temperature_unit() {
        SevenSegmentTemperatureUnit::Fahrenheit => (Unit::Fahrenheit, 0x71),
        SevenSegmentTemperatureUnit::Celsius => (Unit::Celsius, 0x39),
    };
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
fn humidity(value: Quantity) -> Result<(SevenSegmentShow, MaskDisposition), MaskUnavailable> {
    Ok((
        number(rounded(value, Unit::Percent, 0)?, 0, 0x76)?,
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
