//! Fixed-size causal startup evidence from the existing retained Body.
//! Biography validation and durable retention are the independently admitted
//! owner preparation boundary; only this fixed witness enters funded execution.
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
    pub(crate) fn for_play(
        &self,
        plan: &BodyPlan,
        play: &BodyPlayIdentity,
    ) -> Result<&Self, String> {
        self.for_prepared_scope(&PreparedStartupScope::new(plan, play)?)
    }
    pub(crate) fn for_prepared_scope(&self, scope: &PreparedStartupScope) -> Result<&Self, String> {
        if self.plan != scope.plan
            || self.body != scope.body
            || self.wake != scope.wake
            || self.play != scope.play
        {
            return Err("committed startup differs from exact Body Plan/Play".into());
        }
        Ok(self)
    }
}
/// Fixed exact comparison scope; not admission, authority or eligibility.
pub(crate) struct PreparedStartupScope {
    plan: [u8; 32],
    body: [u8; 32],
    wake: [u8; 32],
    play: [u8; 32],
}
impl PreparedStartupScope {
    pub(crate) fn new(plan: &BodyPlan, play: &BodyPlayIdentity) -> Result<Self, String> {
        if !play.validate_for_borrowed(plan) {
            return Err("prepared startup differs from canonical Body Plan/Play".into());
        }
        Ok(Self {
            plan: identity(plan.plan_id.as_str()),
            body: identity(plan.body_id.as_str()),
            wake: identity(plan.wake_id.as_str()),
            play: identity(play.active_play_id.as_str()),
        })
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

impl crate::StdHost {
    pub fn run_body_plan_with_committed_startup_to<W: std::io::Write, T: crate::TimerAdapter, F>(
        &mut self,
        request: super::BodyRunRequest<'_>,
        output: &mut W,
        timer: &mut T,
        fore: Option<&mut dyn super::BodyForeOutputAdapter>,
        started: F,
    ) -> Result<super::BodyRunReport, String>
    where
        F: FnMut(&BodyPlayIdentity, &conduit_body::Wake) -> Result<CommittedBodyStartup, String>,
    {
        self.run_body_plan_to_with_start_and_clock(
            request,
            output,
            timer,
            None,
            None,
            fore.map(|adapter| (&[][..], false, adapter)),
            None,
            None,
            None,
            started,
        )
    }
}

/// Exact latest cue Host Call outcome; never evidence that a human heard it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StartupChimeCallReceipt {
    pub node: conduit_kernel::NodeId,
    pub request: Option<conduit_kernel::RequestId>,
    pub disposition: Option<conduit_kernel::HostCallDisposition>,
    pub failure: Option<conduit_kernel::Failure>,
    /// False when global cancellation retired a pending call without completion.
    pub kernel_accepted: bool,
}
