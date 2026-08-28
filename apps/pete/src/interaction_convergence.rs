//! One semantic control surface shared by materially different Presenters.

use conduit_core::{
    BoundKind, CheckedFormId, ExpandedFormId, HumanInteractionProposal, InfoBool,
    InteractionContract, InteractionCurrentState, InteractionDomain, InteractionFamily,
    InteractionOption, InteractionProposalPayload, InteractionRefusal, InteractionValue,
    KindContractRevision, KindId, OptionAvailability, Quantity, QuantityUnit,
    RealizationRangePolicy, ScalarQuantization, ScalarRealizationMapping, SourceDocumentId,
    BOOL_INFO_ID, QUANTITY_INFO_ID, TEXT_INFO_ID,
};
use conduit_form::{parse, ConfigurationField, KindDefinition, ProfileCatalog};

use crate::{
    CalibrationProfile, ChoiceBinding, DebounceProfile, PhysicalInteractionPlanProjection,
    PhysicalResourceBinding, PICO_INTERACTION_IMPLEMENTATION,
};

pub const CONTROL_SURFACE_FORM: &str =
    "form instrument_control_surface {\n    surface: human-interaction/control-surface\n}\n";
pub const CONTROL_SURFACE_KIND: &str = "human-interaction/control-surface";
pub const CONTROL_SURFACE_KIND_REVISION: &str = "human-interaction/control-surface@1";
pub const CONTROL_SURFACE_BODY_ID: &str = "body/human-interaction-capstone@1";
pub const CONTROL_SURFACE_PLAN_ID: &str = "plan/human-interaction-capstone@1";
pub const CONTROL_SURFACE_PLAY_ID: &str = "play/human-interaction-capstone@1";
pub const BROWSER_IMPLEMENTATION_ID: &str = "browser/interaction-presenter@1";

const WAVEFORM_KIND: &str = "music/waveform@1";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ControlSurfaceContracts {
    pub sustain: InteractionContract,
    pub waveform: InteractionContract,
    pub volume: InteractionContract,
    pub transpose: InteractionContract,
    pub transpose_relative: InteractionContract,
    pub panic: InteractionContract,
    pub name: InteractionContract,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ControlSurfaceStates {
    pub sustain: InteractionCurrentState,
    pub waveform: InteractionCurrentState,
    pub volume: InteractionCurrentState,
    pub transpose: InteractionCurrentState,
    pub transpose_relative: InteractionCurrentState,
    pub panic: InteractionCurrentState,
    pub name: InteractionCurrentState,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PresenterSource {
    Browser {
        host_id: String,
        boot_id: String,
        manifestation_id: String,
    },
    Physical {
        host_id: String,
        boot_id: String,
        manifestation_id: String,
        resource_id: String,
        mapping_identity: Option<String>,
        recursive_composition_identity: String,
    },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct InteractionConvergenceReceipt {
    pub source_document_id: SourceDocumentId,
    pub checked_form_id: CheckedFormId,
    pub expanded_form_id: ExpandedFormId,
    pub body_id: String,
    pub plan_id: String,
    pub play_id: String,
    pub source: PresenterSource,
    pub semantic_id: String,
    pub contract_identity: String,
    pub proposal_identity: String,
    pub resulting_state_identity: String,
    pub resulting_revision: u64,
    pub resulting_values: Vec<InteractionValue>,
    pub action_invoked: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InteractionConvergenceError {
    Catalog,
    UnknownContract,
    WrongPayload,
    Interaction(InteractionRefusal),
}

pub struct InteractionConvergenceApplication {
    source_document_id: SourceDocumentId,
    checked_form_id: CheckedFormId,
    expanded_form_id: ExpandedFormId,
    contracts: ControlSurfaceContracts,
    states: ControlSurfaceStates,
}

impl InteractionConvergenceApplication {
    pub fn new() -> Result<Self, InteractionConvergenceError> {
        let mut catalog = ProfileCatalog::new();
        catalog
            .insert(KindDefinition {
                kind_id: KindId::from(CONTROL_SURFACE_KIND),
                kind_contract_revision: KindContractRevision::from(CONTROL_SURFACE_KIND_REVISION),
                inputs: vec![],
                outputs: vec![],
                configuration: Vec::<ConfigurationField>::new(),
            })
            .map_err(|_| InteractionConvergenceError::Catalog)?;
        let checked = parse(CONTROL_SURFACE_FORM, &catalog)
            .map_err(|_| InteractionConvergenceError::Catalog)?;
        let contracts = contracts()?;
        let states = states(&contracts)?;
        Ok(Self {
            source_document_id: checked.source_document_id,
            checked_form_id: checked.checked_form_id,
            expanded_form_id: checked.expanded_form_id,
            contracts,
            states,
        })
    }

    pub fn source_document_id(&self) -> &SourceDocumentId {
        &self.source_document_id
    }

    pub fn checked_form_id(&self) -> &CheckedFormId {
        &self.checked_form_id
    }

    pub fn expanded_form_id(&self) -> &ExpandedFormId {
        &self.expanded_form_id
    }

    pub fn contracts(&self) -> &ControlSurfaceContracts {
        &self.contracts
    }

    pub fn states(&self) -> &ControlSurfaceStates {
        &self.states
    }

    pub fn submit(
        &mut self,
        source: PresenterSource,
        proposal: HumanInteractionProposal,
    ) -> Result<InteractionConvergenceReceipt, InteractionConvergenceError> {
        let contract_identity = proposal.contract_identity.clone();
        let (contract, state) = self
            .contract_and_state(&contract_identity)
            .ok_or(InteractionConvergenceError::UnknownContract)?;
        proposal
            .validate_against(contract, state)
            .map_err(InteractionConvergenceError::Interaction)?;
        let semantic_id = contract.semantic_id.clone();
        let proposal_identity = proposal.proposal_identity.clone();
        let action_invoked = matches!(contract.family, InteractionFamily::Activate);
        let next = apply(contract, state, proposal.payload)?;
        *self
            .state_mut(&contract_identity)
            .ok_or(InteractionConvergenceError::UnknownContract)? = next.clone();
        Ok(InteractionConvergenceReceipt {
            source_document_id: self.source_document_id.clone(),
            checked_form_id: self.checked_form_id.clone(),
            expanded_form_id: self.expanded_form_id.clone(),
            body_id: CONTROL_SURFACE_BODY_ID.into(),
            plan_id: CONTROL_SURFACE_PLAN_ID.into(),
            play_id: CONTROL_SURFACE_PLAY_ID.into(),
            source,
            semantic_id,
            contract_identity,
            proposal_identity,
            resulting_state_identity: next.state_identity,
            resulting_revision: next.revision,
            resulting_values: next.current,
            action_invoked,
        })
    }

    fn contract_and_state(
        &self,
        identity: &str,
    ) -> Option<(&InteractionContract, &InteractionCurrentState)> {
        let pairs = [
            (&self.contracts.sustain, &self.states.sustain),
            (&self.contracts.waveform, &self.states.waveform),
            (&self.contracts.volume, &self.states.volume),
            (&self.contracts.transpose, &self.states.transpose),
            (
                &self.contracts.transpose_relative,
                &self.states.transpose_relative,
            ),
            (&self.contracts.panic, &self.states.panic),
            (&self.contracts.name, &self.states.name),
        ];
        pairs
            .into_iter()
            .find(|(contract, _)| contract.contract_identity == identity)
    }

    fn state_mut(&mut self, identity: &str) -> Option<&mut InteractionCurrentState> {
        if self.contracts.sustain.contract_identity == identity {
            Some(&mut self.states.sustain)
        } else if self.contracts.waveform.contract_identity == identity {
            Some(&mut self.states.waveform)
        } else if self.contracts.volume.contract_identity == identity {
            Some(&mut self.states.volume)
        } else if self.contracts.transpose.contract_identity == identity {
            Some(&mut self.states.transpose)
        } else if self.contracts.transpose_relative.contract_identity == identity {
            Some(&mut self.states.transpose_relative)
        } else if self.contracts.panic.contract_identity == identity {
            Some(&mut self.states.panic)
        } else if self.contracts.name.contract_identity == identity {
            Some(&mut self.states.name)
        } else {
            None
        }
    }
}

fn contracts() -> Result<ControlSurfaceContracts, InteractionConvergenceError> {
    let new = |semantic_id, family| {
        InteractionContract::new(semantic_id, family)
            .map_err(InteractionConvergenceError::Interaction)
    };
    Ok(ControlSurfaceContracts {
        sustain: new("instrument/sustain", InteractionFamily::Boolean)?,
        waveform: new(
            "instrument/waveform",
            InteractionFamily::ChooseOne {
                value_kind: KindId::from(WAVEFORM_KIND),
                maximum_options: 4,
            },
        )?,
        volume: new(
            "instrument/volume",
            InteractionFamily::Scalar {
                unit: QuantityUnit::Percent,
                minimum: 0,
                minimum_bound: BoundKind::Inclusive,
                maximum: 100,
                maximum_bound: BoundKind::Inclusive,
                granularity: 1,
            },
        )?,
        transpose: new(
            "instrument/transpose",
            InteractionFamily::Scalar {
                unit: QuantityUnit::One,
                minimum: -24,
                minimum_bound: BoundKind::Inclusive,
                maximum: 24,
                maximum_bound: BoundKind::Inclusive,
                granularity: 1,
            },
        )?,
        transpose_relative: new(
            "instrument/transpose-relative",
            InteractionFamily::RelativeAdjustment {
                unit: QuantityUnit::One,
                minimum_delta: -12,
                maximum_delta: 12,
                granularity: 1,
            },
        )?,
        panic: new("instrument/panic", InteractionFamily::Activate)?,
        name: new(
            "instrument/name",
            InteractionFamily::Text {
                maximum_bytes: 32,
                allow_empty: false,
            },
        )?,
    })
}

fn states(
    contracts: &ControlSurfaceContracts,
) -> Result<ControlSurfaceStates, InteractionConvergenceError> {
    let waveforms = ["sine", "triangle", "saw", "pulse"]
        .into_iter()
        .map(|name| InteractionOption {
            identity: format!("waveform/{name}"),
            value: value(WAVEFORM_KIND, name.as_bytes()).expect("fixed waveform is valid"),
            availability: OptionAvailability::Available,
        })
        .collect();
    let state = |contract: &InteractionContract, domain, current| {
        InteractionCurrentState::new(contract, 1, domain, current)
            .map_err(InteractionConvergenceError::Interaction)
    };
    Ok(ControlSurfaceStates {
        sustain: state(
            &contracts.sustain,
            None,
            vec![value(BOOL_INFO_ID, &InfoBool::FALSE.encode())?],
        )?,
        waveform: state(
            &contracts.waveform,
            Some(InteractionDomain {
                revision: 1,
                options: waveforms,
            }),
            vec![value(WAVEFORM_KIND, b"sine")?],
        )?,
        volume: state(
            &contracts.volume,
            None,
            vec![quantity(50, QuantityUnit::Percent)?],
        )?,
        transpose: state(
            &contracts.transpose,
            None,
            vec![quantity(0, QuantityUnit::One)?],
        )?,
        transpose_relative: state(&contracts.transpose_relative, None, vec![])?,
        panic: state(&contracts.panic, None, vec![])?,
        name: state(
            &contracts.name,
            None,
            vec![value(TEXT_INFO_ID, b"Conduit")?],
        )?,
    })
}

fn apply(
    contract: &InteractionContract,
    state: &InteractionCurrentState,
    payload: InteractionProposalPayload,
) -> Result<InteractionCurrentState, InteractionConvergenceError> {
    let current = match (&contract.family, payload) {
        (InteractionFamily::Activate, InteractionProposalPayload::Activate) => {
            state.current.clone()
        }
        (InteractionFamily::RelativeAdjustment { .. }, InteractionProposalPayload::Relative(_)) => {
            state.current.clone()
        }
        (_, InteractionProposalPayload::Values(values)) => values,
        _ => return Err(InteractionConvergenceError::WrongPayload),
    };
    InteractionCurrentState::new(contract, state.revision + 1, state.domain.clone(), current)
        .map_err(InteractionConvergenceError::Interaction)
}

pub fn value(kind: &str, bytes: &[u8]) -> Result<InteractionValue, InteractionConvergenceError> {
    InteractionValue::new(KindId::from(kind), bytes.to_vec())
        .map_err(InteractionConvergenceError::Interaction)
}

pub fn quantity(
    number: i64,
    unit: QuantityUnit,
) -> Result<InteractionValue, InteractionConvergenceError> {
    value(QUANTITY_INFO_ID, &Quantity::new(number, unit).encode())
}

pub fn physical_control_surface_projection(
    application: &InteractionConvergenceApplication,
) -> Result<PhysicalInteractionPlanProjection, InteractionConvergenceError> {
    let resource = |resource_id: &str, generation| PhysicalResourceBinding {
        resource_id: resource_id.into(),
        generation,
    };
    let choices = application
        .states
        .waveform
        .domain
        .as_ref()
        .expect("fixed waveform domain")
        .options
        .iter()
        .enumerate()
        .map(|(index, option)| ChoiceBinding {
            resource: resource(&format!("pico/gpio/{}/switch", index + 2), 4),
            option_identity: option.identity.clone(),
            value: option.value.clone(),
        })
        .collect();
    let scalar_mapping = ScalarRealizationMapping::new(
        &application.contracts.volume,
        "pico/adc12@1",
        200,
        3800,
        1,
        RealizationRangePolicy::Refuse,
        ScalarQuantization::Nearest,
    )
    .map_err(InteractionConvergenceError::Interaction)?;
    Ok(PhysicalInteractionPlanProjection {
        plan_id: CONTROL_SURFACE_PLAN_ID.into(),
        host_id: "host/pico-w/control-surface".into(),
        boot_id: "boot/pico-w/control-surface/1".into(),
        implementation_id: PICO_INTERACTION_IMPLEMENTATION.into(),
        action_contract: application.contracts.panic.clone(),
        action_state: application.states.panic.clone(),
        action_switch: resource("pico/gpio/10/switch", 4),
        choice_contract: application.contracts.waveform.clone(),
        choice_state: application.states.waveform.clone(),
        choices,
        scalar_contract: application.contracts.volume.clone(),
        scalar_state: application.states.volume.clone(),
        scalar_resource: resource("pico/adc/0/potentiometer", 6),
        scalar_mapping,
        display_resource: resource("pico/i2c/ssd1306", 9),
        debounce: DebounceProfile {
            stable_scans: 3,
            maximum_transitions_per_window: 4,
        },
        calibration: CalibrationProfile {
            minimum_sample: 200,
            maximum_sample: 3800,
            maximum_sample_delta: 512,
        },
        maximum_pending_events: 2,
    })
}
