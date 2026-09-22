//! Bounded evidence and authority for adopting learned realizations.

use alloc::{string::String, vec::Vec};

use crate::ModelInvocationEvidence;

pub const MAXIMUM_LIFECYCLE_METRICS: usize = 32;
pub const MAXIMUM_HUMAN_ASSESSMENTS: usize = 16;
pub const MAXIMUM_LIFECYCLE_IDENTITY_BYTES: usize = 128;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LearnedRealizationIdentity {
    pub artifact: [u8; 32],
    pub checkpoint: Option<[u8; 32]>,
    pub signature: [u8; 32],
    pub provider: [u8; 32],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShadowResourceEnvelope {
    pub maximum_runs: u32,
    pub maximum_input_bytes: u64,
    pub maximum_output_bytes: u64,
    pub maximum_work_units: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShadowContract {
    pub identity: [u8; 32],
    pub subject_identity: [u8; 32],
    pub baseline: LearnedRealizationIdentity,
    pub candidate: LearnedRealizationIdentity,
    pub shared_input_set_identity: [u8; 32],
    pub resources: ShadowResourceEnvelope,
    /// A separately admitted observational route, never a consequential sink.
    pub candidate_output_route: String,
    pub protected_effect_routes: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShadowTerminal {
    Compared,
    Cancelled,
    Pressured,
    CandidateProviderLost,
    BaselineFailed,
    CandidateFailed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShadowRun {
    pub identity: [u8; 32],
    pub contract_identity: [u8; 32],
    pub shared_input_set_identity: [u8; 32],
    pub shared_input_identity: [u8; 32],
    pub baseline_evidence: ModelInvocationEvidence,
    pub candidate_evidence: ModelInvocationEvidence,
    pub baseline_provider_identity: [u8; 32],
    pub candidate_provider_identity: [u8; 32],
    pub baseline_output_identity: [u8; 32],
    pub candidate_output_identity: [u8; 32],
    pub consumed_input_bytes: u64,
    pub consumed_output_bytes: u64,
    pub consumed_work_units: u64,
    pub terminal: ShadowTerminal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvaluationMetric {
    pub identity: String,
    pub baseline_value_millionths: i64,
    pub candidate_value_millionths: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HumanAssessmentDisposition {
    SupportsCandidate,
    SupportsBaseline,
    Disagrees,
    Inconclusive,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HumanAssessment {
    pub evaluator_identity: [u8; 32],
    pub evidence_identity: [u8; 32],
    pub disposition: HumanAssessmentDisposition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EvaluationDisposition {
    Sufficient,
    Insufficient,
    Disagreement,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CandidateEvaluation {
    pub identity: [u8; 32],
    pub subject_identity: [u8; 32],
    pub suite_identity: [u8; 32],
    pub baseline: LearnedRealizationIdentity,
    pub candidate: LearnedRealizationIdentity,
    pub shared_input_set_identity: [u8; 32],
    pub shadow_run_identities: Vec<[u8; 32]>,
    pub metrics: Vec<EvaluationMetric>,
    pub human_assessments: Vec<HumanAssessment>,
    pub disposition: EvaluationDisposition,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromotionDecision {
    Approved,
    Denied,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromotionGrant {
    pub identity: [u8; 32],
    pub authority_identity: [u8; 32],
    pub subject_identity: [u8; 32],
    pub evaluation_identity: [u8; 32],
    pub candidate: LearnedRealizationIdentity,
    pub rollback_target: LearnedRealizationIdentity,
    pub valid_from_tick: u64,
    pub valid_until_tick: Option<u64>,
    pub decision: PromotionDecision,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromotionTerminal {
    Promoted,
    PreparationFailed,
    CommitUnknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromotionReceipt {
    pub identity: [u8; 32],
    pub grant_identity: [u8; 32],
    pub subject_identity: [u8; 32],
    pub before_plan_identity: [u8; 32],
    pub before_play_identity: Option<[u8; 32]>,
    pub after_plan_identity: Option<[u8; 32]>,
    pub after_play_identity: Option<[u8; 32]>,
    pub selected: LearnedRealizationIdentity,
    pub rollback_target: LearnedRealizationIdentity,
    pub terminal: PromotionTerminal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RollbackGrant {
    pub identity: [u8; 32],
    pub authority_identity: [u8; 32],
    pub subject_identity: [u8; 32],
    pub promotion_receipt_identity: [u8; 32],
    pub current: LearnedRealizationIdentity,
    pub target: LearnedRealizationIdentity,
    pub maximum_attempts: u16,
    pub valid_until_tick: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RollbackTerminal {
    RolledBack,
    Unavailable,
    Refused,
    Failed,
    CommitUnknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RollbackReceipt {
    pub identity: [u8; 32],
    pub grant_identity: [u8; 32],
    pub subject_identity: [u8; 32],
    pub before_plan_identity: [u8; 32],
    pub after_plan_identity: Option<[u8; 32]>,
    pub attempt: u16,
    pub selected: Option<LearnedRealizationIdentity>,
    pub terminal: RollbackTerminal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LearnedLifecycleRefusal {
    InvalidIdentity,
    SameRealization,
    UnboundedResources,
    EffectfulShadowRoute,
    WrongContract,
    WrongInput,
    WrongRealization,
    ResourceBoundExceeded,
    InvalidEvidence,
    InvalidEvaluation,
    EvidenceInsufficient,
    ApprovalDenied,
    GrantNotYetValid,
    GrantExpired,
    StaleCandidate,
    IncompatibleSignature,
    InvalidPlanTransition,
    RollbackTargetMismatch,
    AttemptLimitExceeded,
}

impl LearnedRealizationIdentity {
    pub fn validate(&self) -> Result<(), LearnedLifecycleRefusal> {
        if self.artifact == [0; 32]
            || self.signature == [0; 32]
            || self.provider == [0; 32]
            || self.checkpoint.is_some_and(|value| value == [0; 32])
        {
            return Err(LearnedLifecycleRefusal::InvalidIdentity);
        }
        Ok(())
    }

    fn matches_evidence(&self, evidence: &ModelInvocationEvidence) -> bool {
        self.artifact == evidence.artifact_identity
            && self.checkpoint == evidence.checkpoint_identity
            && self.signature == evidence.signature_identity
    }
}

impl ShadowContract {
    pub fn validate(&self) -> Result<(), LearnedLifecycleRefusal> {
        nonzero(self.identity)?;
        nonzero(self.subject_identity)?;
        nonzero(self.subject_identity)?;
        nonzero(self.shared_input_set_identity)?;
        self.baseline.validate()?;
        self.candidate.validate()?;
        if self.baseline == self.candidate {
            return Err(LearnedLifecycleRefusal::SameRealization);
        }
        if self.baseline.signature != self.candidate.signature {
            return Err(LearnedLifecycleRefusal::IncompatibleSignature);
        }
        if self.resources.maximum_runs == 0
            || self.resources.maximum_input_bytes == 0
            || self.resources.maximum_output_bytes == 0
            || self.resources.maximum_work_units == 0
        {
            return Err(LearnedLifecycleRefusal::UnboundedResources);
        }
        text(&self.candidate_output_route)?;
        if self.protected_effect_routes.len() > 32
            || self
                .protected_effect_routes
                .iter()
                .any(|route| text(route).is_err())
            || self
                .protected_effect_routes
                .contains(&self.candidate_output_route)
        {
            return Err(LearnedLifecycleRefusal::EffectfulShadowRoute);
        }
        Ok(())
    }

    pub fn validate_run(&self, run: &ShadowRun) -> Result<(), LearnedLifecycleRefusal> {
        self.validate()?;
        nonzero(run.identity)?;
        if run.contract_identity != self.identity {
            return Err(LearnedLifecycleRefusal::WrongContract);
        }
        if run.shared_input_set_identity != self.shared_input_set_identity
            || run.shared_input_identity == [0; 32]
            || run.baseline_evidence.input_identities != run.candidate_evidence.input_identities
            || !run
                .baseline_evidence
                .input_identities
                .contains(&run.shared_input_identity)
        {
            return Err(LearnedLifecycleRefusal::WrongInput);
        }
        if !self.baseline.matches_evidence(&run.baseline_evidence)
            || !self.candidate.matches_evidence(&run.candidate_evidence)
            || run.baseline_provider_identity != self.baseline.provider
            || run.candidate_provider_identity != self.candidate.provider
        {
            return Err(LearnedLifecycleRefusal::WrongRealization);
        }
        run.baseline_evidence
            .validate()
            .and_then(|()| run.candidate_evidence.validate())
            .map_err(|_| LearnedLifecycleRefusal::InvalidEvidence)?;
        if run.baseline_output_identity == [0; 32]
            || run.candidate_output_identity == [0; 32]
            || run.consumed_input_bytes == 0
            || run.consumed_output_bytes == 0
            || run.consumed_work_units == 0
            || run.consumed_input_bytes > self.resources.maximum_input_bytes
            || run.consumed_output_bytes > self.resources.maximum_output_bytes
            || run.consumed_work_units > self.resources.maximum_work_units
        {
            return Err(LearnedLifecycleRefusal::ResourceBoundExceeded);
        }
        Ok(())
    }
}

impl CandidateEvaluation {
    pub fn validate(&self, contract: &ShadowContract) -> Result<(), LearnedLifecycleRefusal> {
        nonzero(self.identity)?;
        nonzero(self.suite_identity)?;
        if self.subject_identity != contract.subject_identity
            || self.baseline != contract.baseline
            || self.candidate != contract.candidate
            || self.shared_input_set_identity != contract.shared_input_set_identity
            || self.shadow_run_identities.is_empty()
            || self.shadow_run_identities.len() > contract.resources.maximum_runs as usize
            || self.shadow_run_identities.contains(&[0; 32])
            || self.metrics.is_empty()
            || self.metrics.len() > MAXIMUM_LIFECYCLE_METRICS
            || self.human_assessments.len() > MAXIMUM_HUMAN_ASSESSMENTS
            || self
                .metrics
                .iter()
                .any(|metric| text(&metric.identity).is_err())
            || self.human_assessments.iter().any(|assessment| {
                assessment.evaluator_identity == [0; 32] || assessment.evidence_identity == [0; 32]
            })
        {
            return Err(LearnedLifecycleRefusal::InvalidEvaluation);
        }
        Ok(())
    }
}

impl PromotionGrant {
    pub fn admit(
        &self,
        evaluation: &CandidateEvaluation,
        now_tick: u64,
    ) -> Result<(), LearnedLifecycleRefusal> {
        nonzero(self.identity)?;
        nonzero(self.authority_identity)?;
        nonzero(self.subject_identity)?;
        self.candidate.validate()?;
        self.rollback_target.validate()?;
        if self.decision != PromotionDecision::Approved {
            return Err(LearnedLifecycleRefusal::ApprovalDenied);
        }
        if evaluation.disposition != EvaluationDisposition::Sufficient {
            return Err(LearnedLifecycleRefusal::EvidenceInsufficient);
        }
        if self.subject_identity != evaluation.subject_identity
            || self.evaluation_identity != evaluation.identity
            || self.candidate != evaluation.candidate
            || self.rollback_target != evaluation.baseline
        {
            return Err(LearnedLifecycleRefusal::StaleCandidate);
        }
        if now_tick < self.valid_from_tick {
            return Err(LearnedLifecycleRefusal::GrantNotYetValid);
        }
        if self.valid_until_tick.is_some_and(|until| now_tick > until) {
            return Err(LearnedLifecycleRefusal::GrantExpired);
        }
        Ok(())
    }
}

impl PromotionReceipt {
    pub fn validate(&self, grant: &PromotionGrant) -> Result<(), LearnedLifecycleRefusal> {
        nonzero(self.identity)?;
        if grant.decision != PromotionDecision::Approved {
            return Err(LearnedLifecycleRefusal::ApprovalDenied);
        }
        if self.grant_identity != grant.identity
            || self.subject_identity != grant.subject_identity
            || self.selected != grant.candidate
            || self.rollback_target != grant.rollback_target
        {
            return Err(LearnedLifecycleRefusal::WrongRealization);
        }
        if self.terminal == PromotionTerminal::Promoted
            && (self.after_plan_identity.is_none()
                || self.after_plan_identity == Some(self.before_plan_identity))
        {
            return Err(LearnedLifecycleRefusal::InvalidPlanTransition);
        }
        Ok(())
    }
}

impl RollbackGrant {
    pub fn admit(
        &self,
        promotion: &PromotionReceipt,
        now_tick: u64,
    ) -> Result<(), LearnedLifecycleRefusal> {
        nonzero(self.identity)?;
        nonzero(self.authority_identity)?;
        if self.maximum_attempts == 0 {
            return Err(LearnedLifecycleRefusal::UnboundedResources);
        }
        if self.subject_identity != promotion.subject_identity
            || self.promotion_receipt_identity != promotion.identity
            || self.current != promotion.selected
            || self.target != promotion.rollback_target
        {
            return Err(LearnedLifecycleRefusal::RollbackTargetMismatch);
        }
        if promotion.terminal != PromotionTerminal::Promoted {
            return Err(LearnedLifecycleRefusal::InvalidPlanTransition);
        }
        if self.current.signature != self.target.signature {
            return Err(LearnedLifecycleRefusal::IncompatibleSignature);
        }
        if self.valid_until_tick.is_some_and(|until| now_tick > until) {
            return Err(LearnedLifecycleRefusal::GrantExpired);
        }
        Ok(())
    }
}

impl RollbackReceipt {
    pub fn validate(&self, grant: &RollbackGrant) -> Result<(), LearnedLifecycleRefusal> {
        nonzero(self.identity)?;
        if self.grant_identity != grant.identity || self.subject_identity != grant.subject_identity
        {
            return Err(LearnedLifecycleRefusal::RollbackTargetMismatch);
        }
        if self.attempt == 0 || self.attempt > grant.maximum_attempts {
            return Err(LearnedLifecycleRefusal::AttemptLimitExceeded);
        }
        if self.terminal == RollbackTerminal::RolledBack
            && (self.selected != Some(grant.target)
                || self.after_plan_identity.is_none()
                || self.after_plan_identity == Some(self.before_plan_identity))
        {
            return Err(LearnedLifecycleRefusal::InvalidPlanTransition);
        }
        Ok(())
    }
}

fn nonzero(value: [u8; 32]) -> Result<(), LearnedLifecycleRefusal> {
    (value != [0; 32])
        .then_some(())
        .ok_or(LearnedLifecycleRefusal::InvalidIdentity)
}

fn text(value: &str) -> Result<(), LearnedLifecycleRefusal> {
    (!value.is_empty() && value.len() <= MAXIMUM_LIFECYCLE_IDENTITY_BYTES)
        .then_some(())
        .ok_or(LearnedLifecycleRefusal::InvalidIdentity)
}
