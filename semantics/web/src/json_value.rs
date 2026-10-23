//! One finite, canonical JSON syntax tree and whole-document codec.
//!
//! Ports carry the canonical bounded `value/json@1` byte value. `JsonValue`
//! is the checked construction/parser tree used to produce and inspect that
//! value (C); it is not a second portable wire or Fore representation.

use alloc::string::String;
use alloc::vec::Vec;
use conduit_core::Scalar;

use crate::JsonRefusal;
mod text;
pub use text::validate_json_text;

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

impl JsonRefusal {
    pub const fn detail(self) -> u16 {
        match self {
            Self::MalformedSyntax => 1,
            Self::InvalidUtf8 => 2,
            Self::DepthOverflow => 3,
            Self::ArrayItemOverflow => 4,
            Self::ObjectMemberOverflow => 5,
            Self::NodeOverflow => 6,
            Self::KeyByteOverflow => 7,
            Self::StringByteOverflow => 8,
            Self::TotalStringByteOverflow => 9,
            Self::EncodedByteOverflow => 10,
            Self::NumericOverflow => 11,
            Self::DuplicateKey => 12,
            Self::NonCanonicalValue => 13,
        }
    }
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
        text::decode(input)
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

#[cfg(test)]
mod tests;
