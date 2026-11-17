//! Borrowed bounded Open-Meteo JSON scanner; no dynamic value tree.
use super::*;

pub(super) struct Reading {
    pub(super) time: u64,
    pub(super) temperature: i64,
    pub(super) code: u16,
}
// Full JSON syntax validation is bounded by body size and sixteen nesting levels.
// Strings are decoded into stack scratch so escaped relevant keys cannot bypass
// duplicate detection. Unknown members are validated, then discarded.
struct Json<'a> {
    bytes: &'a [u8],
    at: usize,
}
impl<'a> Json<'a> {
    fn ws(&mut self) {
        while self
            .bytes
            .get(self.at)
            .is_some_and(|b| matches!(b, b' ' | b'\n' | b'\r' | b'\t'))
        {
            self.at += 1;
        }
    }
    fn take(&mut self, b: u8) -> Result<()> {
        self.ws();
        if self.bytes.get(self.at) != Some(&b) {
            return Err(malformed(()));
        }
        self.at += 1;
        Ok(())
    }
    fn peek(&mut self) -> Option<u8> {
        self.ws();
        self.bytes.get(self.at).copied()
    }
    fn hex(&mut self) -> Result<u16> {
        let mut n = 0;
        for _ in 0..4 {
            let b = *self.bytes.get(self.at).ok_or_else(|| malformed(()))?;
            self.at += 1;
            n = n * 16
                + match b {
                    b'0'..=b'9' => (b - b'0') as u16,
                    b'a'..=b'f' => (b - b'a' + 10) as u16,
                    b'A'..=b'F' => (b - b'A' + 10) as u16,
                    _ => return Err(malformed(())),
                };
        }
        Ok(n)
    }
    fn string(&mut self) -> Result<u8> {
        self.take(b'"')?;
        let mut scratch = [0u8; LIMIT];
        let mut n = 0;
        loop {
            let b = *self.bytes.get(self.at).ok_or_else(|| malformed(()))?;
            self.at += 1;
            if b == b'"' {
                break;
            }
            if b < 0x20 {
                return Err(malformed(()));
            }
            if b != b'\\' {
                scratch[n] = b;
                n += 1;
                continue;
            }
            let b = *self.bytes.get(self.at).ok_or_else(|| malformed(()))?;
            self.at += 1;
            let value = match b {
                b'"' => b'"',
                b'\\' => b'\\',
                b'/' => b'/',
                b'b' => 8,
                b'f' => 12,
                b'n' => 10,
                b'r' => 13,
                b't' => 9,
                b'u' => {
                    let first = self.hex()?;
                    let scalar = if (0xd800..=0xdbff).contains(&first) {
                        if self.bytes.get(self.at..self.at + 2) != Some(b"\\u") {
                            return Err(malformed(()));
                        }
                        self.at += 2;
                        let second = self.hex()?;
                        if !(0xdc00..=0xdfff).contains(&second) {
                            return Err(malformed(()));
                        }
                        0x10000 + ((u32::from(first) - 0xd800) << 10) + (u32::from(second) - 0xdc00)
                    } else {
                        u32::from(first)
                    };
                    let ch = char::from_u32(scalar).ok_or_else(|| malformed(()))?;
                    n += ch.encode_utf8(&mut scratch[n..]).len();
                    continue;
                }
                _ => return Err(malformed(())),
            };
            scratch[n] = value;
            n += 1;
        }
        let text = std::str::from_utf8(&scratch[..n]).map_err(malformed)?;
        Ok(match text {
            "current" => 1,
            "current_units" => 2,
            "time" => 3,
            "temperature_2m" => 4,
            "weather_code" => 5,
            "unixtime" => 6,
            "°C" => 7,
            "wmo code" => 8,
            _ => 0,
        })
    }
    fn number(&mut self) -> Result<&'a [u8]> {
        self.ws();
        let start = self.at;
        if self.bytes.get(self.at) == Some(&b'-') {
            self.at += 1;
        }
        match self.bytes.get(self.at) {
            Some(b'0') => self.at += 1,
            Some(b'1'..=b'9') => {
                self.at += 1;
                while self.bytes.get(self.at).is_some_and(u8::is_ascii_digit) {
                    self.at += 1;
                }
            }
            _ => return Err(malformed(())),
        }
        if self.bytes.get(self.at) == Some(&b'.') {
            self.at += 1;
            self.digits()?;
        }
        if matches!(self.bytes.get(self.at), Some(b'e' | b'E')) {
            self.at += 1;
            if matches!(self.bytes.get(self.at), Some(b'+' | b'-')) {
                self.at += 1;
            }
            self.digits()?;
        }
        Ok(&self.bytes[start..self.at])
    }
    fn digits(&mut self) -> Result<()> {
        let start = self.at;
        while self.bytes.get(self.at).is_some_and(u8::is_ascii_digit) {
            self.at += 1;
        }
        if self.at == start {
            return Err(malformed(()));
        }
        Ok(())
    }
    fn skip(&mut self, depth: u8) -> Result<()> {
        if depth > 16 {
            return Err(malformed(()));
        }
        match self.peek() {
            Some(b'"') => {
                self.string()?;
            }
            Some(b'{' | b'[') => {
                let object = self.peek() == Some(b'{');
                self.at += 1;
                let end = if object { b'}' } else { b']' };
                if self.peek() == Some(end) {
                    self.at += 1;
                    return Ok(());
                }
                loop {
                    if object {
                        self.string()?;
                        self.take(b':')?;
                    }
                    self.skip(depth + 1)?;
                    if self.peek() == Some(end) {
                        self.at += 1;
                        break;
                    }
                    self.take(b',')?;
                }
            }
            Some(b't') => self.literal(b"true")?,
            Some(b'f') => self.literal(b"false")?,
            Some(b'n') => self.literal(b"null")?,
            _ => {
                self.number()?;
            }
        }
        Ok(())
    }
    fn literal(&mut self, value: &[u8]) -> Result<()> {
        if self.bytes.get(self.at..self.at + value.len()) != Some(value) {
            return Err(malformed(()));
        }
        self.at += value.len();
        Ok(())
    }
    fn object(&mut self, mut member: impl FnMut(&mut Self, u8) -> Result<()>) -> Result<()> {
        self.take(b'{')?;
        if self.peek() == Some(b'}') {
            self.at += 1;
            return Ok(());
        }
        loop {
            let key = self.string()?;
            self.take(b':')?;
            member(self, key)?;
            if self.peek() == Some(b'}') {
                self.at += 1;
                break;
            }
            self.take(b',')?;
        }
        Ok(())
    }
}
pub(super) fn decode(body: &[u8]) -> Result<Reading> {
    let mut json = Json { bytes: body, at: 0 };
    let mut seen = 0u8;
    let mut reading = Reading {
        time: 0,
        temperature: 0,
        code: 0,
    };
    json.object(|json, key| {
        if key != 1 && key != 2 {
            return json.skip(1);
        }
        let bit = 1 << key;
        if seen & bit != 0 {
            return Err(malformed(()));
        }
        seen |= bit;
        let mut fields = 0u8;
        let mut bad_units = false;
        json.object(|json, field| {
            if !(3..=5).contains(&field) {
                return json.skip(2);
            }
            let bit = 1 << field;
            if fields & bit != 0 {
                return Err(malformed(()));
            }
            fields |= bit;
            if key == 2 {
                let unit = json.string()?;
                bad_units |= unit != field + 3;
                return Ok(());
            }
            let token = json.number()?;
            match field {
                3 => reading.time = unsigned(token)?,
                4 => reading.temperature = temperature(token)?,
                5 => reading.code = unsigned(token)?.try_into().map_err(malformed)?,
                _ => unreachable!(),
            };
            Ok(())
        })?;
        if fields != 0b111000 {
            return Err(malformed(()));
        }
        if bad_units {
            return Err(AmbientWeatherRefusal::UnsupportedUnits);
        }
        Ok(())
    })?;
    if seen != 0b110 || json.peek().is_some() {
        return Err(malformed(()));
    }
    Ok(reading)
}
fn unsigned(token: &[u8]) -> Result<u64> {
    let mut n = 0u64;
    for &b in token {
        if !b.is_ascii_digit() {
            return Err(malformed(()));
        }
        n = n
            .checked_mul(10)
            .and_then(|v| v.checked_add(u64::from(b - b'0')))
            .ok_or(AmbientWeatherRefusal::TimestampOverflow)?;
    }
    Ok(n)
}
fn temperature(token: &[u8]) -> Result<i64> {
    let fail = || AmbientWeatherRefusal::TemperaturePrecision;
    if token.len() > 32 {
        return Err(fail());
    }
    let text = std::str::from_utf8(token).map_err(|_| fail())?;
    let (negative, text) = text.strip_prefix('-').map_or((false, text), |t| (true, t));
    let (mantissa, exponent) = match text.find(['e', 'E']) {
        Some(i) => (
            &text[..i],
            text[i + 1..].parse::<i32>().map_err(|_| fail())?,
        ),
        None => (text, 0),
    };
    let (whole, fraction) = mantissa.split_once('.').unwrap_or((mantissa, ""));
    let mut n = 0u64;
    for b in whole.bytes().chain(fraction.bytes()) {
        n = n
            .checked_mul(10)
            .and_then(|v| v.checked_add(u64::from(b - b'0')))
            .ok_or_else(fail)?;
    }
    let scale = exponent
        .checked_add(3)
        .and_then(|v| v.checked_sub(fraction.len() as i32))
        .ok_or_else(fail)?;
    let factor = 10u64.checked_pow(scale.unsigned_abs()).ok_or_else(fail)?;
    n = if scale >= 0 {
        n.checked_mul(factor).ok_or_else(fail)?
    } else {
        if !n.is_multiple_of(factor) {
            return Err(fail());
        }
        n / factor
    };
    let n = i64::try_from(n).map_err(|_| fail())?;
    Ok(if negative { -n } else { n })
}
