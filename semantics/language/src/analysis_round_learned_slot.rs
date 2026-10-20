//! Deferred binding to the same actual prepared numerical channel.
use super::*;
use numerical::PreparedCursorLearnedChannel;
/// All preparation is admitted before execution. Each bind uses the channel's
/// actual next invocation, retaining its exact private numerical identity.
/// Keep consumed slots alive through Play, alongside drivers/completions/spent.
pub struct PreparedLearnedRoundSlot<'d>(ReservedRoundSlot<'d>);
impl<'d> PreparedLearnedRoundSlot<'d> {
    pub fn reserve(
        owner: &AnalysisEpochOwner<'_>,
        document: &'d CheckedSyntaxDocument,
        programs: &PreparedFallbackRoundPrograms,
        channel: &mut PreparedCursorLearnedChannel,
    ) -> R<Self> {
        Ok(Self(ReservedRoundSlot::reserve(
            PreparedRoundCursor::prepare_learned(owner, document, programs, channel)?,
        )?))
    }
    pub fn is_consumed(&self) -> bool {
        self.0.is_consumed()
    }
    /// Pressure/foreign refusal consumes neither a cursor nor model ingress.
    /// The free-slot observation does not replace actual outlet CAS/admission.
    pub fn bind(
        &mut self,
        owner: &AnalysisEpochOwner<'_>,
        channel: &mut PreparedCursorLearnedChannel,
    ) -> R<PreparedRoundCursor<'d>> {
        let cursor = self.0.cursor.as_ref().ok_or(CursorRefusal::Lifecycle)?;
        let invocation = channel.reserved_binding_invocation(cursor)?;
        self.0.bind(owner, invocation)
    }
}
