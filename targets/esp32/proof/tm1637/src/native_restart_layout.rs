//! Bounded raw persisted receipts; validation grants no physical storage custody.
#[cfg(any(feature = "local-evidence-store", feature = "observation-store"))]
pub(crate) use crate::native_observed_archive::custody_journal::layout::Receipt;
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};
const BYTES: usize = 152;
#[derive(Clone, Default, PartialEq, Eq)]
pub(crate) struct Layouts(Vec<Raw>);
#[derive(Clone, PartialEq, Eq)]
struct Raw([u8; BYTES]);
impl Serialize for Raw {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut hex = [0; BYTES * 2];
        for (i, byte) in self.0.iter().enumerate() {
            hex[i * 2] = b"0123456789abcdef"[usize::from(byte >> 4)];
            hex[i * 2 + 1] = b"0123456789abcdef"[usize::from(byte & 15)];
        }
        s.serialize_str(core::str::from_utf8(&hex).map_err(serde::ser::Error::custom)?)
    }
}
impl<'de> Deserialize<'de> for Raw {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl serde::de::Visitor<'_> for V {
            type Value = Raw;
            fn expecting(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                f.write_str("152 bytes as lowercase hex")
            }
            fn visit_str<E: serde::de::Error>(self, s: &str) -> Result<Raw, E> {
                if s.len() != BYTES * 2 {
                    return Err(E::custom("layout receipt extent"));
                }
                let mut out = [0; BYTES];
                for (i, pair) in s.as_bytes().as_chunks::<2>().0.iter().enumerate() {
                    let nibble = |b| match b {
                        b'0'..=b'9' => Some(b - b'0'),
                        b'a'..=b'f' => Some(b - b'a' + 10),
                        _ => None,
                    };
                    out[i] = nibble(pair[0])
                        .zip(nibble(pair[1]))
                        .map(|(a, b)| a * 16 + b)
                        .ok_or_else(|| E::custom("layout receipt hex"))?;
                }
                Ok(Raw(out))
            }
        }
        d.deserialize_str(V)
    }
}
#[cfg(test)]
impl Layouts {
    pub(crate) fn backing_identity(&self) -> (*const (), usize) {
        (self.0.as_ptr().cast(), self.0.capacity())
    }
}
impl Serialize for Layouts {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        self.0.serialize(s)
    }
}
impl<'de> Deserialize<'de> for Layouts {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct V;
        impl<'de> serde::de::Visitor<'de> for V {
            type Value = Layouts;
            fn expecting(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                f.write_str("at most five distinct layout receipts")
            }
            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut a: A,
            ) -> Result<Layouts, A::Error> {
                let mut out = Layouts::default();
                while let Some(raw) = a.next_element::<Raw>()? {
                    if out.0.len() == 5 || out.0.iter().any(|old| old.0[56..88] == raw.0[56..88]) {
                        return Err(serde::de::Error::custom(
                            "layout receipt capacity or duplicate region",
                        ));
                    }
                    out.0
                        .try_reserve_exact(1)
                        .map_err(serde::de::Error::custom)?;
                    out.0.push(raw);
                }
                Ok(out)
            }
        }
        d.deserialize_seq(V)
    }
}
impl Layouts {
    pub(crate) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
    pub(crate) fn validate(
        &self,
        body: &conduit_body::BodyId,
        key: &ed25519_dalek::VerifyingKey,
    ) -> Result<(), &'static str> {
        #[cfg(not(any(feature = "local-evidence-store", feature = "observation-store")))]
        {
            let _ = (body, key);
            if !self.is_empty() {
                return Err("restart-layout-profile-unavailable");
            }
            Ok(())
        }
        #[cfg(any(feature = "local-evidence-store", feature = "observation-store"))]
        {
            for raw in &self.0 {
                Receipt::decode(raw.0)
                    .validate(body, key)
                    .map_err(|_| "restart-layout-receipt")?;
            }
            Ok(())
        }
    }
    #[cfg(any(feature = "local-evidence-store", feature = "observation-store"))]
    pub(crate) fn add(&mut self, receipt: &Receipt) -> Result<(), &'static str> {
        if self
            .0
            .iter()
            .any(|old| old.0[56..88] == receipt.bytes()[56..88])
        {
            return Err("restart-layout-duplicate");
        }
        if self.0.len() == 5 {
            return Err("restart-layout-capacity");
        }
        self.0
            .try_reserve_exact(1)
            .map_err(|_| "restart-layout-memory")?;
        self.0.push(Raw(*receipt.bytes()));
        Ok(())
    }
    #[cfg(any(feature = "local-evidence-store", feature = "observation-store"))]
    pub(crate) fn find(&self, region: &[u8; 32]) -> Option<Receipt> {
        self.0
            .iter()
            .find(|r| &r.0[56..88] == region)
            .map(|r| Receipt::decode(r.0))
    }
}
