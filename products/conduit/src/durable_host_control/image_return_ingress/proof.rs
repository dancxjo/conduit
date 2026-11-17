//! Proof shape before owned decoding, not signature verification or funding.
//! The enclosing parser remains test-fenced. Even inspection needs separately
//! admitted parser scratch: escaped JSON strings can allocate in serde_json.
use super::{absent, end, required, validated_span, Tag, Unsigned};
use serde::de::{Error, MapAccess, SeqAccess, Visitor};
use serde::Deserialize;
use serde_json::value::RawValue;
use std::fmt;

const FIELDS: &[&str] = &[
    "admission_id",
    "body_id",
    "part_id",
    "host_id",
    "boot_id",
    "nonce",
    "signature",
];

/// Borrowed facts about exactly these bytes. Counts are not allocation grants,
/// constructor capacities, parser peaks or permission to mutate Core.
pub(in crate::durable_host_control) struct ProofShape<'a> {
    raw: &'a RawValue,
    pub(super) identifier_bytes: [usize; 5],
}
impl ProofShape<'_> {
    /// Bypass Request/Command's internally tagged Content buffers. This still
    /// needs an independently admitted owned-constructor envelope, including
    /// String scratch, signature Vec growth and result/descendant custody.
    pub(in crate::durable_host_control) fn decode(
        &self,
    ) -> Result<conduit_body::MemberReturnProofDocument, serde_json::Error> {
        serde_json::from_str(self.raw.get())
    }
}

struct Fields<'a>([&'a RawValue; 7]);
impl<'de> Deserialize<'de> for Fields<'de> {
    fn deserialize<D: serde::Deserializer<'de>>(decoder: D) -> Result<Self, D::Error> {
        struct Parts;
        impl<'de> Visitor<'de> for Parts {
            type Value = Fields<'de>;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("a member-return proof")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<Self::Value, A::Error> {
                let mut values = [None; 7];
                while let Some(index) = map.next_key_seed(Tag(FIELDS))? {
                    absent(&values[index])?;
                    values[index] = Some(map.next_value()?);
                }
                Ok(Fields([
                    required(values[0])?,
                    required(values[1])?,
                    required(values[2])?,
                    required(values[3])?,
                    required(values[4])?,
                    required(values[5])?,
                    required(values[6])?,
                ]))
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
                let fields = Fields([
                    required(seq.next_element()?)?,
                    required(seq.next_element()?)?,
                    required(seq.next_element()?)?,
                    required(seq.next_element()?)?,
                    required(seq.next_element()?)?,
                    required(seq.next_element()?)?,
                    required(seq.next_element()?)?,
                ]);
                end(&mut seq)?;
                Ok(fields)
            }
        }
        decoder.deserialize_any(Parts)
    }
}

struct TextBytes;
impl<'de> Visitor<'de> for TextBytes {
    type Value = usize;
    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a finite lifecycle identifier")
    }
    fn visit_str<E: Error>(self, value: &str) -> Result<usize, E> {
        if value.is_empty() || value.len() > conduit_body::MAX_LIFECYCLE_ID_BYTES {
            Err(E::custom("proof identifier bounds"))
        } else {
            Ok(value.len())
        }
    }
}
fn identifier_bytes(input: &str) -> Result<usize, serde_json::Error> {
    // Shape precedes typed string dispatch: a wrong numeric type must never
    // invoke float_roundtrip/lexical merely to format an expected-string error.
    let raw = validated_span(input, b"\"")?;
    let mut decoder = serde_json::Deserializer::from_str(raw.get());
    let length = serde::Deserializer::deserialize_str(&mut decoder, TextBytes)?;
    decoder.end()?;
    Ok(length)
}
struct FixedBytes<const N: usize>;
impl<'de, const N: usize> Visitor<'de> for FixedBytes<N> {
    type Value = ();
    fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("a fixed integer-byte array")
    }
    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<(), A::Error> {
        for _ in 0..N {
            required(seq.next_element::<Unsigned<255>>()?)?;
        }
        end(&mut seq)
    }
}
fn fixed_bytes<const N: usize>(input: &str) -> Result<(), serde_json::Error> {
    let raw = validated_span(input, b"[")?;
    let mut decoder = serde_json::Deserializer::from_str(raw.get());
    serde::Deserializer::deserialize_any(&mut decoder, FixedBytes::<N>)?;
    decoder.end()
}

/// The successful profile matches original document deserialization followed
/// by consuming decode, not unrestricted document deserialization alone.
/// No challenge, signature or Body authorization is checked here.
pub(in crate::durable_host_control) fn inspect(
    input: &str,
) -> Result<ProofShape<'_>, serde_json::Error> {
    let raw = validated_span(input, b"{[")?;
    let Fields(values) = serde_json::from_str(raw.get())?;
    let identifier_bytes = [
        identifier_bytes(values[0].get())?,
        identifier_bytes(values[1].get())?,
        identifier_bytes(values[2].get())?,
        identifier_bytes(values[3].get())?,
        identifier_bytes(values[4].get())?,
    ];
    fixed_bytes::<32>(values[5].get())?;
    fixed_bytes::<64>(values[6].get())?;
    Ok(ProofShape {
        raw,
        identifier_bytes,
    })
}

#[cfg(test)]
#[path = "proof_tests.rs"]
mod tests;
