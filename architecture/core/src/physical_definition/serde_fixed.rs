//! Fixed-bound byte sequence admission avoids allocating an unchecked Vec.
use core::fmt;
use serde::de::{Error, SeqAccess, Visitor};
pub(crate) fn deserialize_fixed<'de, D: serde::Deserializer<'de>, const N: usize>(
    deserializer: D,
) -> Result<[u8; N], D::Error> {
    struct Fixed<const N: usize>;
    impl<'de, const N: usize> Visitor<'de> for Fixed<N> {
        type Value = [u8; N];
        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            write!(formatter, "exactly {N} canonical bytes")
        }
        fn visit_seq<A: SeqAccess<'de>>(self, mut sequence: A) -> Result<Self::Value, A::Error> {
            if sequence.size_hint().is_some_and(|size| size != N) {
                return Err(A::Error::custom("wrong canonical byte count"));
            }
            let mut bytes = [0; N];
            for (index, byte) in bytes.iter_mut().enumerate() {
                *byte = sequence
                    .next_element()?
                    .ok_or_else(|| A::Error::invalid_length(index, &self))?;
            }
            if sequence.next_element::<u8>()?.is_some() {
                return Err(A::Error::invalid_length(N + 1, &self));
            }
            Ok(bytes)
        }
    }
    deserializer.deserialize_seq(Fixed::<N>)
}
