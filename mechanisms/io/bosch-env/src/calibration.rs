// Compensation adapted from Bosch BME280_SensorAPI c90d419492e26dd95586598a794e65eb2760753a.
// Copyright (c) 2020 Bosch Sensortec GmbH. All rights reserved.
// SPDX-License-Identifier: BSD-3-Clause; full terms in ../LICENSE-BOSCH.
use crate::{Chip, DataRefusal, Humidity, HumidityUnavailable, Reading};
#[derive(Clone, Copy)]
pub(super) struct Calibration {
    t1: u16,
    t2: i16,
    t3: i16,
    p1: u16,
    p: [i16; 8],
    h1: u8,
    h2: i16,
    h3: u8,
    h4: i16,
    h5: i16,
    h6: i8,
}
impl Calibration {
    pub(super) fn parse(chip: Chip, tp: &[u8; 26], h: &[u8; 7]) -> Result<Self, DataRefusal> {
        if tp[..24].iter().all(|v| *v == 0) || tp[..24].iter().all(|v| *v == 255) {
            return Err(DataRefusal::Calibration);
        }
        let u = |i| u16::from_le_bytes([tp[i], tp[i + 1]]);
        let mut p = [0; 8];
        for (i, v) in p.iter_mut().enumerate() {
            *v = u(8 + 2 * i) as i16;
        }
        if u(6) == 0 {
            return Err(DataRefusal::Calibration);
        }
        let (h1, h2, h3, h4, h5, h6) = if chip == Chip::Bme280 {
            if h.iter().all(|v| *v == 0) || h.iter().all(|v| *v == 255) {
                return Err(DataRefusal::Calibration);
            }
            (
                tp[25],
                i16::from_le_bytes([h[0], h[1]]),
                h[2],
                (i16::from(h[3] as i8) * 16) | i16::from(h[4] & 15),
                (i16::from(h[5] as i8) * 16) | i16::from(h[4] >> 4),
                h[6] as i8,
            )
        } else {
            (0, 0, 0, 0, 0, 0)
        };
        Ok(Self {
            t1: u(0),
            t2: u(2) as i16,
            t3: u(4) as i16,
            p1: u(6),
            p,
            h1,
            h2,
            h3,
            h4,
            h5,
            h6,
        })
    }
    pub(super) fn compensate(&self, chip: Chip, raw: &[u8; 8]) -> Result<Reading, DataRefusal> {
        let adc = |i| {
            (i128::from(raw[i]) << 12)
                | (i128::from(raw[i + 1]) << 4)
                | (i128::from(raw[i + 2]) >> 4)
        };
        let pressure = adc(0);
        let temperature = adc(3);
        let humidity = i128::from(u16::from_be_bytes([raw[6], raw[7]]));
        // Conservatively refuse documented reset/skipped sentinels, never export them.
        if pressure == 0x80000
            || temperature == 0x80000
            || (chip == Chip::Bme280 && humidity == 0x8000)
        {
            return Err(DataRefusal::SkippedData);
        }
        let v1 = ((temperature / 8) - i128::from(self.t1) * 2) * i128::from(self.t2) / 2048;
        let v2 = temperature / 16 - i128::from(self.t1);
        let v2 = ((v2 * v2) / 4096) * i128::from(self.t3) / 16384;
        let fine = v1 + v2;
        let centi = (fine * 5 + 128) / 256;
        if !(-4000..=8500).contains(&centi) {
            return Err(DataRefusal::OutOfRange);
        }
        let p = |i| i128::from(self.p[i]);
        let v = fine - 128000;
        let offset = v * v * p(4) + v * p(3) * 131072 + p(2) * 34359738368;
        let denominator = ((140737488355328_i128 + (v * v * p(1)) / 256 + v * p(0) * 4096)
            * i128::from(self.p1))
            / 8589934592;
        if denominator == 0 {
            return Err(DataRefusal::Calibration);
        }
        let value = (((1048576 - pressure) * 2147483648 - offset) * 3125) / denominator;
        let squared = (value / 8192)
            .checked_mul(value / 8192)
            .and_then(|v| v.checked_mul(p(7)))
            .ok_or(DataRefusal::Arithmetic)?
            / 33554432;
        let value = (value + squared + p(6) * value / 524288) / 256 + p(5) * 16;
        let pascal_centi = ((value / 2) * 100) / 128;
        if !(3000000..=11000000).contains(&pascal_centi) {
            return Err(DataRefusal::OutOfRange);
        }
        let humidity = if chip == Chip::Bme280 {
            let v1 = fine - 76800;
            let v5 =
                ((humidity * 16384 - i128::from(self.h4) * 1048576 - i128::from(self.h5) * v1)
                    + 16384)
                    / 32768;
            let v2 = v1 * i128::from(self.h6) / 1024;
            let v3 = v1 * i128::from(self.h3) / 2048;
            let v4 = (v2 * (v3 + 32768)) / 1024 + 2097152;
            let v2 = (v4 * i128::from(self.h2) + 8192) / 16384;
            let v3 = v5 * v2;
            let v4 = ((v3 / 32768) * (v3 / 32768)) / 128;
            let v5 = (v3 - v4 * i128::from(self.h1) / 16).clamp(0, 419430400);
            Humidity::PercentQ10((v5 / 4096) as u32)
        } else {
            Humidity::Unavailable(HumidityUnavailable::UnsupportedChip)
        };
        Ok(Reading {
            temperature_centi_celsius: centi as i32,
            pressure_centi_pascal: pascal_centi as u32,
            humidity,
        })
    }
}
#[cfg(test)]
#[path = "../tests/unit/compensation.rs"]
mod tests;
