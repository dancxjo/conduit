//! Finite Pico control-surface realization of portable interaction contracts.

use conduit_core::{
    HumanInteractionProposal, InteractionContract, InteractionCurrentState,
    InteractionProposalPayload, InteractionRefusal, InteractionValue, ScalarRealizationMapping,
};

pub const PICO_INTERACTION_IMPLEMENTATION: &str = "conduit-pete/pico-interaction-surface@1";
pub const MAXIMUM_PROJECTED_CONTROLS: usize = 8;
pub const MAXIMUM_PENDING_EVENTS: usize = 4;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PhysicalResourceBinding {
    pub resource_id: String,
    pub generation: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DebounceProfile {
    pub stable_scans: u8,
    pub maximum_transitions_per_window: u8,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CalibrationProfile {
    pub minimum_sample: i64,
    pub maximum_sample: i64,
    pub maximum_sample_delta: i64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ChoiceBinding {
    pub resource: PhysicalResourceBinding,
    pub option_identity: String,
    pub value: InteractionValue,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PhysicalInteractionPlanProjection {
    pub plan_id: String,
    pub host_id: String,
    pub boot_id: String,
    pub implementation_id: String,
    pub action_contract: InteractionContract,
    pub action_state: InteractionCurrentState,
    pub action_switch: PhysicalResourceBinding,
    pub choice_contract: InteractionContract,
    pub choice_state: InteractionCurrentState,
    pub choices: Vec<ChoiceBinding>,
    pub scalar_contract: InteractionContract,
    pub scalar_state: InteractionCurrentState,
    pub scalar_resource: PhysicalResourceBinding,
    pub scalar_mapping: ScalarRealizationMapping,
    pub display_resource: PhysicalResourceBinding,
    pub debounce: DebounceProfile,
    pub calibration: CalibrationProfile,
    pub maximum_pending_events: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PhysicalInteractionFailure {
    InvalidPlan,
    MissingSwitch {
        resource_id: String,
    },
    ChoiceInputUnavailable {
        resource_id: String,
    },
    ScalarInputUnavailable {
        resource_id: String,
    },
    DisplayUnavailable {
        resource_id: String,
    },
    OutOfCalibration {
        sample: i64,
    },
    NoiseBeyondProfile {
        observed_delta: i64,
    },
    BounceBeyondProfile {
        transitions: u8,
    },
    StalePlan {
        expected_plan_id: String,
    },
    OldGeneration {
        resource_id: String,
        expected: u64,
        observed: u64,
    },
    QueuePressure {
        maximum: usize,
    },
    Cancelled,
    Interaction(InteractionRefusal),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PhysicalInput {
    ActionPressed,
    ChoicePressed {
        resource_id: String,
    },
    ScalarSample {
        sample: i64,
        prior_sample: Option<i64>,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PhysicalEvent {
    pub plan_id: String,
    pub resource_id: String,
    pub resource_generation: u64,
    pub sequence: u64,
    pub transitions_in_window: u8,
    pub input: PhysicalInput,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticStateManifestation {
    pub plan_id: String,
    pub display_resource_id: String,
    pub display_generation: u64,
    pub contract_identity: String,
    pub state_identity: String,
    pub values: Vec<InteractionValue>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PhysicalResourceStatus {
    pub available_resource_ids: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PhysicalInteractionOffers {
    pub action: bool,
    pub choice_option_identities: Vec<String>,
    pub scalar: bool,
    pub presentation: bool,
}

pub struct PicoInteractionSurface {
    projection: PhysicalInteractionPlanProjection,
    pending: usize,
    cancelled: bool,
}

impl PicoInteractionSurface {
    pub fn prepare(
        projection: PhysicalInteractionPlanProjection,
    ) -> Result<Self, PhysicalInteractionFailure> {
        if projection.plan_id.is_empty()
            || projection.host_id.is_empty()
            || projection.boot_id.is_empty()
            || projection.implementation_id != PICO_INTERACTION_IMPLEMENTATION
            || projection.choices.is_empty()
            || projection.choices.len() > MAXIMUM_PROJECTED_CONTROLS
            || projection.maximum_pending_events == 0
            || projection.maximum_pending_events > MAXIMUM_PENDING_EVENTS
            || projection.debounce.stable_scans == 0
            || projection.debounce.maximum_transitions_per_window == 0
            || projection.calibration.minimum_sample >= projection.calibration.maximum_sample
            || projection.calibration.maximum_sample_delta < 0
            || projection.scalar_mapping.contract_identity
                != projection.scalar_contract.contract_identity
            || projection.action_state.contract_identity
                != projection.action_contract.contract_identity
            || projection.choice_state.contract_identity
                != projection.choice_contract.contract_identity
            || projection.scalar_state.contract_identity
                != projection.scalar_contract.contract_identity
            || !matches!(
                projection.action_contract.family,
                conduit_core::InteractionFamily::Activate
            )
            || !matches!(
                projection.scalar_contract.family,
                conduit_core::InteractionFamily::Scalar { .. }
            )
            || projection.calibration.minimum_sample != projection.scalar_mapping.source_minimum
            || projection.calibration.maximum_sample != projection.scalar_mapping.source_maximum
            || projection.choices.iter().any(|binding| {
                binding.resource.resource_id.is_empty()
                    || binding.option_identity.is_empty()
                    || binding.value.value_kind
                        != match &projection.choice_contract.family {
                            conduit_core::InteractionFamily::ChooseOne { value_kind, .. } => {
                                value_kind.clone()
                            }
                            _ => return true,
                        }
            })
        {
            return Err(PhysicalInteractionFailure::InvalidPlan);
        }
        Ok(Self {
            projection,
            pending: 0,
            cancelled: false,
        })
    }

    pub fn projection(&self) -> &PhysicalInteractionPlanProjection {
        &self.projection
    }

    pub fn offers(&self, status: &PhysicalResourceStatus) -> PhysicalInteractionOffers {
        let available = |binding: &PhysicalResourceBinding| {
            status
                .available_resource_ids
                .iter()
                .any(|resource| resource == &binding.resource_id)
        };
        PhysicalInteractionOffers {
            action: available(&self.projection.action_switch),
            choice_option_identities: self
                .projection
                .choices
                .iter()
                .filter(|choice| available(&choice.resource))
                .map(|choice| choice.option_identity.clone())
                .collect(),
            scalar: available(&self.projection.scalar_resource),
            presentation: available(&self.projection.display_resource),
        }
    }

    pub fn cancel(&mut self) {
        self.cancelled = true;
    }

    pub fn complete_one(&mut self) {
        self.pending = self.pending.saturating_sub(1);
    }

    pub fn propose(
        &mut self,
        event: PhysicalEvent,
    ) -> Result<HumanInteractionProposal, PhysicalInteractionFailure> {
        if self.cancelled {
            return Err(PhysicalInteractionFailure::Cancelled);
        }
        if event.plan_id != self.projection.plan_id {
            return Err(PhysicalInteractionFailure::StalePlan {
                expected_plan_id: self.projection.plan_id.clone(),
            });
        }
        if self.pending == self.projection.maximum_pending_events {
            return Err(PhysicalInteractionFailure::QueuePressure {
                maximum: self.projection.maximum_pending_events,
            });
        }
        if event.transitions_in_window > self.projection.debounce.maximum_transitions_per_window {
            return Err(PhysicalInteractionFailure::BounceBeyondProfile {
                transitions: event.transitions_in_window,
            });
        }
        let (binding, contract, state, payload) = match event.input {
            PhysicalInput::ActionPressed => (
                &self.projection.action_switch,
                &self.projection.action_contract,
                &self.projection.action_state,
                InteractionProposalPayload::Activate,
            ),
            PhysicalInput::ChoicePressed { ref resource_id } => {
                let choice = self
                    .projection
                    .choices
                    .iter()
                    .find(|choice| &choice.resource.resource_id == resource_id)
                    .ok_or_else(|| PhysicalInteractionFailure::ChoiceInputUnavailable {
                        resource_id: resource_id.clone(),
                    })?;
                (
                    &choice.resource,
                    &self.projection.choice_contract,
                    &self.projection.choice_state,
                    InteractionProposalPayload::Values(vec![choice.value.clone()]),
                )
            }
            PhysicalInput::ScalarSample {
                sample,
                prior_sample,
            } => {
                if sample < self.projection.calibration.minimum_sample
                    || sample > self.projection.calibration.maximum_sample
                {
                    return Err(PhysicalInteractionFailure::OutOfCalibration { sample });
                }
                if let Some(prior) = prior_sample {
                    let delta = sample.abs_diff(prior) as i64;
                    if delta > self.projection.calibration.maximum_sample_delta {
                        return Err(PhysicalInteractionFailure::NoiseBeyondProfile {
                            observed_delta: delta,
                        });
                    }
                }
                let value = self
                    .projection
                    .scalar_mapping
                    .map(sample)
                    .map_err(PhysicalInteractionFailure::Interaction)?;
                (
                    &self.projection.scalar_resource,
                    &self.projection.scalar_contract,
                    &self.projection.scalar_state,
                    InteractionProposalPayload::Values(vec![value]),
                )
            }
        };
        if event.resource_id != binding.resource_id {
            return Err(match event.input {
                PhysicalInput::ActionPressed => PhysicalInteractionFailure::MissingSwitch {
                    resource_id: event.resource_id,
                },
                PhysicalInput::ChoicePressed { .. } => {
                    PhysicalInteractionFailure::ChoiceInputUnavailable {
                        resource_id: event.resource_id,
                    }
                }
                PhysicalInput::ScalarSample { .. } => {
                    PhysicalInteractionFailure::ScalarInputUnavailable {
                        resource_id: event.resource_id,
                    }
                }
            });
        }
        if event.resource_generation != binding.generation {
            return Err(PhysicalInteractionFailure::OldGeneration {
                resource_id: binding.resource_id.clone(),
                expected: binding.generation,
                observed: event.resource_generation,
            });
        }
        let proposal = HumanInteractionProposal::new(contract, state, event.sequence, payload)
            .map_err(PhysicalInteractionFailure::Interaction)?;
        self.pending += 1;
        Ok(proposal)
    }

    pub fn manifest(
        &self,
        state: &InteractionCurrentState,
        display_available: bool,
    ) -> Result<SemanticStateManifestation, PhysicalInteractionFailure> {
        if !display_available {
            return Err(PhysicalInteractionFailure::DisplayUnavailable {
                resource_id: self.projection.display_resource.resource_id.clone(),
            });
        }
        Ok(SemanticStateManifestation {
            plan_id: self.projection.plan_id.clone(),
            display_resource_id: self.projection.display_resource.resource_id.clone(),
            display_generation: self.projection.display_resource.generation,
            contract_identity: state.contract_identity.clone(),
            state_identity: state.state_identity.clone(),
            values: state.current.clone(),
        })
    }
}
