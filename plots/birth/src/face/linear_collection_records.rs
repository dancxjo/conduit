//! Narrow test-only reader of actual linear records; never executes an action.
use alloc::{collections::BTreeMap, string::String, vec::Vec};

#[derive(Debug, PartialEq, Eq)]
pub(super) struct Records {
    pub subjects: BTreeMap<String, (String, String)>,
    pub contains: Vec<(String, String)>,
    pub properties: BTreeMap<(String, String), String>,
    pub actions: BTreeMap<String, BTreeMap<String, String>>,
}

/// Split only unquoted top-level field boundaries. Debug strings, contracts and
/// argument vectors can contain spaces, punctuation and nested byte vectors.
fn fields(line: &str) -> BTreeMap<String, String> {
    assert!(line.len() <= 2 * 1024 * 1024);
    let mut starts = Vec::new();
    let (mut quoted, mut escaped, mut depth) = (false, false, 0usize);
    let bytes = line.as_bytes();
    for (i, &byte) in bytes.iter().enumerate() {
        if quoted {
            if escaped {
                escaped = false;
            } else if byte == b'\\' {
                escaped = true;
            } else if byte == b'"' {
                quoted = false;
            }
        } else {
            match byte {
                b'"' => quoted = true,
                b'[' | b'{' | b'(' => depth += 1,
                b']' | b'}' | b')' => depth = depth.checked_sub(1).unwrap(),
                b'=' if depth == 0 => {
                    let start = line[..i].rfind(' ').map_or(0, |p| p + 1);
                    starts.push((start, i));
                }
                _ => {}
            }
        }
    }
    assert!(!quoted && depth == 0 && starts.len() <= 16);
    let mut result = BTreeMap::new();
    for (index, &(start, equal)) in starts.iter().enumerate() {
        let end = starts.get(index + 1).map_or(line.len(), |s| s.0 - 1);
        assert!(
            result
                .insert(line[start..equal].into(), line[equal + 1..end].into())
                .is_none()
        );
    }
    result
}

pub(super) fn string(raw: &str) -> String {
    // Current Source identities and fixture names use JSON-compatible Debug
    // strings. Refuse unsupported Rust-only escapes rather than guessing.
    serde_json::from_str(raw).expect("supported bounded quoted string")
}

impl Records {
    pub fn read(lines: &[String]) -> Self {
        assert!(lines.len() <= conduit_face::MAX_LINEAR_FACE_LINES);
        let mut result = Self {
            subjects: BTreeMap::new(),
            contains: Vec::new(),
            properties: BTreeMap::new(),
            actions: BTreeMap::new(),
        };
        for line in lines {
            if line.starts_with("SUBJECT ") {
                let f = fields(line);
                assert!(
                    result
                        .subjects
                        .insert(string(&f["id"]), (f["role"].clone(), string(&f["name"])))
                        .is_none()
                );
            } else if line.starts_with("RELATIONSHIP ") {
                let f = fields(line);
                if f["kind"] == "Contains" {
                    result
                        .contains
                        .push((string(&f["source"]), string(&f["target"])));
                }
            } else if line.starts_with("PROPERTY ") {
                // value is the complete remaining record, including its exact
                // contract/canonical fields, not a truncated display prefix.
                let (prefix, value) = line.split_once(" value=").unwrap();
                let f = fields(prefix);
                assert!(
                    result
                        .properties
                        .insert((string(&f["subject"]), string(&f["name"])), value.into())
                        .is_none()
                );
            } else if line.starts_with("ACTION ") {
                let (prefix, availability) = line.split_once(" availability=").unwrap();
                let mut f = fields(prefix);
                f.insert("availability".into(), availability.into());
                assert!(result.actions.insert(string(&f["id"]), f).is_none());
            }
        }
        result
    }
    pub fn property(&self, subject: &str, name: &str) -> &str {
        &self.properties[&(subject.into(), name.into())]
    }
    pub fn count(&self, subject: &str, name: &str) -> u64 {
        self.property(subject, name)
            .strip_prefix("count:")
            .unwrap()
            .parse()
            .unwrap()
    }
    pub fn identity(&self, subject: &str, name: &str) -> String {
        string(
            self.property(subject, name)
                .strip_prefix("identity:")
                .unwrap(),
        )
    }
    pub fn flag(&self, subject: &str, name: &str) -> bool {
        self.property(subject, name)
            .strip_prefix("flag:")
            .unwrap()
            .parse()
            .unwrap()
    }
    pub fn items(&self, collection: &str) -> Vec<String> {
        let mut items: Vec<_> = self
            .contains
            .iter()
            .filter(|(parent, _)| parent == collection)
            .map(|(_, child)| child.clone())
            .collect();
        assert!(items.len() <= 64);
        items.sort_by_key(|id| self.count(id, "order"));
        items
    }
    pub fn typed(&self, subject: &str, name: &str) -> (&str, Vec<u8>) {
        let (_, exact) = self
            .property(subject, name)
            .split_once(" contract=")
            .unwrap();
        let (contract, canonical) = exact.rsplit_once(" canonical=").unwrap();
        let bytes: Vec<u8> = serde_json::from_str(canonical).unwrap();
        assert!(bytes.len() <= 128);
        (contract, bytes)
    }
}

pub(super) fn membership(arguments: &str) -> Vec<Vec<u8>> {
    let start = arguments.find("CanonicalMembership { members: ").unwrap();
    let tail = &arguments[start + "CanonicalMembership { members: ".len()..];
    let end = tail.find(", negated: false }").unwrap();
    let members: Vec<Vec<u8>> = serde_json::from_str(&tail[..end]).unwrap();
    assert!(members.len() <= 64 && members.iter().all(|m| m.len() <= 128));
    members
}
