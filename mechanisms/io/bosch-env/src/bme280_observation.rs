//! Pure BME280 fixed-point compensation matching the authored observation stages.
//! No I/O, allocation, calibration cache, admission or execution authority.
//! Unlike `Sensor::measure_exact`, this profile uses signed arithmetic shifts,
//! I128 coordinates and malformed -> disabled -> invalid-calibration precedence.
//! Formulas derive from Bosch's fixed-point compensation (see LICENSE-BOSCH).
use arithmetic::Checked as C;

mod arithmetic;

/// Decoded factory coefficients, before semantic range validation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Calibration {
    pub t1: i128,
    pub t2: i128,
    pub t3: i128,
    pub p1: i128,
    pub p2: i128,
    pub p3: i128,
    pub p4: i128,
    pub p5: i128,
    pub p6: i128,
    pub p7: i128,
    pub p8: i128,
    pub p9: i128,
    pub h1: i128,
    pub h2: i128,
    pub h3: i128,
    pub h4: i128,
    pub h5: i128,
    pub h6: i128,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sample {
    pub pressure: i128,
    pub temperature: i128,
    pub humidity: i128,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Observation {
    pub temperature_centidegrees: i128,
    pub pressure_q24_8_pa: i128,
    pub humidity_q22_10_percent: i128,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unavailable {
    Malformed,
    SampleDisabled,
    InvalidCalibration,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    Unavailable(Unavailable),
    /// A checked I128 stage failed; this is not a sensor-unavailable value.
    Arithmetic,
}

/// One finite, allocation-free calculation over already decoded values.
/// This is a candidate native implementation, not a registered Back. The caller
/// must still bind the exact Forms, admitted operation and resource ownership.
/// No physical-range clamp is added beyond the Source's humidity clamp.
pub fn compensate(c: Calibration, s: Sample) -> Result<Observation, Error> {
    validate(c, s).map_err(Error::Unavailable)?;
    let t = C::from(s.temperature);
    let t1 = C::from(c.t1);
    let delta = (t >> 4) - t1;
    let fine = ((((t >> 3) - (t1 << 1)) * C::from(c.t2)) >> 11)
        + ((((delta * delta) >> 12) * C::from(c.t3)) >> 14);
    let temperature = ((fine * C::from(5) + C::from(128)) >> 8).value()?;
    let v = fine - C::from(128000);
    let offset = v * v * C::from(c.p6) + ((v * C::from(c.p5)) << 17) + (C::from(c.p4) << 35);
    let numerator = (((C::from(1048576) - C::from(s.pressure)) << 31) - offset) * C::from(3125);
    let denominator =
        (((C::from(1) << 47) + ((v * v * C::from(c.p3)) >> 8) + ((v * C::from(c.p2)) << 12))
            * C::from(c.p1))
            >> 33;
    // Resolve each earlier stage before testing the semantic zero divisor.
    let numerator = C::from(numerator.value()?);
    let denominator = denominator.value()?;
    if denominator == 0 {
        return Err(Error::Unavailable(Unavailable::InvalidCalibration));
    }
    let pressure = numerator / C::from(denominator);
    let pressure = ((pressure
        + ((C::from(c.p9) * (pressure >> 13) * (pressure >> 13)) >> 25)
        + ((C::from(c.p8) * pressure) >> 19))
        >> 8)
        + (C::from(c.p7) << 4);
    let pressure = pressure.value()?;
    let hv = fine - C::from(76800);
    let humidity_first = ((C::from(s.humidity) << 14) - (C::from(c.h4) << 20) - C::from(c.h5) * hv
        + C::from(16384))
        >> 15;
    let humidity_second = ((((((hv * C::from(c.h6)) >> 10)
        * (((hv * C::from(c.h3)) >> 11) + C::from(32768)))
        >> 10)
        + C::from(2097152))
        * C::from(c.h2)
        + C::from(8192))
        >> 14;
    let humidity = humidity_first * humidity_second;
    let humidity = humidity - (((((humidity >> 15) * (humidity >> 15)) >> 7) * C::from(c.h1)) >> 4);
    let humidity = humidity.value()?;
    let humidity = if humidity < 0 {
        0
    } else if humidity > 419430400 {
        102400
    } else {
        humidity >> 12
    };
    Ok(Observation {
        temperature_centidegrees: temperature,
        pressure_q24_8_pa: pressure,
        humidity_q22_10_percent: humidity,
    })
}

fn validate(c: Calibration, s: Sample) -> Result<(), Unavailable> {
    let signed16 = [
        c.t2, c.t3, c.p2, c.p3, c.p4, c.p5, c.p6, c.p7, c.p8, c.p9, c.h2,
    ];
    if ![s.pressure, s.temperature]
        .into_iter()
        .all(|v| (0..=1048575).contains(&v))
        || !(0..=65535).contains(&s.humidity)
        || ![c.t1, c.p1].into_iter().all(|v| (0..=65535).contains(&v))
        || ![c.h1, c.h3].into_iter().all(|v| (0..=255).contains(&v))
        || !signed16.into_iter().all(|v| (-32768..=32767).contains(&v))
        || ![c.h4, c.h5]
            .into_iter()
            .all(|v| (-2048..=2047).contains(&v))
        || !(-128..=127).contains(&c.h6)
    {
        return Err(Unavailable::Malformed);
    }
    if s.pressure == 524288 || s.temperature == 524288 || s.humidity == 32768 {
        return Err(Unavailable::SampleDisabled);
    }
    if c.p1 == 0 {
        return Err(Unavailable::InvalidCalibration);
    }
    Ok(())
}
