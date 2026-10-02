//! Native human-interaction meaning with bounded construction and state machinery.
//!
//! Conduitese owns every portable family, value, proposal, availability,
//! outcome and refusal. The remaining Rust contracts validate relationships,
//! retain canonical structured views, and advance bounded proposal state
//! (classes C/M). Presentation, renderer-local focus, manifestation,
//! application acceptance, and resulting state remain separate identities.

use crate::{
    BoundKind, InteractionApplicationOutcome, InteractionCanonicalBytes, InteractionFamily,
    InteractionProposalPayload, InteractionRefusal, InteractionTypeDigest, InteractionValue,
    InteractionValueKind, InteractionValues, OptionAvailability,
};
use alloc::{collections::VecDeque, string::String, vec::Vec};
use conduit_core::{KindId, QuantityUnit, StructuredInfoValue};

#[path = "human_interaction/canonical.rs"]
mod canonical;
#[path = "human_interaction/flow.rs"]
mod flow;
#[path = "human_interaction/validation.rs"]
mod validation;
use canonical::{encode_domain, encode_family, encode_value, field, identity, values};
pub use flow::*;
use validation::{
    validate_family, validate_identity, validate_outcome, validate_proposal, validate_state,
};

pub const TEXT_INFO_ID: &str = "value/text";
pub const MAXIMUM_INTERACTION_ID_BYTES: usize = 128;
pub const MAXIMUM_INTERACTION_VALUE_BYTES: usize = 65_536;
pub const MAXIMUM_INTERACTION_OPTIONS: usize = 256;
pub const MAXIMUM_INTERACTION_SELECTIONS: usize = 64;
pub const MAXIMUM_INTERACTION_QUEUE: usize = 8;

impl InteractionFamily {
    pub fn choice_one(value_kind: KindId, maximum_options: u16) -> Self {
        Self::choose_one(
            maximum_options,
            InteractionValueKind::new(value_kind.as_str().into())
                .expect("Kind identities are nonempty bounded text"),
        )
        .expect("Kind identities satisfy the native interaction contract")
    }

    pub fn choice_many(
        value_kind: KindId,
        maximum_options: u16,
        minimum_selections: u16,
        maximum_selections: u16,
    ) -> Self {
        Self::choose_many(
            maximum_options,
            maximum_selections,
            minimum_selections,
            InteractionValueKind::new(value_kind.as_str().into())
                .expect("Kind identities are nonempty bounded text"),
        )
        .expect("Kind identities satisfy the native interaction contract")
    }

    pub fn scalar_range(
        unit: QuantityUnit,
        minimum: i64,
        minimum_bound: BoundKind,
        maximum: i64,
        maximum_bound: BoundKind,
        granularity: i64,
    ) -> Self {
        Self::scalar(
            granularity,
            maximum,
            maximum_bound,
            minimum,
            minimum_bound,
            unit,
        )
        .expect("primitive scalar fields always have native representations")
    }

    pub fn relative_range(
        unit: QuantityUnit,
        minimum_delta: i64,
        maximum_delta: i64,
        granularity: i64,
    ) -> Self {
        Self::relative_adjustment(granularity, maximum_delta, minimum_delta, unit)
            .expect("primitive relative fields always have native representations")
    }

    pub fn text_value(maximum_bytes: u32, allow_empty: bool) -> Self {
        Self::text(allow_empty, maximum_bytes)
            .expect("primitive text policy fields always have native representations")
    }

    pub fn structured_value(value_kind: KindId, type_digest: [u8; 32], maximum_bytes: u32) -> Self {
        Self::structured(
            maximum_bytes,
            InteractionTypeDigest::new(type_digest).expect("digest length is exact"),
            InteractionValueKind::new(value_kind.as_str().into())
                .expect("Kind identities are nonempty bounded text"),
        )
        .expect("structured interaction fields satisfy the native contract")
    }
}

impl InteractionValue {
    pub fn new(value_kind: KindId, canonical_bytes: Vec<u8>) -> Result<Self, InteractionRefusal> {
        validate_identity(value_kind.as_str())?;
        if canonical_bytes.len() > MAXIMUM_INTERACTION_VALUE_BYTES {
            return Err(InteractionRefusal::ValueBoundExceeded);
        }
        Self::new_native(
            InteractionCanonicalBytes::new(
                conduit_form::rust_binding::BoundedBytes::new(&canonical_bytes)
                    .ok_or(InteractionRefusal::ValueBoundExceeded)?,
            )
            .map_err(|_| InteractionRefusal::ValueBoundExceeded)?,
            InteractionValueKind::new(value_kind.as_str().into())
                .map_err(|_| InteractionRefusal::MalformedValue)?,
        )
        .map_err(|_| InteractionRefusal::MalformedValue)
    }

    pub fn kind(&self) -> &str {
        self.value_kind().get().as_str()
    }

    pub fn bytes(&self) -> &[u8] {
        self.canonical_bytes().get().as_slice()
    }

    pub fn structured(value: &StructuredInfoValue) -> Result<Self, InteractionRefusal> {
        let profile = value
            .value_type()
            .profile()
            .map_err(|_| InteractionRefusal::MalformedValue)?;
        Self::new(
            profile.value_kind().clone(),
            value
                .canonical_bytes()
                .map_err(|_| InteractionRefusal::MalformedValue)?,
        )
    }
}

impl PartialOrd for InteractionValue {
    fn partial_cmp(&self, other: &Self) -> Option<core::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for InteractionValue {
    fn cmp(&self, other: &Self) -> core::cmp::Ordering {
        (self.kind(), self.bytes()).cmp(&(other.kind(), other.bytes()))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InteractionContract {
    pub semantic_id: String,
    pub contract_identity: String,
    pub family: InteractionFamily,
}

impl InteractionContract {
    pub fn new(
        semantic_id: impl Into<String>,
        family: InteractionFamily,
    ) -> Result<Self, InteractionRefusal> {
        let semantic_id = semantic_id.into();
        validate_identity(&semantic_id)?;
        validate_family(&family)?;
        let mut value = Self {
            semantic_id,
            contract_identity: String::new(),
            family,
        };
        value.contract_identity = identity("interaction-contract", &value.canonical_bytes());
        Ok(value)
    }

    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut output = Vec::new();
        field(&mut output, self.semantic_id.as_bytes());
        encode_family(&mut output, &self.family);
        output
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InteractionOption {
    pub identity: String,
    pub value: InteractionValue,
    pub availability: OptionAvailability,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InteractionDomain {
    pub revision: u64,
    pub options: Vec<InteractionOption>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InteractionCurrentState {
    pub state_identity: String,
    pub contract_identity: String,
    pub revision: u64,
    pub domain: Option<InteractionDomain>,
    pub current: Vec<InteractionValue>,
}

impl InteractionCurrentState {
    pub fn new(
        contract: &InteractionContract,
        revision: u64,
        domain: Option<InteractionDomain>,
        mut current: Vec<InteractionValue>,
    ) -> Result<Self, InteractionRefusal> {
        validate_state(contract, domain.as_ref(), &current)?;
        if matches!(&contract.family, InteractionFamily::ChooseMany(_)) {
            current.sort();
        }
        let mut value = Self {
            state_identity: String::new(),
            contract_identity: contract.contract_identity.clone(),
            revision,
            domain,
            current,
        };
        value.state_identity = identity("interaction-state", &value.canonical_bytes());
        Ok(value)
    }

    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut output = Vec::new();
        field(&mut output, self.contract_identity.as_bytes());
        output.extend_from_slice(&self.revision.to_le_bytes());
        match &self.domain {
            Some(domain) => {
                output.push(1);
                encode_domain(&mut output, domain);
            }
            None => output.push(0),
        }
        values(&mut output, &self.current);
        output
    }
}

impl InteractionProposalPayload {
    pub fn selected(values: Vec<InteractionValue>) -> Result<Self, InteractionRefusal> {
        Self::values(
            InteractionValues::new(
                conduit_form::rust_binding::BoundedSequence::try_from_iter(values)
                    .map_err(|_| InteractionRefusal::InvalidCardinality)?,
            )
            .map_err(|_| InteractionRefusal::InvalidCardinality)?,
        )
        .map_err(|_| InteractionRefusal::InvalidCardinality)
    }

    pub fn relative_value(value: InteractionValue) -> Result<Self, InteractionRefusal> {
        Self::relative(value).map_err(|_| InteractionRefusal::MalformedValue)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HumanInteractionProposal {
    pub proposal_identity: String,
    pub contract_identity: String,
    pub state_identity: String,
    pub state_revision: u64,
    pub sequence: u64,
    pub payload: InteractionProposalPayload,
}

impl HumanInteractionProposal {
    pub fn new(
        contract: &InteractionContract,
        state: &InteractionCurrentState,
        sequence: u64,
        mut payload: InteractionProposalPayload,
    ) -> Result<Self, InteractionRefusal> {
        validate_proposal(contract, state, &payload)?;
        if matches!(&contract.family, InteractionFamily::ChooseMany(_)) {
            if let InteractionProposalPayload::Values(items) = &payload {
                let mut sorted = items.get().as_slice().to_vec();
                sorted.sort();
                payload = InteractionProposalPayload::selected(sorted)?;
            }
        }
        let mut value = Self {
            proposal_identity: String::new(),
            contract_identity: contract.contract_identity.clone(),
            state_identity: state.state_identity.clone(),
            state_revision: state.revision,
            sequence,
            payload,
        };
        value.proposal_identity = identity("interaction-proposal", &value.canonical_bytes());
        Ok(value)
    }

    pub fn validate_against(
        &self,
        contract: &InteractionContract,
        state: &InteractionCurrentState,
    ) -> Result<(), InteractionRefusal> {
        if self.contract_identity != contract.contract_identity
            || self.state_identity != state.state_identity
            || self.state_revision != state.revision
        {
            return Err(InteractionRefusal::StaleState);
        }
        validate_proposal(contract, state, &self.payload)?;
        if identity("interaction-proposal", &self.canonical_bytes()) != self.proposal_identity {
            return Err(InteractionRefusal::MalformedValue);
        }
        Ok(())
    }

    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut output = Vec::new();
        field(&mut output, self.contract_identity.as_bytes());
        field(&mut output, self.state_identity.as_bytes());
        output.extend_from_slice(&self.state_revision.to_le_bytes());
        output.extend_from_slice(&self.sequence.to_le_bytes());
        match &self.payload {
            InteractionProposalPayload::Activate => output.push(0),
            InteractionProposalPayload::Values(items) => {
                output.push(1);
                values(&mut output, items.get().as_slice());
            }
            InteractionProposalPayload::Relative(value) => {
                output.push(2);
                encode_value(&mut output, value.value());
            }
        }
        output
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InteractionApplicationResult {
    pub result_identity: String,
    pub proposal_identity: String,
    pub outcome: InteractionApplicationOutcome,
}

impl InteractionApplicationResult {
    pub fn new(
        proposal: &HumanInteractionProposal,
        outcome: InteractionApplicationOutcome,
    ) -> Result<Self, InteractionRefusal> {
        validate_outcome(&outcome)?;
        let mut value = Self {
            result_identity: String::new(),
            proposal_identity: proposal.proposal_identity.clone(),
            outcome,
        };
        value.result_identity = identity("interaction-result", &value.canonical_bytes());
        Ok(value)
    }

    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut output = Vec::new();
        field(&mut output, self.proposal_identity.as_bytes());
        match &self.outcome {
            InteractionApplicationOutcome::Accepted(accepted) => {
                output.push(0);
                field(&mut output, accepted.resulting_state_identity().as_bytes());
            }
            InteractionApplicationOutcome::Refused(refused) => {
                output.push(1);
                field(&mut output, refused.reason_code().as_bytes());
            }
            InteractionApplicationOutcome::Failed(failed) => {
                output.push(2);
                field(&mut output, failed.reason_code().as_bytes());
            }
            InteractionApplicationOutcome::Cancelled => output.push(3),
        }
        output
    }
}

#[derive(Debug)]
pub struct InteractionProposalQueue {
    maximum_queued: usize,
    maximum_results: usize,
    queued: VecDeque<HumanInteractionProposal>,
    completed: Vec<String>,
}

impl InteractionProposalQueue {
    pub fn new(maximum_queued: usize, maximum_results: usize) -> Result<Self, InteractionRefusal> {
        if maximum_queued == 0 || maximum_queued > MAXIMUM_INTERACTION_QUEUE {
            return Err(InteractionRefusal::QueuePressure);
        }
        if maximum_results == 0 || maximum_results > MAXIMUM_INTERACTION_QUEUE {
            return Err(InteractionRefusal::ResultPressure);
        }
        Ok(Self {
            maximum_queued,
            maximum_results,
            queued: VecDeque::with_capacity(maximum_queued),
            completed: Vec::with_capacity(maximum_results),
        })
    }

    pub fn admit(&mut self, proposal: HumanInteractionProposal) -> Result<(), InteractionRefusal> {
        if self
            .queued
            .iter()
            .any(|item| item.proposal_identity == proposal.proposal_identity)
            || self.completed.contains(&proposal.proposal_identity)
        {
            return Err(InteractionRefusal::DuplicateProposal);
        }
        if self.queued.len() == self.maximum_queued {
            return Err(InteractionRefusal::QueuePressure);
        }
        self.queued.push_back(proposal);
        Ok(())
    }

    pub fn finish_front(
        &mut self,
        outcome: InteractionApplicationOutcome,
    ) -> Result<InteractionApplicationResult, InteractionRefusal> {
        if self.completed.len() == self.maximum_results {
            return Err(InteractionRefusal::ResultPressure);
        }
        let proposal = self
            .queued
            .pop_front()
            .ok_or(InteractionRefusal::UnknownProposal)?;
        let result = InteractionApplicationResult::new(&proposal, outcome)?;
        self.completed.push(proposal.proposal_identity);
        Ok(result)
    }

    pub fn cancel_front(&mut self) -> Result<InteractionApplicationResult, InteractionRefusal> {
        self.finish_front(InteractionApplicationOutcome::Cancelled)
    }

    pub fn queued_len(&self) -> usize {
        self.queued.len()
    }
}
