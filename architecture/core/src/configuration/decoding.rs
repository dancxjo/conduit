//! Deserialize retained canonical bytes without per-byte reconstruction when
//! a binary carrier already exposes the exact contiguous byte slice.
use super::StructuredConfigurationValue;
use crate::KindId;
use alloc::vec::Vec;
use core::fmt;
use serde::{
    de::{self, Error as _, SeqAccess, Visitor},
    Deserialize, Deserializer,
};

impl<'de> Deserialize<'de> for StructuredConfigurationValue {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Encoded {
            profile: KindId,
            #[serde(deserialize_with = "canonical_bytes")]
            canonical_value: Vec<u8>,
        }

        let encoded = Encoded::deserialize(deserializer)?;
        Self::new(encoded.profile, encoded.canonical_value)
            .ok_or_else(|| D::Error::custom("invalid structured configuration value"))
    }
}

// Serialization remains the existing numeric sequence. Human-readable
// decoders keep their exact original Vec<u8> admission, including rejecting
// strings. Binary carriers may supply either the old sequence or packed bytes.
fn canonical_bytes<'de, D: Deserializer<'de>>(deserializer: D) -> Result<Vec<u8>, D::Error> {
    if deserializer.is_human_readable() {
        return Vec::<u8>::deserialize(deserializer);
    }
    struct Bytes;
    impl<'de> Visitor<'de> for Bytes {
        type Value = Vec<u8>;
        fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
            formatter.write_str("a sequence")
        }
        fn visit_seq<A: SeqAccess<'de>>(self, sequence: A) -> Result<Vec<u8>, A::Error> {
            // Delegate the fallback to the original Vec visitor, preserving its
            // allocation hints and element validation rather than duplicating it.
            Vec::<u8>::deserialize(de::value::SeqAccessDeserializer::new(sequence))
        }
        fn visit_bytes<E: de::Error>(self, bytes: &[u8]) -> Result<Vec<u8>, E> {
            Ok(bytes.to_vec())
        }
        fn visit_byte_buf<E: de::Error>(self, bytes: Vec<u8>) -> Result<Vec<u8>, E> {
            Ok(bytes)
        }
    }
    deserializer.deserialize_byte_buf(Bytes)
}
