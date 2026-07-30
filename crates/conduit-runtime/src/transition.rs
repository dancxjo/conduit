//! Hosted execution of the bounded plan-transition transaction.
//!
//! The portable controller owns phase legality and normative evidence. This
//! adapter drives concrete old/candidate generations, a stable-boundary
//! router, caller-owned state storage, and an explicit retained-event
//! provider. It does not discover candidates, provision hosts, fetch
//! artifacts, or keep a private replay history.

use std::fmt;

use conduit_core::{
    AdministrativeProof, ArtifactDigest, AuthorityTime, ContainmentContext, ContainmentReason,
    EventClass, HazardClosureContext, HazardClosurePolicy, HazardClosureReason, HazardPermit,
    HazardProofNode, HazardousHostBinding, Id, InhibitReason, PersistentBudgetLedger,
    PersistentBudgetPolicy, PinnedDescriptor, PlanEpoch, PolicyBudgetReason, PolicyBudgetRequest,
    PolicyBudgetStatus, PolicyReservation, ReplacementSupport, ResonanceEnvelope, ResonanceError,
    SemanticHash, TransitionAdmissionProofs, TransitionContract, TransitionController,
    TransitionDrainObservation, TransitionEffectClosure, TransitionEvidence, TransitionPhase,
    TransitionReason, TransitionReplayContract, TransitionReplayObservation,
    TransitionStateContract, TransitionUsage, analyze_transition_effect_closure,
    validate_effect_containment, validate_envelope, validate_hazardous_host_binding,
    validate_policy_budget_status, validate_replacement_support,
};
use sha2::{Digest, Sha256};

use crate::{ResolvedPlacement, RuntimeValueEnvelope};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HostedGenerationBinding<'a> {
    pub epoch: PlanEpoch,
    pub implementation: PinnedDescriptor<'a>,
    pub artifact: ArtifactDigest,
    pub replacement: ReplacementSupport<'a>,
}

pub type HostedDrainObservation = TransitionDrainObservation;

/// A concrete generation behind one exact implementation/artifact binding.
///
/// State and replay bytes are supplied through caller-owned bounded buffers.
/// Implementations cannot substitute an ambient queue or callback history for
/// those explicit inputs.
pub trait HostedTransitionGeneration {
    fn binding(&self) -> HostedGenerationBinding<'_>;

    fn prepare(&mut self) -> Result<(), Id<'static>>;

    fn stop_admission(&mut self, boundary: PinnedDescriptor<'_>) -> Result<(), Id<'static>>;

    fn drain(
        &mut self,
        boundary: PinnedDescriptor<'_>,
    ) -> Result<HostedDrainObservation, Id<'static>>;

    fn export_state(
        &mut self,
        contract: TransitionStateContract<'_>,
        output: &mut [u8],
    ) -> Result<usize, Id<'static>>;

    fn import_state(
        &mut self,
        contract: TransitionStateContract<'_>,
        input: &[u8],
    ) -> Result<usize, Id<'static>>;

    fn accept_replayed_value(
        &mut self,
        cursor: u64,
        value: &[u8],
        envelope: Option<RuntimeValueEnvelope>,
        redelivered: bool,
    ) -> Result<(), Id<'static>>;

    fn retire(&mut self) -> Result<(), Id<'static>>;

    fn abort_candidate(&mut self) -> Result<(), Id<'static>>;

    fn restore_old(&mut self) -> Result<(), Id<'static>>;
}

/// Stable external endpoint routing is separate from either generation.
///
/// Implementations must make each method atomic: an error leaves the prior
/// binding authoritative.
pub trait StableBoundaryRouter {
    /// Atomically route only new boundary admissions to the prepared
    /// candidate while already-admitted work remains pinned to the old
    /// generation.
    fn begin_handoff(
        &mut self,
        subject: &str,
        boundary: PinnedDescriptor<'_>,
        old: PlanEpoch,
        candidate: PlanEpoch,
    ) -> Result<(), Id<'static>>;

    /// Atomically finalize the stable boundary after old work reaches an
    /// exact disposition. This is still pre-commit: the controller changes
    /// authoritative epoch only in its subsequent commit operation.
    fn rebind(
        &mut self,
        subject: &str,
        boundary: PinnedDescriptor<'_>,
        old: PlanEpoch,
        candidate: PlanEpoch,
    ) -> Result<(), Id<'static>>;

    fn restore(
        &mut self,
        subject: &str,
        boundary: PinnedDescriptor<'_>,
        old: PlanEpoch,
        candidate: PlanEpoch,
    ) -> Result<(), Id<'static>>;
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RetainedReplayItem {
    pub cursor: u64,
    pub bytes: usize,
    pub redelivered: bool,
    pub gap: bool,
    pub value_envelope: Option<RuntimeValueEnvelope>,
}

/// Explicit #79 retained-event source. The transaction never owns hidden
/// history and never asks an implementation to reconstruct it implicitly.
pub trait RetainedReplayProvider {
    fn stream(&self) -> PinnedDescriptor<'_>;
    fn stream_epoch(&self) -> u64;
    fn first_cursor(&self) -> u64;

    fn next(&mut self, output: &mut [u8]) -> Result<Option<RetainedReplayItem>, Id<'static>>;
}

/// Exact external facts consumed before a transition may reserve or disturb
/// either generation.
#[derive(Clone, Copy)]
pub struct HostedTransitionAdmission<'facts, 'a> {
    pub contract: TransitionContract<'a>,
    pub request: ResonanceEnvelope<'a>,
    pub decision: ResonanceEnvelope<'a>,
    pub effect_class: PinnedDescriptor<'a>,
    pub authorization: AdministrativeProof<'a>,
    pub containment: ContainmentContext<'a>,
    pub resolution: &'facts ResolvedPlacement,
    pub budget_policy: PersistentBudgetPolicy<'a>,
    pub budget_status: PolicyBudgetStatus<'a>,
    pub budget_request: PolicyBudgetRequest<'a>,
    pub budget_ledger_available: bool,
    pub hazard_policy: HazardClosurePolicy<'a>,
    pub effect_closure: TransitionEffectClosure<'a>,
    pub hazard_permits: &'a [HazardPermit<'a>],
    pub hazard_context: HazardClosureContext<'a>,
    pub inhibit: Option<HazardousHostBinding<'a>>,
    pub inhibit_required: bool,
    pub now: AuthorityTime<'a>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HostedTransitionReservation {
    proofs: TransitionAdmissionProofs,
    reservation: PolicyReservation,
    checkpoint: SemanticHash,
    valid_until_tick: u64,
}

impl HostedTransitionReservation {
    #[must_use]
    pub const fn proofs(self) -> TransitionAdmissionProofs {
        self.proofs
    }

    #[must_use]
    pub const fn reservation(self) -> PolicyReservation {
        self.reservation
    }

    #[must_use]
    pub const fn checkpoint(self) -> SemanticHash {
        self.checkpoint
    }

    #[must_use]
    pub const fn valid_until_tick(self) -> u64 {
        self.valid_until_tick
    }
}

/// Validate current control, independent authorization, exact candidate
/// resolution, durable budget, combined effect closure, and inhibit facts as
/// one admission transaction. The authoritative ledger is copied and is
/// replaced only after every validator succeeds.
pub fn admit_hosted_transition<'facts, 'a, const RESERVATIONS: usize>(
    admission: HostedTransitionAdmission<'facts, 'a>,
    ledger: &mut PersistentBudgetLedger<'a, RESERVATIONS>,
    hazard_proof: &mut [Option<HazardProofNode<'a>>],
    inhibit_scratch: &mut [SemanticHash],
) -> Result<HostedTransitionReservation, HostedTransitionAdmissionError> {
    validate_control_pair(admission)?;
    validate_admission_subject(admission)?;
    validate_effect_containment(
        admission.effect_class,
        core::slice::from_ref(&admission.effect_class),
        Some(admission.authorization),
        admission.containment,
    )
    .map_err(HostedTransitionAdmissionError::Containment)?;
    let candidate_binding = validate_candidate_resolution(admission)
        .ok_or(HostedTransitionAdmissionError::Resolution)?;
    validate_resolved_replacement(candidate_binding, admission.contract)
        .map_err(HostedTransitionAdmissionError::Replacement)?;
    validate_policy_budget_status(
        admission.budget_policy,
        admission.budget_status,
        admission.now,
        admission.budget_request.units,
    )
    .map_err(HostedTransitionAdmissionError::Budget)?;

    let mut candidate_ledger = *ledger;
    let (reservation, _) = candidate_ledger
        .reserve(
            admission.budget_request,
            admission.now,
            admission.budget_ledger_available,
        )
        .map_err(HostedTransitionAdmissionError::Budget)?;
    let hazard = analyze_transition_effect_closure(
        admission.hazard_policy,
        admission.effect_closure,
        admission.hazard_permits,
        admission.hazard_context,
        hazard_proof,
    )
    .map_err(|denial| HostedTransitionAdmissionError::Hazard(denial.reason))?;
    let inhibit_decision = match admission.inhibit {
        Some(binding) => {
            if binding.host.as_str() != candidate_binding.host {
                return Err(HostedTransitionAdmissionError::Inhibit(
                    InhibitReason::IdentityMismatch,
                ));
            }
            validate_hazardous_host_binding(
                binding,
                admission.now.basis,
                admission.now.tick,
                inhibit_scratch,
            )
            .map_err(HostedTransitionAdmissionError::Inhibit)?;
            inhibit_decision_identity(admission.contract.identity, Some(binding))
        }
        None if admission.inhibit_required => {
            return Err(HostedTransitionAdmissionError::Inhibit(
                InhibitReason::ObservationAbsentOrStale,
            ));
        }
        None => inhibit_decision_identity(admission.contract.identity, None),
    };
    let checkpoint = candidate_ledger.checkpoint().checkpoint;
    let valid_until_tick = admission_valid_until(admission, candidate_binding);
    let value = HostedTransitionReservation {
        proofs: TransitionAdmissionProofs {
            request: admission.request.integrity,
            decision: admission.decision.integrity,
            authorization: admission.authorization.execution.identity,
            candidate_resolution: admission.resolution.computed_identity(),
            persistent_budget_status: admission.budget_status.identity,
            hazard_closure: hazard.decision_identity,
            inhibit_decision,
        },
        reservation,
        checkpoint,
        valid_until_tick,
    };
    *ledger = candidate_ledger;
    Ok(value)
}

fn admission_valid_until(
    admission: HostedTransitionAdmission<'_, '_>,
    candidate: &crate::ResolvedPlacementBinding,
) -> u64 {
    let mut valid_until = candidate
        .report_valid_until_tick
        .min(admission.budget_status.valid_until_tick)
        .min(admission.authorization.proposal.expires_at_tick)
        .min(admission.authorization.execution.expires_at_tick);
    for approval in admission.authorization.approvals {
        valid_until = valid_until.min(approval.expires_at_tick);
    }
    for permit in admission.hazard_permits {
        valid_until = valid_until.min(permit.expires_at_tick);
    }
    if let Some(inhibit) = admission.inhibit {
        valid_until = valid_until.min(inhibit.observation.valid_until_tick);
    }
    valid_until
}

fn validate_resolved_replacement(
    binding: &crate::ResolvedPlacementBinding,
    contract: TransitionContract<'_>,
) -> Result<(), TransitionReason> {
    let support = match &binding.replacement {
        crate::ResolvedReplacementSupport::Cold => ReplacementSupport::Cold,
        crate::ResolvedReplacementSupport::Quiescent {
            boundary_id,
            boundary_schema_version,
            boundary_identity,
            maximum_ticks,
        } => ReplacementSupport::Quiescent {
            boundary: PinnedDescriptor {
                id: Id(boundary_id),
                schema_version: *boundary_schema_version,
                semantic_hash: *boundary_identity,
            },
            maximum_ticks: *maximum_ticks,
        },
        crate::ResolvedReplacementSupport::Stateful {
            state_contract_id,
            state_contract_schema_version,
            state_contract_identity,
            maximum_export_bytes,
            maximum_import_bytes,
            maximum_ticks,
        } => ReplacementSupport::Stateful {
            state_contract: PinnedDescriptor {
                id: Id(state_contract_id),
                schema_version: *state_contract_schema_version,
                semantic_hash: *state_contract_identity,
            },
            maximum_export_bytes: *maximum_export_bytes,
            maximum_import_bytes: *maximum_import_bytes,
            maximum_ticks: *maximum_ticks,
        },
    };
    validate_replacement_support(support, contract)
}

fn validate_control_pair(
    admission: HostedTransitionAdmission<'_, '_>,
) -> Result<(), HostedTransitionAdmissionError> {
    validate_envelope(&admission.request).map_err(HostedTransitionAdmissionError::Resonance)?;
    validate_envelope(&admission.decision).map_err(HostedTransitionAdmissionError::Resonance)?;
    if admission.request.class != EventClass::Control
        || admission.decision.class != EventClass::Control
        || admission.request.plan_epoch != admission.contract.old.plan
        || admission.decision.plan_epoch != admission.contract.old.plan
        || admission.request.subject != admission.contract.stable_subject
        || admission.decision.subject != admission.contract.stable_subject
        || admission.request.correlation.is_none()
        || admission.request.correlation != admission.decision.correlation
        || admission.decision.relations.caused_by != Some(admission.request.event)
        || admission.request.integrity == admission.decision.integrity
    {
        return Err(HostedTransitionAdmissionError::ControlMismatch);
    }
    Ok(())
}

fn validate_admission_subject(
    admission: HostedTransitionAdmission<'_, '_>,
) -> Result<(), HostedTransitionAdmissionError> {
    let subject = admission.containment.subject;
    if subject.plan != admission.contract.candidate.plan
        || subject.epoch != admission.contract.candidate.epoch
        || subject.artifact != Some(admission.contract.candidate_artifact)
        || admission.containment.time_basis != admission.now.basis
        || admission.containment.now_tick != admission.now.tick
        || admission.budget_request.correlation != admission.request.integrity
        || admission.budget_request.consumer.plan != admission.contract.candidate.plan
        || admission.budget_request.consumer.epoch != admission.contract.candidate.epoch
        || admission.budget_request.policy_identity != admission.budget_policy.identity
    {
        return Err(HostedTransitionAdmissionError::SubjectMismatch);
    }
    Ok(())
}

fn validate_candidate_resolution<'a>(
    admission: HostedTransitionAdmission<'a, '_>,
) -> Option<&'a crate::ResolvedPlacementBinding> {
    admission.resolution.bindings.iter().find(|binding| {
        binding.instance == admission.contract.stable_subject.as_str()
            && binding.implementation_id == admission.contract.candidate_implementation.id.as_str()
            && binding.implementation_identity
                == admission.contract.candidate_implementation.semantic_hash
            && binding
                .artifacts
                .iter()
                .any(|(_, artifact)| *artifact == admission.contract.candidate_artifact)
            && binding.report_time_basis == admission.now.basis.as_str()
            && binding.report_observed_at_tick <= admission.now.tick
            && admission.now.tick < binding.report_valid_until_tick
    })
}

fn inhibit_decision_identity(
    transition: SemanticHash,
    binding: Option<HazardousHostBinding<'_>>,
) -> SemanticHash {
    let mut digest = Sha256::new();
    digest.update(b"conduit/transition-inhibit-decision/v1");
    digest.update(transition.as_bytes());
    if let Some(binding) = binding {
        digest.update([1]);
        digest.update(binding.host.as_str().as_bytes());
        digest.update(binding.profile.identity.as_bytes());
        digest.update(binding.observation.identity.as_bytes());
    } else {
        digest.update([0]);
    }
    SemanticHash::from_bytes(digest.finalize().into())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostedTransitionAdmissionError {
    Resonance(ResonanceError),
    ControlMismatch,
    SubjectMismatch,
    Containment(ContainmentReason),
    Resolution,
    Replacement(TransitionReason),
    Budget(PolicyBudgetReason),
    Hazard(HazardClosureReason),
    Inhibit(InhibitReason),
}

impl HostedTransitionAdmissionError {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Resonance(error) => error.code(),
            Self::ControlMismatch => "CND-TRN-ADM-001",
            Self::SubjectMismatch => "CND-TRN-ADM-002",
            Self::Containment(reason) => reason.code(),
            Self::Resolution => "CND-TRN-ADM-003",
            Self::Replacement(reason) => reason.code(),
            Self::Budget(reason) => reason.code(),
            Self::Hazard(reason) => reason.code(),
            Self::Inhibit(reason) => reason.code(),
        }
    }
}

pub struct HostedTransitionTransaction<'a, Old, Candidate, Router, const EVIDENCE: usize> {
    controller: TransitionController<'a, EVIDENCE>,
    old: Old,
    candidate: Candidate,
    router: Router,
    budget_reservation: Option<SemanticHash>,
    admission_valid_until_tick: Option<u64>,
}

impl<'a, Old, Candidate, Router, const EVIDENCE: usize>
    HostedTransitionTransaction<'a, Old, Candidate, Router, EVIDENCE>
where
    Old: HostedTransitionGeneration,
    Candidate: HostedTransitionGeneration,
    Router: StableBoundaryRouter,
{
    pub fn new(
        contract: TransitionContract<'a>,
        current: PlanEpoch,
        old: Old,
        candidate: Candidate,
        router: Router,
        tick: u64,
        scratch: &mut [SemanticHash],
    ) -> Result<Self, HostedTransitionError> {
        let old_binding = old.binding();
        let candidate_binding = candidate.binding();
        if old_binding.epoch != contract.old
            || old_binding.implementation != contract.old_implementation
            || old_binding.artifact != contract.old_artifact
            || candidate_binding.epoch != contract.candidate
            || candidate_binding.implementation != contract.candidate_implementation
            || candidate_binding.artifact != contract.candidate_artifact
        {
            return Err(HostedTransitionError::BindingMismatch);
        }
        validate_replacement_support(old_binding.replacement, contract)
            .map_err(HostedTransitionError::Contract)?;
        validate_replacement_support(candidate_binding.replacement, contract)
            .map_err(HostedTransitionError::Contract)?;
        let controller = TransitionController::new(contract, current, tick, scratch)
            .map_err(HostedTransitionError::Contract)?;
        Ok(Self {
            controller,
            old,
            candidate,
            router,
            budget_reservation: None,
            admission_valid_until_tick: None,
        })
    }

    #[must_use]
    pub const fn phase(&self) -> TransitionPhase {
        self.controller.phase()
    }

    #[must_use]
    pub const fn active_epoch(&self) -> PlanEpoch {
        self.controller.active_epoch()
    }

    #[must_use]
    pub fn evidence(&self) -> &[Option<TransitionEvidence<'a>>] {
        self.controller.evidence()
    }

    #[must_use]
    pub const fn contract(&self) -> TransitionContract<'a> {
        self.controller.contract()
    }

    pub fn reserve(
        &mut self,
        admission: HostedTransitionReservation,
        usage: TransitionUsage,
        tick: u64,
    ) -> Result<(), HostedTransitionError> {
        if self.budget_reservation.is_some() {
            return Err(HostedTransitionError::BudgetReservationMismatch);
        }
        if tick >= admission.valid_until_tick {
            return Err(HostedTransitionError::AdmissionExpired);
        }
        self.controller
            .reserve(admission.proofs, usage, tick)
            .map_err(HostedTransitionError::Contract)?;
        self.budget_reservation = Some(admission.reservation.identity);
        self.admission_valid_until_tick = Some(admission.valid_until_tick);
        Ok(())
    }

    pub fn prepare(&mut self, tick: u64) -> Result<(), HostedTransitionError> {
        self.require_fresh_admission(tick)?;
        self.controller
            .preflight_prepared(tick)
            .map_err(HostedTransitionError::Contract)?;
        self.candidate
            .prepare()
            .map_err(HostedTransitionError::Generation)?;
        self.controller
            .prepared(tick)
            .map_err(HostedTransitionError::Contract)
    }

    pub fn barrier(&mut self, tick: u64) -> Result<(), HostedTransitionError> {
        self.require_fresh_admission(tick)?;
        let boundary = self.controller.contract().boundary;
        self.controller
            .preflight_barrier(boundary, tick)
            .map_err(HostedTransitionError::Contract)?;
        let contract = self.controller.contract();
        self.old
            .stop_admission(boundary)
            .map_err(HostedTransitionError::Generation)?;
        self.router
            .begin_handoff(
                contract.stable_subject.as_str(),
                boundary,
                contract.old,
                contract.candidate,
            )
            .map_err(HostedTransitionError::Router)?;
        self.controller
            .barrier(boundary, tick)
            .map_err(HostedTransitionError::Contract)
    }

    pub fn drain(&mut self, tick: u64) -> Result<HostedDrainObservation, HostedTransitionError> {
        self.require_fresh_admission(tick)?;
        self.controller
            .preflight_drain(tick)
            .map_err(HostedTransitionError::Contract)?;
        let observation = self
            .old
            .drain(self.controller.contract().boundary)
            .map_err(HostedTransitionError::Generation)?;
        self.controller
            .drained(observation, tick)
            .map_err(HostedTransitionError::Contract)?;
        Ok(observation)
    }

    /// Export and import through one caller-owned bounded scratch region.
    /// Sensitive bytes are overwritten before this method returns.
    pub fn transfer_state(
        &mut self,
        scratch: &mut [u8],
        tick: u64,
    ) -> Result<(), HostedTransitionError> {
        self.require_fresh_admission(tick)?;
        let state = self
            .controller
            .contract()
            .state
            .ok_or(HostedTransitionError::Contract(
                TransitionReason::StateContractMismatch,
            ))?;
        self.controller
            .preflight_transfer_state(state.descriptor, 0, 0, tick)
            .map_err(HostedTransitionError::Contract)?;
        let export_limit = usize::try_from(state.maximum_export_bytes)
            .map_err(|_| HostedTransitionError::StateBufferTooSmall)?;
        if scratch.len() < export_limit {
            return Err(HostedTransitionError::StateBufferTooSmall);
        }
        let exported = self
            .old
            .export_state(state, &mut scratch[..export_limit])
            .map_err(HostedTransitionError::Generation)?;
        if exported > export_limit {
            scratch.fill(0);
            return Err(HostedTransitionError::StateBoundsViolated);
        }
        let imported = match self.candidate.import_state(state, &scratch[..exported]) {
            Ok(imported) => imported,
            Err(error) => {
                scratch.fill(0);
                return Err(HostedTransitionError::Generation(error));
            }
        };
        scratch.fill(0);
        if u64::try_from(imported)
            .ok()
            .is_none_or(|bytes| bytes > state.maximum_import_bytes)
        {
            return Err(HostedTransitionError::StateBoundsViolated);
        }
        self.controller
            .transfer_state(
                state.descriptor,
                u64::try_from(exported).map_err(|_| HostedTransitionError::StateBoundsViolated)?,
                u64::try_from(imported).map_err(|_| HostedTransitionError::StateBoundsViolated)?,
                tick,
            )
            .map_err(HostedTransitionError::Contract)
    }

    pub fn replay<P: RetainedReplayProvider>(
        &mut self,
        provider: &mut P,
        item_buffer: &mut [u8],
        tick: u64,
    ) -> Result<(u32, u64), HostedTransitionError> {
        self.require_fresh_admission(tick)?;
        let replay = self
            .controller
            .contract()
            .replay
            .ok_or(HostedTransitionError::Contract(
                TransitionReason::ReplayContractMismatch,
            ))?;
        self.controller
            .preflight_replayed(
                TransitionReplayObservation {
                    stream: replay.stream,
                    stream_epoch: replay.stream_epoch,
                    first_cursor: replay.first_cursor,
                    items: 0,
                    bytes: 0,
                    duplicate_items: 0,
                    gap: false,
                },
                tick,
            )
            .map_err(HostedTransitionError::Contract)?;
        validate_replay_binding(replay, provider)?;
        if item_buffer.is_empty() {
            return Err(HostedTransitionError::ReplayBufferTooSmall);
        }
        let mut items = 0_u32;
        let mut bytes = 0_u64;
        let mut duplicate_items = 0_u32;
        let mut expected_cursor = replay.first_cursor;
        let mut gap = false;
        while items < replay.maximum_items {
            let Some(item) = provider
                .next(item_buffer)
                .map_err(HostedTransitionError::ReplayProvider)?
            else {
                break;
            };
            if item.bytes > item_buffer.len()
                || (!item.redelivered && item.cursor != expected_cursor)
                || (item.redelivered && !replay.duplicates_permitted)
            {
                return Err(HostedTransitionError::ReplaySequenceInvalid);
            }
            if item.gap {
                gap = true;
                break;
            }
            if item.redelivered {
                duplicate_items = duplicate_items
                    .checked_add(1)
                    .ok_or(HostedTransitionError::ReplayBoundsViolated)?;
            }
            self.candidate
                .accept_replayed_value(
                    item.cursor,
                    &item_buffer[..item.bytes],
                    item.value_envelope,
                    item.redelivered,
                )
                .map_err(HostedTransitionError::Generation)?;
            items = items
                .checked_add(1)
                .ok_or(HostedTransitionError::ReplayBoundsViolated)?;
            bytes = bytes
                .checked_add(
                    u64::try_from(item.bytes)
                        .map_err(|_| HostedTransitionError::ReplayBoundsViolated)?,
                )
                .ok_or(HostedTransitionError::ReplayBoundsViolated)?;
            if bytes > replay.maximum_bytes {
                return Err(HostedTransitionError::ReplayBoundsViolated);
            }
            if !item.redelivered {
                expected_cursor = expected_cursor
                    .checked_add(1)
                    .ok_or(HostedTransitionError::ReplaySequenceInvalid)?;
            }
        }
        self.controller
            .replayed(
                TransitionReplayObservation {
                    stream: replay.stream,
                    stream_epoch: replay.stream_epoch,
                    first_cursor: replay.first_cursor,
                    items,
                    bytes,
                    duplicate_items,
                    gap,
                },
                tick,
            )
            .map_err(HostedTransitionError::Contract)?;
        Ok((items, bytes))
    }

    pub fn rebind(&mut self, tick: u64) -> Result<(), HostedTransitionError> {
        self.require_fresh_admission(tick)?;
        self.controller
            .preflight_rebind(tick)
            .map_err(HostedTransitionError::Contract)?;
        let contract = self.controller.contract();
        self.router
            .rebind(
                contract.stable_subject.as_str(),
                contract.boundary,
                contract.old,
                contract.candidate,
            )
            .map_err(HostedTransitionError::Router)?;
        self.controller
            .rebind(tick)
            .map_err(HostedTransitionError::Contract)
    }

    pub fn commit<const RESERVATIONS: usize>(
        &mut self,
        ledger: &mut PersistentBudgetLedger<'a, RESERVATIONS>,
        tick: u64,
    ) -> Result<(), HostedTransitionError> {
        self.require_fresh_admission(tick)?;
        self.controller
            .preflight_commit(tick)
            .map_err(HostedTransitionError::Contract)?;
        let reservation = self
            .budget_reservation
            .ok_or(HostedTransitionError::BudgetReservationMismatch)?;
        ledger
            .commit(reservation, tick)
            .map_err(HostedTransitionError::Budget)?;
        self.controller
            .commit(tick)
            .map_err(HostedTransitionError::Contract)
    }

    pub fn retire_old(&mut self, tick: u64) -> Result<(), HostedTransitionError> {
        self.controller
            .preflight_retire_old(tick)
            .map_err(HostedTransitionError::Contract)?;
        self.old
            .retire()
            .map_err(HostedTransitionError::Generation)?;
        self.controller
            .retire_old(tick)
            .map_err(HostedTransitionError::Contract)
    }

    pub fn complete(&mut self, tick: u64) -> Result<(), HostedTransitionError> {
        self.controller
            .complete(tick)
            .map_err(HostedTransitionError::Contract)
    }

    pub fn retry(&mut self, tick: u64) -> Result<(), HostedTransitionError> {
        if self.budget_reservation.is_some() {
            return Err(HostedTransitionError::BudgetReservationMismatch);
        }
        self.controller
            .retry(tick)
            .map_err(HostedTransitionError::Contract)
    }

    /// Deterministic rollback restores routing and the old generation before
    /// core records the old epoch as authoritative. A host-side restoration
    /// failure becomes terminal instead of manufacturing rollback evidence.
    pub fn rollback<const RESERVATIONS: usize>(
        &mut self,
        ledger: &mut PersistentBudgetLedger<'a, RESERVATIONS>,
        cause: SemanticHash,
        tick: u64,
    ) -> Result<(), HostedTransitionError> {
        self.controller
            .preflight_rollback(cause, tick)
            .map_err(HostedTransitionError::Contract)?;
        let contract = self.controller.contract();
        let restored = self
            .candidate
            .abort_candidate()
            .and_then(|()| {
                self.router.restore(
                    contract.stable_subject.as_str(),
                    contract.boundary,
                    contract.old,
                    contract.candidate,
                )
            })
            .and_then(|()| self.old.restore_old());
        if let Err(error) = restored {
            let _ = self.controller.terminal(cause, tick);
            return Err(HostedTransitionError::RollbackFailed(error));
        }
        let reservation = self
            .budget_reservation
            .ok_or(HostedTransitionError::BudgetReservationMismatch)?;
        if let Err(error) = ledger.release(reservation) {
            let _ = self.controller.terminal(cause, tick);
            return Err(HostedTransitionError::Budget(error));
        }
        self.controller
            .rollback(cause, tick)
            .map_err(HostedTransitionError::Contract)?;
        self.budget_reservation = None;
        self.admission_valid_until_tick = None;
        Ok(())
    }

    fn require_fresh_admission(&self, tick: u64) -> Result<(), HostedTransitionError> {
        if self
            .admission_valid_until_tick
            .is_none_or(|valid_until| tick >= valid_until)
        {
            Err(HostedTransitionError::AdmissionExpired)
        } else {
            Ok(())
        }
    }

    pub fn into_parts(self) -> (Old, Candidate, Router) {
        (self.old, self.candidate, self.router)
    }
}

fn validate_replay_binding(
    replay: TransitionReplayContract<'_>,
    provider: &impl RetainedReplayProvider,
) -> Result<(), HostedTransitionError> {
    if provider.stream() != replay.stream
        || provider.stream_epoch() != replay.stream_epoch
        || provider.first_cursor() != replay.first_cursor
    {
        Err(HostedTransitionError::ReplayBindingMismatch)
    } else {
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostedTransitionError {
    Contract(TransitionReason),
    BindingMismatch,
    Generation(Id<'static>),
    Router(Id<'static>),
    ReplayProvider(Id<'static>),
    StateBufferTooSmall,
    StateBoundsViolated,
    ReplayBufferTooSmall,
    ReplayBindingMismatch,
    ReplaySequenceInvalid,
    ReplayBoundsViolated,
    Budget(PolicyBudgetReason),
    BudgetReservationMismatch,
    AdmissionExpired,
    RollbackFailed(Id<'static>),
}

impl HostedTransitionError {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Contract(reason) => reason.code(),
            Self::BindingMismatch => "CND-TRN-HOST-001",
            Self::Generation(_) => "CND-TRN-HOST-002",
            Self::Router(_) => "CND-TRN-HOST-003",
            Self::ReplayProvider(_) => "CND-TRN-HOST-004",
            Self::StateBufferTooSmall | Self::StateBoundsViolated => "CND-TRN-HOST-005",
            Self::ReplayBufferTooSmall
            | Self::ReplayBindingMismatch
            | Self::ReplaySequenceInvalid
            | Self::ReplayBoundsViolated => "CND-TRN-HOST-006",
            Self::Budget(reason) => reason.code(),
            Self::BudgetReservationMismatch => "CND-TRN-HOST-007",
            Self::RollbackFailed(_) => "CND-TRN-HOST-008",
            Self::AdmissionExpired => "CND-TRN-HOST-009",
        }
    }
}

impl fmt::Display for HostedTransitionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.code())
    }
}

impl std::error::Error for HostedTransitionError {}
