//! Test-only reader of the emitted observation grammar, not a product parser.
//! Supports the complete contract variants used by these declared specimens.
use alloc::{string::String, vec::Vec};

#[derive(Debug, PartialEq, Eq)]
pub(super) enum Constraint {
    ByteLength {
        minimum: u32,
        maximum: u32,
    },
    Membership {
        members: Vec<Vec<u8>>,
        negated: bool,
    },
}
#[derive(Debug, PartialEq, Eq)]
pub(super) struct Record {
    pub subject: String,
    pub name: String,
    pub wording: String,
    pub kind: String,
    pub maximum: u32,
    pub constraints: Vec<Constraint>,
    pub bytes: Vec<u8>,
}
struct Reader<'a>(&'a str);
impl<'a> Reader<'a> {
    fn token(&mut self, token: &str) {
        self.0 = self
            .0
            .strip_prefix(token)
            .expect("declared emitted grammar");
    }
    fn number(&mut self) -> u32 {
        let length = self.0.bytes().take_while(u8::is_ascii_digit).count();
        assert!(length > 0);
        let value = self.0[..length].parse().unwrap();
        self.0 = &self.0[length..];
        value
    }
    fn character(&mut self) -> char {
        let value = self.0.chars().next().expect("complete emitted string");
        self.0 = &self.0[value.len_utf8()..];
        value
    }
    /// Rust Debug strings are not JSON: handle NUL and braced Unicode escapes.
    fn quoted(&mut self) -> String {
        self.token("\"");
        let mut output = String::new();
        loop {
            match self.character() {
                '"' => break,
                '\\' => output.push(match self.character() {
                    '0' => '\0',
                    'n' => '\n',
                    'r' => '\r',
                    't' => '\t',
                    '\\' => '\\',
                    '"' => '"',
                    '\'' => '\'',
                    'u' => {
                        self.token("{");
                        let end = self.0.find('}').unwrap();
                        assert!((1..=6).contains(&end));
                        let scalar = u32::from_str_radix(&self.0[..end], 16).unwrap();
                        self.0 = &self.0[end + 1..];
                        char::from_u32(scalar).unwrap()
                    }
                    'x' => {
                        let value = u8::from_str_radix(&self.0[..2], 16).unwrap();
                        self.0 = &self.0[2..];
                        char::from(value)
                    }
                    _ => panic!("unsupported emitted escape"),
                }),
                value => output.push(value),
            }
        }
        output
    }
    fn bytes(&mut self) -> Vec<u8> {
        self.token("[");
        let mut bytes = Vec::new();
        while !self.0.starts_with(']') {
            bytes.push(u8::try_from(self.number()).unwrap());
            assert!(bytes.len() <= crate::MAX_FACE_VALUE_BYTES as usize);
            if !self.0.starts_with(']') {
                self.token(", ");
            }
        }
        self.token("]");
        bytes
    }
    fn constraints(&mut self) -> Vec<Constraint> {
        self.token("[");
        let mut constraints = Vec::new();
        while !self.0.starts_with(']') {
            let constraint = if self.0.starts_with("ByteLength") {
                self.token("ByteLength { minimum: ");
                let minimum = self.number();
                self.token(", maximum: ");
                let maximum = self.number();
                self.token(" }");
                Constraint::ByteLength { minimum, maximum }
            } else {
                self.token("CanonicalMembership { members: [");
                let mut members = Vec::new();
                while !self.0.starts_with(']') {
                    members.push(self.bytes());
                    assert!(members.len() <= conduit_core::MAX_MEMBERSHIP_VALUES);
                    if !self.0.starts_with(']') {
                        self.token(", ");
                    }
                }
                self.token("], negated: ");
                let negated = if self.0.starts_with("true") {
                    self.token("true");
                    true
                } else {
                    self.token("false");
                    false
                };
                self.token(" }");
                Constraint::Membership { members, negated }
            };
            constraints.push(constraint);
            assert!(constraints.len() <= conduit_core::MAX_VALUE_CONSTRAINTS);
            if !self.0.starts_with(']') {
                self.token(", ");
            }
        }
        self.token("]");
        constraints
    }
}

fn split_unquoted_marker<'a>(value: &'a str, marker: &str) -> Option<(&'a str, &'a str)> {
    let mut quoted = false;
    let mut escaped = false;
    let mut last = None;
    for (index, character) in value.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match character {
            '\\' => escaped = true,
            '"' => quoted = !quoted,
            _ => {}
        }
        if !quoted && value[index..].starts_with(marker) {
            last = Some(index);
        }
    }
    last.map(|index| (&value[..index], &value[index + marker.len()..]))
}

pub(super) fn read(line: &str) -> Record {
    assert!(line.len() <= super::super::MAX_LINEAR_FACE_BYTES);
    assert!(!line.contains(['\n', '\r']));
    let (before_bytes, bytes) = line
        .rsplit_once(" canonical=")
        .expect("exact canonical payload");
    let mut byte_reader = Reader(bytes);
    let bytes = byte_reader.bytes();
    assert!(byte_reader.0.is_empty());
    let (header, contract) =
        split_unquoted_marker(before_bytes, " contract=").expect("full checked contract");
    let mut header = Reader(header);
    header.token("PROPERTY subject=");
    let subject = header.quoted();
    header.token(" name=");
    let name = header.quoted();
    header.token(" value=");
    let wording = header.0.into();
    let mut contract = Reader(contract);
    contract.token("CheckedValueContract { value_kind: KindId(");
    let kind = contract.quoted();
    contract.token("), maximum_bytes: ");
    let maximum = contract.number();
    contract.token(", constraints: ");
    let constraints = contract.constraints();
    contract.token(" }");
    assert!(contract.0.is_empty());
    Record {
        subject,
        name,
        wording,
        kind,
        maximum,
        constraints,
        bytes,
    }
}
