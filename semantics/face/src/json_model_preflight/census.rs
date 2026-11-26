//! Allocation-free structural census before deserializing expanded Face JSON.
//! This is storage admission input, not a substitute for serde or Face validation.
use super::{JsonModelRefusal, MAX_JSON_MODEL_BYTES};
#[path = "fence.rs"]
mod fence;
use fence::Grammar;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct JsonModelShape {
    pub objects: usize,
    pub object_fields: usize,
    pub arrays: usize,
    pub object_or_string_array_elements: usize,
    /// Numbers, booleans and null: each needs at most one scalar slot.
    pub numeric_array_elements: usize,
    /// Source bytes between quotes, including escape spelling. This bounds the
    /// decoded UTF-8 bytes of all keys and values without decoding them here.
    pub string_source_bytes: usize,
}

pub fn json_shape(bytes: &[u8]) -> Result<JsonModelShape, JsonModelRefusal> {
    scan(bytes, Grammar::Census)
}

/// Census with the selected generic Face grammar enforced before serde. This
/// excludes generated composition and typed/action constructor families; it is
/// not complete Face validation or authority. Escaped string values are allowed.
pub fn generic_face_json_shape(bytes: &[u8]) -> Result<JsonModelShape, JsonModelRefusal> {
    if bytes.len() > super::MAX_FACE_JSON_BYTES {
        return Err(JsonModelRefusal::Capacity);
    }
    scan(bytes, Grammar::Face)
}

pub(crate) fn generic_mask_plan_json_shape(
    bytes: &[u8],
) -> Result<JsonModelShape, JsonModelRefusal> {
    scan(bytes, Grammar::GenericPlan)
}

fn scan(bytes: &[u8], grammar: Grammar) -> Result<JsonModelShape, JsonModelRefusal> {
    if bytes.is_empty() || bytes.len() > MAX_JSON_MODEL_BYTES {
        return Err(JsonModelRefusal::Capacity);
    }
    let mut scan = Scan {
        bytes,
        at: 0,
        shape: JsonModelShape::default(),
        grammar,
    };
    scan.value(0)?;
    scan.space();
    if scan.at != bytes.len() {
        return Err(JsonModelRefusal::Json);
    }
    Ok(scan.shape)
}

struct Scan<'a> {
    bytes: &'a [u8],
    at: usize,
    shape: JsonModelShape,
    grammar: Grammar,
}
impl Scan<'_> {
    fn space(&mut self) {
        while self
            .bytes
            .get(self.at)
            .is_some_and(|b| matches!(b, b' ' | b'\n' | b'\r' | b'\t'))
        {
            self.at += 1;
        }
    }
    fn take(&mut self, byte: u8) -> bool {
        self.space();
        if self.bytes.get(self.at) == Some(&byte) {
            self.at += 1;
            true
        } else {
            false
        }
    }
    fn need(&mut self, byte: u8) -> Result<(), JsonModelRefusal> {
        if self.take(byte) {
            Ok(())
        } else {
            Err(JsonModelRefusal::Json)
        }
    }
    fn value(&mut self, depth: usize) -> Result<(), JsonModelRefusal> {
        self.space();
        match self.bytes.get(self.at) {
            Some(b'{') => {
                if depth == 128 {
                    return Err(JsonModelRefusal::Json);
                }
                self.shape.objects += 1;
                self.at += 1;
                if self.take(b'}') {
                    return Ok(());
                }
                loop {
                    self.space();
                    let key_start = self.at + 1;
                    self.string()?;
                    let key = &self.bytes[key_start..self.at - 1];
                    self.shape.object_fields += 1;
                    self.need(b':')?;
                    self.grammar.check_field(key, &self.bytes[self.at..])?;
                    self.value(depth + 1)?;
                    if self.take(b'}') {
                        return Ok(());
                    }
                    self.need(b',')?;
                }
            }
            Some(b'[') => {
                if depth == 128 {
                    return Err(JsonModelRefusal::Json);
                }
                self.shape.arrays += 1;
                self.at += 1;
                if self.take(b']') {
                    return Ok(());
                }
                loop {
                    self.space();
                    if matches!(self.bytes.get(self.at), Some(b'{' | b'[' | b'"')) {
                        self.shape.object_or_string_array_elements += 1;
                    } else {
                        self.shape.numeric_array_elements += 1;
                    }
                    self.value(depth + 1)?;
                    if self.take(b']') {
                        return Ok(());
                    }
                    self.need(b',')?;
                }
            }
            Some(b'"') => self.string(),
            Some(b't') => self.literal(b"true"),
            Some(b'f') => self.literal(b"false"),
            Some(b'n') => self.literal(b"null"),
            Some(b'-' | b'0'..=b'9') => self.number(),
            _ => Err(JsonModelRefusal::Json),
        }
    }
    fn string(&mut self) -> Result<(), JsonModelRefusal> {
        self.need(b'"')?;
        let start = self.at;
        loop {
            match self.bytes.get(self.at).copied() {
                Some(b'"') => {
                    self.shape.string_source_bytes += self.at - start;
                    self.at += 1;
                    return Ok(());
                }
                Some(b'\\') => {
                    self.at += 1;
                    match self.bytes.get(self.at).copied() {
                        Some(b'"' | b'\\' | b'/' | b'b' | b'f' | b'n' | b'r' | b't') => {
                            self.at += 1
                        }
                        Some(b'u') => {
                            self.at += 1;
                            for _ in 0..4 {
                                if !self.bytes.get(self.at).is_some_and(u8::is_ascii_hexdigit) {
                                    return Err(JsonModelRefusal::Json);
                                }
                                self.at += 1;
                            }
                        }
                        _ => return Err(JsonModelRefusal::Json),
                    }
                }
                Some(0..=31) | None => return Err(JsonModelRefusal::Json),
                Some(_) => self.at += 1,
            }
        }
    }
    fn literal(&mut self, literal: &[u8]) -> Result<(), JsonModelRefusal> {
        if self.bytes.get(self.at..self.at + literal.len()) == Some(literal) {
            self.at += literal.len();
            Ok(())
        } else {
            Err(JsonModelRefusal::Json)
        }
    }
    fn digits(&mut self) -> bool {
        let start = self.at;
        while self.bytes.get(self.at).is_some_and(u8::is_ascii_digit) {
            self.at += 1;
        }
        self.at != start
    }
    fn number(&mut self) -> Result<(), JsonModelRefusal> {
        if self.bytes.get(self.at) == Some(&b'-') {
            self.at += 1;
        }
        if self.bytes.get(self.at) == Some(&b'0') {
            self.at += 1;
        } else if !self.digits() {
            return Err(JsonModelRefusal::Json);
        }
        if self.bytes.get(self.at) == Some(&b'.') {
            self.at += 1;
            if !self.digits() {
                return Err(JsonModelRefusal::Json);
            }
        }
        if matches!(self.bytes.get(self.at), Some(b'e' | b'E')) {
            self.at += 1;
            if matches!(self.bytes.get(self.at), Some(b'+' | b'-')) {
                self.at += 1;
            }
            if !self.digits() {
                return Err(JsonModelRefusal::Json);
            }
        }
        Ok(())
    }
}
