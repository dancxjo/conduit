//! Independent test-only reader for the bounded Birth LinearFace observation.
//! No Face/serde/digest input is available while decoding these records.
use alloc::{collections::BTreeMap, string::String, vec::Vec};

#[derive(Debug, PartialEq, Eq)]
pub(super) struct Contract {
    pub kind: String,
    pub maximum: u32,
    pub constraints: Vec<Constraint>,
}
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Constraint {
    Length(u32, u32),
    Membership(Vec<Vec<u8>>, bool),
}
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Value {
    Identity(String),
    Text(String),
    Count(u64),
    Flag(bool),
    Typed(Contract, Vec<u8>),
}
#[derive(Debug)]
pub(super) struct Argument {
    pub name: String,
    pub label: String,
    pub contract: Contract,
}
#[derive(Debug, PartialEq, Eq)]
pub(super) enum Availability {
    Available,
    Unavailable(String, String),
    Refused(String, String),
}
#[derive(Debug)]
pub(super) struct Action {
    pub intent: String,
    pub target: String,
    pub name: String,
    pub arguments: Vec<Argument>,
    pub availability: Availability,
    pub disclosure: String,
}
#[derive(Debug)]
pub(super) struct Records {
    pub subjects: BTreeMap<String, (String, String)>,
    pub contains: Vec<(String, String)>,
    pub relationships: Vec<(String, String, String)>,
    pub properties: BTreeMap<(String, String), Value>,
    pub actions: BTreeMap<String, Action>,
}
struct Reader<'a>(&'a str);
impl Reader<'_> {
    fn token(&mut self, token: &str) {
        self.0 = self
            .0
            .strip_prefix(token)
            .expect("declared Birth record grammar");
    }
    fn number(&mut self) -> u64 {
        let n = self.0.bytes().take_while(u8::is_ascii_digit).count();
        assert!(n > 0);
        let value = self.0[..n].parse().unwrap();
        self.0 = &self.0[n..];
        value
    }
    fn character(&mut self) -> char {
        let c = self.0.chars().next().unwrap();
        self.0 = &self.0[c.len_utf8()..];
        c
    }
    fn quoted(&mut self) -> String {
        self.token("\"");
        let mut out = String::new();
        loop {
            match self.character() {
                '"' => break,
                '\\' => out.push(match self.character() {
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
                        let b = u8::from_str_radix(&self.0[..2], 16).unwrap();
                        self.0 = &self.0[2..];
                        char::from(b)
                    }
                    _ => panic!("unsupported emitted escape"),
                }),
                c => out.push(c),
            }
        }
        out
    }
    fn bytes(&mut self) -> Vec<u8> {
        self.token("[");
        let mut bytes = Vec::new();
        while !self.0.starts_with(']') {
            bytes.push(u8::try_from(self.number()).unwrap());
            assert!(bytes.len() <= conduit_face::MAX_FACE_VALUE_BYTES as usize);
            if !self.0.starts_with(']') {
                self.token(", ");
            }
        }
        self.token("]");
        bytes
    }
    fn boolean(&mut self) -> bool {
        if self.0.starts_with("true") {
            self.token("true");
            true
        } else {
            self.token("false");
            false
        }
    }
    fn contract(&mut self) -> Contract {
        self.token("CheckedValueContract { value_kind: KindId(");
        let kind = self.quoted();
        self.token("), maximum_bytes: ");
        let maximum = u32::try_from(self.number()).unwrap();
        self.token(", constraints: [");
        let mut constraints = Vec::new();
        while !self.0.starts_with(']') {
            let value = if self.0.starts_with("ByteLength") {
                self.token("ByteLength { minimum: ");
                let minimum = u32::try_from(self.number()).unwrap();
                self.token(", maximum: ");
                let maximum = u32::try_from(self.number()).unwrap();
                self.token(" }");
                Constraint::Length(minimum, maximum)
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
                let negated = self.boolean();
                self.token(" }");
                Constraint::Membership(members, negated)
            };
            constraints.push(value);
            assert!(constraints.len() <= conduit_core::MAX_VALUE_CONSTRAINTS);
            if !self.0.starts_with(']') {
                self.token(", ");
            }
        }
        self.token("] }");
        Contract {
            kind,
            maximum,
            constraints,
        }
    }
}
fn split_marker<'a>(text: &'a str, marker: &str) -> (&'a str, &'a str) {
    let (mut quoted, mut escaped, mut at) = (false, false, None);
    for (i, c) in text.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        match c {
            '\\' => escaped = true,
            '"' => quoted = !quoted,
            _ => {}
        }
        if !quoted && text[i..].starts_with(marker) {
            at = Some(i);
        }
    }
    let at = at.expect("lossless typed record suffix");
    (&text[..at], &text[at + marker.len()..])
}
impl Records {
    pub fn read(lines: &[String]) -> Self {
        assert!(lines.len() <= conduit_face::MAX_LINEAR_FACE_LINES);
        assert!(
            lines.iter().map(|s| s.len() + 1).sum::<usize>() <= conduit_face::MAX_LINEAR_FACE_BYTES
        );
        let mut out = Self {
            subjects: BTreeMap::new(),
            contains: Vec::new(),
            relationships: Vec::new(),
            properties: BTreeMap::new(),
            actions: BTreeMap::new(),
        };
        for line in lines {
            assert!(!line.contains(['\n', '\r']));
            if let Some(tail) = line.strip_prefix("SUBJECT role=") {
                let (role, tail) = tail.split_once(" id=").unwrap();
                let mut r = Reader(tail);
                let id = r.quoted();
                r.token(" name=");
                let name = r.quoted();
                assert!(r.0.is_empty());
                assert!(out.subjects.insert(id, (role.into(), name)).is_none());
            } else if let Some(tail) = line.strip_prefix("RELATIONSHIP kind=") {
                let (kind, tail) = tail.split_once(" source=").unwrap();
                let mut r = Reader(tail);
                let source = r.quoted();
                r.token(" target=");
                let target = r.quoted();
                assert!(r.0.is_empty());
                out.relationships
                    .push((kind.into(), source.clone(), target.clone()));
                if kind == "Contains" {
                    out.contains.push((source, target));
                }
            } else if let Some(tail) = line.strip_prefix("PROPERTY subject=") {
                let mut r = Reader(tail);
                let subject = r.quoted();
                r.token(" name=");
                let name = r.quoted();
                r.token(" value=");
                let value = if r.0.starts_with("identity:") {
                    r.token("identity:");
                    Value::Identity(r.quoted())
                } else if r.0.starts_with("text:") {
                    r.token("text:");
                    Value::Text(r.quoted())
                } else if r.0.starts_with("count:") {
                    r.token("count:");
                    Value::Count(r.number())
                } else if r.0.starts_with("flag:") {
                    r.token("flag:");
                    Value::Flag(r.boolean())
                } else {
                    let (_, tail) = split_marker(r.0, " contract=");
                    let (contract, bytes) = split_marker(tail, " canonical=");
                    let mut c = Reader(contract);
                    let contract = c.contract();
                    assert!(c.0.is_empty());
                    let mut b = Reader(bytes);
                    let bytes = b.bytes();
                    assert!(b.0.is_empty());
                    r.0 = "";
                    Value::Typed(contract, bytes)
                };
                assert!(r.0.is_empty());
                assert!(out.properties.insert((subject, name), value).is_none());
            } else if let Some(tail) = line.strip_prefix("ACTION id=") {
                let mut r = Reader(tail);
                let id = r.quoted();
                r.token(" intent=");
                let intent = r.quoted();
                r.token(" target=");
                let target = r.quoted();
                r.token(" name=");
                let name = r.quoted();
                r.token(" arguments=[");
                let mut arguments = Vec::new();
                while !r.0.starts_with(']') {
                    r.token("FaceActionArgument { name: ");
                    let name = r.quoted();
                    r.token(", value_name: ");
                    let label = r.quoted();
                    r.token(", contract: ");
                    let contract = r.contract();
                    r.token(" }");
                    arguments.push(Argument {
                        name,
                        label,
                        contract,
                    });
                    assert!(arguments.len() <= conduit_face::MAX_FACE_ACTION_ARGUMENTS);
                    if !r.0.starts_with(']') {
                        r.token(", ");
                    }
                }
                r.token("] disclosure=");
                let (disclosure, availability) = r.0.split_once(" availability=").unwrap();
                assert!(matches!(disclosure, "Primary" | "CurrentAction"));
                let disclosure = disclosure.into();
                let mut a = Reader(availability);
                let availability = if a.0 == "available" {
                    a.token("available");
                    Availability::Available
                } else {
                    let unavailable = a.0.starts_with("unavailable");
                    a.token(if unavailable {
                        "unavailable code="
                    } else {
                        "refused code="
                    });
                    let code = a.quoted();
                    a.token(" explanation=");
                    let message = a.quoted();
                    if unavailable {
                        Availability::Unavailable(code, message)
                    } else {
                        Availability::Refused(code, message)
                    }
                };
                assert!(a.0.is_empty());
                assert!(
                    out.actions
                        .insert(
                            id,
                            Action {
                                intent,
                                target,
                                name,
                                arguments,
                                disclosure,
                                availability
                            }
                        )
                        .is_none()
                );
            } else {
                // Basis, readable prose and subject disclosure are outside this collection gate.
                // New record categories must be consciously classified rather than discarded.
                assert!(
                    [
                        "FACE ",
                        "BODY ",
                        "PLOT ",
                        "PLAN ",
                        "SIGNS ",
                        "SIGN ",
                        "INTERACTION_CONTEXT ",
                        "CONTEXT_BASIS ",
                        "TEXT ",
                        "DISCLOSURE "
                    ]
                    .iter()
                    .any(|prefix| line.starts_with(prefix)),
                    "unclassified record: {line}"
                );
            }
        }
        out
    }
    pub fn value(&self, subject: &str, name: &str) -> &Value {
        &self.properties[&(subject.into(), name.into())]
    }
    pub fn count(&self, subject: &str, name: &str) -> u64 {
        let Value::Count(value) = self.value(subject, name) else {
            panic!()
        };
        *value
    }
    pub fn identity(&self, subject: &str, name: &str) -> &str {
        let Value::Identity(value) = self.value(subject, name) else {
            panic!()
        };
        value
    }
    pub fn flag(&self, subject: &str, name: &str) -> bool {
        let Value::Flag(value) = self.value(subject, name) else {
            panic!()
        };
        *value
    }
    pub fn typed(&self, subject: &str, name: &str) -> (&Contract, &[u8]) {
        let Value::Typed(contract, value) = self.value(subject, name) else {
            panic!()
        };
        (contract, value)
    }
    pub fn collection(&self, suffix: &str) -> &str {
        let mut found = self
            .subjects
            .iter()
            .filter(|(id, (role, _))| role == "Collection" && id.ends_with(suffix));
        let id = found.next().unwrap().0;
        assert!(found.next().is_none());
        id
    }
    pub fn members(&self, collection: &str) -> Vec<&str> {
        let mut items: Vec<_> = self
            .contains
            .iter()
            .filter(|(source, _)| source == collection)
            .map(|(_, target)| target.as_str())
            .collect();
        assert!(items.len() <= 64);
        items.sort_by_key(|id| self.count(id, "order"));
        assert_eq!(items.len() as u64, self.count(collection, "item-count"));
        items
    }
}
impl Contract {
    pub fn matches(&self, source: &conduit_core::CheckedValueContract) {
        assert_eq!(self.kind, source.value_kind.as_str());
        assert_eq!(self.maximum, source.maximum_bytes);
        let expected: Vec<_> = source
            .constraints
            .iter()
            .map(|c| match c {
                conduit_core::ValueConstraint::ByteLength { minimum, maximum } => {
                    Constraint::Length(*minimum, *maximum)
                }
                conduit_core::ValueConstraint::CanonicalMembership { members, negated } => {
                    Constraint::Membership(members.clone(), *negated)
                }
                _ => panic!("new Birth contract variant needs independent reader coverage"),
            })
            .collect();
        assert_eq!(self.constraints, expected);
    }
}
