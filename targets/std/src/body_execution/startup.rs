//! Fixed-size causal startup evidence from the existing retained Body.
use conduit_body::{BodyBiographyEvidence, BodyPlan, BodyPlayIdentity, StartupScope};
use sha2::{Digest, Sha256};
fn identity(value: &str) -> [u8; 32] {
    Sha256::digest(value.as_bytes()).into()
}

/// Causal startup input, not admission or playback authority. Private fields
/// prevent manufacturing eligibility by relabelling a public projection.
/// Fixed identity digests keep the owner acknowledgment from transferring new
/// heap backing into an already admitted execution. Original Signs stay in
/// the retained biography; these fingerprints are exact comparison evidence.
pub struct CommittedBodyStartup {
    plan: [u8; 32],
    body: [u8; 32],
    wake: [u8; 32],
    play: [u8; 32],
    wake_sign: [u8; 32],
    play_start_sign: [u8; 32],
    eligible_wake: bool,
    eligible_body: bool,
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
            plan: identity(plan.plan_id.as_str()),
            body: identity(startup.body_id.as_str()),
            wake: identity(startup.wake_id.as_str()),
            play: identity(startup.active_play_id.as_str()),
            wake_sign: identity(startup.wake_sign_id.as_str()),
            play_start_sign: identity(startup.play_start_sign_id.as_str()),
            eligible_wake: startup.eligible(StartupScope::Wake),
            eligible_body: startup.eligible(StartupScope::Body),
        })
    }
    pub fn eligible(&self, scope: StartupScope) -> bool {
        match scope {
            StartupScope::Wake => self.eligible_wake,
            StartupScope::Body => self.eligible_body,
        }
    }
    pub fn causal_sign_fingerprints(&self) -> (&[u8; 32], &[u8; 32]) {
        (&self.wake_sign, &self.play_start_sign)
    }
    pub(super) fn for_play(
        &self,
        plan: &BodyPlan,
        play: &BodyPlayIdentity,
    ) -> Result<&Self, String> {
        if !play.validate_for(plan)
            || self.plan != identity(plan.plan_id.as_str())
            || self.body != identity(plan.body_id.as_str())
            || self.wake != identity(plan.wake_id.as_str())
            || self.play != identity(play.active_play_id.as_str())
        {
            return Err("committed startup differs from exact Body Plan/Play".into());
        }
        Ok(self)
    }
}
/// Unit observers acknowledge storage only; they never imply startup eligibility.
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

pub(super) trait StartupAcknowledgmentValue: Into<BodyStartedAcknowledgment> {
    const PROVIDES_STARTUP: bool;
}
impl StartupAcknowledgmentValue for () {
    const PROVIDES_STARTUP: bool = false;
}
impl StartupAcknowledgmentValue for CommittedBodyStartup {
    const PROVIDES_STARTUP: bool = true;
}
