use super::canonical::{encode_value, field, identity};
use super::{
    BoundKind, HumanInteractionProposal, InteractionApplicationOutcome,
    InteractionApplicationResult, InteractionContract, InteractionCurrentState, InteractionFamily,
    InteractionProposalPayload, InteractionProposalQueue, InteractionRefusal, InteractionValue,
    MAXIMUM_INTERACTION_SELECTIONS,
};
use alloc::{string::String, vec::Vec};
use conduit_core::{KindId, Quantity, QuantityUnit, QUANTITY_INFO_ID};

pub const MAXIMUM_INTERACTION_COMBINATION_RULES: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MutuallyExclusiveValues {
    pub values: Vec<InteractionValue>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InteractionSelectionRules {
    pub rules_identity: String,
    pub contract_identity: String,
    pub mutually_exclusive: Vec<MutuallyExclusiveValues>,
}

impl InteractionSelectionRules {
    pub fn new(
        contract: &InteractionContract,
        mut mutually_exclusive: Vec<MutuallyExclusiveValues>,
    ) -> Result<Self, InteractionRefusal> {
        let (value_kind, maximum_options) = match &contract.family {
            InteractionFamily::ChooseMany {
                value_kind,
                maximum_options,
                ..
            } => (value_kind, usize::from(*maximum_options)),
            _ => return Err(InteractionRefusal::InvalidContract),
        };
        if mutually_exclusive.len() > MAXIMUM_INTERACTION_COMBINATION_RULES {
            return Err(InteractionRefusal::ValueBoundExceeded);
        }
        for rule in &mut mutually_exclusive {
            rule.values.sort();
            rule.values.dedup();
            if rule.values.len() < 2
                || rule.values.len() > maximum_options
                || rule.values.len() > MAXIMUM_INTERACTION_SELECTIONS
                || rule
                    .values
                    .iter()
                    .any(|value| &value.value_kind != value_kind)
            {
                return Err(InteractionRefusal::InvalidContract);
            }
        }
        mutually_exclusive.sort_by_key(|rule| canonical_rule(&rule.values));
        let mut value = Self {
            rules_identity: String::new(),
            contract_identity: contract.contract_identity.clone(),
            mutually_exclusive,
        };
        value.rules_identity = identity("interaction-selection-rules", &value.canonical_bytes());
        Ok(value)
    }

    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut output = Vec::new();
        field(&mut output, self.contract_identity.as_bytes());
        output.extend_from_slice(&(self.mutually_exclusive.len() as u16).to_le_bytes());
        for rule in &self.mutually_exclusive {
            field(&mut output, &canonical_rule(&rule.values));
        }
        output
    }

    fn validate(&self, proposal: &HumanInteractionProposal) -> Result<(), InteractionRefusal> {
        if proposal.contract_identity != self.contract_identity {
            return Err(InteractionRefusal::StaleState);
        }
        let InteractionProposalPayload::Values(selected) = &proposal.payload else {
            return Err(InteractionRefusal::WrongValueKind);
        };
        if self.mutually_exclusive.iter().any(|rule| {
            rule.values
                .iter()
                .filter(|value| selected.contains(value))
                .take(2)
                .count()
                > 1
        }) {
            Err(InteractionRefusal::InvalidCombination)
        } else {
            Ok(())
        }
    }
}

fn canonical_rule(values: &[InteractionValue]) -> Vec<u8> {
    let mut output = Vec::new();
    output.extend_from_slice(&(values.len() as u16).to_le_bytes());
    for value in values {
        encode_value(&mut output, value);
    }
    output
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RealizationRangePolicy {
    Refuse,
    Clamp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScalarQuantization {
    Exact,
    Nearest,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScalarRealizationMapping {
    pub mapping_identity: String,
    pub contract_identity: String,
    pub realization_kind: String,
    pub source_minimum: i64,
    pub source_maximum: i64,
    pub source_granularity: i64,
    pub range_policy: RealizationRangePolicy,
    pub quantization: ScalarQuantization,
    semantic_unit: QuantityUnit,
    semantic_minimum: i64,
    semantic_maximum: i64,
    semantic_granularity: i64,
}

impl ScalarRealizationMapping {
    pub fn new(
        contract: &InteractionContract,
        realization_kind: impl Into<String>,
        source_minimum: i64,
        source_maximum: i64,
        source_granularity: i64,
        range_policy: RealizationRangePolicy,
        quantization: ScalarQuantization,
    ) -> Result<Self, InteractionRefusal> {
        let (unit, minimum, minimum_bound, maximum, maximum_bound, granularity) =
            match contract.family {
                InteractionFamily::Scalar {
                    unit,
                    minimum,
                    minimum_bound,
                    maximum,
                    maximum_bound,
                    granularity,
                } => (
                    unit,
                    minimum,
                    minimum_bound,
                    maximum,
                    maximum_bound,
                    granularity,
                ),
                _ => return Err(InteractionRefusal::InvalidContract),
            };
        if source_minimum >= source_maximum
            || source_granularity <= 0
            || (i128::from(source_maximum) - i128::from(source_minimum))
                .rem_euclid(i128::from(source_granularity))
                != 0
        {
            return Err(InteractionRefusal::InvalidContract);
        }
        let semantic_minimum = admitted_boundary(minimum, minimum_bound, granularity, true)?;
        let semantic_maximum = admitted_boundary(maximum, maximum_bound, granularity, false)?;
        if semantic_minimum > semantic_maximum {
            return Err(InteractionRefusal::InvalidContract);
        }
        let realization_kind = realization_kind.into();
        super::validation::validate_identity(&realization_kind)?;
        let mut value = Self {
            mapping_identity: String::new(),
            contract_identity: contract.contract_identity.clone(),
            realization_kind,
            source_minimum,
            source_maximum,
            source_granularity,
            range_policy,
            quantization,
            semantic_unit: unit,
            semantic_minimum,
            semantic_maximum,
            semantic_granularity: granularity,
        };
        value.mapping_identity = identity("interaction-scalar-mapping", &value.canonical_bytes());
        Ok(value)
    }

    pub fn canonical_bytes(&self) -> Vec<u8> {
        let mut output = Vec::new();
        field(&mut output, self.contract_identity.as_bytes());
        field(&mut output, self.realization_kind.as_bytes());
        for number in [
            self.source_minimum,
            self.source_maximum,
            self.source_granularity,
            self.semantic_minimum,
            self.semantic_maximum,
            self.semantic_granularity,
        ] {
            output.extend_from_slice(&number.to_le_bytes());
        }
        field(&mut output, self.semantic_unit.semantic_id().as_bytes());
        output.push(self.range_policy as u8);
        output.push(self.quantization as u8);
        output
    }

    pub fn map(&self, source: i64) -> Result<InteractionValue, InteractionRefusal> {
        let source = if source < self.source_minimum || source > self.source_maximum {
            match self.range_policy {
                RealizationRangePolicy::Refuse => return Err(InteractionRefusal::OutOfRange),
                RealizationRangePolicy::Clamp => {
                    source.clamp(self.source_minimum, self.source_maximum)
                }
            }
        } else {
            source
        };
        if (i128::from(source) - i128::from(self.source_minimum))
            .rem_euclid(i128::from(self.source_granularity))
            != 0
        {
            return Err(InteractionRefusal::UnsupportedGranularity);
        }
        let source_span = i128::from(self.source_maximum) - i128::from(self.source_minimum);
        let semantic_span = i128::from(self.semantic_maximum) - i128::from(self.semantic_minimum);
        let numerator = (i128::from(source) - i128::from(self.source_minimum)) * semantic_span;
        let step = i128::from(self.semantic_granularity);
        let denominator = source_span * step;
        let lower_steps = numerator.div_euclid(denominator);
        let remainder = numerator.rem_euclid(denominator);
        let quantized_steps = match self.quantization {
            ScalarQuantization::Exact if remainder != 0 => {
                return Err(InteractionRefusal::UnsupportedGranularity)
            }
            ScalarQuantization::Exact => lower_steps,
            ScalarQuantization::Nearest if remainder * 2 >= denominator => lower_steps + 1,
            ScalarQuantization::Nearest => lower_steps,
        };
        let mapped = (i128::from(self.semantic_minimum) + quantized_steps * step).clamp(
            i128::from(self.semantic_minimum),
            i128::from(self.semantic_maximum),
        );
        let mapped = i64::try_from(mapped).map_err(|_| InteractionRefusal::OutOfRange)?;
        InteractionValue::new(
            KindId::from(QUANTITY_INFO_ID),
            Quantity::new(mapped, self.semantic_unit).encode().to_vec(),
        )
    }
}

fn admitted_boundary(
    boundary: i64,
    kind: BoundKind,
    granularity: i64,
    minimum: bool,
) -> Result<i64, InteractionRefusal> {
    if kind == BoundKind::Inclusive {
        return Ok(boundary);
    }
    if minimum {
        boundary.checked_add(granularity)
    } else {
        boundary.checked_sub(granularity)
    }
    .ok_or(InteractionRefusal::InvalidContract)
}

#[derive(Debug)]
pub struct TypedInteractionFlow {
    contract: InteractionContract,
    current: InteractionCurrentState,
    selection_rules: Option<InteractionSelectionRules>,
    queue: InteractionProposalQueue,
}

impl TypedInteractionFlow {
    pub fn new(
        contract: InteractionContract,
        current: InteractionCurrentState,
        selection_rules: Option<InteractionSelectionRules>,
        maximum_queued: usize,
        maximum_results: usize,
    ) -> Result<Self, InteractionRefusal> {
        if current.contract_identity != contract.contract_identity
            || selection_rules
                .as_ref()
                .is_some_and(|rules| rules.contract_identity != contract.contract_identity)
        {
            return Err(InteractionRefusal::StaleState);
        }
        Ok(Self {
            contract,
            current,
            selection_rules,
            queue: InteractionProposalQueue::new(maximum_queued, maximum_results)?,
        })
    }

    pub fn admit(&mut self, proposal: HumanInteractionProposal) -> Result<(), InteractionRefusal> {
        proposal.validate_against(&self.contract, &self.current)?;
        if let Some(rules) = &self.selection_rules {
            rules.validate(&proposal)?;
        }
        self.queue.admit(proposal)
    }

    pub fn finish_front(
        &mut self,
        outcome: InteractionApplicationOutcome,
    ) -> Result<InteractionApplicationResult, InteractionRefusal> {
        self.queue.finish_front(outcome)
    }

    pub fn cancel_front(&mut self) -> Result<InteractionApplicationResult, InteractionRefusal> {
        self.queue.cancel_front()
    }

    pub fn replace_current(
        &mut self,
        current: InteractionCurrentState,
    ) -> Result<(), InteractionRefusal> {
        if self.queue.queued_len() != 0 {
            return Err(InteractionRefusal::ConcurrentStateChange);
        }
        if current.contract_identity != self.contract.contract_identity {
            return Err(InteractionRefusal::StaleState);
        }
        self.current = current;
        Ok(())
    }
}
