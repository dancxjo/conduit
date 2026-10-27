//! Typed closed observation custody drafts. Not a membership or erase grant.
use conduit_body::BodyId;
use ed25519_dalek::{Signature, VerifyingKey};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use sha2::{Digest, Sha256};
pub const OFFER: &str = "conduit.environment/closed-observed-epoch@1";
pub const ACK: &str = "conduit.environment/observed-custody-request@1";
pub const MAX_REQUEST: usize = 1024;
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
#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Offer<'a> {
    #[serde(borrow)]
    pub schema: &'a str,
    pub region: Hex<32>,
    pub scope: Hex<96>,
    pub forms: Hex<32>,
    pub committed: u32,
    pub digest: Hex<32>,
    pub signature: Hex<64>,
}
pub fn offer_transcript(offer: &Offer<'_>, root: &[u8; 32]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"conduit.environment/closed-observed-epoch@1\0");
    hash.update(root);
    hash.update(offer.region.0);
    hash.update(offer.scope.0);
    hash.update(offer.forms.0);
    hash.update(offer.committed.to_le_bytes());
    hash.update(offer.digest.0);
    hash.finalize().into()
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Request<'a> {
    #[serde(borrow)]
    pub schema: &'a str,
    pub offer: Hex<32>,
    #[serde(borrow)]
    pub part: &'a str,
    #[serde(borrow)]
    pub host: &'a str,
    #[serde(borrow)]
    pub boot: &'a str,
    pub signature: Hex<64>,
}
impl Request<'_> {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.schema != ACK
            || [self.part, self.host, self.boot]
                .iter()
                .any(|id| id.is_empty() || id.len() > 128 || id.chars().any(char::is_control))
        {
            return Err("custody request scope");
        }
        Ok(())
    }
    pub fn transcript(&self) -> [u8; 32] {
        let mut hash = Sha256::new();
        hash.update(b"conduit.environment/observed-custody-request@1\0");
        hash.update(self.offer.0);
        for id in [self.part, self.host, self.boot] {
            hash.update((id.len() as u64).to_le_bytes());
            hash.update(id.as_bytes());
        }
        hash.finalize().into()
    }
}
pub fn accepted_transcript(accepted: &[u8; 416]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"conduit.environment/accepted-epoch-custody@1\0");
    hash.update(&accepted[..352]);
    hash.finalize().into()
}

pub(crate) struct VerifiedOffer {
    offer: Offer<'static>,
    id: [u8; 32],
}
impl VerifiedOffer {
    pub(crate) fn scope(&self) -> &[u8; 96] {
        &self.offer.scope.0
    }
    pub(crate) fn forms(&self) -> &[u8; 32] {
        &self.offer.forms.0
    }
    pub(crate) fn count(&self) -> u32 {
        self.offer.committed
    }
    pub(crate) fn digest(&self) -> &[u8; 32] {
        &self.offer.digest.0
    }
    pub(crate) fn id(&self) -> [u8; 32] {
        self.id
    }
}
pub(crate) fn verify_offer(
    offer: &Offer<'_>,
    body: &BodyId,
    key: &VerifyingKey,
) -> Result<VerifiedOffer, &'static str> {
    if offer.schema != OFFER
        || offer.committed == 0
        || offer.committed > 960
        || offer.scope.0[..32] != Sha256::digest(body.as_str().as_bytes())[..]
    {
        return Err("custody offer scope");
    }
    let id = offer_transcript(offer, &key.to_bytes());
    key.verify_strict(&id, &Signature::from_bytes(&offer.signature.0))
        .map_err(|_| "custody offer signature")?;
    Ok(VerifiedOffer {
        offer: Offer {
            schema: OFFER,
            region: offer.region,
            scope: offer.scope,
            forms: offer.forms,
            committed: offer.committed,
            digest: offer.digest,
            signature: offer.signature,
        },
        id,
    })
}
