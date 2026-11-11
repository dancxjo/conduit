//! Optional finite debugger history beside the authoritative Body Sign stream.
use conduit_body::{BodyPlan, BodyPlayIdentity};
use conduit_core::semantic_digest;
use conduit_kernel::debug_observation::{
    DebugExecutionIdentity, DebugObservationGap, DebugObservationRecord, DebugObservationRefusal,
};

/// One explicitly requested, 128-record projection. Collection never grows
/// during Play. Export allocates only after terminal cleanup; gaps and observer
/// refusal do not change the Body's execution disposition.
#[derive(Debug)]
pub struct BodyDebugHistory {
    pub execution: DebugExecutionIdentity,
    pub records: Vec<DebugObservationRecord>,
    pub gap: Option<DebugObservationGap>,
    /// Exact admitted value-kind names, indexed by record.type_identity.
    pub type_names: Vec<String>,
    pub refusal: Option<DebugObservationRefusal>,
}

pub fn body_debug_execution_identity(
    plan: &BodyPlan,
    play: &BodyPlayIdentity,
) -> Result<DebugExecutionIdentity, String> {
    if !play.validate_for(plan) {
        return Err("debug observation differs from exact Body Plan/Play".into());
    }
    Ok(DebugExecutionIdentity {
        body: semantic_digest("conduit.debug/body@1", plan.body_id.as_str().as_bytes()),
        plan: semantic_digest(
            "conduit.debug/body-plan@1",
            plan.plan_id.as_str().as_bytes(),
        ),
        play: semantic_digest(
            "conduit.debug/body-play@1",
            play.active_play_id.as_str().as_bytes(),
        ),
    })
}
