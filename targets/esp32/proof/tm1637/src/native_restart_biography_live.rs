//! Compaction of the committed prefix while Root retains the actual live Body.
//! Root fences renewal and Body mutations for this job; admitted sampler/Mask
//! kernels may continue. No lull, new Boot, archive ACK, or owner clone occurs.
use super::hot::{Scope, Transaction};
use super::*;
use crate::native_body_checkpoint::incremental::{Operation, SectorStorage};
use conduit_body::BodyPlanEvidence;
use conduit_monotonic_wait::flash_window::FlashPermit;

struct LiveScope {
    body: conduit_body::BodyId,
    root: [u8; 32],
    host: conduit_core::HostId,
    boot: conduit_core::BootId,
    generation: u64,
    binding: [u8; 32],
    owner_state: OwnerMark,
}
#[derive(PartialEq, Eq)]
struct OwnerMark {
    sequence: u64,
    last_sign: Option<conduit_core::SignId>,
    membership_revision: conduit_body::BodyMembershipRevision,
    pending_count: usize,
    pending_head: Option<[u8; 32]>,
    state: conduit_body::BodyState,
    foreground: Option<conduit_body::ResidentPlot>,
    realization: Option<(
        conduit_body::WakeId,
        conduit_core::PlanId,
        Option<conduit_core::ActivePlayId>,
    )>,
}
impl OwnerMark {
    fn capture(owner: &SealedBodyLifecycleSession) -> Self {
        Self {
            sequence: owner.evidence().last_sequence(),
            last_sign: owner.evidence().records.last().map(|r| r.sign_id.clone()),
            membership_revision: owner.evidence().membership.revision,
            pending_count: owner.pending_archives().len(),
            pending_head: owner.pending_archives().last().map(|s| s.digest),
            state: owner.evidence().body.state.clone(),
            foreground: owner.foreground().cloned(),
            realization: owner.realization().map(|r| {
                (
                    r.wake.wake_id.clone(),
                    r.plan.plan_id().clone(),
                    r.play.as_ref().map(|p| p.active_play_id.clone()),
                )
            }),
        }
    }
    fn matches(&self, owner: &SealedBodyLifecycleSession) -> bool {
        self.sequence == owner.evidence().last_sequence()
            && self.last_sign.as_ref() == owner.evidence().records.last().map(|r| &r.sign_id)
            && self.membership_revision == owner.evidence().membership.revision
            && self.pending_count == owner.pending_archives().len()
            && self.pending_head == owner.pending_archives().last().map(|s| s.digest)
            && self.state == owner.evidence().body.state
            && self.foreground.as_ref() == owner.foreground()
            && self
                .realization
                .as_ref()
                .map(|(wake, plan, play)| (wake, plan, play.as_ref()))
                == owner.realization().map(|r| {
                    (
                        &r.wake.wake_id,
                        r.plan.plan_id(),
                        r.play.as_ref().map(|p| &p.active_play_id),
                    )
                })
    }
}
impl LiveScope {
    fn prepare(
        store: &Store,
        owner: &SealedBodyLifecycleSession,
        root: &[u8; 32],
        host: &conduit_core::HostId,
        boot: &conduit_core::BootId,
    ) -> Result<Self, &'static str> {
        let scope = Self {
            body: owner.evidence().body_id.clone(),
            root: *root,
            host: host.clone(),
            boot: boot.clone(),
            generation: store
                .journal
                .generation()
                .ok_or("biography-live-generation")?,
            binding: store.binding,
            owner_state: OwnerMark::capture(owner),
        };
        scope.source(store)?;
        if store.custody.as_ref().and_then(|c| c.summary.body.as_ref()) != Some(&scope.body) {
            return Err("biography-live-body-binding");
        }
        scope.owner(owner)?;
        // Bind the live pending suffix to this exact committed chain without
        // replay, serialization or signature work in Root's memory-only phase.
        let prior = store.custody.as_ref().ok_or("biography-live-source")?;
        let pending = owner.pending_archives();
        let count = prior
            .summary
            .count
            .checked_add(pending.len() as u64)
            .ok_or("biography-live-tail")?;
        let head = pending.last().map(|s| s.digest).or(prior.summary.head);
        let compact = owner.evidence().compaction.as_ref();
        if compact.map_or(0, |c| c.sealed_segments) != count
            || compact.and_then(|c| c.archive_head_digest) != head
            || pending.first().is_some_and(|s| {
                s.previous_digest != prior.summary.head
                    || prior.summary.count.checked_add(1) != Some(s.ordinal)
                    || s.body_id != scope.body
            })
        {
            return Err("biography-live-tail");
        }
        Ok(scope)
    }
    fn owner(&self, owner: &SealedBodyLifecycleSession) -> Result<(), &'static str> {
        let evidence = owner.evidence();
        if evidence.body_id != self.body
            || evidence.body.body_id != self.body
            || evidence.membership.body_id != self.body
            || !evidence.membership.parts.iter().any(|part| {
                part.state == conduit_body::MembershipState::Admitted
                    && part
                        .current
                        .as_ref()
                        .is_some_and(|c| c.host_id == self.host && c.boot_id == self.boot)
            })
            || !self.owner_state.matches(owner)
        {
            return Err("biography-live-owner-scope");
        }
        Ok(())
    }
}
impl Scope for LiveScope {
    fn source(&self, store: &Store) -> Result<(), &'static str> {
        let prior = store.custody.as_ref().ok_or("biography-live-source")?;
        if store.failed
            || !store.ready
            || store.parked
            || store.admissions.is_none()
            || self.binding != store.binding
            || prior.source.binding() != &self.binding
            || store.journal.generation() != Some(self.generation)
            || prior.source.generation() != self.generation
            || prior.proof.is_none()
        {
            return Err("biography-live-source-state");
        }
        Ok(())
    }
    fn check(&self, store: &Store) -> Result<(), &'static str> {
        self.source(store)?;
        let key =
            ed25519_dalek::SigningKey::from_bytes(store.seed.as_ref().ok_or("biography-live-key")?)
                .verifying_key();
        if key.as_bytes() != &self.root
            || <[u8; 32]>::from(Sha256::digest(self.body.as_str().as_bytes())) != self.binding
        {
            return Err("biography-live-root-binding");
        }
        Ok(())
    }
    fn body(&self) -> &conduit_body::BodyId {
        &self.body
    }
    fn root(&self) -> &[u8; 32] {
        &self.root
    }
    fn adopted(&mut self, generation: u64) {
        self.generation = generation;
    }
}

// Reserved -> Prepared -> SourceFinished/Cancelled -> Root-adopted. None of
// these phases owns, moves, mutates, or reconstructs Root's actual live Body.
#[must_use = "finish/cancel source under WaitLane, then adopt on Root"]
pub(crate) struct LiveCustody {
    scope: Option<LiveScope>,
    transaction: Option<Transaction<LiveScope>>,
    lease: StoreLease,
    released: bool,
}
impl LiveCustody {
    /// Memory only: no flash, codec replay, or key derivation. This affine job
    /// may cross to the admitted WaitLane without carrying a Body borrow.
    pub(crate) fn reserve(
        owner: &SealedBodyLifecycleSession,
        root: &[u8; 32],
        host: &conduit_core::HostId,
        boot: &conduit_core::BootId,
    ) -> Result<Box<Self>, &'static str> {
        let raw = unsafe { alloc::alloc::alloc(core::alloc::Layout::new::<Self>()) }
            .cast::<core::mem::MaybeUninit<Self>>();
        if raw.is_null() {
            return Err("biography-live-job-capacity");
        }
        // SAFETY: unique exact allocation, initialized only by Box::write.
        let backing = unsafe { Box::from_raw(raw) };
        let lease = take_store()?;
        let store = lease.0.as_ref().ok_or("restart-not-ready")?;
        let scope = LiveScope::prepare(store, owner, root, host, boot)?;
        Ok(Box::write(
            backing,
            Self {
                scope: Some(scope),
                transaction: None,
                lease,
                released: false,
            },
        ))
    }
    pub(crate) fn prepare_source(
        &mut self,
        _quiet: &FlashPermit<'_, '_>,
    ) -> Result<(), &'static str> {
        if self.transaction.is_some() || self.released {
            return Err("biography-live-phase");
        }
        let scope = self.scope.take().ok_or("biography-live-phase")?;
        match Transaction::prepare(self.lease.0.as_mut().ok_or("restart-not-ready")?, scope) {
            Ok(transaction) => {
                self.transaction = Some(transaction);
                Ok(())
            }
            Err((scope, e)) => {
                self.scope = Some(scope);
                Err(e)
            }
        }
    }
    fn transaction(&self) -> Result<&Transaction<LiveScope>, &'static str> {
        self.transaction
            .as_ref()
            .filter(|_| !self.released)
            .ok_or("biography-live-phase")
    }
    pub(crate) fn offer(&self) -> Result<&Offer, &'static str> {
        Ok(self.transaction()?.offer().offer())
    }
    pub(crate) fn export_witness(&self) -> Result<&Export, &'static str> {
        Ok(self.transaction()?.offer())
    }
    pub(crate) fn export_chunk(
        &mut self,
        offset: usize,
        out: &mut [u8; 128],
        _quiet: &FlashPermit<'_, '_>,
    ) -> Result<usize, &'static str> {
        self.transaction()?;
        self.transaction.as_mut().unwrap().export_chunk(
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
        self.transaction()?;
        self.transaction
            .as_mut()
            .unwrap()
            .begin(self.lease.0.as_mut().ok_or("restart-not-ready")?, receipt)
    }
    pub(crate) fn planned_bytes(&self) -> Option<usize> {
        self.transaction().ok().and_then(|t| t.planned_bytes())
    }
    pub(crate) fn operation(&self) -> Option<Operation> {
        self.transaction().ok().and_then(|t| t.operation())
    }
    pub(crate) fn step(&self) -> u32 {
        self.transaction().map_or(0, |t| t.step())
    }
    pub(crate) fn needs_chunk(&self) -> bool {
        self.transaction().is_ok_and(|t| t.needs_chunk())
    }
    pub(crate) fn is_complete(&self) -> bool {
        self.transaction().is_ok_and(|t| t.is_complete())
    }
    pub(crate) fn prepare_chunk(
        &mut self,
        _quiet: &FlashPermit<'_, '_>,
    ) -> Result<(), &'static str> {
        self.transaction()?;
        self.transaction
            .as_mut()
            .unwrap()
            .prepare_chunk(self.lease.0.as_mut().ok_or("restart-not-ready")?)
    }
    pub(crate) fn advance(
        &mut self,
        access: &mut impl SectorStorage<BodyRegion>,
    ) -> Result<bool, &'static str> {
        self.transaction()?;
        self.transaction
            .as_mut()
            .unwrap()
            .advance(self.lease.0.as_mut().ok_or("restart-not-ready")?, access)
    }
    /// Authenticate/readback and adopt only the on-flash prefix in WaitLane.
    /// The Store lease remains owned until Root checks its actual Body again.
    pub(crate) fn finish_source(
        &mut self,
        _quiet: &FlashPermit<'_, '_>,
    ) -> Result<(), &'static str> {
        self.transaction()?;
        let scope = self
            .transaction
            .as_mut()
            .unwrap()
            .finish(self.lease.0.as_mut().ok_or("restart-not-ready")?)?;
        self.scope = Some(scope);
        self.released = true;
        Ok(())
    }
    pub(crate) fn cancel_source(
        &mut self,
        _quiet: &FlashPermit<'_, '_>,
    ) -> Result<(), &'static str> {
        if self.released {
            return Err("biography-live-phase");
        }
        if let Some(transaction) = self.transaction.as_mut() {
            self.scope =
                Some(transaction.cancel(self.lease.0.as_mut().ok_or("restart-not-ready")?)?);
        }
        // A never-prepared job made no flash access or effect.
        self.released = true;
        Ok(())
    }
    pub(crate) fn validate_owner(
        &self,
        owner: &SealedBodyLifecycleSession,
    ) -> Result<(), &'static str> {
        let scope = match self.scope.as_ref() {
            Some(scope) => scope,
            None => self.transaction()?.scope()?,
        };
        scope.source(self.lease.0.as_ref().ok_or("restart-not-ready")?)?;
        scope.owner(owner)
    }
    /// Root-only memory check. Failure returns the entire job/lease for retry;
    /// no reconstructed Body or buffered records are substituted on any path.
    pub(crate) fn adopt(
        self: Box<Self>,
        owner: &SealedBodyLifecycleSession,
    ) -> Result<(), (Box<Self>, &'static str)> {
        let result = if self.released {
            self.validate_owner(owner)
        } else {
            Err("biography-live-phase")
        };
        result.map_err(|e| (self, e))
    }
}
#[cfg(all(test, feature = "observation-store"))]
#[path = "native_restart_biography_live_tests.rs"]
mod tests;
