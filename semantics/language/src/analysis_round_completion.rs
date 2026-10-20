//! Consuming post-Play conversion, without executing the round traversal again.
//! Native decode/copies allocate here; this is explicitly hosted preparation.
use super::*;
use conduit_plot::rust_binding::NativeRustBinding;

/// Every refusal returns all retained Source/model evidence in the unchanged cursor.
pub struct CursorCompletionFailure<'d> {
    pub refusal: FallbackRoundRefusal,
    pub cursor: PreparedRoundCursor<'d>,
}
impl<'d> PreparedRoundCursor<'d> {
    /// Consumes already completed Source-law custody into the existing owner receipt.
    /// Does not execute Source or numerical kernels or grant Current authority.
    /// The owner must still use its normal transactional apply entrance.
    #[allow(unused_mut)] // Only authenticated-model builds move numerical evidence.
    pub fn into_hosted_prepared_round(
        mut self,
        owner: &AnalysisEpochOwner<'_>,
    ) -> Result<PreparedFallbackRound, CursorCompletionFailure<'d>> {
        let refuse = |cursor, refusal| CursorCompletionFailure { refusal, cursor };
        if self.progress() != CursorProgress::Complete {
            return Err(refuse(self, FallbackRoundRefusal::Admission));
        }
        if owner.session.is_closed() {
            return Err(refuse(self, FallbackRoundRefusal::Closed));
        }
        if !self.identity.owner_instance.matches(owner)
            || self.identity.context != round_context(owner)
            || self.identity.generation != owner.generation
            || owner
                .frontiers()
                .stable_through
                .is_some_and(|stable| stable > owner.frontiers().committed)
        {
            return Err(refuse(self, FallbackRoundRefusal::Stale));
        }
        if owner.rounds.len() >= owner.round_limit || owner.rounds.len() >= owner.rounds.capacity()
        {
            return Err(refuse(self, FallbackRoundRefusal::Capacity));
        }
        if owner.generation.checked_add(1).is_none() {
            return Err(refuse(self, FallbackRoundRefusal::Generation));
        }
        // Full canonical equality, not digest equality or shape-only custody.
        let prior = match owner.beam.clone().encode() {
            Ok(prior) => prior,
            Err(error) => return Err(refuse(self, FallbackRoundRefusal::Native(error))),
        };
        let lexical = match owner.lexical().tape().clone().encode() {
            Ok(lexical) => lexical,
            Err(error) => return Err(refuse(self, FallbackRoundRefusal::Native(error))),
        };
        if self.identity.prior != prior || self.identity.lexical != lexical {
            return Err(refuse(self, FallbackRoundRefusal::Stale));
        }
        let mut prepared = match hosted::finish_owned(owner, &self) {
            Ok(prepared) => prepared,
            Err((refusal, _)) => return Err(refuse(self, refusal)),
        };
        #[cfg(all(
            feature = "parser-model",
            target_has_atomic = "ptr",
            target_has_atomic = "8"
        ))]
        {
            prepared.learned = match hosted::owned_learned(&mut self) {
                Ok(learned) => learned,
                Err(refusal) => return Err(refuse(self, refusal)),
            };
        }
        Ok(prepared)
    }
}
