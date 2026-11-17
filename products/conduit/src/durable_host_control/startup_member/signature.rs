//! Fixed proof extent; deserialization never grows a signature Vec.
use serde::{
    de::{Error, SeqAccess, Visitor},
    ser::SerializeTuple,
    Deserialize, Deserializer, Serialize, Serializer,
};
#[derive(Clone, Copy)]
pub(crate) struct Signature(pub(crate) [u8; 64]);
impl Serialize for Signature {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut tuple = serializer.serialize_tuple(64)?;
        for byte in &self.0 {
            tuple.serialize_element(byte)?;
        }
        tuple.end()
    }
}
impl<'de> Deserialize<'de> for Signature {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Proof;
        impl<'de> Visitor<'de> for Proof {
            type Value = Signature;
            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("one exact 64-byte signature")
            }
            fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Signature, A::Error> {
                let mut bytes = [0; 64];
                for byte in &mut bytes {
                    *byte = sequence
                        .next_element()?
                        .ok_or_else(|| A::Error::custom("short signature"))?;
                }
                if sequence.next_element::<u8>()?.is_some() {
                    return Err(A::Error::custom("long signature"));
                }
                Ok(Signature(bytes))
            }
        }
        deserializer.deserialize_tuple(64, Proof)
    }
}
