//! Share only explicit, byte-identical dimensions of canonical provenance tuples.
//! Null instants remain unknown; defaults never create an instant or a field.
use super::{array, field, fields, put, required, R};
use alloc::vec::Vec;
pub(super) const CLASS_FIELDS: &[&str] = &[
    "domain",
    "origin",
    "availability",
    "certainty",
    "temporal_role",
];
pub(super) const TIME_FIELDS: &[&str] = &[
    "clock_basis",
    "scale",
    "ticks",
    "resolution_ticks",
    "uncertainty_ticks",
];
pub(super) struct Dictionary<'a> {
    names: [&'a [u8]; 5],
    defaults: [Option<&'a [u8]>; 5],
    changed: bool,
}
impl<'a> Dictionary<'a> {
    pub(super) fn prepare(
        document: &'a [u8],
        declaration: &str,
        keys: &[&[u8]],
        canonical: &[&str],
    ) -> Result<Self, R> {
        let mut result = Self {
            names: [&[]; 5],
            defaults: [None; 5],
            changed: false,
        };
        let declared = field(document, declaration)?;
        if declared.is_empty() {
            return Ok(result);
        }
        let mut count = 0;
        for value in array(declared)? {
            let value = value?;
            if count == 5
                || value.len() != canonical[count].len() + 2
                || &value[1..value.len() - 1] != canonical[count].as_bytes()
            {
                return Ok(result);
            }
            result.names[count] = value;
            count += 1;
        }
        if count != 5 {
            return Ok(result);
        }
        let mut equal = [true; 5];
        let mut tuples = 0;
        for item in array(required(document, "evidence")?)? {
            for pair in fields(item?)? {
                let (key, value) = pair?;
                if !keys.contains(&key) || value == b"null" || value == br#""same_as_observed_at""# {
                    continue;
                }
                if value.first() != Some(&b'[') {
                    return Ok(result);
                }
                let mut dimensions = 0;
                for (i, v) in array(value)?.enumerate() {
                    let v = v?;
                    if i == 5 {
                        return Err(R::MalformedExperience);
                    }
                    if tuples == 0 {
                        result.defaults[i] = Some(v);
                    } else if result.defaults[i] != Some(v) {
                        equal[i] = false;
                    }
                    dimensions += 1;
                }
                if dimensions != 5 {
                    return Err(R::MalformedExperience);
                }
                tuples += 1;
            }
        }
        if tuples < 2 {
            result.defaults = [None; 5];
            return Ok(result);
        }
        for (i, same) in equal.into_iter().enumerate() {
            if !same {
                result.defaults[i] = None;
            }
        }
        result.changed = result.defaults.iter().any(Option::is_some);
        Ok(result)
    }
    pub(super) fn defaults(&self, out: &mut Vec<u8>, key: &[u8]) -> Result<(), R> {
        if !self.changed {
            return Ok(());
        }
        put(out, key)?;
        put(out, b":{")?;
        let mut written = false;
        for (name, value) in self.names.iter().zip(self.defaults) {
            if let Some(value) = value {
                if written {
                    put(out, b",")?;
                }
                put(out, name)?;
                put(out, b":")?;
                put(out, value)?;
                written = true;
            }
        }
        put(out, b"},")
    }
    pub(super) fn declaration(&self, out: &mut Vec<u8>, original: &[u8]) -> Result<(), R> {
        if !self.changed {
            return put(out, original);
        }
        put(out, b"[")?;
        let mut written = false;
        for (i, name) in self.names.iter().enumerate() {
            if self.defaults[i].is_some() {
                continue;
            }
            if written {
                put(out, b",")?;
            }
            put(out, name)?;
            written = true;
        }
        put(out, b"]")
    }
    pub(super) fn value(&self, out: &mut Vec<u8>, original: &[u8]) -> Result<(), R> {
        if !self.changed || original.first() != Some(&b'[') {
            return put(out, original);
        }
        put(out, b"[")?;
        let mut written = false;
        for (i, value) in array(original)?.enumerate() {
            if self.defaults[i].is_some() {
                continue;
            }
            if written {
                put(out, b",")?;
            }
            put(out, value?)?;
            written = true;
        }
        put(out, b"]")
    }
}
