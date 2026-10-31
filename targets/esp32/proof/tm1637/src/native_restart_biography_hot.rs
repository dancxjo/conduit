//! Shared source-prefix transaction, with no new Boot/member checkpoint.
//! The actual Store lease and parked token are held across host retention and
//! incremental flash steps. This module never constructs a physical permit.
use super::*;
use crate::native_body_checkpoint::{
    incremental::{IncrementalRewrite, Operation, SectorStorage},
    PayloadReader,
};
use conduit_monotonic_wait::flash_window::FlashPermit;
use payload::biography_hot::Recipe;

// Private implementations distinguish actual parked custody from a validated
// short-borrow live scope. Neither can be constructed by a management client.
pub(super) trait Scope {
    fn source(&self, store: &Store) -> Result<(), &'static str>;
    fn check(&self, store: &Store) -> Result<(), &'static str>;
    fn body(&self) -> &conduit_body::BodyId;
    fn root(&self) -> &[u8; 32];
    fn adopted(&mut self, generation: u64);
}
pub(super) struct Transaction<O = ParkedBody> {
    owner: Option<O>,
    offered: Export,
    recipe: Option<Recipe>,
    writer: Option<IncrementalRewrite>,
    poisoned: bool,
}
/// Root's management slot owns this affine value. Cancellation must explicitly
/// return its original parked owner; completing returns the updated witness.
#[must_use = "cancel or finish must return the actual parked owner"]
pub(crate) struct Custody {
    transaction: Transaction,
    lease: StoreLease,
}
fn check_source(store: &Store, owner: &ParkedBody) -> Result<(), &'static str> {
    if store.failed
        || store.ready
        || !store.parked
        || store.evidence.is_some()
        || store.admissions.is_some()
        || owner.proposal.is_none()
    {
        return Err("biography-hot-owner-state");
    }
    let previous = store.custody.as_ref().ok_or("biography-hot-source")?;
    if owner.generation
        != store
            .journal
            .generation()
            .ok_or("biography-hot-generation")?
        || previous.source.generation() != owner.generation
        || owner.binding != store.binding
        || previous.source.binding() != &owner.binding
        || previous.proof.is_none()
    {
        return Err("biography-hot-owner-source");
    }
    Ok(())
}
fn check(store: &Store, owner: &ParkedBody) -> Result<(), &'static str> {
    check_source(store, owner)?;
    if Sha256::digest(owner.body_id.as_str().as_bytes())[..] != owner.binding {
        return Err("biography-hot-owner-binding");
    }
    let root =
        ed25519_dalek::SigningKey::from_bytes(store.seed.as_ref().ok_or("biography-hot-key")?)
            .verifying_key();
    if root.as_bytes() != &owner.key {
        return Err("biography-hot-owner-root");
    }
    Ok(())
}
fn check_offer_source(store: &Store, offered: &Offer) -> Result<(), &'static str> {
    let source = &store.custody.as_ref().ok_or("biography-hot-source")?.source;
    if source.generation() != offered.generation || source.digest() != &offered.source.0 {
        return Err("biography-hot-offer-source");
    }
    Ok(())
}
fn emit(
    recipe: &Recipe,
    reader: &mut PayloadReader<'_, BodyRegion>,
    sink: &mut dyn FnMut(&[u8]) -> Result<(), Refusal>,
) -> Result<(), Refusal> {
    let mut refused = None;
    recipe
        .emit(
            |out| {
                reader.read(out).map_err(|e| {
                    refused = Some(e);
                })
            },
            sink,
        )
        .map(|_| ())
        .map_err(|e| refused.unwrap_or_else(|| stream_refusal(e)))
}
impl Scope for ParkedBody {
    fn source(&self, store: &Store) -> Result<(), &'static str> {
        check_source(store, self)
    }
    fn check(&self, store: &Store) -> Result<(), &'static str> {
        check(store, self)
    }
    fn body(&self) -> &conduit_body::BodyId {
        &self.body_id
    }
    fn root(&self) -> &[u8; 32] {
        &self.key
    }
    fn adopted(&mut self, generation: u64) {
        self.generation = generation;
    }
}
impl<O: Scope> Transaction<O> {
    #[allow(
        clippy::result_large_err,
        reason = "refusal returns the actual affine owner without allocating on the failure path"
    )]
    pub(super) fn prepare(store: &mut Store, owner: O) -> Result<Self, (O, &'static str)> {
        let offered = (|| {
            owner.check(store)?;
            prepare_store(store)
        })();
        match offered {
            Err(error) => Err((owner, error)),
            Ok(offered) => Ok(Self {
                owner: Some(owner),
                offered,
                recipe: None,
                writer: None,
                poisoned: false,
            }),
        }
    }
    pub(super) fn export_chunk(
        &self,
        store: &mut Store,
        offset: usize,
        out: &mut [u8; 128],
    ) -> Result<usize, &'static str> {
        out.fill(0);
        self.owner
            .as_ref()
            .ok_or("biography-hot-owner")?
            .source(store)?;
        check_offer_source(store, &self.offered.offer)?;
        if self.writer.is_some() || offset > payload::MAX_PLAIN_BYTES {
            return Err("biography-hot-export-state");
        }
        let end = offset
            .checked_add(out.len())
            .ok_or("biography-hot-export-offset")?;
        let mut count = 0usize;
        let streamed = stream(store, &mut |bytes| {
            let next = count.checked_add(bytes.len()).ok_or(Refusal::Capacity)?;
            let lo = count.max(offset);
            let hi = next.min(end);
            if lo < hi {
                out[lo - offset..hi - offset].copy_from_slice(&bytes[lo - count..hi - count]);
            }
            count = next;
            Ok(())
        });
        match streamed {
            Ok(digest) if digest == self.offered.offer.stream.0 && offset <= count => {
                Ok((count - offset).min(out.len()))
            }
            _ => {
                out.fill(0);
                Err("biography-hot-export-source")
            }
        }
    }
    pub(super) fn begin(
        &mut self,
        store: &mut Store,
        receipt: payload::wire::Receipt,
    ) -> Result<(), &'static str> {
        if self.writer.is_some() || self.recipe.is_some() || self.poisoned {
            return Err("biography-hot-already-started");
        }
        let owner = self.owner.as_ref().ok_or("biography-hot-owner")?;
        owner.check(store)?;
        check_offer_source(store, &self.offered.offer)?;
        let current = prepare_store(store)?;
        let key = ed25519_dalek::VerifyingKey::from_bytes(owner.root())
            .map_err(|_| "biography-hot-key")?;
        if current.offer.transcript(owner.root()) != self.offered.offer.transcript(owner.root())
            || receipt.offer.0 != current.offer.transcript(owner.root())
            || receipt.summary != current.offer.end
        {
            return Err("biography-hot-receipt-source");
        }
        receipt.verify(owner.body(), &key)?;
        let previous = store.custody.as_ref().ok_or("biography-hot-source")?;
        let recipe = Recipe::prepare(
            &previous.source,
            owner.body(),
            owner.root(),
            previous.span,
            previous.prefix.is_some(),
            receipt,
            &previous.layouts,
            previous.network.as_ref(),
        )?;
        let writer = IncrementalRewrite::prepare(
            &mut store.journal,
            &previous.source,
            &store.binding,
            store.seed.as_ref().ok_or("biography-hot-key")?,
            |reader, sink| emit(&recipe, reader, sink),
        )
        .map_err(checkpoint_commit_refusal)?;
        self.recipe = Some(recipe);
        self.writer = Some(writer);
        Ok(())
    }
    pub(super) fn prepare_chunk(&mut self, store: &mut Store) -> Result<(), &'static str> {
        if self.poisoned {
            return Err("biography-hot-failed");
        }
        self.owner
            .as_ref()
            .ok_or("biography-hot-owner")?
            .check(store)?;
        let recipe = self.recipe.as_ref().ok_or("biography-hot-not-started")?;
        let result = self
            .writer
            .as_mut()
            .ok_or("biography-hot-not-started")?
            .prepare_chunk(&mut store.journal, |reader, sink| {
                emit(recipe, reader, sink)
            })
            .map_err(checkpoint_commit_refusal);
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }
    pub(super) fn advance(
        &mut self,
        store: &mut Store,
        access: &mut impl SectorStorage<BodyRegion>,
    ) -> Result<bool, &'static str> {
        if self.poisoned {
            return Err("biography-hot-failed");
        }
        // No signature, key derivation, codec or allocation runs inside the
        // caller's physical permit. The exclusive lease retains those pins.
        self.owner
            .as_ref()
            .ok_or("biography-hot-owner")?
            .source(store)?;
        let result = self
            .writer
            .as_mut()
            .ok_or("biography-hot-not-started")?
            .advance(&mut store.journal, access)
            .map_err(checkpoint_commit_refusal);
        if result.is_err() {
            self.poisoned = true;
        }
        result
    }
    pub(super) fn cancel(&mut self, store: &mut Store) -> Result<O, &'static str> {
        self.owner
            .as_ref()
            .ok_or("biography-hot-owner")?
            .check(store)?;
        // Only the inactive bank was ever writable. Even an ambiguous commit
        // failure retains the old affine source in this running journal. A
        // later Boot may select the complete endorsed successor, never a
        // partial payload; neither case loses canonical or receiver custody.
        let source = &store.custody.as_ref().ok_or("biography-hot-source")?.source;
        store
            .journal
            .read_snapshot(source, |reader| {
                let mut bytes = [0; 128];
                while reader.read(&mut bytes)? != 0 {}
                bytes.fill(0);
                Ok(())
            })
            .map_err(|_| "biography-hot-cancel-source")?;
        self.writer = None;
        self.recipe = None;
        self.owner.take().ok_or("biography-hot-owner")
    }
    pub(super) fn finish(&mut self, store: &mut Store) -> Result<O, &'static str> {
        if self.poisoned || !self.writer.as_ref().is_some_and(|w| w.is_complete()) {
            return Err("biography-hot-incomplete");
        }
        self.owner
            .as_ref()
            .ok_or("biography-hot-owner")?
            .check(store)?;
        check_offer_source(store, &self.offered.offer)?;
        let previous = store.custody.as_ref().ok_or("biography-hot-source")?;
        let recipe = self.recipe.as_mut().ok_or("biography-hot-not-started")?;
        // Authenticate the entire original again before binding the physically
        // read-back successor. No resident DTO or new signature pass is needed.
        store
            .journal
            .read_snapshot(&previous.source, |reader| {
                emit(recipe, reader, &mut |_| Ok(()))
            })
            .map_err(|_| "biography-hot-finish-source")?;
        let completed = self
            .writer
            .take()
            .ok_or("biography-hot-not-started")?
            .finish(&mut store.journal)
            .map_err(checkpoint_commit_refusal)?;
        let (source, proof, span) = recipe.committed(completed)?;
        let generation = source.generation();
        let prefix = self
            .recipe
            .take()
            .ok_or("biography-hot-not-started")?
            .into_receipt();
        let current = store.custody.as_mut().ok_or("biography-hot-source")?;
        current.source = source;
        current.proof = Some(proof);
        current.span = span;
        current.prefix = Some(prefix);
        current.local_count = 0;
        let mut owner = self.owner.take().ok_or("biography-hot-owner")?;
        owner.adopted(generation);
        // Body evidence, proposal, current Boot, admissions, network cursor,
        // layout receipts, summary and the parked/ready flags are unchanged.
        Ok(owner)
    }
}
impl<O> Transaction<O> {
    pub(super) fn offer(&self) -> &Export {
        &self.offered
    }
    pub(super) fn scope(&self) -> Result<&O, &'static str> {
        self.owner.as_ref().ok_or("biography-hot-owner")
    }
    pub(super) fn planned_bytes(&self) -> Option<usize> {
        self.recipe.as_ref().and_then(|r| r.planned_bytes())
    }
    pub(super) fn operation(&self) -> Option<Operation> {
        if self.poisoned {
            None
        } else {
            self.writer.as_ref().and_then(|w| w.operation())
        }
    }
    pub(super) fn step(&self) -> u32 {
        self.writer.as_ref().map_or(0, |w| w.step())
    }
    pub(super) fn needs_chunk(&self) -> bool {
        !self.poisoned && self.writer.as_ref().is_some_and(|w| w.needs_chunk())
    }
    pub(super) fn is_complete(&self) -> bool {
        !self.poisoned && self.writer.as_ref().is_some_and(|w| w.is_complete())
    }
}
impl Custody {
    #[allow(
        clippy::result_large_err,
        reason = "a failed exact job allocation must return the actual owner without another allocation"
    )]
    pub(crate) fn prepare(
        owner: ParkedBody,
        _quiet: &FlashPermit<'_, '_>,
    ) -> Result<Box<Self>, (ParkedBody, &'static str)> {
        let raw = unsafe { alloc::alloc::alloc(core::alloc::Layout::new::<Self>()) }
            .cast::<core::mem::MaybeUninit<Self>>();
        if raw.is_null() {
            return Err((owner, "biography-hot-job-capacity"));
        }
        // SAFETY: exact nonzero allocation; only Box::write initializes it.
        let backing = unsafe { Box::from_raw(raw) };
        let mut lease = match take_store() {
            Ok(lease) => lease,
            Err(reason) => return Err((owner, reason)),
        };
        let store = match lease.0.as_mut() {
            Some(store) => store,
            None => return Err((owner, "restart-not-ready")),
        };
        let transaction = Transaction::prepare(store, owner)?;
        Ok(Box::write(backing, Self { transaction, lease }))
    }
    pub(crate) fn offer(&self) -> &payload::wire::Offer {
        &self.transaction.offered.offer
    }
    /// Same opaque authenticated export accepted by Root's existing typed
    /// original-owner challenge/endorsement entrance; not a second authority.
    pub(crate) fn export_witness(&self) -> &Export {
        &self.transaction.offered
    }
    /// Return only an authenticated bounded chunk. Root must release the quiet
    /// window and asynchronously flush/yield before requesting another chunk.
    pub(crate) fn export_chunk(
        &mut self,
        offset: usize,
        out: &mut [u8; 128],
        _quiet: &FlashPermit<'_, '_>,
    ) -> Result<usize, &'static str> {
        self.transaction.export_chunk(
            self.lease.0.as_mut().ok_or("restart-not-ready")?,
            offset,
            out,
        )
    }
    pub(crate) fn begin(
        &mut self,
        receipt: payload::wire::Receipt,
        _quiet: &FlashPermit<'_, '_>,
    ) -> Result<(), &'static str> {
        self.transaction
            .begin(self.lease.0.as_mut().ok_or("restart-not-ready")?, receipt)
    }
    pub(crate) fn planned_bytes(&self) -> Option<usize> {
        self.transaction
            .recipe
            .as_ref()
            .and_then(|r| r.planned_bytes())
    }
    pub(crate) fn operation(&self) -> Option<Operation> {
        if self.transaction.poisoned {
            None
        } else {
            self.transaction.writer.as_ref().and_then(|w| w.operation())
        }
    }
    pub(crate) fn step(&self) -> u32 {
        self.transaction.writer.as_ref().map_or(0, |w| w.step())
    }
    pub(crate) fn needs_chunk(&self) -> bool {
        !self.transaction.poisoned
            && self
                .transaction
                .writer
                .as_ref()
                .is_some_and(|w| w.needs_chunk())
    }
    pub(crate) fn is_complete(&self) -> bool {
        !self.transaction.poisoned
            && self
                .transaction
                .writer
                .as_ref()
                .is_some_and(|w| w.is_complete())
    }
    pub(crate) fn prepare_chunk(
        &mut self,
        _quiet: &FlashPermit<'_, '_>,
    ) -> Result<(), &'static str> {
        self.transaction
            .prepare_chunk(self.lease.0.as_mut().ok_or("restart-not-ready")?)
    }
    pub(crate) fn advance(
        &mut self,
        access: &mut impl SectorStorage<BodyRegion>,
    ) -> Result<bool, &'static str> {
        self.transaction
            .advance(self.lease.0.as_mut().ok_or("restart-not-ready")?, access)
    }
    pub(crate) fn cancel(
        mut self: Box<Self>,
        _quiet: &FlashPermit<'_, '_>,
    ) -> Result<ParkedBody, (Box<Self>, &'static str)> {
        let result = self
            .lease
            .0
            .as_mut()
            .ok_or("restart-not-ready")
            .and_then(|store| self.transaction.cancel(store));
        result.map_err(|error| (self, error))
    }
    pub(crate) fn finish(
        mut self: Box<Self>,
        _quiet: &FlashPermit<'_, '_>,
    ) -> Result<ParkedBody, (Box<Self>, &'static str)> {
        let result = self
            .lease
            .0
            .as_mut()
            .ok_or("restart-not-ready")
            .and_then(|store| self.transaction.finish(store));
        result.map_err(|error| (self, error))
    }
}

#[cfg(test)]
#[path = "native_restart_biography_hot_tests.rs"]
mod tests;
