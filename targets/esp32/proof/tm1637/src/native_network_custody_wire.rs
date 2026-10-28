//! Persisted network-retirement receipt authentication. No erase authority.
use conduit_body::BodyId;
use ed25519_dalek::{Signature, VerifyingKey};
use sha2::{Digest, Sha256};
pub(crate) const CURSOR_BYTES: usize = 480;
const RECEIPT_BYTES: usize = 448;
const OFFER_DOMAIN: &[u8] = b"conduit.environment/network-retirement-offer@1\0";
const ACK_DOMAIN: &[u8] = b"conduit.environment/network-retirement-custody@1\0";
const RECEIPT_DOMAIN: &[u8] = b"conduit.environment/network-retirement-receipt@1\0";
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct Cursor([u8; CURSOR_BYTES]);
impl Cursor {
    /// Descriptive decoding; callers must authenticate before use.
    pub(crate) fn decode(bytes: [u8; CURSOR_BYTES]) -> Self {
        Self(bytes)
    }
    pub(crate) fn bytes(&self) -> &[u8; CURSOR_BYTES] {
        &self.0
    }
    pub(crate) fn validate(&self, body: &BodyId, root: &VerifyingKey) -> Result<(), &'static str> {
        let bytes = &self.0;
        if &bytes[..8] != b"NCURSOR1"
            || !matches!(bytes[8], 1 | 2)
            || bytes[9..16]
                .iter()
                .chain(bytes[472..].iter())
                .any(|b| *b != 0)
        {
            return Err("network cursor encoding");
        }
        let receipt: &[u8; RECEIPT_BYTES] = bytes[24..472].try_into().unwrap();
        validate_receipt(receipt, &bytes[..24], body, root)?;
        let retired = u64::from_le_bytes(receipt[64..72].try_into().unwrap());
        let epoch = u64::from_le_bytes(bytes[16..24].try_into().unwrap());
        let expected = if bytes[8] == 1 {
            retired
        } else {
            retired
                .checked_add(1)
                .ok_or("network cursor epoch overflow")?
        };
        if epoch != expected {
            return Err("network cursor epoch");
        }
        Ok(())
    }
}
fn offer_transcript(receipt: &[u8; RECEIPT_BYTES], root: &VerifyingKey) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(OFFER_DOMAIN);
    hash.update(root.as_bytes());
    hash.update(&receipt[..144]);
    hash.update(&receipt[336..368]);
    hash.finalize().into()
}
fn ack_transcript(offer: &[u8], member_scope: &[u8]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(ACK_DOMAIN);
    hash.update(offer);
    hash.update(member_scope);
    hash.finalize().into()
}
fn receipt_transcript(receipt: &[u8; RECEIPT_BYTES], header: &[u8]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(RECEIPT_DOMAIN);
    hash.update(header);
    hash.update(&receipt[..384]);
    hash.finalize().into()
}
fn validate_receipt(
    receipt: &[u8; RECEIPT_BYTES],
    header: &[u8],
    body: &BodyId,
    root: &VerifyingKey,
) -> Result<(), &'static str> {
    let body_hash: [u8; 32] = Sha256::digest(body.as_str().as_bytes()).into();
    if receipt[..32] != body_hash
        || receipt[104..108] != 32768u32.to_le_bytes()
        || !(1..=8).contains(&receipt[108])
        || receipt[109..112]
            .iter()
            .chain(receipt[368..384].iter())
            .any(|b| *b != 0)
    {
        return Err("network custody geometry or Body");
    }
    let offer = offer_transcript(receipt, root);
    let ack = ack_transcript(&offer, &receipt[176..208]);
    if receipt[144..176] != offer || receipt[304..336] != ack {
        return Err("network custody transcript");
    }
    let member = VerifyingKey::from_bytes(receipt[208..240].try_into().unwrap())
        .map_err(|_| "network custody member key")?;
    member
        .verify_strict(
            &ack,
            &Signature::from_bytes(receipt[240..304].try_into().unwrap()),
        )
        .map_err(|_| "network custody member signature")?;
    root.verify_strict(
        &receipt_transcript(receipt, header),
        &Signature::from_bytes(receipt[384..448].try_into().unwrap()),
    )
    .map_err(|_| "network custody Root signature")?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use ed25519_dalek::{Signer, SigningKey};
    fn fixture(phase: u8, epoch: u64) -> (Cursor, BodyId, VerifyingKey) {
        let body = serde_json::from_str::<BodyId>("\"body/network-custody-test\"").unwrap();
        let root = SigningKey::from_bytes(&[71; 32]);
        let member = SigningKey::from_bytes(&[72; 32]);
        let mut bytes = [0; CURSOR_BYTES];
        bytes[..8].copy_from_slice(b"NCURSOR1");
        bytes[8] = phase;
        bytes[16..24].copy_from_slice(&epoch.to_le_bytes());
        let header: [u8; 24] = bytes[..24].try_into().unwrap();
        let r: &mut [u8; RECEIPT_BYTES] = (&mut bytes[24..472]).try_into().unwrap();
        r[..32].copy_from_slice(&Sha256::digest(body.as_str().as_bytes()));
        r[32..64].fill(1);
        r[64..72].copy_from_slice(&3u64.to_le_bytes());
        r[72..104].fill(2);
        r[104..108].copy_from_slice(&32768u32.to_le_bytes());
        r[108] = 7;
        r[112..144].fill(3);
        r[176..208].fill(4);
        r[208..240].copy_from_slice(member.verifying_key().as_bytes());
        let offer = offer_transcript(r, &root.verifying_key());
        r[144..176].copy_from_slice(&offer);
        let ack = ack_transcript(&offer, &r[176..208]);
        r[240..304].copy_from_slice(&member.sign(&ack).to_bytes());
        r[304..336].copy_from_slice(&ack);
        let signature = root.sign(&receipt_transcript(r, &header));
        r[384..448].copy_from_slice(&signature.to_bytes());
        (Cursor::decode(bytes), body, root.verifying_key())
    }
    #[test]
    fn pending_and_clean_bind_exact_retired_epoch() {
        for (phase, epoch) in [(1, 3), (2, 4)] {
            let (cursor, body, root) = fixture(phase, epoch);
            assert!(cursor.validate(&body, &root).is_ok());
        }
        for (phase, epoch) in [(1, 4), (2, 3), (0, 3), (3, 4)] {
            let (cursor, body, root) = fixture(phase, epoch);
            assert!(cursor.validate(&body, &root).is_err());
        }
    }
    #[test]
    fn every_receipt_and_cursor_byte_is_authenticated_or_canonical() {
        let (cursor, body, root) = fixture(1, 3);
        let mut forged_clean = cursor.clone();
        forged_clean.0[8] = 2;
        forged_clean.0[16..24].copy_from_slice(&4u64.to_le_bytes());
        assert!(forged_clean.validate(&body, &root).is_err());
        for index in 0..CURSOR_BYTES {
            let mut changed = cursor.clone();
            changed.0[index] ^= 0x80;
            assert!(changed.validate(&body, &root).is_err(), "byte {index}");
        }
        assert!(cursor
            .validate(
                &serde_json::from_str::<BodyId>("\"body/another\"").unwrap(),
                &root
            )
            .is_err());
        assert!(cursor
            .validate(&body, &SigningKey::from_bytes(&[73; 32]).verifying_key())
            .is_err());
    }
}
