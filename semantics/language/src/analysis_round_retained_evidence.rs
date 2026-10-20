//! Moving a completed cursor's existing evidence, without hosted copies.
//! This is checked semantic custody, not an installed-execution attestation or Current grant.
use super::*;

/// Lifetime-free Source and numerical evidence. Construction consumes one exact
/// completed cursor; there is no raw-byte or caller-assembled constructor.
pub struct RetainedRoundEvidence {
    pub(super) identity: Arc<RoundIdentity>,
    io: SourceIo,
    result: Vec<u8>,
    mode: AnalysisRoundMode,
    next_identity: u64,
    #[cfg(all(
        feature = "parser-model",
        target_has_atomic = "ptr",
        target_has_atomic = "8"
    ))]
    numerical: Vec<numerical::CursorNumericalEvidence>,
}
impl RetainedRoundEvidence {
    pub fn mode(&self) -> AnalysisRoundMode {
        self.mode
    }
    pub fn next_identity(&self) -> u64 {
        self.next_identity
    }
    /// Exact instance membership, independent of Current authority.
    pub fn matches_owner_instance(&self, owner: &AnalysisEpochOwner<'_>) -> bool {
        self.identity.owner_instance.matches(owner)
    }
    pub fn context(&self) -> &[String; 5] {
        &self.identity.context
    }
    pub fn generation(&self) -> u64 {
        self.identity.generation
    }
    pub fn prior_canonical(&self) -> &[u8] {
        &self.identity.prior
    }
    pub fn lexical_canonical(&self) -> &[u8] {
        &self.identity.lexical
    }
    pub fn result_canonical(&self) -> &[u8] {
        &self.result
    }
    pub fn observation_count(&self) -> usize {
        self.io.len()
    }
    pub fn observation(&self, index: usize) -> Option<CursorObservation<'_>> {
        self.io.observation(index)
    }
    #[cfg(all(
        feature = "parser-model",
        target_has_atomic = "ptr",
        target_has_atomic = "8"
    ))]
    pub fn numerical_evidence(&self) -> &[numerical::CursorNumericalEvidence] {
        &self.numerical
    }
    /// Source query/features remain distinct from the numeric indices payload.
    /// The opaque numerical invocation offers its own caller-buffer input copy.
    #[cfg(all(
        feature = "parser-model",
        target_has_atomic = "ptr",
        target_has_atomic = "8"
    ))]
    pub fn numerical_source(&self, index: usize) -> R<CursorObservation<'_>> {
        let evidence = self.numerical.get(index).ok_or(CursorRefusal::Lifecycle)?;
        let frame = self
            .io
            .observation(evidence.source_ordinal())
            .ok_or(CursorRefusal::Canonical)?;
        if frame.operation != "language-parser-scorer-model-features" || !frame.admitted {
            return Err(CursorRefusal::ForeignOutput);
        }
        Ok(frame)
    }
}

/// Controller-owned spent preparation. Keep this in an admitted finite slot
/// through Play: dropping it would release the cursor's prepared allocations.
/// It cannot resume a cursor or supply an owner/currentness authority.
pub struct SpentCursorResources<'d> {
    _native: NativeBank<'d>,
    _material: Materializer,
    _machine: Machine,
    #[cfg(all(
        feature = "parser-model",
        target_has_atomic = "ptr",
        target_has_atomic = "8"
    ))]
    _numeric_binding: Option<Arc<()>>,
}
pub struct RetainEvidenceFailure<'d> {
    pub refusal: CursorRefusal,
    pub cursor: PreparedRoundCursor<'d>,
}
impl<'d> PreparedRoundCursor<'d> {
    /// Move the exact original arenas/frames/tokens into evidence. The caller
    /// must retain returned spent resources; this method neither publishes a
    /// round nor verifies actual child-Play/ACK custody held by a host driver.
    #[allow(clippy::result_large_err)]
    pub fn into_retained_evidence(
        self,
    ) -> Result<(RetainedRoundEvidence, SpentCursorResources<'d>), RetainEvidenceFailure<'d>> {
        if let Err(refusal) = self.completed_result_admission() {
            return Err(RetainEvidenceFailure {
                refusal,
                cursor: self,
            });
        }
        let Self {
            identity,
            mut machine,
            material,
            native,
            io,
            stage: _,
            failure: _,
            #[cfg(all(
                feature = "parser-model",
                target_has_atomic = "ptr",
                target_has_atomic = "8"
            ))]
            numerical,
            #[cfg(all(
                feature = "parser-model",
                target_has_atomic = "ptr",
                target_has_atomic = "8"
            ))]
            numeric_binding,
        } = self;
        // Empty replacement is allocation-free. All other Machine buffers are
        // retained in spent storage, including the original prior and queries.
        let result = core::mem::take(&mut machine.next);
        Ok((
            RetainedRoundEvidence {
                identity,
                io,
                result,
                mode: machine.mode,
                next_identity: machine.identity,
                #[cfg(all(
                    feature = "parser-model",
                    target_has_atomic = "ptr",
                    target_has_atomic = "8"
                ))]
                numerical,
            },
            SpentCursorResources {
                _native: native,
                _material: material,
                _machine: machine,
                #[cfg(all(
                    feature = "parser-model",
                    target_has_atomic = "ptr",
                    target_has_atomic = "8"
                ))]
                _numeric_binding: numeric_binding,
            },
        ))
    }
}

/// Refusal preserves the whole original cursor; completion storage remains
/// borrowed from its controller and retains all of its pending material.
pub struct RetainedRoundPreparationFailure<'d> {
    pub refusal: FallbackRoundRefusal,
    pub cursor: PreparedRoundCursor<'d>,
}
impl<'d> PreparedRoundCursor<'d> {
    /// Prepare the same transactional owner receipt by moving existing evidence.
    /// The caller must retain spent resources and completion storage in finite
    /// controller slots. Apply still checks the full owner guard again; this
    /// method does not mutate the owner or grant Current authority.
    #[allow(clippy::result_large_err)]
    pub fn into_retained_prepared_round(
        self,
        owner: &AnalysisEpochOwner<'_>,
        completion: &installed_completion::PreparedInstalledRoundCompletion<'_>,
    ) -> Result<
        (PreparedFallbackRound, SpentCursorResources<'d>),
        RetainedRoundPreparationFailure<'d>,
    > {
        if let Some(refusal) = owner.round_refusal(
            &self.identity.owner_instance,
            &self.identity.context,
            self.identity.generation,
            self.identity.prior_shared.get(),
            &self.identity.lexical_shared,
        ) {
            return Err(RetainedRoundPreparationFailure {
                refusal,
                cursor: self,
            });
        }
        let result = match completion.completed_slot(&self) {
            Ok(result) => result,
            Err(refusal) => {
                return Err(RetainedRoundPreparationFailure {
                    refusal: FallbackRoundRefusal::Completion(refusal),
                    cursor: self,
                })
            }
        };
        let (evidence, spent) = match self.into_retained_evidence() {
            Ok(retained) => retained,
            Err(failure) => {
                return Err(RetainedRoundPreparationFailure {
                    refusal: FallbackRoundRefusal::Cursor(failure.refusal),
                    cursor: failure.cursor,
                })
            }
        };
        let prepared = PreparedFallbackRound {
            owner_instance: evidence.identity.owner_instance.clone(),
            mode: evidence.mode,
            generation: evidence.identity.generation,
            prior: evidence.identity.prior_shared.clone(),
            lexical: evidence.identity.lexical_shared.clone(),
            result,
            next_identity: evidence.next_identity,
            evidence: RoundEvidence::Retained(evidence),
        };
        Ok((prepared, spent))
    }
}
