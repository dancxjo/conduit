//! Affine live-owner checkpoint. Scheduling and preparation budgets belong to
//! the caller; no phase grants flash authority or claims a replay time bound.
use super::*;
use crate::native_body_checkpoint::{
    incremental::{IncrementalRewrite, SectorStorage},
    PayloadReader,
};
use conduit_monotonic_wait::flash_window::FlashPermit;

#[path = "native_restart_checkpoint_phase.rs"]
mod phase;
pub(crate) use phase::Phase;
#[must_use = "retain the actual mutated owner and Store lease until adoption or reconciliation"]
pub(crate) struct Job<T> {
    pending_output: Option<T>,
    owner: Option<Box<SealedBodyLifecycleSession>>,
    lease: StoreLease,
    root: [u8; 32],
    writer: Option<IncrementalRewrite>,
    measurement: Option<payload::rewrite::FrozenMeasurement>,
    frozen: Option<payload::rewrite::FrozenRewrite>,
    metadata: Option<(payload::archive::Summary, payload::archive::Span)>,
    phase: Phase,
    step: u32,
    staged: bool,
}
impl<T> Job<T> {
    /// Memory-only acquisition. Failure returns the identical Body allocation.
    pub(crate) fn reserve(
        owner: Box<SealedBodyLifecycleSession>,
        root: [u8; 32],
    ) -> Result<Box<Self>, (Box<SealedBodyLifecycleSession>, &'static str)> {
        let layout = core::alloc::Layout::new::<Self>();
        let raw = unsafe { alloc::alloc::alloc(layout) }.cast::<core::mem::MaybeUninit<Self>>();
        if raw.is_null() {
            return Err((owner, "checkpoint-job-capacity"));
        }
        let backing = unsafe { Box::from_raw(raw) };
        let lease = match take_store() {
            Ok(v) => v,
            Err(e) => return Err((owner, e)),
        };
        let store = lease.0.as_ref().expect("actual Store lease");
        if store.failed
            || store.parked
            || !store.ready
            || store
                .custody
                .as_ref()
                .and_then(|c| c.proof.as_ref())
                .is_none()
        {
            return Err((owner, "checkpoint-job-source"));
        }
        if Sha256::digest(owner.evidence().body_id.as_str().as_bytes())[..] != store.binding {
            return Err((owner, "checkpoint-job-binding"));
        }
        Ok(Box::write(
            backing,
            Self {
                pending_output: None,
                owner: Some(owner),
                lease,
                root,
                writer: None,
                measurement: None,
                frozen: None,
                metadata: None,
                phase: Phase::Staging,
                step: 0,
                staged: false,
            },
        ))
    }
    pub(crate) fn phase(&self) -> Phase {
        self.phase
    }
    #[cfg(test)]
    pub(crate) fn capture_prefix_bytes(&self) -> Option<usize> {
        self.measurement.as_ref().map(|m| m.prefix_bytes())
    }
    pub(crate) fn cached_bytes(&self) -> usize {
        self.frozen.as_ref().map_or(0, |f| f.retained_bytes())
    }
    pub(crate) fn step(&self) -> u32 {
        self.step
    }
    pub(crate) fn owner(&self) -> &SealedBodyLifecycleSession {
        self.owner.as_deref().expect("owned Body")
    }
    /// Caller performs an arbitrary mutation; an error may have changed state.
    pub(crate) fn stage(
        &mut self,
        mutate: impl FnOnce(
            &mut SealedBodyLifecycleSession,
            &mut AdmissionManager,
        ) -> Result<T, &'static str>,
    ) -> Result<(), &'static str> {
        if self.phase != Phase::Staging {
            return Err("checkpoint-job-phase");
        }
        self.staged = true;
        let store = self.lease.0.as_mut().ok_or("restart-not-ready")?;
        store.ready = false;
        let value = mutate(
            self.owner.as_deref_mut().ok_or("checkpoint-job-owner")?,
            store
                .admissions
                .as_mut()
                .ok_or("restart-admissions-missing")?,
        );
        match value {
            Ok(value) => {
                self.pending_output = Some(value);
                self.seal_staged()
            }
            Err(reason) => {
                self.phase = Phase::Failed;
                Err(reason)
            }
        }
    }
    /// This entrance relies specifically on the Core admission transaction:
    /// lifecycle_session/membership stages manager/membership privately, and
    /// biography::MembershipAppend restores original records on every refusal.
    /// No caller-supplied mutation or assertion of atomicity is accepted.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn stage_invited_host_scope(
        &mut self,
        host: conduit_body::SpawnAdmissionHostScope<'_>,
        proof: &conduit_body::SpawnAdmissionProof,
        now_millis: u64,
        authority_host: &conduit_core::HostId,
        authority_boot: &conduit_core::BootId,
        map_output: impl FnOnce(&SealedBodyLifecycleSession, conduit_body::MembershipCredential) -> T,
    ) -> Result<(), &'static str> {
        if self.phase != Phase::Staging {
            return Err("checkpoint-job-phase");
        }
        let store = self.lease.0.as_mut().ok_or("restart-not-ready")?;
        let owner = self.owner.as_deref_mut().ok_or("checkpoint-job-owner")?;
        let credential = owner
            .admit_invited_host_scope(
                store
                    .admissions
                    .as_mut()
                    .ok_or("restart-admissions-missing")?,
                host,
                proof,
                now_millis,
                authority_host,
                authority_boot,
            )
            .map_err(|_| "body-membership-admission")?;
        // Canonical success is the first point at which publication must fence.
        self.staged = true;
        store.ready = false;
        self.pending_output = Some(map_output(owner, credential));
        self.seal_staged()
    }
    fn seal_staged(&mut self) -> Result<(), &'static str> {
        let owner = self.owner.as_deref_mut().ok_or("checkpoint-job-owner")?;
        let result = if owner.realization().is_some_and(|r| r.play.is_some()) {
            owner
                .seal_started_membership_history_for_checkpoint()
                .map(|_| ())
                .map_err(|_| "restart-active-membership-seal")
        } else {
            Ok(())
        };
        self.phase = if result.is_ok() {
            Phase::Prepare
        } else {
            Phase::Failed
        };
        result
    }
    /// Source replay is a distinct scheduled preparation operation. Caller must
    /// supply both this actual quiet permit and its own qualified replay budget.
    pub(crate) fn prepare(&mut self, _permit: &FlashPermit<'_, '_>) -> Result<(), &'static str> {
        if self.phase != Phase::Prepare {
            return Err("checkpoint-job-phase");
        }
        let result = (|| {
            let owner = self.owner.as_deref().ok_or("checkpoint-job-owner")?;
            let store = self.lease.0.as_mut().ok_or("restart-not-ready")?;
            let seed = store.seed.as_ref().ok_or("restart-key-missing")?;
            if ed25519_dalek::SigningKey::from_bytes(seed)
                .verifying_key()
                .as_bytes()
                != &self.root
            {
                return Err("restart-signer-mismatch");
            }
            let previous = store.custody.as_ref().ok_or("restart-custody-missing")?;
            let prepared = prepare(previous, owner, store.admissions.as_ref(), &self.root)?;
            if let Some(measured) = prepared.measure_frozen(&mut store.journal, &previous.source)? {
                self.metadata = Some(measured.metadata());
                self.measurement = Some(measured);
                return Ok(());
            }
            let mut metadata = None;
            let writer = IncrementalRewrite::prepare(
                &mut store.journal,
                &previous.source,
                &store.binding,
                seed,
                |reader, sink| {
                    metadata = Some(emit(&prepared, reader, sink)?);
                    Ok(())
                },
            )
            .map_err(checkpoint_commit_refusal)?;
            self.writer = Some(writer);
            self.metadata = metadata;
            Ok(())
        })();
        if result.is_ok() && self.measurement.is_some() {
            self.step = self.step.checked_add(1).ok_or("checkpoint-job-step")?;
            self.phase = Phase::Capture;
            Ok(())
        } else {
            self.transition(result)
        }
    }
    pub(crate) fn capture(&mut self, _permit: &FlashPermit<'_, '_>) -> Result<(), &'static str> {
        if self.phase != Phase::Capture {
            return Err("checkpoint-job-phase");
        }
        let result = (|| {
            let owner = self.owner.as_deref().ok_or("checkpoint-job-owner")?;
            let store = self.lease.0.as_mut().ok_or("restart-not-ready")?;
            let previous = store.custody.as_ref().ok_or("restart-custody-missing")?;
            let prepared = prepare(previous, owner, store.admissions.as_ref(), &self.root)?;
            let measured = self
                .measurement
                .as_ref()
                .ok_or("checkpoint-job-measurement")?;
            let seed = store.seed.as_ref().ok_or("restart-key-missing")?;
            if let Some((frozen, writer)) = prepared.capture_frozen(
                &mut store.journal,
                &previous.source,
                &store.binding,
                seed,
                measured,
            )? {
                self.frozen = Some(frozen);
                self.writer = Some(writer);
            } else {
                // Allocation refusal preserves the actual owner and source;
                // the existing full replay writer has not produced any effect.
                let expected = self.metadata.as_ref().ok_or("checkpoint-job-metadata")?;
                self.writer = Some(
                    IncrementalRewrite::prepare(
                        &mut store.journal,
                        &previous.source,
                        &store.binding,
                        seed,
                        |reader, sink| {
                            if &emit(&prepared, reader, sink)? != expected {
                                return Err(Refusal::Corrupt);
                            }
                            Ok(())
                        },
                    )
                    .map_err(checkpoint_commit_refusal)?,
                );
            }
            self.measurement = None;
            Ok(())
        })();
        self.transition(result)
    }
    pub(crate) fn prepare_chunk(
        &mut self,
        _permit: &FlashPermit<'_, '_>,
    ) -> Result<(), &'static str> {
        if self.phase != self.chunk_phase() {
            return Err("checkpoint-job-phase");
        }
        let result = (|| {
            let store = self.lease.0.as_mut().ok_or("restart-not-ready")?;
            if let Some(frozen) = self.frozen.as_ref() {
                return frozen
                    .stage(
                        self.writer.as_mut().ok_or("checkpoint-job-writer")?,
                        &mut store.journal,
                    )
                    .map_err(checkpoint_commit_refusal);
            }
            let owner = self.owner.as_deref().ok_or("checkpoint-job-owner")?;
            let previous = store.custody.as_ref().ok_or("restart-custody-missing")?;
            let prepared = prepare(previous, owner, store.admissions.as_ref(), &self.root)?;
            let metadata = self.metadata.as_ref().ok_or("checkpoint-job-metadata")?;
            self.writer
                .as_mut()
                .ok_or("checkpoint-job-writer")?
                .prepare_chunk(&mut store.journal, |reader, sink| {
                    if &emit(&prepared, reader, sink)? != metadata {
                        return Err(Refusal::Corrupt);
                    }
                    Ok(())
                })
                .map_err(checkpoint_commit_refusal)
        })();
        self.transition(result)
    }
    pub(crate) fn advance(
        &mut self,
        access: &mut impl SectorStorage<BodyRegion>,
    ) -> Result<(), &'static str> {
        if !matches!(self.phase, Phase::Physical(_)) {
            return Err("checkpoint-job-phase");
        }
        let result = self
            .writer
            .as_mut()
            .ok_or("checkpoint-job-writer")?
            .advance(
                &mut self.lease.0.as_mut().ok_or("restart-not-ready")?.journal,
                access,
            )
            .map(|_| ())
            .map_err(checkpoint_commit_refusal);
        self.transition(result)
    }
    fn transition(&mut self, result: Result<(), &'static str>) -> Result<(), &'static str> {
        if result.is_err() {
            self.phase = Phase::Failed;
            return result;
        }
        self.step = self.step.checked_add(1).ok_or("checkpoint-job-step")?;
        let writer = self.writer.as_ref().ok_or("checkpoint-job-writer")?;
        self.phase = if writer.is_complete() {
            Phase::Finish
        } else if writer.needs_chunk() {
            self.chunk_phase()
        } else {
            Phase::Physical(writer.operation().ok_or("checkpoint-job-operation")?)
        };
        Ok(())
    }
    fn chunk_phase(&self) -> Phase {
        if self.frozen.is_some() {
            Phase::FrozenChunk
        } else {
            Phase::ReplayChunk
        }
    }
    pub(crate) fn finish_source(
        &mut self,
        _permit: &FlashPermit<'_, '_>,
    ) -> Result<(), &'static str> {
        if self.phase != Phase::Finish {
            return Err("checkpoint-job-phase");
        }
        // Reserve the mailbox progress ordinal before source verification or
        // adoption can have effects. Overflow must not publish completion.
        let Some(next_step) = self.step.checked_add(1) else {
            self.phase = Phase::Failed;
            return Err("checkpoint-job-step");
        };
        // Encoded cache is no longer needed after authenticated physical commit.
        self.frozen = None;
        let result = (|| {
            let owner = self.owner.as_deref_mut().ok_or("checkpoint-job-owner")?;
            let store = self.lease.0.as_mut().ok_or("restart-not-ready")?;
            let previous = store.custody.as_ref().ok_or("restart-custody-missing")?;
            let prepared = prepare(previous, owner, store.admissions.as_ref(), &self.root)?;
            let metadata = self.metadata.as_ref().ok_or("checkpoint-job-metadata")?;
            let checked = store
                .journal
                .read_snapshot(&previous.source, |reader| {
                    emit(&prepared, reader, &mut |_| Ok(()))
                })
                .map_err(checkpoint_commit_refusal)?;
            if &checked != metadata {
                return Err("checkpoint-job-metadata");
            }
            let completion = self
                .writer
                .take()
                .ok_or("checkpoint-job-writer")?
                .finish(&mut store.journal)
                .map_err(checkpoint_commit_refusal)?;
            let (source, proof) = prepared.committed(completion)?;
            drop(prepared);
            let local_count = previous
                .local_count
                .checked_add(owner.pending_archives().len() as u64)
                .ok_or("restart-archive-count")?;
            let previous = store.custody.as_mut().ok_or("restart-custody-missing")?;
            let next = ArchiveCustody {
                proof: Some(proof),
                source,
                summary: checked.0,
                span: checked.1,
                layouts: core::mem::take(&mut previous.layouts),
                prefix: previous.prefix.take(),
                network: previous.network.take(),
                local_count,
            };
            store.custody = Some(next);
            if let Some(head) = owner.pending_archives().last().map(|s| s.digest) {
                owner
                    .acknowledge_archives(head)
                    .map_err(|_| "restart-archive-ack")?;
            }
            store.archives = Vec::new();
            store.ready = true;
            Ok(())
        })();
        self.phase = if result.is_ok() {
            self.step = next_step;
            Phase::Complete
        } else {
            Phase::Failed
        };
        result
    }
    /// Memory-only durable adoption. Output cannot escape before source commit.
    pub(crate) fn adopt(
        mut self: Box<Self>,
    ) -> Result<(Box<SealedBodyLifecycleSession>, T), Box<Self>> {
        if self.phase != Phase::Complete {
            return Err(self);
        }
        Ok((
            self.owner.take().expect("owned Body"),
            self.pending_output.take().expect("staged output"),
        ))
    }
    pub(crate) fn release_unstaged(
        mut self: Box<Self>,
    ) -> Result<Box<SealedBodyLifecycleSession>, Box<Self>> {
        if self.phase != Phase::Staging || self.staged {
            return Err(self);
        }
        Ok(self.owner.take().expect("owned Body"))
    }
}
impl<T> Drop for Job<T> {
    fn drop(&mut self) {
        if self.staged && self.phase != Phase::Complete {
            if let Some(store) = self.lease.0.as_mut() {
                store.ready = false;
            }
        }
    }
}
fn prepare<'a>(
    previous: &'a ArchiveCustody,
    owner: &'a SealedBodyLifecycleSession,
    admissions: Option<&'a AdmissionManager>,
    root: &[u8; 32],
) -> Result<payload::rewrite::PreparedRewrite<'a>, &'static str> {
    payload::rewrite::PreparedRewrite::prepare_framed_network(
        previous.proof.as_ref().ok_or("restart-custody-proof")?,
        &previous.source,
        owner.evidence(),
        owner.pending_archives(),
        admissions.ok_or("restart-admissions-missing")?,
        &previous.summary,
        previous.span,
        previous.prefix.as_ref(),
        previous.local_count,
        root,
        &previous.layouts,
        previous.network.as_ref(),
    )
}
fn emit<S: crate::native_body_checkpoint::Storage>(
    prepared: &payload::rewrite::PreparedRewrite<'_>,
    reader: &mut PayloadReader<'_, S>,
    sink: &mut dyn FnMut(&[u8]) -> Result<(), Refusal>,
) -> Result<(payload::archive::Summary, payload::archive::Span), Refusal> {
    let mut refused = None;
    prepared
        .emit(
            |out| {
                reader.read(out).map_err(|e| {
                    refused = Some(e);
                })
            },
            sink,
        )
        .map_err(|e| refused.unwrap_or_else(|| stream_refusal(e)))
}

#[cfg(all(test, feature = "observation-store"))]
#[path = "native_restart_owned_checkpoint_tests.rs"]
mod tests;

#[path = "native_restart_checkpoint_probe.rs"]
mod checkpoint_probe;
pub(crate) use checkpoint_probe::{probe, ProbeReport};

#[cfg(any(test, feature = "checkpoint-preparation-check"))]
pub(crate) use checkpoint_probe::{probe_frozen, FrozenProbeReport};
