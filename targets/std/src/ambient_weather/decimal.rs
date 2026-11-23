//! Preserve the provider's original numeric lexeme before binary JSON-number
//! normalization can erase a sub-millidegree fraction. The enclosing response
//! must already have passed serde's full JSON/schema/duplicate-field checks.
use super::AmbientWeatherRefusal;

pub(super) fn temperature_millidegrees(body: &[u8]) -> Result<i64, AmbientWeatherRefusal> {
    let fail = || AmbientWeatherRefusal::TemperaturePrecision;
    let current = member(body, "current").ok_or_else(fail)?;
    let token = member(current, "temperature_2m").ok_or_else(fail)?;
    if token.len() > 32 {
        return Err(fail());
    }
    let text = std::str::from_utf8(token).map_err(|_| fail())?;
    let (negative, text) = text.strip_prefix('-').map_or((false, text), |v| (true, v));
    let (mantissa, exponent) = match text.find(['e', 'E']) {
        Some(at) => (
            &text[..at],
            text[at + 1..].parse::<i32>().map_err(|_| fail())?,
        ),
        None => (text, 0),
    };
    let (whole, fraction) = mantissa.split_once('.').unwrap_or((mantissa, ""));
    let mut magnitude = 0_u64;
    for digit in whole.bytes().chain(fraction.bytes()) {
        if !digit.is_ascii_digit() {
            return Err(fail());
        }
        magnitude = magnitude
            .checked_mul(10)
            .and_then(|v| v.checked_add(u64::from(digit - b'0')))
            .ok_or_else(fail)?;
    }
    let scale = exponent
        .checked_add(3)
        .and_then(|v| v.checked_sub(fraction.len() as i32))
        .ok_or_else(fail)?;
    let factor = 10_u64.checked_pow(scale.unsigned_abs()).ok_or_else(fail)?;
    magnitude = if scale >= 0 {
        magnitude.checked_mul(factor).ok_or_else(fail)?
    } else {
        if !magnitude.is_multiple_of(factor) {
            return Err(fail());
        }
        magnitude / factor
    };
    let value = i64::try_from(magnitude).map_err(|_| fail())?;
    Ok(if negative { -value } else { value })
}

fn member<'a>(object: &'a [u8], name: &str) -> Option<&'a [u8]> {
    let mut at = whitespace(object, 0);
    (object.get(at) == Some(&b'{')).then_some(())?;
    at += 1;
    loop {
        at = whitespace(object, at);
        let key_end = string_end(object, at)?;
        let key: String = serde_json::from_slice(&object[at..key_end]).ok()?;
        at = whitespace(object, key_end);
        (object.get(at) == Some(&b':')).then_some(())?;
        at = whitespace(object, at + 1);
        let end = value_end(object, at)?;
        if key == name {
            return Some(&object[at..end]);
        }
        at = whitespace(object, end);
        (object.get(at) == Some(&b',')).then_some(())?;
        at += 1;
    }
}

fn whitespace(bytes: &[u8], mut at: usize) -> usize {
    while bytes.get(at).is_some_and(|b| b.is_ascii_whitespace()) {
        at += 1;
    }
    at
}

fn string_end(bytes: &[u8], at: usize) -> Option<usize> {
    (bytes.get(at) == Some(&b'"')).then_some(())?;
    let mut at = at + 1;
    loop {
        match bytes.get(at)? {
            b'"' => return Some(at + 1),
            b'\\' => at += 2,
            _ => at += 1,
        }
    }
}

fn value_end(bytes: &[u8], mut at: usize) -> Option<usize> {
    if bytes.get(at) == Some(&b'"') {
        return string_end(bytes, at);
    }
    if matches!(bytes.get(at), Some(b'{' | b'[')) {
        let mut depth = 0_u32;
        loop {
            match bytes.get(at)? {
                b'"' => {
                    at = string_end(bytes, at)?;
                    continue;
                }
                b'{' | b'[' => depth += 1,
                b'}' | b']' => {
                    depth -= 1;
                    if depth == 0 {
                        return Some(at + 1);
                    }
                }
                _ => (),
            }
            at += 1;
        }
    }
    let start = at;
    while bytes
        .get(at)
        .is_some_and(|b| !b.is_ascii_whitespace() && !matches!(b, b',' | b'}' | b']'))
    {
        at += 1;
    }
    (at > start).then_some(at)
}
