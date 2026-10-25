//! Canonical Crèche encounter hosted by one admitted ordinary Plot operation.
//! The one-slot mailbox carries exact Face interactions; it is not a scheduler
//! or a second action protocol. Review remains pending until real make completes.
use conduit_body_make::deployment_runtime::{CrecheDisposition, CrecheOutcome, CrecheRequest};
use conduit_body_make::{
    DeploymentDraft, DeploymentEncounter, DeploymentEncounterBasis, DeploymentEncounterOutcome,
    ReviewedDeployment,
};
use conduit_core::{ResourceOffer, ResourcePoolId};
use conduit_face::{Face, FaceBasis, FaceInteraction, ManifestationLifecycle, MaskShow};
use conduit_host_make::{CheckedHostConfiguration, MakeCatalog, MakePackageSet};
use conduit_std_host::hosted_creche::{
    CrecheExecutionContext, CrecheLimits, CrechePoll, HostedCrecheAdapter, PreparedCrecheRequest,
};
use sha2::{Digest, Sha256};
use std::sync::{Arc, Mutex, MutexGuard, TryLockError};

mod admission;
mod controller;
pub use controller::CrecheEncounterController;

/// Trusted selected renderer custody, supplied explicitly by the native owner.
/// Structural MaskShow validity is insufficient: implementations must compare
/// with the actual renderer's retained acknowledged Show and exact render receipt.
/// There is deliberately no synthetic/default acknowledgement implementation.
/// Validation must be bounded/nonblocking and must not call back into this
/// encounter's controller. Only retirement may wait for effects to drain.
pub trait CrecheMaskShowCustody: Send {
    fn validate_acknowledged(&mut self, face: &Face, show: &MaskShow) -> Result<(), String>;
    /// Require this exact occurrence returned by the selected Mask's typed
    /// interaction Fore. A previously acknowledged Show alone grants no input.
    /// Retain the one completed correlation until queued semantic consumption;
    /// validation is repeated and must not consume that receipt on first check.
    fn validate_returned_interaction(
        &mut self,
        face: &Face,
        show: &MaskShow,
        interaction: &FaceInteraction,
    ) -> Result<(), String>;

    /// Stop/drain the selected presentation/input effects. Must be idempotent;
    /// success is required before the installed operation can retire.
    fn retire_and_drain(&mut self) -> Result<(), String>;
}

/// Exact one-time canonical make handoff. Consuming the draft and review through
/// DeploymentDraft::prepare births once; merely taking this value births nothing.
/// The reviewed state and its executing request cannot be substituted externally.
/// ```compile_fail
/// fn substitute(handoff: &mut conduit::creche_encounter::CrecheReviewedHandoff) {
///     handoff.request.0 = [0; 32];
/// }
/// ```
pub struct CrecheReviewedHandoff {
    pub(crate) request: CrecheRequest,
    pub(crate) context: CrecheExecutionContext,
    pub(crate) draft: DeploymentDraft,
    pub(crate) review: ReviewedDeployment,
}
impl CrecheReviewedHandoff {
    pub fn request(&self) -> CrecheRequest {
        self.request
    }
    pub fn context(&self) -> &CrecheExecutionContext {
        &self.context
    }
    pub fn draft(&self) -> &DeploymentDraft {
        &self.draft
    }
}

/// Classified failure from the actual make owner. There is no public raw-byte
/// setter for Prepared/Realized and review cannot synthesize a successful image.
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CrecheMakeRefusal {
    UnsupportedTarget,
    SourceUnavailable,
    Unauthorized,
    ArtifactRejected,
    BuildFailed,
    BoundExhausted,
}
#[derive(Clone, Copy, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
enum TerminalReason {
    Make(CrecheMakeRefusal),
    InteractionRefused,
    Cancelled,
}

struct Command {
    show: Arc<MaskShow>,
    interaction: FaceInteraction,
}
struct Shared {
    active: bool,
    face: Option<Arc<Face>>,
    acknowledged: Option<Arc<MaskShow>>,
    command: Option<Command>,
    reviewed: bool,
    handoff: Option<CrecheReviewedHandoff>,
    handoff_taken: bool,
    handoff_request: Option<CrecheRequest>,
    products: Option<crate::creche_production::CrecheProducedImages>,
    products_completed: bool,
    make_refusal: Option<CrecheMakeRefusal>,
}
struct Channel {
    state: Mutex<Shared>,
    custody: Mutex<Box<dyn CrecheMaskShowCustody>>,
}
impl Channel {
    fn lock(&self) -> Result<MutexGuard<'_, Shared>, String> {
        self.state
            .lock()
            .map_err(|_| "Crèche encounter mailbox is poisoned".into())
    }
}

pub struct CrecheEncounterAdapter {
    channel: Arc<Channel>,
    encounter: Option<DeploymentEncounter>,
    context: Option<CrecheExecutionContext>,
    basis: Option<DeploymentEncounterBasis>,
    encounter_id: String,
    catalog: MakeCatalog,
    packages: MakePackageSet,
    resource: ResourceOffer,
    limits: CrecheLimits,
    maximum_interactions: u32,
    interactions: u32,
    admission_digest: [u8; 32],
    request: Option<CrecheRequest>,
    evidence: Vec<u8>,
    outcome: Option<CrecheOutcome>,
    prepared: bool,
    begun: bool,
    retired: bool,
}
impl CrecheEncounterAdapter {
    /// All catalogues, profiles, slots and evidence capacity are fixed here.
    /// Model projection/edit work remains bounded hosted preparation work; this
    /// adapter does not claim allocation-free Face/model implementation.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        draft: DeploymentDraft,
        profiles: Vec<CheckedHostConfiguration>,
        encounter_id: String,
        resource_pool: ResourcePoolId,
        limits: CrecheLimits,
        maximum_interactions: u32,
        custody: Box<dyn CrecheMaskShowCustody>,
    ) -> Result<(Self, CrecheEncounterController), String> {
        if !limits.valid()
            || limits.maximum_evidence_bytes < 1024
            || maximum_interactions == 0
            || maximum_interactions > 4096
            || encounter_id.is_empty()
            || encounter_id.len() > 128
            || encounter_id.chars().any(char::is_control)
            || resource_pool.as_str().is_empty()
        {
            return Err("Crèche encounter limits/identity are not admitted".into());
        }
        let catalog = conduit_workspace_make::catalog();
        let packages = conduit_workspace_make::package_set();
        let admission_digest = admission::digest(
            &draft,
            &profiles,
            &encounter_id,
            &resource_pool,
            limits,
            maximum_interactions,
            &packages,
        )?;
        let encounter = DeploymentEncounter::with_profiles(draft, profiles)
            .map_err(|e| format!("Crèche catalogue: {e:?}"))?;
        let resource = conduit_core::resource_offer(
            resource_pool.as_str(),
            conduit_body_make::deployment_runtime_catalog::CRECHE_RESOURCE,
            1,
        );
        let mut evidence = Vec::new();
        evidence
            .try_reserve_exact(limits.maximum_evidence_bytes as usize)
            .map_err(|_| "Crèche evidence storage unavailable")?;
        let channel = Arc::new(Channel {
            state: Mutex::new(Shared {
                active: false,
                face: None,
                acknowledged: None,
                command: None,
                reviewed: false,
                handoff: None,
                handoff_taken: false,
                handoff_request: None,
                products: None,
                products_completed: false,
                make_refusal: None,
            }),
            custody: Mutex::new(custody),
        });
        let controller = CrecheEncounterController {
            channel: channel.clone(),
        };
        Ok((
            Self {
                channel,
                encounter: Some(encounter),
                context: None,
                basis: None,
                encounter_id,
                catalog,
                packages,
                resource,
                limits,
                maximum_interactions,
                interactions: 0,
                admission_digest,
                request: None,
                evidence,
                outcome: None,
                prepared: false,
                begun: false,
                retired: false,
            },
            controller,
        ))
    }
    fn terminal(&mut self, reason: TerminalReason) -> Result<CrechePoll, String> {
        let request = self
            .request
            .ok_or("Crèche terminal has no admitted request")?;
        if let Some(outcome) = self.outcome {
            return Ok(CrechePoll::Outcome(outcome));
        }
        self.evidence.clear();
        #[derive(serde::Serialize)]
        struct Evidence {
            schema: &'static str,
            request: [u8; 32],
            reason: TerminalReason,
            artifact_prepared: bool,
            carrier_realized: bool,
            boot_observed: bool,
            join_observed: bool,
        }
        serde_json::to_writer(
            &mut self.evidence,
            &Evidence {
                schema: "conduit.creche/classified-operation-evidence@1",
                request: request.0,
                reason,
                artifact_prepared: false,
                carrier_realized: false,
                boot_observed: false,
                join_observed: false,
            },
        )
        .map_err(|e| format!("Crèche terminal evidence: {e}"))?;
        if self.evidence.len() as u64 > self.limits.maximum_evidence_bytes {
            self.evidence.clear();
            return Err("Crèche terminal evidence bound exhausted".into());
        }
        let outcome = CrecheOutcome {
            request,
            disposition: match reason {
                TerminalReason::Cancelled => CrecheDisposition::Cancelled,
                _ => CrecheDisposition::Refused,
            },
            evidence_digest: Sha256::digest(&self.evidence).into(),
        };
        self.outcome = Some(outcome);
        Ok(CrechePoll::Outcome(outcome))
    }
}

impl HostedCrecheAdapter for CrecheEncounterAdapter {
    fn resource_offer(&self) -> ResourceOffer {
        self.resource.clone()
    }
    fn limits(&self) -> CrecheLimits {
        self.limits
    }
    fn prepare(
        &mut self,
        context: &CrecheExecutionContext,
    ) -> Result<PreparedCrecheRequest, String> {
        if self.prepared
            || self.begun
            || self.retired
            || context.resource_offer() != &self.resource
            || context.limits() != self.limits
        {
            return Err("Crèche preparation is stale or repeated".into());
        }
        let basis = admission::basis(&self.encounter_id, context)?;
        let face = self
            .encounter
            .as_ref()
            .ok_or("Crèche draft missing")?
            .face(&basis)
            .map_err(|e| format!("Crèche Face preparation: {e:?}"))?;
        self.request = Some(context.bind_request(self.admission_digest)?);
        self.context = Some(context.clone());
        self.basis = Some(basis);
        self.channel.lock()?.face = Some(Arc::new(face));
        self.prepared = true;
        Ok(PreparedCrecheRequest {
            admitted_state_digest: self.admission_digest,
        })
    }
    fn begin(&mut self, request: CrecheRequest) -> Result<CrechePoll, String> {
        if !self.prepared || self.begun || self.retired || self.request != Some(request) {
            return Err("Crèche begin lacks its exact prepared request".into());
        }
        let mut state = self
            .channel
            .state
            .try_lock()
            .map_err(|_| "Crèche mailbox occupied at begin")?;
        if state.face.is_none() {
            return Err("Crèche initial Face was not prepared".into());
        }
        state.active = true;
        self.begun = true;
        Ok(CrechePoll::Pending)
    }
    fn poll(&mut self) -> Result<CrechePoll, String> {
        if !self.begun || self.retired {
            return Err("Crèche encounter is not active".into());
        }
        if let Some(outcome) = self.outcome {
            return Ok(CrechePoll::Outcome(outcome));
        }
        let channel = self.channel.clone();
        let mut state = match channel.state.try_lock() {
            Ok(state) => state,
            Err(TryLockError::WouldBlock) => return Ok(CrechePoll::Pending),
            Err(TryLockError::Poisoned(_)) => {
                return Err("Crèche encounter mailbox is poisoned".into())
            }
        };
        if let Some(reason) = state.make_refusal.take() {
            state.active = false;
            state.acknowledged = None;
            drop(state);
            return self.terminal(TerminalReason::Make(reason));
        }
        if let Some(products) = &state.products {
            self.evidence = products
                .evidence(self.limits.maximum_evidence_bytes)
                .map_err(|e| format!("Crèche prepared image evidence: {e:?}"))?;
            let outcome = CrecheOutcome {
                request: self
                    .request
                    .ok_or("Crèche image completion lacks request")?,
                disposition: CrecheDisposition::Prepared,
                evidence_digest: Sha256::digest(&self.evidence).into(),
            };
            state.products_completed = true;
            state.active = false;
            state.acknowledged = None;
            self.outcome = Some(outcome);
            return Ok(CrechePoll::Outcome(outcome));
        }
        let Some(command) = state.command.take() else {
            return Ok(CrechePoll::Pending);
        };
        if self.interactions >= self.maximum_interactions {
            state.active = false;
            state.acknowledged = None;
            drop(state);
            return self.terminal(TerminalReason::Make(CrecheMakeRefusal::BoundExhausted));
        }
        // The queued occurrence must still belong to the selected actual Mask
        // when the semantic owner consumes it, including renderer revocation.
        let custody = match channel.custody.try_lock() {
            Ok(custody) => custody,
            Err(TryLockError::WouldBlock) => {
                state.command = Some(command);
                return Ok(CrechePoll::Pending);
            }
            Err(TryLockError::Poisoned(_)) => return Err("Crèche Mask custody poisoned".into()),
        };
        let mut custody = custody;
        let current = state.face.as_ref().ok_or("Crèche current Face absent")?;
        let admitted = custody
            .validate_acknowledged(current, &command.show)
            .and_then(|()| {
                custody.validate_returned_interaction(current, &command.show, &command.interaction)
            });
        drop(custody);
        if admitted.is_err() {
            state.active = false;
            state.acknowledged = None;
            drop(state);
            return self.terminal(TerminalReason::InteractionRefused);
        }
        self.interactions += 1;
        let encounter = self
            .encounter
            .as_mut()
            .ok_or("Crèche review was already handed off")?;
        let basis = self
            .basis
            .as_ref()
            .ok_or("Crèche producing execution missing")?;
        let result = encounter.apply_interaction(
            basis,
            &command.show,
            &command.interaction,
            &self.catalog,
            &self.packages,
        );
        state.acknowledged = None;
        match result {
            Ok(DeploymentEncounterOutcome::Changed) => {
                state.face = Some(Arc::new(
                    encounter
                        .face(basis)
                        .map_err(|e| format!("Crèche changed Face: {e:?}"))?,
                ));
            }
            Ok(DeploymentEncounterOutcome::Reviewed(review)) => {
                state.reviewed = true;
                state.handoff = Some(CrecheReviewedHandoff {
                    request: self.request.unwrap(),
                    context: self
                        .context
                        .take()
                        .ok_or("Crèche handoff execution missing")?,
                    draft: self.encounter.take().unwrap().into_draft(),
                    review,
                });
                // No successful image outcome is inferred from canonical review.
            }
            Err(_) => {
                state.active = false;
                drop(state);
                return self.terminal(TerminalReason::InteractionRefused);
            }
        }
        Ok(CrechePoll::Pending)
    }
    fn retained_evidence(&self, outcome: &CrecheOutcome) -> Result<&[u8], String> {
        if self.outcome.as_ref() != Some(outcome) {
            return Err("Crèche outcome lacks retained canonical evidence".into());
        }
        Ok(&self.evidence)
    }
    fn cancel_and_drain(&mut self) -> Result<(), String> {
        if self.retired {
            return Ok(());
        }
        {
            let mut state = self.channel.lock()?;
            state.active = false;
            state.command = None;
            state.acknowledged = None;
        }
        self.channel
            .custody
            .lock()
            .map_err(|_| "Crèche Mask custody poisoned")?
            .retire_and_drain()?;
        if self.request.is_some() && self.outcome.is_none() {
            self.terminal(TerminalReason::Cancelled)?;
        }
        self.retired = true;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
