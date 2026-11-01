//! Preparation evidence is separate from the ROM flash-operation profile.
//! No target qualification has yet been retained for membership Start/replay.
//! A quiet Gate permit is necessary but cannot manufacture this evidence.
use crate::native_restart::owned_checkpoint::Phase;

#[derive(Clone, Copy)]
pub(crate) struct ReviewedPreparation {
    // Private: received requests, HTTP status and observed free heap cannot
    // construct or enlarge a reviewed preparation envelope.
    start_ms: u32,
    replay_ms: u32,
    chunk_ms: u32,
    finish_ms: u32,
}
impl ReviewedPreparation {
    pub(crate) fn budget_ms(
        self,
        phase: crate::native_body_join::membership::job::Phase,
    ) -> Option<u32> {
        use crate::native_body_join::membership::job::Phase as Membership;
        let budget = match phase {
            Membership::Start => self.start_ms,
            Membership::Checkpoint(Phase::Prepare) => self.replay_ms,
            Membership::Checkpoint(Phase::Chunk) => self.chunk_ms,
            Membership::Checkpoint(Phase::Finish) => self.finish_ms,
            _ => return None,
        };
        (budget > 0 && budget <= super::MAXIMUM_BUDGET_MS).then_some(budget)
    }
}
/// Bootstrap selection only. There is deliberately no profile yet: CPU fixture
/// allocation observations and physical readback timing do not qualify target
/// canonical admission, source replay, codec work or final authentication.
pub(super) fn reviewed_for_boot() -> Option<ReviewedPreparation> {
    None
}
