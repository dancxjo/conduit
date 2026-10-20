//! One-use same-epoch round storage, reserved before outer execution.
//! No commitment publication, rebase, or Current authority.
use super::*;
/// One-use fallback slot. Keep consumed storage alive through Play.
pub struct PreparedFallbackRoundSlot<'d>(ReservedRoundSlot<'d>);
impl<'d> PreparedFallbackRoundSlot<'d> {
    pub fn reserve(
        owner: &AnalysisEpochOwner<'_>,
        document: &'d CheckedSyntaxDocument,
        programs: &PreparedFallbackRoundPrograms,
    ) -> R<Self> {
        Ok(Self(ReservedRoundSlot::reserve(
            PreparedRoundCursor::prepare_fallback(owner, document, programs)?,
        )?))
    }
    pub fn is_consumed(&self) -> bool {
        self.0.is_consumed()
    }
    pub fn bind(&mut self, owner: &AnalysisEpochOwner<'_>) -> R<PreparedRoundCursor<'d>> {
        self.0.bind(owner, 0)
    }
}
#[cfg(all(
    feature = "parser-model",
    target_has_atomic = "ptr",
    target_has_atomic = "8"
))]
#[path = "analysis_round_learned_slot.rs"]
mod learned_slot;
#[cfg(all(
    feature = "parser-model",
    target_has_atomic = "ptr",
    target_has_atomic = "8"
))]
pub use learned_slot::PreparedLearnedRoundSlot;

/// Owns all cursor/evaluator/identity buffers before the future Beam is known.
/// Driver and completion storage must separately be reserved before execution.
/// Keep the consumed slot through Play: it retains the old preparation share.
struct ReservedRoundSlot<'d> {
    cursor: Option<PreparedRoundCursor<'d>>,
    retired_prior: Option<crate::analysis_epoch_owner::beam_slot::SharedBeam>,
}
impl<'d> ReservedRoundSlot<'d> {
    fn reserve(mut cursor: PreparedRoundCursor<'d>) -> R<Self> {
        cursor.io.reserve_binding()?;
        cursor.machine.reserve_binding();
        let identity = Arc::get_mut(&mut cursor.identity).ok_or(CursorRefusal::Lifecycle)?;
        identity.prior.reserve_exact(BYTES - identity.prior.len());
        Ok(Self {
            cursor: Some(cursor),
            retired_prior: None,
        })
    }
    pub fn is_consumed(&self) -> bool {
        self.cursor.is_none()
    }
    /// Consume only after current-epoch/full Native guards and finite capacity checks.
    /// Refusal retains ownership; scratch may change. A committed/new-epoch Beam without an
    /// exact retained canonical origin refuses; it is never reencoded here.
    fn bind(
        &mut self,
        owner: &AnalysisEpochOwner<'_>,
        invocation: u64,
    ) -> R<PreparedRoundCursor<'d>> {
        let cursor = self.cursor.as_mut().ok_or(CursorRefusal::Lifecycle)?;
        if let Some(reason) = owner.round_refusal(
            &cursor.identity.owner_instance,
            &cursor.identity.context,
            owner.generation,
            owner.beam(),
            &cursor.identity.lexical_shared,
        ) {
            return Err(match reason {
                FallbackRoundRefusal::Closed => CursorRefusal::Closed,
                FallbackRoundRefusal::Capacity => CursorRefusal::Pressure,
                _ => CursorRefusal::Lifecycle,
            });
        }
        if number(member(&cursor.machine.lexical, &["token_count"])?)?
            != *owner.beam().candidate0().parser().state().token_count()
            || Arc::strong_count(&cursor.identity) != 1
        {
            return Err(CursorRefusal::Lifecycle);
        }
        let canonical = if cursor.identity.prior_shared.get() == owner.beam() {
            &cursor.identity.prior[..]
        } else {
            let round = owner.rounds.last().ok_or(CursorRefusal::Lifecycle)?;
            if round.result() != owner.beam() {
                return Err(CursorRefusal::Lifecycle);
            }
            let evidence = round.retained_evidence().ok_or(CursorRefusal::Lifecycle)?;
            if !evidence.matches_owner_instance(owner)
                || evidence.context() != &cursor.identity.context
            {
                return Err(CursorRefusal::Lifecycle);
            }
            evidence.result_canonical()
        };
        cursor.machine.bind_current(
            canonical,
            &mut cursor.material,
            owner.identity_cursor,
            invocation,
        )?;
        let identity = Arc::get_mut(&mut cursor.identity).ok_or(CursorRefusal::Lifecycle)?;
        put(&mut identity.prior, &cursor.machine.prior_beam)?;
        // Retain even the last old preparation share in the consumed slot.
        // A bind never releases its Native storage during execution.
        self.retired_prior = Some(core::mem::replace(
            &mut identity.prior_shared,
            owner.beam.clone(),
        ));
        identity.generation = owner.generation;
        cursor.io.bind(cursor.identity.clone())?;
        self.cursor.take().ok_or(CursorRefusal::Lifecycle)
    }
}
