//! Borrowed JSON structure and scalar scans; no Value/Content owned tree.
use super::{Layout, Refusal};
use core::fmt;
use serde::de::{DeserializeSeed, Error, MapAccess, SeqAccess, Visitor};
pub(super) use serde_json::value::RawValue;
pub(super) type R<T> = Result<T, Refusal>;
const FIELDS: usize = 20;

pub(super) fn raw(bytes: &[u8]) -> R<&RawValue> {
    serde_json::from_slice(bytes).map_err(|_| Refusal::Json)
}
pub(super) fn shape(raw: &RawValue, kind: u8) -> R<()> {
    if raw.get().as_bytes().first() == Some(&kind) {
        Ok(())
    } else {
        Err(Refusal::Unsupported)
    }
}
pub(super) fn null(raw: &RawValue) -> bool {
    raw.get() == "null"
}
pub(super) fn boolean(raw: &RawValue) -> R<()> {
    matches!(raw.get(), "true" | "false")
        .then_some(())
        .ok_or(Refusal::Unsupported)
}

struct Word<'a>(&'a [&'a str]);
impl<'de> DeserializeSeed<'de> for Word<'_> {
    type Value = usize;
    fn deserialize<D: serde::Deserializer<'de>>(self, d: D) -> Result<usize, D::Error> {
        d.deserialize_str(self)
    }
}
impl Visitor<'_> for Word<'_> {
    type Value = usize;
    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a supported portable-offer name")
    }
    fn visit_str<E: Error>(self, value: &str) -> Result<usize, E> {
        self.0
            .iter()
            .position(|s| *s == value)
            .ok_or_else(|| E::custom("unsupported portable-offer name"))
    }
}
pub(super) fn word(raw: &RawValue, names: &[&str]) -> R<usize> {
    shape(raw, b'"')?;
    let mut d = serde_json::Deserializer::from_str(raw.get());
    let value = Word(names)
        .deserialize(&mut d)
        .map_err(|_| Refusal::Unsupported)?;
    d.end().map_err(|_| Refusal::Json)?;
    Ok(value)
}

pub(super) struct Object<'a>([Option<&'a RawValue>; FIELDS]);
impl<'a> Object<'a> {
    pub(super) fn get(&self, index: usize) -> R<&'a RawValue> {
        self.0[index].ok_or(Refusal::Unsupported)
    }
    pub(super) fn optional(&self, index: usize) -> Option<&'a RawValue> {
        self.0[index]
    }
    pub(super) fn one(&self) -> R<(usize, &'a RawValue)> {
        let mut values = self
            .0
            .iter()
            .enumerate()
            .filter_map(|(i, v)| v.map(|v| (i, v)));
        let value = values.next().ok_or(Refusal::Unsupported)?;
        if values.next().is_some() {
            Err(Refusal::Unsupported)
        } else {
            Ok(value)
        }
    }
}
pub(super) fn object<'a>(raw: &'a RawValue, names: &[&str], required: u32) -> R<Object<'a>> {
    shape(raw, b'{')?;
    if names.len() > FIELDS {
        return Err(Refusal::Unsupported);
    }
    struct Map<'a> {
        names: &'a [&'a str],
        required: u32,
    }
    impl<'de> Visitor<'de> for Map<'_> {
        type Value = Object<'de>;
        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("a portable-offer record")
        }
        fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
            let mut fields = [None; FIELDS];
            let mut seen = 0u32;
            while let Some(i) = map.next_key_seed(Word(self.names))? {
                let bit = 1u32 << i;
                if seen & bit != 0 {
                    return Err(A::Error::custom("duplicate portable-offer field"));
                }
                seen |= bit;
                fields[i] = Some(map.next_value()?);
            }
            if seen & self.required != self.required {
                return Err(A::Error::custom("missing portable-offer field"));
            }
            Ok(Object(fields))
        }
    }
    let mut d = serde_json::Deserializer::from_str(raw.get());
    let value = serde::Deserializer::deserialize_map(&mut d, Map { names, required })
        .map_err(|_| Refusal::Unsupported)?;
    d.end().map_err(|_| Refusal::Json)?;
    Ok(value)
}
pub(super) fn variant<'a>(raw: &'a RawValue, names: &[&str]) -> R<(usize, &'a RawValue)> {
    object(raw, names, 0)?.one()
}

pub(super) fn text(raw: &RawValue, layout: &mut Layout) -> R<()> {
    text_bytes(raw, layout).map(|_| ())
}
pub(super) fn text_bytes(raw: &RawValue, layout: &mut Layout) -> R<usize> {
    struct Text;
    impl Visitor<'_> for Text {
        type Value = usize;
        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("portable-offer text")
        }
        fn visit_str<E: Error>(self, value: &str) -> Result<usize, E> {
            Ok(value.len())
        }
    }
    shape(raw, b'"')?;
    let mut d = serde_json::Deserializer::from_str(raw.get());
    let bytes =
        serde::Deserializer::deserialize_str(&mut d, Text).map_err(|_| Refusal::Unsupported)?;
    d.end().map_err(|_| Refusal::Json)?;
    layout.text(bytes)?;
    Ok(bytes)
}
fn digits(bytes: &[u8], maximum: u64) -> R<()> {
    if bytes.is_empty() || (bytes.len() > 1 && bytes[0] == b'0') {
        return Err(Refusal::Unsupported);
    }
    let mut value = 0u64;
    for b in bytes {
        if !b.is_ascii_digit() {
            return Err(Refusal::Unsupported);
        }
        value = value
            .checked_mul(10)
            .and_then(|v| v.checked_add(u64::from(*b - b'0')))
            .filter(|v| *v <= maximum)
            .ok_or(Refusal::Unsupported)?;
    }
    Ok(())
}
pub(super) fn unsigned(raw: &RawValue, maximum: u64, human: bool) -> R<()> {
    if raw.get().starts_with('"') {
        if !human {
            return Err(Refusal::Unsupported);
        }
        struct Decimal(u64);
        impl Visitor<'_> for Decimal {
            type Value = ();
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("canonical u64 decimal text")
            }
            fn visit_str<E: Error>(self, s: &str) -> Result<(), E> {
                digits(s.as_bytes(), self.0).map_err(|_| E::custom("invalid canonical integer"))
            }
        }
        let mut d = serde_json::Deserializer::from_str(raw.get());
        serde::Deserializer::deserialize_str(&mut d, Decimal(maximum))
            .map_err(|_| Refusal::Unsupported)?;
        return d.end().map_err(|_| Refusal::Json);
    }
    digits(
        raw.get().as_bytes(),
        if human {
            maximum.min(9_007_199_254_740_991)
        } else {
            maximum
        },
    )
}
pub(super) fn array<T>(
    raw: &RawValue,
    layout: &mut Layout,
    item: fn(&RawValue, &mut Layout) -> R<()>,
) -> R<()> {
    shape(raw, b'[')?;
    struct Items<'a> {
        layout: &'a mut Layout,
        item: fn(&RawValue, &mut Layout) -> R<()>,
    }
    impl<'de> Visitor<'de> for Items<'_> {
        type Value = usize;
        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("portable-offer elements")
        }
        fn visit_seq<A: SeqAccess<'de>>(self, mut array: A) -> Result<usize, A::Error> {
            let mut count = 0usize;
            while let Some(raw) = array.next_element::<&RawValue>()? {
                (self.item)(raw, self.layout)
                    .map_err(|_| A::Error::custom("unsupported portable-offer element"))?;
                count = count
                    .checked_add(1)
                    .ok_or_else(|| A::Error::custom("portable-offer count overflow"))?;
            }
            Ok(count)
        }
    }
    let mut d = serde_json::Deserializer::from_str(raw.get());
    let count = serde::Deserializer::deserialize_seq(&mut d, Items { layout, item })
        .map_err(|_| Refusal::Unsupported)?;
    d.end().map_err(|_| Refusal::Json)?;
    layout.vector::<T>(count)
}
pub(super) fn empty_array(raw: &RawValue) -> R<()> {
    shape(raw, b'[')?;
    struct Empty;
    impl<'de> Visitor<'de> for Empty {
        type Value = ();
        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("an empty supported field")
        }
        fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<(), A::Error> {
            if seq.next_element::<&RawValue>()?.is_some() {
                Err(A::Error::custom("unsupported nonempty field"))
            } else {
                Ok(())
            }
        }
    }
    let mut d = serde_json::Deserializer::from_str(raw.get());
    serde::Deserializer::deserialize_seq(&mut d, Empty).map_err(|_| Refusal::Unsupported)?;
    d.end().map_err(|_| Refusal::Json)
}
