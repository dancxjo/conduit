//! Allocating hosted adapters drive the same retained cursor; never a second traversal.
use super::*;
use alloc::string::ToString;
use conduit_plot::rust_binding::NativeRustBinding;
pub(super) fn source_step(
    cursor: &mut PreparedRoundCursor<'_>,
    provider: &mut dyn FallbackObservationProvider,
) -> Result<(), FallbackRoundRefusal> {
    match cursor.progress() {
        CursorProgress::ReadySource => {
            let lease = cursor.issue_source().map_err(refusal)?;
            let request = cursor.source_request(&lease).map_err(refusal)?;
            let output = provider.observe(request.operation, request.input)?;
            cursor
                .submit_source(lease, &output)
                .map_err(|v| FallbackRoundRefusal::Cursor(v.refusal))?;
        }
        CursorProgress::ReadyNumerical => return Err(FallbackRoundRefusal::PendingNumerical),
        CursorProgress::AwaitingSource => return Err(FallbackRoundRefusal::ForeignObservation),
        _ => {
            cursor.step().map_err(refusal)?;
        }
    }
    Ok(())
}
fn observations(cursor: &PreparedRoundCursor<'_>) -> Vec<FallbackOperationObservation> {
    (0..cursor.observation_count())
        .filter_map(|i| cursor.observation(i))
        .map(|v| FallbackOperationObservation {
            operation: v.operation.to_string(),
            input: v.input.to_vec(),
            output: v.output.to_vec(),
        })
        .collect()
}
pub(in crate::analysis_epoch_owner::fallback_round) fn fallback(
    owner: &AnalysisEpochOwner<'_>,
    programs: &PreparedFallbackRoundPrograms,
    provider: &mut dyn FallbackObservationProvider,
) -> Result<PreparedFallbackRound, FallbackRoundPreparationFailure> {
    let mut cursor =
        PreparedRoundCursor::prepare_fallback(owner, programs.checked_document(), programs)
            .map_err(|v| FallbackRoundPreparationFailure {
                refusal: refusal(v),
                observations: Vec::new(),
            })?;
    while cursor.progress() != CursorProgress::Complete {
        if let Err(refusal) = source_step(&mut cursor, provider) {
            cursor.cancel();
            return Err(FallbackRoundPreparationFailure {
                refusal,
                observations: observations(&cursor),
            });
        }
    }
    finish_owned(owner, &cursor).map_err(|(refusal, observations)| {
        FallbackRoundPreparationFailure {
            refusal,
            observations,
        }
    })
}
pub(super) fn finish_owned(
    owner: &AnalysisEpochOwner<'_>,
    cursor: &PreparedRoundCursor<'_>,
) -> Result<PreparedFallbackRound, (FallbackRoundRefusal, Vec<FallbackOperationObservation>)> {
    let observations = observations(cursor);
    let result = cursor
        .result_canonical()
        .map_err(refusal)
        .and_then(|v| LanguageParserScorerBeam::decode(v).map_err(FallbackRoundRefusal::Native));
    let result = match result {
        Ok(result) => result,
        Err(reason) => return Err((reason, observations)),
    };
    Ok(PreparedFallbackRound {
        owner_instance: cursor.identity.owner_instance.clone(),
        mode: cursor.mode(),
        context: cursor.identity.context.clone(),
        generation: cursor.identity.generation,
        prior: owner.beam.clone(),
        lexical: owner.lexical().tape().clone(),
        result: crate::analysis_epoch_owner::beam_slot::SharedBeam::prepare(result),
        next_identity: cursor.next_identity(),
        observations: io::compact_observations(observations),
        #[cfg(all(
            feature = "parser-model",
            target_has_atomic = "8",
            target_has_atomic = "ptr"
        ))]
        learned: Vec::new(),
    })
}
struct Trace<'a> {
    observations: &'a [FallbackOperationObservation],
    next: usize,
}
impl FallbackObservationProvider for Trace<'_> {
    fn observe(&mut self, operation: &str, input: &[u8]) -> Result<Vec<u8>, FallbackRoundRefusal> {
        let frame = self.observations.get(self.next).ok_or_else(|| {
            FallbackRoundRefusal::IncompleteTrace {
                operation: operation.to_string(),
                input: input.to_vec(),
            }
        })?;
        if frame.operation != operation || frame.input != input {
            return Err(FallbackRoundRefusal::ForeignObservation);
        }
        self.next += 1;
        Ok(frame.output.clone())
    }
}
pub(in crate::analysis_epoch_owner::fallback_round) fn trace(
    owner: &AnalysisEpochOwner<'_>,
    programs: &PreparedFallbackRoundPrograms,
    observations: Vec<FallbackOperationObservation>,
) -> Result<PreparedFallbackRound, FallbackRoundPreparationFailure> {
    if let Err(refusal) = io::Replay::new(programs, &observations) {
        return Err(FallbackRoundPreparationFailure {
            refusal,
            observations,
        });
    }
    let mut provider = Trace {
        observations: &observations,
        next: 0,
    };
    let result = fallback(owner, programs, &mut provider);
    let extra = provider.next != observations.len();
    match result {
        Ok(_) if extra => Err(FallbackRoundPreparationFailure {
            refusal: FallbackRoundRefusal::ExtraObservation,
            observations,
        }),
        Err(failure) => Err(FallbackRoundPreparationFailure {
            refusal: failure.refusal,
            observations,
        }),
        other => other,
    }
}
#[cfg(all(
    feature = "parser-model",
    target_has_atomic = "8",
    target_has_atomic = "ptr"
))]
// Keep original numerical custody/observations in the by-value failure result.
#[allow(clippy::result_large_err)]
pub(in crate::analysis_epoch_owner::fallback_round) fn learned(
    owner: &AnalysisEpochOwner<'_>,
    programs: &PreparedFallbackRoundPrograms,
    provider: &mut dyn FallbackObservationProvider,
    channel: &mut learned::PreparedLearnedRoundChannel,
) -> Result<PreparedFallbackRound, learned::LearnedRoundPreparationFailure> {
    use learned::*;
    if channel.is_stopped() || channel.pending().is_some() || channel.numerical().is_closed() {
        return Err(LearnedRoundPreparationFailure {
            refusal: FallbackRoundRefusal::PendingNumerical,
            observations: Vec::new(),
            learned: Vec::new(),
            retained_numerical: Vec::new(),
        });
    }
    if owner.session.is_closed() {
        return Err(LearnedRoundPreparationFailure {
            refusal: FallbackRoundRefusal::Closed,
            observations: Vec::new(),
            learned: Vec::new(),
            retained_numerical: Vec::new(),
        });
    }
    if owner.rounds.len() >= owner.round_limit || owner.rounds.len() >= owner.rounds.capacity() {
        return Err(LearnedRoundPreparationFailure {
            refusal: FallbackRoundRefusal::Capacity,
            observations: Vec::new(),
            learned: Vec::new(),
            retained_numerical: Vec::new(),
        });
    }
    let mut cursor =
        PreparedRoundCursor::prepare_learned(owner, programs.checked_document(), programs, channel)
            .map_err(|v| LearnedRoundPreparationFailure {
                refusal: refusal(v),
                observations: Vec::new(),
                learned: Vec::new(),
                retained_numerical: Vec::new(),
            })?;
    let mut polls = 0;
    while cursor.progress() != CursorProgress::Complete {
        let result = if cursor.progress() == CursorProgress::ReadyNumerical {
            polls += 1;
            match channel.step(&mut cursor) {
                Ok(true) => {
                    polls = 0;
                    Ok(())
                }
                Ok(false) if polls < MAX_KERNEL_POLLS_PER_REQUEST => Ok(()),
                Ok(false) => Err(FallbackRoundRefusal::PendingNumerical),
                Err(reason) => Err(FallbackRoundRefusal::Cursor(reason)),
            }
        } else {
            source_step(&mut cursor, provider)
        };
        if let Err(refusal) = result {
            channel.stop_hosted();
            cursor.cancel();
            let observations = observations(&cursor);
            let learned = owned_learned(&mut cursor).unwrap_or_default();
            return Err(LearnedRoundPreparationFailure {
                refusal,
                observations,
                learned,
                retained_numerical: core::mem::take(&mut cursor.numerical),
            });
        }
    }
    let mut prepared = finish_owned(owner, &cursor).map_err(|(refusal, observations)| {
        LearnedRoundPreparationFailure {
            refusal,
            observations,
            learned: Vec::new(),
            retained_numerical: core::mem::take(&mut cursor.numerical),
        }
    })?;
    prepared.learned =
        owned_learned(&mut cursor).map_err(|refusal| LearnedRoundPreparationFailure {
            refusal,
            observations: observations(&cursor),
            learned: Vec::new(),
            retained_numerical: core::mem::take(&mut cursor.numerical),
        })?;
    Ok(prepared)
}
#[cfg(all(
    feature = "parser-model",
    target_has_atomic = "8",
    target_has_atomic = "ptr"
))]
pub(super) fn owned_learned(
    cursor: &mut PreparedRoundCursor<'_>,
) -> Result<Vec<learned::LearnedBranchEvidence>, FallbackRoundRefusal> {
    let mut requests = Vec::with_capacity(cursor.numerical.len());
    for evidence in &cursor.numerical {
        let frame = cursor
            .io
            .observation(evidence.lease.source)
            .ok_or(FallbackRoundRefusal::ForeignObservation)?;
        if frame.operation != "language-parser-scorer-model-features" || !frame.admitted {
            return Err(FallbackRoundRefusal::ForeignObservation);
        }
        let query = LanguageParserScorerChoiceQuery::decode(frame.input)
            .map_err(FallbackRoundRefusal::Native)?;
        let features = LanguageParserScorerFeatures::decode(frame.output)
            .map_err(FallbackRoundRefusal::Native)?;
        let prior = LanguageParserScorerBeam::decode(&evidence.lease.identity.prior)
            .map_err(FallbackRoundRefusal::Native)?;
        let mut input = alloc::vec![0;evidence.numerical.input_len()];
        evidence
            .numerical
            .copy_input_into(&mut input)
            .map_err(|_| FallbackRoundRefusal::Numerical)?;
        requests.push(learned::PendingParserScore {
            context: evidence.lease.identity.context.clone(),
            generation: evidence.lease.identity.generation,
            prior,
            slot: evidence.lease.slot,
            choice: evidence.lease.choice,
            ordinal: evidence.lease.ordinal,
            query,
            features,
            input,
        });
    }
    Ok(requests
        .into_iter()
        .zip(core::mem::take(&mut cursor.numerical))
        .map(|(request, evidence)| learned::LearnedBranchEvidence {
            request,
            numerical: evidence.numerical,
            selection: evidence.selection,
            profile: evidence.profile,
            definition: evidence.definition,
        })
        .collect())
}

fn refusal(reason: CursorRefusal) -> FallbackRoundRefusal {
    match reason {
        CursorRefusal::Closed => FallbackRoundRefusal::Closed,
        CursorRefusal::ForeignLease => FallbackRoundRefusal::Stale,
        CursorRefusal::Bounds => FallbackRoundRefusal::Bound,
        other => FallbackRoundRefusal::Cursor(other),
    }
}
