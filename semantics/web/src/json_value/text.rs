//! One JSON text parser with two construction policies. Syntax inspection
//! retains no tree; canonical value decoding keeps its existing tighter bounds.
use super::*;

/// Validate bounded UTF-8 JSON text without constructing a JSON value tree.
/// The envelope is 4 KiB and nesting is at most eight values. Object keys may
/// repeat and numeric text is not converted to Scalar. Tree/profile admission
/// remains the separate responsibility of JsonValue::decode_text.
pub fn validate_json_text(input: &[u8]) -> Result<(), JsonRefusal> {
    parse::<false>(input).map(|_| ())
}
pub(super) fn decode(input: &[u8]) -> Result<JsonValue, JsonRefusal> {
    parse::<true>(input)
}
fn parse<const BUILD: bool>(input: &[u8]) -> Result<JsonValue, JsonRefusal> {
    if input.len() > JSON_MAXIMUM_ENCODED_BYTES {
        return Err(JsonRefusal::EncodedByteOverflow);
    }
    core::str::from_utf8(input).map_err(|_| JsonRefusal::InvalidUtf8)?;
    let mut parser = Parser::<BUILD> { input, offset: 0 };
    let mut budget = Budget::default();
    parser.space();
    let value = parser.value(1, &mut budget)?;
    parser.space();
    if parser.offset != input.len() {
        return Err(JsonRefusal::MalformedSyntax);
    }
    Ok(value)
}
struct Parser<'a, const BUILD: bool> {
    input: &'a [u8],
    offset: usize,
}
impl<const BUILD: bool> Parser<'_, BUILD> {
    fn space(&mut self) {
        while matches!(
            self.input.get(self.offset),
            Some(b' ' | b'\t' | b'\r' | b'\n')
        ) {
            self.offset += 1;
        }
    }
    fn value(&mut self, depth: usize, budget: &mut Budget) -> Result<JsonValue, JsonRefusal> {
        if depth > JSON_MAXIMUM_DEPTH {
            return Err(JsonRefusal::DepthOverflow);
        }
        budget.nodes += 1;
        if BUILD && budget.nodes > JSON_MAXIMUM_NODES {
            return Err(JsonRefusal::NodeOverflow);
        }
        match self.input.get(self.offset).copied() {
            Some(b'n') => {
                self.literal(b"null")?;
                Ok(JsonValue::Null)
            }
            Some(b't') => {
                self.literal(b"true")?;
                Ok(JsonValue::Bool(true))
            }
            Some(b'f') => {
                self.literal(b"false")?;
                Ok(JsonValue::Bool(false))
            }
            Some(b'"') => {
                let value = self.string()?;
                if BUILD {
                    count_string(&value, false, budget)?;
                }
                Ok(JsonValue::String(value))
            }
            Some(b'[') => self.array(depth, budget),
            Some(b'{') => self.object(depth, budget),
            Some(b'-' | b'0'..=b'9') => self.number().map(JsonValue::Number),
            _ => Err(JsonRefusal::MalformedSyntax),
        }
    }
    fn literal(&mut self, value: &[u8]) -> Result<(), JsonRefusal> {
        if self.input.get(self.offset..self.offset + value.len()) == Some(value) {
            self.offset += value.len();
            Ok(())
        } else {
            Err(JsonRefusal::MalformedSyntax)
        }
    }
    fn array(&mut self, depth: usize, budget: &mut Budget) -> Result<JsonValue, JsonRefusal> {
        self.offset += 1;
        self.space();
        let mut items = Vec::new();
        if self.eat(b']') {
            return Ok(JsonValue::Array(items));
        }
        loop {
            if BUILD && items.len() == JSON_MAXIMUM_ARRAY_ITEMS {
                return Err(JsonRefusal::ArrayItemOverflow);
            }
            let value = self.value(depth + 1, budget)?;
            if BUILD {
                items.push(value);
            }
            self.space();
            if self.eat(b']') {
                break;
            }
            if !self.eat(b',') {
                return Err(JsonRefusal::MalformedSyntax);
            }
            self.space();
        }
        Ok(JsonValue::Array(items))
    }
    fn object(&mut self, depth: usize, budget: &mut Budget) -> Result<JsonValue, JsonRefusal> {
        self.offset += 1;
        self.space();
        let mut members: Vec<(String, JsonValue)> = Vec::new();
        if self.eat(b'}') {
            return Ok(JsonValue::Object(members));
        }
        loop {
            if BUILD && members.len() == JSON_MAXIMUM_OBJECT_MEMBERS {
                return Err(JsonRefusal::ObjectMemberOverflow);
            }
            if self.input.get(self.offset) != Some(&b'"') {
                return Err(JsonRefusal::MalformedSyntax);
            }
            let key = self.string()?;
            if BUILD {
                count_string(&key, true, budget)?;
            }
            self.space();
            if !self.eat(b':') {
                return Err(JsonRefusal::MalformedSyntax);
            }
            self.space();
            if members.iter().any(|(existing, _)| existing == &key) {
                return Err(JsonRefusal::DuplicateKey);
            }
            let value = self.value(depth + 1, budget)?;
            if BUILD {
                members.push((key, value));
            }
            self.space();
            if self.eat(b'}') {
                break;
            }
            if !self.eat(b',') {
                return Err(JsonRefusal::MalformedSyntax);
            }
            self.space();
        }
        members.sort_by(|left, right| left.0.cmp(&right.0));
        Ok(JsonValue::Object(members))
    }
    fn eat(&mut self, byte: u8) -> bool {
        if self.input.get(self.offset) == Some(&byte) {
            self.offset += 1;
            true
        } else {
            false
        }
    }
    fn string(&mut self) -> Result<String, JsonRefusal> {
        self.offset += 1;
        let mut output = String::new();
        loop {
            let byte = *self
                .input
                .get(self.offset)
                .ok_or(JsonRefusal::MalformedSyntax)?;
            self.offset += 1;
            match byte {
                b'"' => return Ok(output),
                b'\\' => self.escape(&mut output)?,
                0..=0x1f => return Err(JsonRefusal::MalformedSyntax),
                _ => {
                    let tail = core::str::from_utf8(&self.input[self.offset - 1..])
                        .map_err(|_| JsonRefusal::InvalidUtf8)?;
                    let character = tail.chars().next().ok_or(JsonRefusal::MalformedSyntax)?;
                    self.offset += character.len_utf8() - 1;
                    Self::character(&mut output, character);
                }
            }
        }
    }
    fn character(output: &mut String, character: char) {
        if BUILD {
            output.push(character);
        }
    }
    fn escape(&mut self, output: &mut String) -> Result<(), JsonRefusal> {
        match self
            .input
            .get(self.offset)
            .copied()
            .ok_or(JsonRefusal::MalformedSyntax)?
        {
            b'"' => Self::character(output, '"'),
            b'\\' => Self::character(output, '\\'),
            b'/' => Self::character(output, '/'),
            b'b' => Self::character(output, '\u{08}'),
            b'f' => Self::character(output, '\u{0c}'),
            b'n' => Self::character(output, '\n'),
            b'r' => Self::character(output, '\r'),
            b't' => Self::character(output, '\t'),
            b'u' => {
                self.offset += 1;
                let first = self.hex()?;
                let scalar = if (0xd800..=0xdbff).contains(&first) {
                    if self.input.get(self.offset..self.offset + 2) != Some(b"\\u") {
                        return Err(JsonRefusal::MalformedSyntax);
                    }
                    self.offset += 2;
                    let second = self.hex()?;
                    if !(0xdc00..=0xdfff).contains(&second) {
                        return Err(JsonRefusal::MalformedSyntax);
                    }
                    0x10000 + (((first - 0xd800) as u32) << 10) + (second - 0xdc00) as u32
                } else if (0xdc00..=0xdfff).contains(&first) {
                    return Err(JsonRefusal::MalformedSyntax);
                } else {
                    first as u32
                };
                Self::character(
                    output,
                    char::from_u32(scalar).ok_or(JsonRefusal::MalformedSyntax)?,
                );
                return Ok(());
            }
            _ => return Err(JsonRefusal::MalformedSyntax),
        }
        self.offset += 1;
        Ok(())
    }
    fn hex(&mut self) -> Result<u16, JsonRefusal> {
        let bytes = self
            .input
            .get(self.offset..self.offset + 4)
            .ok_or(JsonRefusal::MalformedSyntax)?;
        let mut value = 0_u16;
        for byte in bytes {
            value = value * 16
                + u16::from(match byte {
                    b'0'..=b'9' => byte - b'0',
                    b'a'..=b'f' => byte - b'a' + 10,
                    b'A'..=b'F' => byte - b'A' + 10,
                    _ => return Err(JsonRefusal::MalformedSyntax),
                });
        }
        self.offset += 4;
        Ok(value)
    }
    fn number(&mut self) -> Result<Scalar, JsonRefusal> {
        let start = self.offset;
        let negative = self.eat(b'-');
        if self.eat(b'0') {
            if self.input.get(self.offset).is_some_and(u8::is_ascii_digit) {
                return Err(JsonRefusal::MalformedSyntax);
            }
        } else {
            let digit_start = self.offset;
            while self.input.get(self.offset).is_some_and(u8::is_ascii_digit) {
                self.offset += 1;
            }
            if self.offset == digit_start {
                return Err(JsonRefusal::MalformedSyntax);
            }
        }
        let mut fraction = 0_i32;
        if self.eat(b'.') {
            let digit_start = self.offset;
            while self.input.get(self.offset).is_some_and(u8::is_ascii_digit) {
                self.offset += 1;
                fraction += 1;
            }
            if self.offset == digit_start {
                return Err(JsonRefusal::MalformedSyntax);
            }
        }
        let mantissa_end = self.offset;
        let mut exponent = 0_i32;
        if matches!(self.input.get(self.offset), Some(b'e' | b'E')) {
            self.offset += 1;
            let exponent_negative = self.eat(b'-');
            if !exponent_negative {
                self.eat(b'+');
            }
            let digit_start = self.offset;
            while let Some(byte @ b'0'..=b'9') = self.input.get(self.offset) {
                if BUILD {
                    exponent = exponent
                        .checked_mul(10)
                        .and_then(|value| value.checked_add(i32::from(*byte - b'0')))
                        .ok_or(JsonRefusal::NumericOverflow)?;
                }
                self.offset += 1;
            }
            if self.offset == digit_start {
                return Err(JsonRefusal::MalformedSyntax);
            }
            if exponent_negative {
                exponent = -exponent;
            }
        }
        if !BUILD {
            return Ok(Scalar::from_raw_microunits(0));
        }
        let digits = self.input[start..mantissa_end]
            .iter()
            .filter(|byte| byte.is_ascii_digit())
            .try_fold(0_i128, |value, byte| {
                value
                    .checked_mul(10)
                    .and_then(|value| value.checked_add(i128::from(*byte - b'0')))
                    .ok_or(JsonRefusal::NumericOverflow)
            })?;
        let scale = fraction - exponent;
        let coefficient = if scale <= 6 {
            let multiplier = 10_i128
                .checked_pow((6 - scale) as u32)
                .ok_or(JsonRefusal::NumericOverflow)?;
            digits
                .checked_mul(multiplier)
                .ok_or(JsonRefusal::NumericOverflow)?
        } else {
            let divisor = 10_i128
                .checked_pow((scale - 6) as u32)
                .ok_or(JsonRefusal::NumericOverflow)?;
            if digits % divisor != 0 {
                return Err(JsonRefusal::NumericOverflow);
            }
            digits / divisor
        };
        let signed = if negative { -coefficient } else { coefficient };
        i64::try_from(signed)
            .map(Scalar::from_raw_microunits)
            .map_err(|_| JsonRefusal::NumericOverflow)
    }
}
