//! Opaque startup input derived from the existing retained Body, not a flag.
use conduit_body::{BodyBiographyEvidence, BodyPlan, BodyPlayIdentity, BodyStartup};

/// Causal startup evidence. This is neither admission nor playback authority.
/// The owner derives it after retaining Started; callers cannot manufacture its
/// eligibility by changing public identity fields on a BodyStartup projection.
pub struct CommittedBodyStartup {
    plan_id: conduit_core::PlanId,
    startup: BodyStartup,
}
impl CommittedBodyStartup {
    pub fn from_retained(
        biography: &BodyBiographyEvidence,
        plan: &BodyPlan,
        play: &BodyPlayIdentity,
    ) -> Result<Self, String> {
        let startup = biography
            .startup_for_play(plan, play)
            .map_err(|e| format!("retained startup: {e:?}"))?;
        Ok(Self {
            plan_id: plan.plan_id.clone(),
            startup,
        })
    }
    pub(super) fn for_play(
        &self,
        plan: &BodyPlan,
        play: &BodyPlayIdentity,
    ) -> Result<&BodyStartup, String> {
        if self.plan_id != plan.plan_id
            || self.startup.body_id != plan.body_id
            || self.startup.wake_id != plan.wake_id
            || self.startup.active_play_id != play.active_play_id
        {
            return Err("committed startup differs from exact Body Plan/Play".into());
        }
        Ok(&self.startup)
    }
}
/// Existing observers acknowledge lifecycle storage with unit. Startup sources
/// additionally need the exact retained evidence; unit never implies eligibility.
pub enum BodyStartedAcknowledgment {
    LifecycleOnly,
    CommittedStartup(CommittedBodyStartup),
}
impl From<()> for BodyStartedAcknowledgment {
    fn from(_: ()) -> Self {
        Self::LifecycleOnly
    }
}
impl From<CommittedBodyStartup> for BodyStartedAcknowledgment {
    fn from(value: CommittedBodyStartup) -> Self {
        Self::CommittedStartup(value)
    }
}
