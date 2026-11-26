//! Narrow preparse fences for the two selected owning-model families.
//! Inspect raw keys and value prefixes without serde scratch or owned strings.
use super::super::JsonModelRefusal;

#[derive(Clone, Copy)]
pub(super) enum Grammar {
    Census,
    Face,
    GenericPlan,
}
const PLAN_EMPTY: &[&[u8]] = &[
    b"activations",
    b"activation_preparations",
    b"realization_backs",
    b"execution_regions",
    b"execution_fusions",
    b"states",
    b"shared_pools",
    b"configuration",
    b"realization_properties",
    b"realization_characteristics",
    b"terminal_transductions",
    b"pool_references",
    b"laws",
];
const PLAN_NULL: &[&[u8]] = &[b"protected", b"compute", b"content", b"resource"];
const FACE_EMPTY: &[&[u8]] = &[b"composition", b"constraints", b"arguments"];

impl Grammar {
    pub(super) fn check_field(self, key: &[u8], remaining: &[u8]) -> Result<(), JsonModelRefusal> {
        if matches!(self, Self::Census) {
            return Ok(());
        }
        // Canonical serde field names never contain escapes. Refuse escaped
        // keys before a borrowed-key deserializer could allocate scratch.
        if key.contains(&b'\\') {
            return Err(JsonModelRefusal::Unsupported);
        }
        let (empty, null) = match self {
            Self::Census => unreachable!(),
            Self::Face => (FACE_EMPTY, &[][..]),
            Self::GenericPlan => (PLAN_EMPTY, PLAN_NULL),
        };
        let value = skip_space(remaining);
        // A selected Line names its Base with a string newtype. A placement's
        // BaseProviderBinding is an owning record, which this model excludes.
        // Strings already have their own conservative census storage charge.
        if matches!(self, Self::GenericPlan)
            && key == b"base"
            && !value.starts_with(b"null")
            && !value.starts_with(b"\"")
        {
            return Err(JsonModelRefusal::Unsupported);
        }

        if empty.contains(&key)
            && !value
                .strip_prefix(b"[")
                .is_some_and(|tail| skip_space(tail).starts_with(b"]"))
            || null.contains(&key) && !value.starts_with(b"null")
        {
            return Err(JsonModelRefusal::Unsupported);
        }
        Ok(())
    }
}
fn skip_space(bytes: &[u8]) -> &[u8] {
    let length = bytes
        .iter()
        .take_while(|byte| matches!(byte, b' ' | b'\n' | b'\r' | b'\t'))
        .count();
    &bytes[length..]
}
