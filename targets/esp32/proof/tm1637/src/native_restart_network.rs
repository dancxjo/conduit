//! Descriptive persisted network custody. Authentication is mandatory before
//! publication; neither decoded hex nor this wrapper authorizes erasure.
use alloc::boxed::Box;
use serde::{Deserialize, Serialize};
#[path = "native_network_custody_wire.rs"]
pub(crate) mod wire;
#[derive(PartialEq, Eq)]
pub(crate) struct Cursor(Box<wire::Cursor>);
impl Cursor {
    #[inline(never)]
    pub(crate) fn decode(bytes: [u8; wire::CURSOR_BYTES]) -> Result<Self, &'static str> {
        let layout = core::alloc::Layout::new::<wire::Cursor>();
        // SAFETY: allocate the exact nonzero Cursor layout and retain refusal
        // before publishing ownership. Box::write initializes its whole value.
        let raw =
            unsafe { alloc::alloc::alloc(layout) }.cast::<core::mem::MaybeUninit<wire::Cursor>>();
        if raw.is_null() {
            return Err("restart-network-cursor-memory");
        }
        let backing = unsafe { Box::from_raw(raw) };
        Ok(Self(Box::write(backing, wire::Cursor::decode(bytes))))
    }
    pub(crate) fn try_clone(&self) -> Result<Self, &'static str> {
        Self::decode(*self.bytes())
    }
    pub(crate) fn bytes(&self) -> &[u8; wire::CURSOR_BYTES] {
        self.0.bytes()
    }
    pub(crate) fn offer_chain(
        &self,
        body: &conduit_body::BodyId,
        root: &ed25519_dalek::VerifyingKey,
    ) -> Result<(u64, [u8; 32]), &'static str> {
        self.0.offer_chain(body, root)
    }
    pub(crate) fn validate(
        &self,
        body: &conduit_body::BodyId,
        root: &ed25519_dalek::VerifyingKey,
    ) -> Result<(), &'static str> {
        self.0.validate(body, root)
    }
}
impl Serialize for Cursor {
    #[inline(never)]
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut hex = [0u8; wire::CURSOR_BYTES * 2];
        for (pair, byte) in hex.as_chunks_mut::<2>().0.iter_mut().zip(self.bytes()) {
            pair[0] = b"0123456789abcdef"[usize::from(byte >> 4)];
            pair[1] = b"0123456789abcdef"[usize::from(byte & 15)];
        }
        serializer.serialize_str(core::str::from_utf8(&hex).map_err(serde::ser::Error::custom)?)
    }
}
impl<'de> Deserialize<'de> for Cursor {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl serde::de::Visitor<'_> for Visitor {
            type Value = Cursor;
            fn expecting(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                f.write_str("480 bytes of lowercase network custody hex")
            }
            #[inline(never)]
            fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<Cursor, E> {
                if value.len() != wire::CURSOR_BYTES * 2 {
                    return Err(E::custom("network cursor extent"));
                }
                let mut bytes = [0; wire::CURSOR_BYTES];
                for (out, pair) in bytes.iter_mut().zip(value.as_bytes().as_chunks::<2>().0) {
                    let digit = |b| match b {
                        b'0'..=b'9' => Some(b - b'0'),
                        b'a'..=b'f' => Some(b - b'a' + 10),
                        _ => None,
                    };
                    *out = digit(pair[0])
                        .zip(digit(pair[1]))
                        .map(|(a, b)| a * 16 + b)
                        .ok_or_else(|| E::custom("network cursor hex"))?;
                }
                Cursor::decode(bytes).map_err(E::custom)
            }
        }
        deserializer.deserialize_str(Visitor)
    }
}

#[cfg(test)]
mod fixture {
    use super::*;
    use conduit_body::BodyId;
    use ed25519_dalek::{Signer, SigningKey};
    use sha2::{Digest, Sha256};
    fn digest(domain: &[u8], parts: &[&[u8]]) -> [u8; 32] {
        let mut h = Sha256::new();
        h.update(domain);
        for part in parts {
            h.update(part);
        }
        h.finalize().into()
    }
    pub(crate) fn fixture(body: &BodyId, root: &SigningKey, phase: u8) -> Cursor {
        let member = SigningKey::from_bytes(&[91; 32]);
        let mut receipt = [0; 448];
        receipt[..32].copy_from_slice(&Sha256::digest(body.as_str().as_bytes()));
        receipt[32..64].fill(13);
        receipt[64..72].copy_from_slice(&7u64.to_le_bytes());
        receipt[72..104].fill(17);
        receipt[104..108].copy_from_slice(&32768u32.to_le_bytes());
        receipt[108] = 7;
        receipt[112..144].fill(19);
        receipt[176..208].fill(23);
        receipt[208..240].copy_from_slice(member.verifying_key().as_bytes());
        receipt[336..368].fill(29);
        let offer = digest(
            b"conduit.environment/network-retirement-offer@1\0",
            &[
                root.verifying_key().as_bytes(),
                &receipt[..144],
                &receipt[336..368],
            ],
        );
        receipt[144..176].copy_from_slice(&offer);
        let ack = digest(
            b"conduit.environment/network-retirement-custody@1\0",
            &[&offer, &receipt[176..208]],
        );
        receipt[240..304].copy_from_slice(&member.sign(&ack).to_bytes());
        receipt[304..336].copy_from_slice(&ack);
        let mut header = [0; 24];
        header[..8].copy_from_slice(b"NCURSOR1");
        header[8] = phase;
        header[16..24].copy_from_slice(&(if phase == 1 { 7u64 } else { 8u64 }).to_le_bytes());
        let transcript = digest(
            b"conduit.environment/network-retirement-receipt@1\0",
            &[&header, &receipt[..384]],
        );
        receipt[384..448].copy_from_slice(&root.sign(&transcript).to_bytes());
        let mut bytes = [0; wire::CURSOR_BYTES];
        bytes[..24].copy_from_slice(&header);
        bytes[24..472].copy_from_slice(&receipt);
        let cursor = Cursor::decode(bytes).unwrap();
        cursor.validate(body, &root.verifying_key()).unwrap();
        cursor
    }
}
#[cfg(test)]
pub(crate) use fixture::fixture;
