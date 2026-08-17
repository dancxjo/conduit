//! Typed, finite model proposals separated from authority and effects.

use alloc::{format, string::String, vec::Vec};
use conduit_core::{KindId, PlanId, SignId, StructuredInfoValue};
use serde::{Deserialize, Serialize};

pub const MAXIMUM_EFFECT_ARGUMENT_BYTES: usize = 4_096;
pub const MAXIMUM_PROPOSAL_EVIDENCE: usize = 16;
pub const MAXIMUM_PROPOSAL_HISTORY: usize = 64;
pub const MAXIMUM_PROPOSAL_ID_BYTES: usize = 128;
pub const MAXIMUM_PROPOSAL_RATIONALE_BYTES: usize = 2_048;
pub const MAXIMUM_RESULTING_SIGNS: usize = 16;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProviderFunctionCall {
    pub function_name: String,
    pub canonical_arguments: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelEffectProposal {
    pub proposal_id: String,
    pub plan_id: PlanId,
    pub operation_kind: KindId,
    pub canonical_arguments: Vec<u8>,
    pub rationale: String,
    pub evidence: Vec<SignId>,
}

impl ModelEffectProposal {
    pub fn from_provider_call(
        proposal_id: String,
        plan_id: PlanId,
        call: ProviderFunctionCall,
        rationale: String,
        evidence: Vec<SignId>,
    ) -> Self {
        Self {
            proposal_id,
            plan_id,
            operation_kind: KindId::from(call.function_name),
            canonical_arguments: call.canonical_arguments,
            rationale,
            evidence,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectAuthority {
    pub authority_id: String,
    pub active_plan_id: PlanId,
    pub wired_operation_kind: KindId,
    pub argument_type_digest: [u8; 32],
    pub maximum_argument_bytes: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProposalRefusal {
    MissingAuthority,
    StalePlan,
    UnwiredOperation,
    ArgumentBoundExceeded,
    MalformedArguments,
    WrongArgumentType,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProposalDecisionOutcome {
    Authorized { request_id: String },
    Refused(ProposalRefusal),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProposalDecision {
    pub decision_id: String,
    pub proposal_id: String,
    pub authority_id: Option<String>,
    pub outcome: ProposalDecisionOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AuthorizedEffectRequest {
    pub request_id: String,
    pub proposal_id: String,
    pub decision_id: String,
    pub authority_id: String,
    pub plan_id: PlanId,
    pub operation_kind: KindId,
    pub canonical_arguments: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EffectReceipt {
    pub effect_id: String,
    pub request_id: String,
    pub resulting_signs: Vec<SignId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProposalDisposition {
    pub decision: ProposalDecision,
    pub request: Option<AuthorizedEffectRequest>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProposalGateError {
    InvalidAuthority,
    InvalidProposal,
    DuplicateProposal,
    DecisionHistoryFull,
    PendingRequestFull,
    UnknownRequest,
    CancelledRequest,
    StaleRequest,
    InvalidEffectReceipt,
    EffectHistoryFull,
}

pub struct ProposalGate {
    authority: Option<EffectAuthority>,
    maximum_history: usize,
    sequence: u64,
    decisions: Vec<ProposalDecision>,
    pending: Vec<AuthorizedEffectRequest>,
    cancelled: Vec<String>,
    effects: Vec<EffectReceipt>,
}

impl ProposalGate {
    pub fn new(
        authority: Option<EffectAuthority>,
        maximum_history: usize,
    ) -> Result<Self, ProposalGateError> {
        if maximum_history == 0
            || maximum_history > MAXIMUM_PROPOSAL_HISTORY
            || authority
                .as_ref()
                .is_some_and(|value| !valid_authority(value))
        {
            return Err(ProposalGateError::InvalidAuthority);
        }
        Ok(Self {
            authority,
            maximum_history,
            sequence: 0,
            decisions: Vec::with_capacity(maximum_history),
            pending: Vec::with_capacity(maximum_history),
            cancelled: Vec::with_capacity(maximum_history),
            effects: Vec::with_capacity(maximum_history),
        })
    }

    pub fn submit(
        &mut self,
        proposal: ModelEffectProposal,
    ) -> Result<ProposalDisposition, ProposalGateError> {
        if !valid_proposal(&proposal) {
            return Err(ProposalGateError::InvalidProposal);
        }
        if self
            .decisions
            .iter()
            .any(|decision| decision.proposal_id == proposal.proposal_id)
        {
            return Err(ProposalGateError::DuplicateProposal);
        }
        if self.decisions.len() == self.maximum_history {
            return Err(ProposalGateError::DecisionHistoryFull);
        }

        self.sequence = self.sequence.saturating_add(1);
        let decision_id = format!("decision/{}/{}", proposal.proposal_id, self.sequence);
        let refusal = self.evaluate(&proposal);
        let (outcome, request) = if let Some(refusal) = refusal {
            (ProposalDecisionOutcome::Refused(refusal), None)
        } else {
            if self.pending.len() == self.maximum_history {
                return Err(ProposalGateError::PendingRequestFull);
            }
            let authority = self.authority.as_ref().expect("evaluated authority");
            let request_id = format!("request/{}/{}", proposal.proposal_id, self.sequence);
            let request = AuthorizedEffectRequest {
                request_id: request_id.clone(),
                proposal_id: proposal.proposal_id.clone(),
                decision_id: decision_id.clone(),
                authority_id: authority.authority_id.clone(),
                plan_id: proposal.plan_id,
                operation_kind: proposal.operation_kind,
                canonical_arguments: proposal.canonical_arguments,
            };
            self.pending.push(request.clone());
            (
                ProposalDecisionOutcome::Authorized { request_id },
                Some(request),
            )
        };
        let decision = ProposalDecision {
            decision_id,
            proposal_id: proposal.proposal_id,
            authority_id: self
                .authority
                .as_ref()
                .map(|authority| authority.authority_id.clone()),
            outcome,
        };
        self.decisions.push(decision.clone());
        Ok(ProposalDisposition { decision, request })
    }

    pub fn replace_plan(&mut self, plan_id: PlanId) {
        if let Some(authority) = &mut self.authority {
            authority.active_plan_id = plan_id;
        }
    }

    pub fn cancel(&mut self, request_id: &str) -> Result<(), ProposalGateError> {
        let index = self
            .pending
            .iter()
            .position(|request| request.request_id == request_id)
            .ok_or(ProposalGateError::UnknownRequest)?;
        if self.cancelled.len() == self.maximum_history {
            return Err(ProposalGateError::EffectHistoryFull);
        }
        self.pending.remove(index);
        self.cancelled.push(request_id.into());
        Ok(())
    }

    pub fn complete(
        &mut self,
        request: &AuthorizedEffectRequest,
        effect_id: String,
        resulting_signs: Vec<SignId>,
    ) -> Result<EffectReceipt, ProposalGateError> {
        if self
            .cancelled
            .iter()
            .any(|value| value == &request.request_id)
        {
            return Err(ProposalGateError::CancelledRequest);
        }
        let index = self
            .pending
            .iter()
            .position(|pending| pending == request)
            .ok_or(ProposalGateError::UnknownRequest)?;
        if self
            .authority
            .as_ref()
            .is_none_or(|authority| authority.active_plan_id != request.plan_id)
        {
            return Err(ProposalGateError::StaleRequest);
        }
        if !valid_identity(&effect_id)
            || resulting_signs.len() > MAXIMUM_RESULTING_SIGNS
            || resulting_signs.iter().any(|sign| sign.as_str().is_empty())
            || has_duplicate_signs(&resulting_signs)
        {
            return Err(ProposalGateError::InvalidEffectReceipt);
        }
        if self.effects.len() == self.maximum_history {
            return Err(ProposalGateError::EffectHistoryFull);
        }
        let receipt = EffectReceipt {
            effect_id,
            request_id: request.request_id.clone(),
            resulting_signs,
        };
        self.pending.remove(index);
        self.effects.push(receipt.clone());
        Ok(receipt)
    }

    pub fn decisions(&self) -> &[ProposalDecision] {
        &self.decisions
    }

    pub fn effects(&self) -> &[EffectReceipt] {
        &self.effects
    }

    fn evaluate(&self, proposal: &ModelEffectProposal) -> Option<ProposalRefusal> {
        let Some(authority) = &self.authority else {
            return Some(ProposalRefusal::MissingAuthority);
        };
        if proposal.plan_id != authority.active_plan_id {
            return Some(ProposalRefusal::StalePlan);
        }
        if proposal.operation_kind != authority.wired_operation_kind {
            return Some(ProposalRefusal::UnwiredOperation);
        }
        if proposal.canonical_arguments.len() > authority.maximum_argument_bytes {
            return Some(ProposalRefusal::ArgumentBoundExceeded);
        }
        let Ok(arguments) =
            StructuredInfoValue::from_canonical_bytes(&proposal.canonical_arguments)
        else {
            return Some(ProposalRefusal::MalformedArguments);
        };
        if arguments.value_type().semantic_digest().ok() != Some(authority.argument_type_digest) {
            return Some(ProposalRefusal::WrongArgumentType);
        }
        None
    }
}

fn valid_authority(authority: &EffectAuthority) -> bool {
    valid_identity(&authority.authority_id)
        && !authority.active_plan_id.as_str().is_empty()
        && valid_identity(authority.wired_operation_kind.as_str())
        && authority.maximum_argument_bytes > 0
        && authority.maximum_argument_bytes <= MAXIMUM_EFFECT_ARGUMENT_BYTES
}

fn valid_proposal(proposal: &ModelEffectProposal) -> bool {
    valid_identity(&proposal.proposal_id)
        && !proposal.plan_id.as_str().is_empty()
        && valid_identity(proposal.operation_kind.as_str())
        && proposal.canonical_arguments.len() <= MAXIMUM_EFFECT_ARGUMENT_BYTES
        && proposal.rationale.len() <= MAXIMUM_PROPOSAL_RATIONALE_BYTES
        && proposal.evidence.len() <= MAXIMUM_PROPOSAL_EVIDENCE
        && !proposal
            .evidence
            .iter()
            .any(|sign| !valid_identity(sign.as_str()))
        && !has_duplicate_signs(&proposal.evidence)
}

fn valid_identity(value: &str) -> bool {
    !value.is_empty() && value.len() <= MAXIMUM_PROPOSAL_ID_BYTES
}

fn has_duplicate_signs(signs: &[SignId]) -> bool {
    signs
        .iter()
        .enumerate()
        .any(|(index, sign)| signs[index + 1..].iter().any(|candidate| candidate == sign))
}
