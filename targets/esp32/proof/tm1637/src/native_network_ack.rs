//! Canonical admitted custody of the actual pinned network snapshot. A borrowed
//! pending candidate is neither a committed checkpoint nor an erase permit.
use super::Draft;
use conduit_body::{AdmissionManager, BodyId, BodyMembership, MembershipCredential};
use sha2::{Digest, Sha256};
pub(crate) const REQUEST_BYTES: usize = 628;
const MAGIC: &[u8; 8] = b"NCACK1\0\0";
/// Fixed finite wire data. The immutable borrow prevents changing signed fields
/// between decoding, canonical member authentication and receipt construction.
pub(crate) struct Request<'a> {
    bytes: &'a [u8; REQUEST_BYTES],
    ids: [&'a str; 4],
}
impl<'a> Request<'a> {
    pub(crate) fn decode(bytes: &'a [u8]) -> Result<Self, &'static str> {
        let bytes: &[u8; REQUEST_BYTES] = bytes.try_into().map_err(|_| "network-ack-extent")?;
        if &bytes[..8] != MAGIC {
            return Err("network-ack-version");
        }
        let mut ids = [""; 4];
        for (index, out) in ids.iter_mut().enumerate() {
            let count = usize::from(bytes[48 + index]);
            let field = &bytes[52 + index * 128..52 + (index + 1) * 128];
            if !(1..=128).contains(&count) || field[count..].iter().any(|b| *b != 0) {
                return Err("network-ack-identity");
            }
            *out = core::str::from_utf8(&field[..count]).map_err(|_| "network-ack-identity")?;
            if out.chars().any(char::is_control) {
                return Err("network-ack-identity");
            }
        }
        Ok(Self { bytes, ids })
    }
    pub(crate) fn for_credential(
        offer: [u8; 32],
        credential: &MembershipCredential,
        signature: [u8; 64],
    ) -> Result<[u8; REQUEST_BYTES], &'static str> {
        let mut bytes = [0; REQUEST_BYTES];
        bytes[..8].copy_from_slice(MAGIC);
        bytes[8..40].copy_from_slice(&offer);
        bytes[40..48].copy_from_slice(&credential.issued_at_millis.to_le_bytes());
        for (index, id) in [
            credential.part_id.as_str(),
            credential.host_id.as_str(),
            credential.boot_id.as_str(),
            credential.credential_id.as_str(),
        ]
        .iter()
        .enumerate()
        {
            if id.is_empty() || id.len() > 128 || id.chars().any(char::is_control) {
                return Err("network-ack-identity");
            }
            bytes[48 + index] = id.len() as u8;
            bytes[52 + index * 128..52 + index * 128 + id.len()].copy_from_slice(id.as_bytes());
        }
        bytes[564..].copy_from_slice(&signature);
        Ok(bytes)
    }
    pub(crate) fn transcript(&self, body: &BodyId) -> [u8; 32] {
        let scope = scope_digest(body, self.ids, self.issued_at());
        acknowledgement(self.offer(), &scope)
    }
    /// Equality-only retry recognition after the Root authenticates a retained
    /// Pending cursor. This creates neither a receipt nor fresh membership.
    pub(crate) fn matches_pending(&self, owner: &Draft, body: &BodyId, bytes: &[u8; 480]) -> bool {
        bytes[8] == 1
            && bytes[24..168] == owner.payload[..144]
            && bytes[168..200] == owner.transcript()
            && bytes[360..392] == owner.payload[144..176]
            && self.matches_receipt(body, bytes)
    }
    /// Equality-only historical receipt read. The caller must authenticate the
    /// retained Root signature first; current presence is not restored.
    pub(crate) fn matches_receipt(&self, body: &BodyId, bytes: &[u8; 480]) -> bool {
        matches!(bytes[8], 1 | 2)
            && bytes[24..56] == Sha256::digest(body.as_str().as_bytes())[..]
            && self.offer()[..] == bytes[168..200]
            && bytes[200..232] == scope_digest(body, self.ids, self.issued_at())
            && bytes[264..328] == self.signature()[..]
            && bytes[328..360] == self.transcript(body)
    }
    fn offer(&self) -> &[u8; 32] {
        self.bytes[8..40].try_into().unwrap()
    }
    fn issued_at(&self) -> u64 {
        u64::from_le_bytes(self.bytes[40..48].try_into().unwrap())
    }
    fn signature(&self) -> &[u8; 64] {
        self.bytes[564..].try_into().unwrap()
    }
}
fn scope_digest(body: &BodyId, ids: [&str; 4], issued: u64) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"conduit.environment/network-retirement-member@1\0");
    for id in [body.as_str(), ids[0], ids[1], ids[2], ids[3]] {
        hash.update((id.len() as u64).to_le_bytes());
        hash.update(id.as_bytes());
    }
    hash.update(issued.to_le_bytes());
    hash.finalize().into()
}
fn acknowledgement(offer: &[u8; 32], scope: &[u8; 32]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"conduit.environment/network-retirement-custody@1\0");
    hash.update(offer);
    hash.update(scope);
    hash.finalize().into()
}
/// Created only after canonical latest-credential/private-key verification.
/// Its borrow keeps the actual physical snapshot pinned through Root signing.
pub(crate) struct PendingDraft<'a> {
    owner: &'a Draft,
    bytes: [u8; 480],
    membership_revision: u64,
}
impl PendingDraft<'_> {
    pub(crate) fn membership_revision(&self) -> u64 {
        self.membership_revision
    }
    pub(crate) fn root(&self) -> &[u8; 32] {
        self.owner.root()
    }
    pub(crate) fn transcript(&self) -> [u8; 32] {
        let mut hash = Sha256::new();
        hash.update(b"conduit.environment/network-retirement-receipt@1\0");
        hash.update(&self.bytes[..24]);
        hash.update(&self.bytes[24..408]);
        hash.finalize().into()
    }
}
impl<'a> PendingDraft<'a> {
    pub(crate) fn signed(mut self, signature: [u8; 64]) -> SignedPending<'a> {
        self.bytes[408..472].copy_from_slice(&signature);
        SignedPending {
            owner: self.owner,
            bytes: self.bytes,
            membership_revision: self.membership_revision,
        }
    }
}
/// Signature presentation only, still pinned to the actual owner.
/// The Root caller must authenticate these bytes before checkpoint installation.
/// No constructor, deserializer, checkpoint installation or erase entrance.
pub(crate) struct SignedPending<'a> {
    owner: &'a Draft,
    bytes: [u8; 480],
    membership_revision: u64,
}
impl SignedPending<'_> {
    pub(crate) fn bytes(&self) -> &[u8; 480] {
        &self.bytes
    }
    pub(crate) fn membership_revision(&self) -> u64 {
        self.membership_revision
    }
    pub(crate) fn offer(&self) -> [u8; 32] {
        self.owner.transcript()
    }
}
pub(crate) fn verify<'a>(
    owner: &'a Draft,
    manager: &AdmissionManager,
    membership: &BodyMembership,
    request: &Request<'_>,
) -> Result<PendingDraft<'a>, &'static str> {
    if request.offer() != &owner.transcript()
        || owner.payload[..32] != Sha256::digest(membership.body_id.as_str().as_bytes())[..]
    {
        return Err("network-ack-snapshot");
    }
    if u64::from_le_bytes(owner.payload[64..72].try_into().unwrap()) == u64::MAX {
        return Err("network-ack-epoch-overflow");
    }
    let part = membership
        .parts
        .iter()
        .find(|p| p.part_id.as_str() == request.ids[0])
        .ok_or("network-ack-member")?;
    let credential = manager
        .latest_completed_credential(&part.part_id)
        .ok_or("network-ack-credential")?;
    if credential.host_id.as_str() != request.ids[1]
        || credential.boot_id.as_str() != request.ids[2]
        || credential.credential_id.as_str() != request.ids[3]
        || credential.issued_at_millis != request.issued_at()
    {
        return Err("network-ack-incarnation");
    }
    let message = request.transcript(&membership.body_id);
    let verified = manager
        .verify_retained_custody_message(membership, credential, &message, request.signature())
        .map_err(|_| "network-ack-admitted-signature")?;
    let mut bytes = [0; 480];
    bytes[..8].copy_from_slice(b"NCURSOR1");
    bytes[8] = 1;
    bytes[16..24].copy_from_slice(&owner.payload[64..72]);
    let receipt = &mut bytes[24..472];
    receipt[..144].copy_from_slice(&owner.payload[..144]);
    receipt[144..176].copy_from_slice(&owner.transcript());
    receipt[176..208].copy_from_slice(&scope_digest(
        &credential.body_id,
        request.ids,
        credential.issued_at_millis,
    ));
    receipt[208..240].copy_from_slice(verified.verifying_key());
    receipt[240..304].copy_from_slice(request.signature());
    receipt[304..336].copy_from_slice(verified.transcript());
    receipt[336..368].copy_from_slice(&owner.payload[144..176]);
    Ok(PendingDraft {
        owner,
        bytes,
        membership_revision: verified.membership_revision().0,
    })
}
