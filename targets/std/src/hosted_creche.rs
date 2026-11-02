//! Scoped owner of one installed Crèche Host Call. The adapter supplies
//! admitted platform effects and canonical receipt custody; this owner binds
//! them to actual execution and does not schedule or invent deployment truth.
use conduit_body::{BodyPlan, BodyPlayIdentity, ResidentPlot};
use conduit_body_make::deployment_runtime::{CrecheOutcome, CrecheRequest};
use conduit_core::{FragmentId, HostAdvertisement, PlannedGear, ResourceOffer};
use sha2::{Digest, Sha256};
use std::time::{Duration, Instant};

mod admission;

#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
pub struct CrecheLimits {
    pub maximum_evidence_bytes: u64,
    pub maximum_polls: u32,
    pub minimum_poll_interval_ms: u32,
}
impl CrecheLimits {
    pub fn valid(self) -> bool {
        self.maximum_evidence_bytes > 0
            && self.maximum_evidence_bytes <= 64 * 1024 * 1024
            && self.maximum_polls > 0
            && self.maximum_polls <= 1_000_000
            && (1..=1000).contains(&self.minimum_poll_interval_ms)
    }
}

/// Digest of the retained encounter admission: initial draft, exact catalogues,
/// allowed resources/operations and finite authority/bounds. Interactive edits
/// still require canonical review and make admission before their effects; this
/// envelope does not pre-authorize a future selection. The owner additionally
/// binds the actual producing execution.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PreparedCrecheRequest {
    pub admitted_state_digest: [u8; 32],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CrechePoll {
    Pending,
    Outcome(CrecheOutcome),
}

/// Preparation admits finite immutable state only. `begin` and `poll` must be
/// nonblocking; platform workers remain adapter-owned. Methods must not invent
/// boot or join observations. Cancellation is idempotent and must drain every
/// worker/platform effect before success, including on failed preparation.
pub trait HostedCrecheAdapter {
    fn resource_offer(&self) -> ResourceOffer;
    fn limits(&self) -> CrecheLimits;
    fn prepare(
        &mut self,
        context: &CrecheExecutionContext,
    ) -> Result<PreparedCrecheRequest, String>;
    fn begin(&mut self, request: CrecheRequest) -> Result<CrechePoll, String>;
    fn poll(&mut self) -> Result<CrechePoll, String>;
    /// Canonical retained receipts or classified refusal/cancellation evidence.
    /// These bytes remain in adapter custody after the call and owner retirement.
    fn retained_evidence(&self, outcome: &CrecheOutcome) -> Result<&[u8], String>;
    fn cancel_and_drain(&mut self) -> Result<(), String>;
}

/// Immutable preparation context. A prospective Play is not permission to begin;
/// the owning installed run must confirm its actually issued matching Play.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CrecheExecutionContext {
    host: HostAdvertisement,
    plan: BodyPlan,
    play: BodyPlayIdentity,
    plot: ResidentPlot,
    fragment_id: FragmentId,
    placement: PlannedGear,
    resource: ResourceOffer,
    limits: CrecheLimits,
}
impl CrecheExecutionContext {
    pub fn host(&self) -> &HostAdvertisement {
        &self.host
    }
    pub fn plan(&self) -> &BodyPlan {
        &self.plan
    }
    pub fn play(&self) -> &BodyPlayIdentity {
        &self.play
    }
    pub fn plot(&self) -> &ResidentPlot {
        &self.plot
    }
    pub fn fragment_id(&self) -> &FragmentId {
        &self.fragment_id
    }
    pub fn placement(&self) -> &PlannedGear {
        &self.placement
    }
    pub fn resource_offer(&self) -> &ResourceOffer {
        &self.resource
    }
    pub fn limits(&self) -> CrecheLimits {
        self.limits
    }
    pub fn bind_request(&self, admitted_state_digest: [u8; 32]) -> Result<CrecheRequest, String> {
        let canonical = serde_json::to_vec(&(
            "conduit.creche/admitted-request@1",
            &self.host,
            &self.plan,
            &self.play,
            &self.plot,
            &self.fragment_id,
            &self.placement,
            &self.resource,
            self.limits,
            admitted_state_digest,
        ))
        .map_err(|e| format!("Crèche request identity: {e}"))?;
        Ok(CrecheRequest(Sha256::digest(canonical).into()))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    Prepared,
    Confirmed,
    Running,
    Completed,
    Poisoned,
    Retired,
}

/// Lives outside BodyKernel so every failure path can drain platform effects
/// before the kernel's terminal cancellation. Drop is a last-resort drain attempt;
/// callers must use `retire` and propagate its failure before kernel retirement.
pub struct CrecheHost<'a> {
    adapter: &'a mut dyn HostedCrecheAdapter,
    context: CrecheExecutionContext,
    request: CrecheRequest,
    phase: Phase,
    polls: u32,
    completed: Option<CrecheOutcome>,
    next_poll: Option<Instant>,
}
impl<'a> CrecheHost<'a> {
    pub fn prepare(
        adapter: &'a mut dyn HostedCrecheAdapter,
        host: &HostAdvertisement,
        plan: &BodyPlan,
        prospective: &BodyPlayIdentity,
    ) -> Result<Self, String> {
        Self::prepare_checked(adapter, host, plan, prospective).map_err(|(error, _)| error)
    }
    pub(crate) fn prepare_checked(
        adapter: &'a mut dyn HostedCrecheAdapter,
        host: &HostAdvertisement,
        plan: &BodyPlan,
        prospective: &BodyPlayIdentity,
    ) -> Result<Self, (String, bool)> {
        let prepared = (|| {
            let context = admission::context(
                host,
                plan,
                prospective,
                adapter.resource_offer(),
                adapter.limits(),
            )?;
            let reviewed = adapter.prepare(&context)?;
            if adapter.resource_offer() != context.resource || adapter.limits() != context.limits {
                return Err("Crèche preparation changed its admitted resource or limits".into());
            }
            let request = context.bind_request(reviewed.admitted_state_digest)?;
            Ok((context, request))
        })();
        match prepared {
            Ok((context, request)) => Ok(Self {
                adapter,
                context,
                request,
                phase: Phase::Prepared,
                polls: 0,
                completed: None,
                next_poll: None,
            }),
            Err(error) => match adapter.cancel_and_drain() {
                Ok(()) => Err((error, false)),
                Err(drain) => Err((
                    format!("{error}; Crèche preparation drain failed: {drain}"),
                    true,
                )),
            },
        }
    }
    pub fn request(&self) -> CrecheRequest {
        self.request
    }
    pub fn context(&self) -> &CrecheExecutionContext {
        &self.context
    }
    pub fn confirm_started(
        &mut self,
        host: &HostAdvertisement,
        issued: &BodyPlayIdentity,
    ) -> Result<(), String> {
        if self.phase != Phase::Prepared
            || host != &self.context.host
            || issued != &self.context.play
            || !issued.validate_for(&self.context.plan)
            || self.adapter.resource_offer() != self.context.resource
            || self.adapter.limits() != self.context.limits
        {
            return Err(
                "Crèche requires its exact current Host and actually issued Body Play".into(),
            );
        }
        self.phase = Phase::Confirmed;
        Ok(())
    }
    pub fn begin(&mut self, request: CrecheRequest) -> Result<CrechePoll, String> {
        if self.phase != Phase::Confirmed || request != self.request {
            return Err("Crèche begin is stale, repeated or not issued".into());
        }
        // Mark before entering effects so an error never permits a second begin.
        self.phase = Phase::Running;
        let result = self.adapter.begin(request);
        self.accept(result)
    }
    pub fn poll(&mut self) -> Result<CrechePoll, String> {
        if self.phase == Phase::Completed {
            // Output pressure may postpone actual kernel completion. Recheck
            // custody without polling or restarting the platform operation.
            let outcome = self.completed.expect("completed Crèche retains outcome");
            return self.accept(Ok(CrechePoll::Outcome(outcome)));
        }
        if self.phase != Phase::Running {
            return Err("Crèche poll has no live operation".into());
        }
        if !self.poll_wait().is_zero() {
            return Ok(CrechePoll::Pending);
        }
        if self.polls >= self.context.limits.maximum_polls {
            self.phase = Phase::Poisoned;
            return Err("Crèche admitted poll bound exhausted; drain required".into());
        }
        self.polls += 1;
        let result = self.adapter.poll();
        self.accept(result)
    }
    /// Remaining admitted cadence delay for one live operation. Waiting does not
    /// consume a provider poll, retry work, or grant new execution authority.
    pub fn poll_wait(&self) -> Duration {
        if self.phase != Phase::Running {
            return Duration::ZERO;
        }
        self.next_poll
            .map_or(Duration::ZERO, |next| {
                next.saturating_duration_since(Instant::now())
            })
            .min(Duration::from_millis(
                self.context.limits.minimum_poll_interval_ms.into(),
            ))
    }
    fn accept(&mut self, result: Result<CrechePoll, String>) -> Result<CrechePoll, String> {
        let checked = result.and_then(|poll| {
            if let CrechePoll::Outcome(outcome) = poll {
                if outcome.request != self.request {
                    return Err("Crèche outcome names another admitted request".into());
                }
                verify_evidence(
                    &outcome,
                    self.adapter.retained_evidence(&outcome)?,
                    self.context.limits,
                )?;
            }
            Ok(poll)
        });
        match checked {
            Ok(CrechePoll::Outcome(outcome)) => {
                self.completed = Some(outcome);
                self.phase = Phase::Completed;
                Ok(CrechePoll::Outcome(outcome))
            }
            Ok(CrechePoll::Pending) => {
                self.next_poll = Some(
                    Instant::now()
                        + Duration::from_millis(
                            self.context.limits.minimum_poll_interval_ms.into(),
                        ),
                );
                Ok(CrechePoll::Pending)
            }
            Err(error) => {
                self.phase = Phase::Poisoned;
                Err(error)
            }
        }
    }
    pub fn retire(&mut self) -> Result<(), String> {
        if self.phase == Phase::Retired {
            return Ok(());
        }
        // No call is permitted after a failed drain, but retirement remains
        // retryable until the adapter confirms that all effects have stopped.
        self.phase = Phase::Poisoned;
        self.adapter.cancel_and_drain()?;
        self.phase = Phase::Retired;
        Ok(())
    }
}
impl Drop for CrecheHost<'_> {
    fn drop(&mut self) {
        if self.phase != Phase::Retired {
            let _ = self.adapter.cancel_and_drain();
        }
    }
}

fn verify_evidence(
    outcome: &CrecheOutcome,
    bytes: &[u8],
    limits: CrecheLimits,
) -> Result<(), String> {
    if bytes.is_empty() || bytes.len() as u64 > limits.maximum_evidence_bytes {
        return Err("Crèche retained evidence is empty or exceeds admitted bounds".into());
    }
    let actual: [u8; 32] = Sha256::digest(bytes).into();
    if actual != outcome.evidence_digest {
        return Err("Crèche outcome lacks exact retained evidence custody".into());
    }
    Ok(())
}

#[cfg(test)]
pub(crate) mod test_support;
#[cfg(test)]
mod tests;
