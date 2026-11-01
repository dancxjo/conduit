//! Shared canonical CPU phases; callers retain either a real quiet permit or
//! exclusive cold ownership. Neither witness qualifies a timing profile.
use super::*;
impl<T> Job<T> {
    pub(super) fn prepare_inner(&mut self) -> Result<(), &'static str> {
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
    pub(super) fn capture_inner(&mut self) -> Result<(), &'static str> {
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
    pub(super) fn prepare_chunk_inner(&mut self) -> Result<(), &'static str> {
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
    pub(super) fn finish_source_inner(&mut self) -> Result<(), &'static str> {
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
}
