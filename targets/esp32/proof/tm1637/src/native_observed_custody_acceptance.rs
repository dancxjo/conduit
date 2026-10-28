//! Typed immutable-epoch custody. The native archive retains this module
//! as a child so only its actual frozen journal can construct an offer draft.
//! Neither a network offer nor a decoded ACK constructs admitted/erase custody.
#[path = "native_observed_custody_wire.rs"]
pub(crate) mod wire;
use super::{custody_journal::WholeEpoch, state::Active};
use conduit_body::{AdmissionManager, BodyId, BodyMembership};
use conduit_environment_runtime::{
    display_evidence_journal::Storage,
    observed_record::{self, ObservedRecord},
};
use ed25519_dalek::{Signature, VerifyingKey};
use sha2::{Digest, Sha256};
use wire::{Hex, Offer, Request};
pub(crate) struct ClosedOfferDraft {
    offer: Offer<'static>,
    root: [u8; 32],
}
impl ClosedOfferDraft {
    pub(crate) fn transcript(&self) -> [u8; 32] {
        wire::offer_transcript(&self.offer, &self.root)
    }
    pub(crate) fn root(&self) -> &[u8; 32] {
        &self.root
    }
    /// Match a Root endorsement to this inspected immutable owner. This checks
    /// identity only; it neither transfers the owner nor authorizes an erase.
    pub(crate) fn matches_accepted(
        &self,
        bytes: &[u8; 416],
        body: &BodyId,
        key: &VerifyingKey,
    ) -> bool {
        self.root == key.to_bytes()
            && wire::verify_accepted(bytes, body, key)
            && bytes[..32] == self.offer.region.0
            && bytes[32..128] == self.offer.scope.0
            && bytes[128..160] == self.offer.forms.0
            && bytes[160..164] == self.offer.committed.to_le_bytes()
            && bytes[164..196] == self.offer.digest.0
    }
    pub(crate) fn signed(self, signature: [u8; 64]) -> Offer<'static> {
        Offer {
            signature: Hex(signature),
            ..self.offer
        }
    }
}
/// `failed` may hide a physically committed record behind stale live counters.
/// Such media must be reopened read-only before any offer can acknowledge it.
pub(crate) fn inspect<S: Storage + WholeEpoch>(
    active: &mut Active<S>,
    body: &BodyId,
    key: &VerifyingKey,
    work: &mut [u8],
) -> Result<ClosedOfferDraft, &'static str> {
    let mut inspector = Inspector::start(active, body, key)?;
    while !inspector.step(work)? {}
    inspector.finish()
}
/// Exclusive borrow pins the actual frozen owner for the entire inspection;
/// neither decoded offers nor caller-selected cursors can construct this state.
/// A failed step is terminal: replay may already have advanced before refusal.
pub(crate) struct Inspector<'a, S> {
    active: &'a mut Active<S>,
    progress: Progress,
}
struct Progress {
    region: [u8; 32],
    scope: [u8; 96],
    forms: [u8; 32],
    count: u32,
    maximum: u32,
    key: VerifyingKey,
    cursor: conduit_environment_runtime::display_evidence_journal::Replay,
    hash: Sha256,
    sequence: u64,
    clock: u64,
    complete: bool,
    failed: bool,
}
impl<'a, S: Storage + WholeEpoch> Inspector<'a, S> {
    pub(crate) fn start(
        active: &'a mut Active<S>,
        body: &BodyId,
        key: &VerifyingKey,
    ) -> Result<Self, &'static str> {
        if !active.recovered || !active.signed || active.failed {
            return Err("custody journal not frozen");
        }
        let count = active.journal.committed_count();
        if count == 0
            || count > 960
            || active.last_sequence != u64::from(count)
            || active.scope[..32] != Sha256::digest(body.as_str().as_bytes())[..]
        {
            return Err("custody journal scope");
        }
        let progress = Progress::new(active, key);
        Ok(Self { active, progress })
    }
    pub(crate) fn step(&mut self, work: &mut [u8]) -> Result<bool, &'static str> {
        self.progress.step(self.active, work)
    }
    pub(crate) fn finish(self) -> Result<ClosedOfferDraft, &'static str> {
        self.progress.finish()
    }
}
impl Progress {
    fn new<S: Storage + WholeEpoch>(active: &Active<S>, key: &VerifyingKey) -> Self {
        Self {
            region: active.journal.inspect_storage(WholeEpoch::identity),
            scope: active.scope,
            forms: active.forms,
            count: active.journal.committed_count(),
            maximum: active.maximum_records,
            key: *key,
            cursor: active.journal.replay_cursor(),
            hash: Sha256::new(),
            sequence: 0,
            clock: 0,
            complete: false,
            failed: false,
        }
    }
    /// At most one signed record is read/verified. End-of-prefix checking is a
    /// separate zero-signature step. This is no scheduler or erase admission.
    fn step<S: Storage + WholeEpoch>(
        &mut self,
        active: &mut Active<S>,
        work: &mut [u8],
    ) -> Result<bool, &'static str> {
        if self.failed {
            return Err("custody inspection failed");
        }
        if self.complete {
            return Ok(true);
        }
        let result = self.step_inner(active, work);
        if result.is_err() {
            self.failed = true;
        }
        result
    }
    fn step_inner<S: Storage + WholeEpoch>(
        &mut self,
        active: &mut Active<S>,
        work: &mut [u8],
    ) -> Result<bool, &'static str> {
        if !active.recovered
            || !active.signed
            || active.failed
            || active.scope != self.scope
            || active.forms != self.forms
            || active.last_sequence != u64::from(self.count)
            || active.journal.committed_count() != self.count
            || active.maximum_records != self.maximum
            || active.journal.inspect_storage(WholeEpoch::identity) != self.region
        {
            return Err("custody inspection changed");
        }
        let mut signed = [0; observed_record::BYTES + 64];
        let Some(length) = active
            .journal
            .replay(&mut self.cursor, &mut signed, work)
            .map_err(|_| "custody journal replay")?
        else {
            if self.sequence != u64::from(self.count) {
                return Err("custody journal count");
            }
            self.complete = true;
            return Ok(true);
        };
        if length != signed.len() {
            return Err("custody signed record extent");
        }
        let record = ObservedRecord::decode(&signed[..observed_record::BYTES])
            .map_err(|_| "custody record")?;
        if record.sequence != self.sequence + 1 || record.observed_at_boot_ms < self.clock {
            return Err("custody record continuity");
        }
        super::history::verify(
            &self.key,
            &self.scope,
            &self.forms,
            &signed[..observed_record::BYTES],
            signed[observed_record::BYTES..].try_into().unwrap(),
        )
        .map_err(|_| "custody record signature")?;
        self.hash.update(&signed[..observed_record::BYTES]);
        self.sequence = record.sequence;
        self.clock = record.observed_at_boot_ms;
        Ok(false)
    }
    pub(crate) fn finish(self) -> Result<ClosedOfferDraft, &'static str> {
        if self.failed || !self.complete {
            return Err("custody inspection incomplete");
        }
        Ok(ClosedOfferDraft {
            offer: Offer {
                schema: wire::OFFER,
                region: Hex(self.region),
                scope: Hex(self.scope),
                forms: Hex(self.forms),
                committed: self.count,
                digest: Hex(self.hash.finalize().into()),
                signature: Hex([0; 64]),
            },
            root: self.key.to_bytes(),
        })
    }
}
/// Keeps the actual frozen journal across bounded Root service turns. No
/// cursor, scope, or journal can be replaced by a transport request mid-flight.
pub(crate) struct OwnedInspector<S> {
    active: Active<S>,
    progress: Progress,
}
impl<S: Storage + WholeEpoch> OwnedInspector<S> {
    #[allow(clippy::result_large_err)]
    pub(crate) fn start(
        mut active: Active<S>,
        body: &BodyId,
        key: &VerifyingKey,
    ) -> Result<Self, (Active<S>, &'static str)> {
        let progress = match Inspector::start(&mut active, body, key) {
            Ok(inspector) => inspector.progress,
            Err(error) => return Err((active, error)),
        };
        Ok(Self { active, progress })
    }
    pub(crate) fn step(&mut self, work: &mut [u8]) -> Result<bool, &'static str> {
        self.progress.step(&mut self.active, work)
    }
    pub(crate) fn finish(self) -> (Active<S>, Result<ClosedOfferDraft, &'static str>) {
        (self.active, self.progress.finish())
    }
}
/// Created only from the actual immutable journal and canonical opaque retained
/// admitted-custody verification. Root's signer must require its exact key.
pub(crate) struct AcceptedDraft {
    bytes: [u8; 416],
    root: [u8; 32],
}
impl AcceptedDraft {
    pub(crate) fn transcript(&self) -> [u8; 32] {
        wire::accepted_transcript(&self.bytes)
    }
    pub(crate) fn root(&self) -> &[u8; 32] {
        &self.root
    }
    pub(crate) fn signed(mut self, signature: [u8; 64]) -> [u8; 416] {
        self.bytes[352..].copy_from_slice(&signature);
        self.bytes
    }
}
pub(crate) fn verify_ack(
    draft: &ClosedOfferDraft,
    manager: &AdmissionManager,
    membership: &BodyMembership,
    request: &Request<'_>,
) -> Result<AcceptedDraft, &'static str> {
    request.validate()?;
    if request.offer.0 != draft.transcript()
        || draft.offer.scope.0[..32] != Sha256::digest(membership.body_id.as_str().as_bytes())[..]
    {
        return Err("custody acknowledgement scope");
    }
    let part = membership
        .parts
        .iter()
        .find(|part| part.part_id.as_str() == request.part)
        .ok_or("custody member")?;
    // Durable files survive a producer restart that canonically detaches
    // presence. Receipt data alone is not authority: the distinct canonical
    // verifier below checks latest credential, admitted state and private key.
    let credential = manager
        .latest_completed_credential(&part.part_id)
        .ok_or("custody retained credential")?;
    if credential.host_id.as_str() != request.host || credential.boot_id.as_str() != request.boot {
        return Err("custody retained incarnation");
    }
    let transcript = request.transcript();
    let verified = manager
        .verify_retained_custody_message(membership, credential, &transcript, &request.signature.0)
        .map_err(|_| "custody admitted signature")?;
    let mut bytes = [0; 416];
    bytes[..32].copy_from_slice(&draft.offer.region.0);
    bytes[32..128].copy_from_slice(&draft.offer.scope.0);
    bytes[128..160].copy_from_slice(&draft.offer.forms.0);
    bytes[160..164].copy_from_slice(&draft.offer.committed.to_le_bytes());
    bytes[164..196].copy_from_slice(&draft.offer.digest.0);
    bytes[196..228].copy_from_slice(verified.transcript());
    bytes[228..292].copy_from_slice(&request.signature.0);
    let mut member = Sha256::new();
    for id in [
        verified.part_id().as_str(),
        verified.host_id().as_str(),
        verified.boot_id().as_str(),
    ] {
        member.update((id.len() as u64).to_le_bytes());
        member.update(id.as_bytes());
    }
    member.update(verified.membership_revision().0.to_le_bytes());
    bytes[292..324].copy_from_slice(&member.finalize());
    Ok(AcceptedDraft {
        bytes,
        root: draft.root,
    })
}
pub(crate) use wire::verify_accepted;
