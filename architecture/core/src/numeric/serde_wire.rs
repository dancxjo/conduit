//! Serde transports the same canonical mathematical carrier, with bounded admission.
use super::*;
use core::fmt;
use serde::de::{Error, SeqAccess, Visitor};
use serde::ser::Error as _;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
impl Serialize for MathValue {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        self.canonical_bytes()
            .map_err(|_| S::Error::custom("invalid mathematical carrier"))?
            .serialize(serializer)
    }
}
impl<'de> Deserialize<'de> for MathValue {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct BoundedWire;
        impl<'de> Visitor<'de> for BoundedWire {
            type Value = MathValue;
            fn expecting(&self, out: &mut fmt::Formatter<'_>) -> fmt::Result {
                out.write_str("bounded canonical mathematical wire bytes")
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<Self::Value, A::Error> {
                if seq
                    .size_hint()
                    .is_some_and(|n| n > MAX_NUMERIC_ENCODED_BYTES)
                {
                    return Err(A::Error::custom("mathematical wire bound exceeded"));
                }
                let mut bytes = Vec::new();
                while let Some(byte) = seq.next_element::<u8>()? {
                    if bytes.len() == MAX_NUMERIC_ENCODED_BYTES {
                        return Err(A::Error::custom("mathematical wire bound exceeded"));
                    }
                    bytes.push(byte);
                }
                MathValue::decode(&bytes)
                    .map_err(|_| A::Error::custom("noncanonical mathematical value"))
            }
            fn visit_bytes<E: Error>(self, bytes: &[u8]) -> Result<Self::Value, E> {
                MathValue::decode(bytes).map_err(|_| E::custom("noncanonical mathematical value"))
            }
        }
        deserializer.deserialize_seq(BoundedWire)
    }
}
