//! One finite, canonical JSON semantic value and whole-document codec.

use alloc::string::String;
use alloc::vec::Vec;
use conduit_core::Scalar;

pub const JSON_INFO_ID: &str = "value/json@1";
pub const JSON_TEXT_INFO_ID: &str = "text/json-utf8@1";
pub const JSON_MAXIMUM_DEPTH: usize = 8;
pub const JSON_MAXIMUM_NODES: usize = 128;
pub const JSON_MAXIMUM_ARRAY_ITEMS: usize = 32;
pub const JSON_MAXIMUM_OBJECT_MEMBERS: usize = 32;
pub const JSON_MAXIMUM_KEY_BYTES: usize = 64;
pub const JSON_MAXIMUM_STRING_BYTES: usize = 1_024;
pub const JSON_MAXIMUM_TOTAL_STRING_BYTES: usize = 2_048;
pub const JSON_MAXIMUM_ENCODED_BYTES: usize = 4_096;

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum JsonValue {
    Null,
    Bool(bool),
    /// A signed fixed-point number with exactly the `Scalar` six-decimal profile.
    Number(Scalar),
    String(String),
    Array(Vec<JsonValue>),
    /// Members are canonical only in strictly increasing UTF-8 key order.
    Object(Vec<(String, JsonValue)>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u16)]
pub enum JsonRefusal {
    MalformedSyntax = 1,
    InvalidUtf8 = 2,
    DepthOverflow = 3,
    ArrayItemOverflow = 4,
    ObjectMemberOverflow = 5,
    NodeOverflow = 6,
    KeyByteOverflow = 7,
    StringByteOverflow = 8,
    TotalStringByteOverflow = 9,
    EncodedByteOverflow = 10,
    NumericOverflow = 11,
    DuplicateKey = 12,
    NonCanonicalValue = 13,
}

impl JsonValue {
    pub fn validate(&self) -> Result<(), JsonRefusal> {
        let mut budget = Budget::default();
        validate_value(self, 1, &mut budget)
    }

    pub fn encode_info(&self) -> Result<Vec<u8>, JsonRefusal> {
        self.validate()?;
        let mut output = Vec::new();
        encode_binary(self, &mut output)?;
        bounded(output)
    }

    pub fn decode_info(input: &[u8]) -> Result<Self, JsonRefusal> {
        if input.len() > JSON_MAXIMUM_ENCODED_BYTES {
            return Err(JsonRefusal::EncodedByteOverflow);
        }
        let mut cursor = BinaryCursor { input, offset: 0 };
        let value = cursor.value()?;
        if cursor.offset != input.len() {
            return Err(JsonRefusal::NonCanonicalValue);
        }
        value.validate()?;
        if value.encode_info()?.as_slice() != input {
            return Err(JsonRefusal::NonCanonicalValue);
        }
        Ok(value)
    }

    pub fn encode_text(&self) -> Result<Vec<u8>, JsonRefusal> {
        self.validate()?;
        let mut output = Vec::new();
        encode_text_value(self, &mut output)?;
        bounded(output)
    }

    pub fn decode_text(input: &[u8]) -> Result<Self, JsonRefusal> {
        if input.len() > JSON_MAXIMUM_ENCODED_BYTES {
            return Err(JsonRefusal::EncodedByteOverflow);
        }
        core::str::from_utf8(input).map_err(|_| JsonRefusal::InvalidUtf8)?;
        let mut parser = Parser { input, offset: 0 };
        let mut budget = Budget::default();
        parser.space();
        let value = parser.value(1, &mut budget)?;
        parser.space();
        if parser.offset != input.len() {
            return Err(JsonRefusal::MalformedSyntax);
        }
        Ok(value)
    }
}

#[derive(Default)]
struct Budget {
    nodes: usize,
    string_bytes: usize,
}

fn validate_value(value: &JsonValue, depth: usize, budget: &mut Budget) -> Result<(), JsonRefusal> {
    if depth > JSON_MAXIMUM_DEPTH {
        return Err(JsonRefusal::DepthOverflow);
    }
    budget.nodes += 1;
    if budget.nodes > JSON_MAXIMUM_NODES {
        return Err(JsonRefusal::NodeOverflow);
    }
    match value {
        JsonValue::Null | JsonValue::Bool(_) | JsonValue::Number(_) => {}
        JsonValue::String(value) => count_string(value, false, budget)?,
        JsonValue::Array(items) => {
            if items.len() > JSON_MAXIMUM_ARRAY_ITEMS {
                return Err(JsonRefusal::ArrayItemOverflow);
            }
            for item in items {
                validate_value(item, depth + 1, budget)?;
            }
        }
        JsonValue::Object(members) => {
            if members.len() > JSON_MAXIMUM_OBJECT_MEMBERS {
                return Err(JsonRefusal::ObjectMemberOverflow);
            }
            let mut previous: Option<&str> = None;
            for (key, value) in members {
                count_string(key, true, budget)?;
                if previous.is_some_and(|previous| previous >= key.as_str()) {
                    return Err(JsonRefusal::NonCanonicalValue);
                }
                previous = Some(key);
                validate_value(value, depth + 1, budget)?;
            }
        }
    }
    Ok(())
}

fn count_string(value: &str, key: bool, budget: &mut Budget) -> Result<(), JsonRefusal> {
    let maximum = if key {
        JSON_MAXIMUM_KEY_BYTES
    } else {
        JSON_MAXIMUM_STRING_BYTES
    };
    if value.len() > maximum {
        return Err(if key {
            JsonRefusal::KeyByteOverflow
        } else {
            JsonRefusal::StringByteOverflow
        });
    }
    budget.string_bytes = budget
        .string_bytes
        .checked_add(value.len())
        .ok_or(JsonRefusal::TotalStringByteOverflow)?;
    if budget.string_bytes > JSON_MAXIMUM_TOTAL_STRING_BYTES {
        return Err(JsonRefusal::TotalStringByteOverflow);
    }
    Ok(())
}

fn bounded(output: Vec<u8>) -> Result<Vec<u8>, JsonRefusal> {
    if output.len() > JSON_MAXIMUM_ENCODED_BYTES {
        Err(JsonRefusal::EncodedByteOverflow)
    } else {
        Ok(output)
    }
}

fn encode_binary(value: &JsonValue, out: &mut Vec<u8>) -> Result<(), JsonRefusal> {
    match value {
        JsonValue::Null => out.push(0),
        JsonValue::Bool(false) => out.push(1),
        JsonValue::Bool(true) => out.push(2),
        JsonValue::Number(value) => {
            out.push(3);
            out.extend_from_slice(&value.encode());
        }
        JsonValue::String(value) => {
            out.push(4);
            binary_string(value, out)?;
        }
        JsonValue::Array(items) => {
            out.push(5);
            out.push(items.len() as u8);
            for item in items {
                encode_binary(item, out)?;
            }
        }
        JsonValue::Object(members) => {
            out.push(6);
            out.push(members.len() as u8);
            for (key, value) in members {
                binary_string(key, out)?;
                encode_binary(value, out)?;
            }
        }
    }
    if out.len() > JSON_MAXIMUM_ENCODED_BYTES {
        Err(JsonRefusal::EncodedByteOverflow)
    } else {
        Ok(())
    }
}

fn binary_string(value: &str, out: &mut Vec<u8>) -> Result<(), JsonRefusal> {
    let len = u16::try_from(value.len()).map_err(|_| JsonRefusal::EncodedByteOverflow)?;
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(value.as_bytes());
    Ok(())
}

struct BinaryCursor<'a> {
    input: &'a [u8],
    offset: usize,
}
impl BinaryCursor<'_> {
    fn take(&mut self, length: usize) -> Result<&[u8], JsonRefusal> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(JsonRefusal::NonCanonicalValue)?;
        let bytes = self
            .input
            .get(self.offset..end)
            .ok_or(JsonRefusal::NonCanonicalValue)?;
        self.offset = end;
        Ok(bytes)
    }
    fn byte(&mut self) -> Result<u8, JsonRefusal> {
        Ok(self.take(1)?[0])
    }
    fn string(&mut self) -> Result<String, JsonRefusal> {
        let length = u16::from_le_bytes(self.take(2)?.try_into().unwrap()) as usize;
        let text =
            core::str::from_utf8(self.take(length)?).map_err(|_| JsonRefusal::InvalidUtf8)?;
        Ok(String::from(text))
    }
    fn value(&mut self) -> Result<JsonValue, JsonRefusal> {
        match self.byte()? {
            0 => Ok(JsonValue::Null),
            1 => Ok(JsonValue::Bool(false)),
            2 => Ok(JsonValue::Bool(true)),
            3 => Ok(JsonValue::Number(
                Scalar::decode(self.take(8)?).map_err(|_| JsonRefusal::NonCanonicalValue)?,
            )),
            4 => Ok(JsonValue::String(self.string()?)),
            5 => {
                let count = self.byte()? as usize;
                if count > JSON_MAXIMUM_ARRAY_ITEMS {
                    return Err(JsonRefusal::ArrayItemOverflow);
                }
                let mut values = Vec::with_capacity(count);
                for _ in 0..count {
                    values.push(self.value()?);
                }
                Ok(JsonValue::Array(values))
            }
            6 => {
                let count = self.byte()? as usize;
                if count > JSON_MAXIMUM_OBJECT_MEMBERS {
                    return Err(JsonRefusal::ObjectMemberOverflow);
                }
                let mut values = Vec::with_capacity(count);
                for _ in 0..count {
                    values.push((self.string()?, self.value()?));
                }
                Ok(JsonValue::Object(values))
            }
            _ => Err(JsonRefusal::NonCanonicalValue),
        }
    }
}

fn encode_text_value(value: &JsonValue, out: &mut Vec<u8>) -> Result<(), JsonRefusal> {
    match value {
        JsonValue::Null => out.extend_from_slice(b"null"),
        JsonValue::Bool(false) => out.extend_from_slice(b"false"),
        JsonValue::Bool(true) => out.extend_from_slice(b"true"),
        JsonValue::Number(value) => encode_number(*value, out),
        JsonValue::String(value) => encode_string(value, out),
        JsonValue::Array(items) => {
            out.push(b'[');
            for (index, item) in items.iter().enumerate() {
                if index > 0 {
                    out.push(b',');
                }
                encode_text_value(item, out)?;
            }
            out.push(b']');
        }
        JsonValue::Object(members) => {
            out.push(b'{');
            for (index, (key, value)) in members.iter().enumerate() {
                if index > 0 {
                    out.push(b',');
                }
                encode_string(key, out);
                out.push(b':');
                encode_text_value(value, out)?;
            }
            out.push(b'}');
        }
    }
    if out.len() > JSON_MAXIMUM_ENCODED_BYTES {
        Err(JsonRefusal::EncodedByteOverflow)
    } else {
        Ok(())
    }
}

fn encode_number(value: Scalar, out: &mut Vec<u8>) {
    use alloc::format;
    let raw = value.raw_microunits();
    if raw == 0 {
        out.push(b'0');
        return;
    }
    let negative = raw < 0;
    let magnitude = i128::from(raw).abs();
    if negative {
        out.push(b'-');
    }
    out.extend_from_slice(format!("{}", magnitude / 1_000_000).as_bytes());
    let fraction = magnitude % 1_000_000;
    if fraction != 0 {
        let mut text = format!("{fraction:06}");
        while text.ends_with('0') {
            text.pop();
        }
        out.push(b'.');
        out.extend_from_slice(text.as_bytes());
    }
}

fn encode_string(value: &str, out: &mut Vec<u8>) {
    use alloc::format;
    out.push(b'"');
    for character in value.chars() {
        match character {
            '"' => out.extend_from_slice(b"\\\""),
            '\\' => out.extend_from_slice(b"\\\\"),
            '\u{08}' => out.extend_from_slice(b"\\b"),
            '\u{0c}' => out.extend_from_slice(b"\\f"),
            '\n' => out.extend_from_slice(b"\\n"),
            '\r' => out.extend_from_slice(b"\\r"),
            '\t' => out.extend_from_slice(b"\\t"),
            c if c < '\u{20}' => out.extend_from_slice(format!("\\u{:04x}", c as u32).as_bytes()),
            c => {
                let mut bytes = [0; 4];
                out.extend_from_slice(c.encode_utf8(&mut bytes).as_bytes());
            }
        }
    }
    out.push(b'"');
}

struct Parser<'a> {
    input: &'a [u8],
    offset: usize,
}
impl Parser<'_> {
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
        if budget.nodes > JSON_MAXIMUM_NODES {
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
                count_string(&value, false, budget)?;
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
            if items.len() == JSON_MAXIMUM_ARRAY_ITEMS {
                return Err(JsonRefusal::ArrayItemOverflow);
            }
            items.push(self.value(depth + 1, budget)?);
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
            if members.len() == JSON_MAXIMUM_OBJECT_MEMBERS {
                return Err(JsonRefusal::ObjectMemberOverflow);
            }
            if self.input.get(self.offset) != Some(&b'"') {
                return Err(JsonRefusal::MalformedSyntax);
            }
            let key = self.string()?;
            count_string(&key, true, budget)?;
            self.space();
            if !self.eat(b':') {
                return Err(JsonRefusal::MalformedSyntax);
            }
            self.space();
            if members.iter().any(|(existing, _)| existing == &key) {
                return Err(JsonRefusal::DuplicateKey);
            }
            let value = self.value(depth + 1, budget)?;
            members.push((key, value));
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
                    output.push(character);
                }
            }
        }
    }
    fn escape(&mut self, output: &mut String) -> Result<(), JsonRefusal> {
        match self
            .input
            .get(self.offset)
            .copied()
            .ok_or(JsonRefusal::MalformedSyntax)?
        {
            b'"' => output.push('"'),
            b'\\' => output.push('\\'),
            b'/' => output.push('/'),
            b'b' => output.push('\u{08}'),
            b'f' => output.push('\u{0c}'),
            b'n' => output.push('\n'),
            b'r' => output.push('\r'),
            b't' => output.push('\t'),
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
                output.push(char::from_u32(scalar).ok_or(JsonRefusal::MalformedSyntax)?);
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
                exponent = exponent
                    .checked_mul(10)
                    .and_then(|value| value.checked_add(i32::from(*byte - b'0')))
                    .ok_or(JsonRefusal::NumericOverflow)?;
                self.offset += 1;
            }
            if self.offset == digit_start {
                return Err(JsonRefusal::MalformedSyntax);
            }
            if exponent_negative {
                exponent = -exponent;
            }
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

#[cfg(test)]
mod tests;
