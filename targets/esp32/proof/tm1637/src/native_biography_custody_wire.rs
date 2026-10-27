//! Biography custody wire. Decoded values grant neither archive discharge nor execution.
use alloc::string::String;
use conduit_body::{BodyBiographyArchiveSegment, BodyId};
use ed25519_dalek::{Signature, VerifyingKey};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use sha2::{Digest, Sha256};
pub const OFFER: &str = "conduit.environment/biography-prefix-offer@1";
pub const ACK: &str = "conduit.environment/biography-prefix-ack@1";
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hex<const N: usize>(pub [u8; N]);
impl<const N: usize> Serialize for Hex<N> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::Error;
        if N > 96 {
            return Err(S::Error::custom("custody byte extent"));
        }
        let mut text = [0u8; 192];
        let digits = b"0123456789abcdef";
        for (pair, byte) in text[..N * 2].as_chunks_mut::<2>().0.iter_mut().zip(self.0) {
            pair[0] = digits[usize::from(byte >> 4)];
            pair[1] = digits[usize::from(byte & 15)];
        }
        serializer.serialize_str(core::str::from_utf8(&text[..N * 2]).map_err(S::Error::custom)?)
    }
}
impl<'de, const N: usize> Deserialize<'de> for Hex<N> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor<const N: usize>;
        impl<const N: usize> serde::de::Visitor<'_> for Visitor<N> {
            type Value = Hex<N>;
            fn expecting(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                write!(f, "{N} bytes in lowercase hexadecimal")
            }
            fn visit_str<E: serde::de::Error>(self, text: &str) -> Result<Self::Value, E> {
                if text.len() != N * 2 {
                    return Err(E::custom("custody byte extent"));
                }
                let mut out = [0; N];
                for (byte, pair) in out.iter_mut().zip(text.as_bytes().as_chunks::<2>().0) {
                    let digit = |b| match b {
                        b'0'..=b'9' => Some(b - b'0'),
                        b'a'..=b'f' => Some(b - b'a' + 10),
                        _ => None,
                    };
                    *byte = (digit(pair[0]).ok_or_else(|| E::custom("custody hexadecimal"))? << 4)
                        | digit(pair[1]).ok_or_else(|| E::custom("custody hexadecimal"))?;
                }
                Ok(Hex(out))
            }
        }
        deserializer.deserialize_str(Visitor::<N>)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Summary {
    pub count: u64,
    pub head: Option<Hex<32>>,
    pub last_sequence: u64,
    pub last_sign: Option<String>,
}
impl Summary {
    pub fn append(
        &mut self,
        body: &BodyId,
        segment: &BodyBiographyArchiveSegment,
    ) -> Result<(), &'static str> {
        segment.validate().map_err(|_| "biography segment")?;
        if &segment.body_id != body
            || self.count.checked_add(1) != Some(segment.ordinal)
            || self.head.map(|v| v.0) != segment.previous_digest
        {
            return Err("biography chain");
        }
        let record = segment.records.last().ok_or("biography empty segment")?;
        if record.sequence < self.last_sequence {
            return Err("biography sequence");
        }
        self.count = segment.ordinal;
        self.head = Some(Hex(segment.digest));
        self.last_sequence = record.sequence;
        self.last_sign = Some(record.sign_id.as_str().into());
        Ok(())
    }
    fn hash(&self, hash: &mut Sha256) {
        hash.update(self.count.to_le_bytes());
        hash.update([u8::from(self.head.is_some())]);
        hash.update(self.head.map_or([0; 32], |v| v.0));
        hash.update(self.last_sequence.to_le_bytes());
        let sign = self.last_sign.as_deref().unwrap_or("");
        hash.update((sign.len() as u64).to_le_bytes());
        hash.update(sign.as_bytes());
    }
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.count == 0 {
            if self != &Self::default() {
                return Err("biography empty summary");
            }
        } else if self.head.is_none()
            || self
                .last_sign
                .as_ref()
                .is_none_or(|s| s.is_empty() || s.len() > 128 || s.chars().any(char::is_control))
        {
            return Err("biography summary");
        }
        Ok(())
    }
}
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Offer {
    pub schema: String,
    pub body: BodyId,
    pub generation: u64,
    pub source: Hex<32>,
    pub base: Summary,
    pub end: Summary,
    pub stream: Hex<32>,
    pub signature: Hex<64>,
}
impl Offer {
    pub fn transcript(&self, root: &[u8; 32]) -> [u8; 32] {
        let mut hash = Sha256::new();
        hash.update(b"conduit.environment/biography-prefix-offer@1\0");
        hash.update(root);
        hash.update((self.body.as_str().len() as u64).to_le_bytes());
        hash.update(self.body.as_str().as_bytes());
        hash.update(self.generation.to_le_bytes());
        hash.update(self.source.0);
        self.base.hash(&mut hash);
        self.end.hash(&mut hash);
        hash.update(self.stream.0);
        hash.finalize().into()
    }
    pub fn verify(&self, body: &BodyId, root: &VerifyingKey) -> Result<[u8; 32], &'static str> {
        self.base.validate()?;
        self.end.validate()?;
        if self.schema != OFFER
            || &self.body != body
            || self.generation == 0
            || self.end.count <= self.base.count
        {
            return Err("biography offer scope");
        }
        let id = self.transcript(&root.to_bytes());
        root.verify_strict(&id, &Signature::from_bytes(&self.signature.0))
            .map_err(|_| "biography offer signature")?;
        Ok(id)
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub schema: String,
    pub offer: Hex<32>,
    pub part: String,
    pub host: String,
    pub boot: String,
    pub signature: Hex<64>,
}
impl Request {
    pub fn transcript(&self) -> Result<[u8; 32], &'static str> {
        if self.schema != ACK
            || [&self.part, &self.host, &self.boot]
                .iter()
                .any(|s| s.is_empty() || s.len() > 128 || s.chars().any(char::is_control))
        {
            return Err("biography ack scope");
        }
        let mut hash = Sha256::new();
        hash.update(b"conduit.environment/biography-prefix-ack@1\0");
        hash.update(self.offer.0);
        for id in [&self.part, &self.host, &self.boot] {
            hash.update((id.len() as u64).to_le_bytes());
            hash.update(id.as_bytes());
        }
        Ok(hash.finalize().into())
    }
}

/// Root endorsement retained in restart@2. An independently pinned Root is
/// always required; no embedded key supplies authority.
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub enum Consumer {
    Admitted { identity: Hex<32> },
    Owner { key: Hex<32> },
}

#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    pub body: BodyId,
    pub summary: Summary,
    pub offer: Hex<32>,
    pub consumer: Consumer,
    pub acknowledged: Hex<32>,
    pub signature: Hex<64>,
}
impl Receipt {
    pub fn transcript(&self, root: &[u8; 32]) -> [u8; 32] {
        let mut hash = Sha256::new();
        hash.update(b"conduit.environment/accepted-biography-prefix@1\0");
        hash.update(root);
        hash.update((self.body.as_str().len() as u64).to_le_bytes());
        hash.update(self.body.as_str().as_bytes());
        self.summary.hash(&mut hash);
        hash.update(self.offer.0);
        match &self.consumer {
            Consumer::Admitted { identity } => {
                hash.update([0]);
                hash.update(identity.0);
            }
            Consumer::Owner { key } => {
                hash.update([1]);
                hash.update(key.0);
            }
        }
        hash.update(self.acknowledged.0);
        hash.finalize().into()
    }
    pub fn verify(&self, body: &BodyId, root: &VerifyingKey) -> Result<(), &'static str> {
        self.summary.validate()?;
        if &self.body != body || self.summary.count == 0 {
            return Err("biography receipt scope");
        }
        root.verify_strict(
            &self.transcript(&root.to_bytes()),
            &Signature::from_bytes(&self.signature.0),
        )
        .map_err(|_| "biography receipt signature")
    }
}
