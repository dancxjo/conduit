#![no_std]

//! Bounded Todo meaning owned by the authored Todo Plot. The exact binary
//! Form and transition use fixed scratch during Play; Masks see semantic state.

extern crate alloc;

#[cfg(feature = "authoring")]
mod authoring;
mod fixed;
#[cfg(feature = "authoring")]
pub use authoring::{admit_empty_todo_initial, install_todo_catalogs};
pub use fixed::{COMMAND_MAX_BYTES, STATE_MAX_BYTES};
#[cfg(feature = "kernel-step")]
mod combine_back;
#[cfg(feature = "kernel-step")]
pub use combine_back::TodoCombineBack;

use alloc::{string::String, vec, vec::Vec};
use conduit_core::{
    kind_id, port_id, CapabilityLimits, CheckedValueContract, FrontValueContract,
    FrontValueLocation, Kind, KindIdentity, KindSemanticLaw, KindTerminalBehavior, PortDescriptor,
    PortDirection, PortTemporal,
};
use fixed::{FixedTodoCommand, FixedTodoState};

pub const MAX_TODO_ITEMS: usize = 20;
pub const MAX_TODO_TEXT_BYTES: usize = 72;
pub const MAX_TODO_TITLE_BYTES: usize = 64;
pub const TODO_STATE_INFO_ID: &str = "conduit.todo/state@1";
pub const TODO_COMMAND_INFO_ID: &str = "conduit.todo/command@1";
pub const TODO_COMBINE_KIND: &str = "todo/combine";
pub const TODO_COMBINE_REVISION: &str = "conduit.todo/combine@1";
pub const TODO_CHECKPOINT_KIND: &str = "todo/checkpoint";
pub const TODO_CHECKPOINT_REVISION: &str = "conduit.todo/checkpoint@1";

/// One planned immutable generation is published before this Gear emits state.
/// A later command requires a new Plan and a separately restored current state.
pub fn todo_checkpoint_kind() -> Kind {
    Kind {
        startup_parameters: Vec::new(),
        shorthand: None,
        kind_id: kind_id(TODO_CHECKPOINT_KIND),
        kind_contract_revision: KindIdentity::from(TODO_CHECKPOINT_REVISION),
        inputs: vec![port("candidate", TODO_STATE_INFO_ID, PortDirection::Input)],
        outputs: vec![port("committed", TODO_STATE_INFO_ID, PortDirection::Output)],
        configuration: Vec::new(),
        semantic_laws: vec![
            KindSemanticLaw::ValueContracts(vec![
                value_contract("candidate", TODO_STATE_INFO_ID, STATE_MAX_BYTES, true),
                value_contract("committed", TODO_STATE_INFO_ID, STATE_MAX_BYTES, false),
            ]),
            KindSemanticLaw::Terminal(KindTerminalBehavior::CompletesWhenInputsClose),
        ],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 2,
            max_queue_bytes: (2 * STATE_MAX_BYTES) as u32,
        },
    }
}

/// The exact two-input combine Kind selected by ordinary bounded `scan`.
/// The scan owns retained state; this Kind applies one typed command to it.
pub fn todo_combine_kind() -> Kind {
    Kind {
        startup_parameters: Vec::new(),
        shorthand: None,
        kind_id: kind_id(TODO_COMBINE_KIND),
        kind_contract_revision: KindIdentity::from(TODO_COMBINE_REVISION),
        inputs: vec![
            port("accumulator", TODO_STATE_INFO_ID, PortDirection::Input),
            port("item", TODO_COMMAND_INFO_ID, PortDirection::Input),
        ],
        outputs: vec![port("combined", TODO_STATE_INFO_ID, PortDirection::Output)],
        configuration: Vec::new(),
        semantic_laws: vec![
            KindSemanticLaw::ValueContracts(vec![
                value_contract("accumulator", TODO_STATE_INFO_ID, STATE_MAX_BYTES, true),
                value_contract("item", TODO_COMMAND_INFO_ID, COMMAND_MAX_BYTES, true),
                value_contract("combined", TODO_STATE_INFO_ID, STATE_MAX_BYTES, false),
            ]),
            KindSemanticLaw::Terminal(KindTerminalBehavior::CompletesWhenInputsClose),
        ],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 3,
            max_queue_bytes: (2 * STATE_MAX_BYTES + COMMAND_MAX_BYTES) as u32,
        },
    }
}

fn value_contract(
    name: &str,
    value_kind: &str,
    maximum_bytes: usize,
    input: bool,
) -> FrontValueContract {
    let port = port_id(name);
    FrontValueContract {
        location: if input {
            FrontValueLocation::Input(port)
        } else {
            FrontValueLocation::Output(port)
        },
        contract: CheckedValueContract::new(kind_id(value_kind), maximum_bytes as u32, vec![])
            .expect("Todo's finite Info Form contract"),
    }
}

fn port(name: &str, value_kind: &str, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(value_kind),
        direction,
        temporal: PortTemporal::Value,
        abnormal_kind: None,
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TodoItem {
    pub id: String,
    pub text: String,
    pub complete: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TodoState {
    pub title: String,
    pub revision: u32,
    pub next_id: u32,
    pub items: Vec<TodoItem>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TodoCommand {
    Add { text: String },
    SetComplete { id: String, complete: bool },
    Remove { id: String },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum TodoRefusal {
    InvalidTitle,
    InvalidText,
    InvalidId,
    InvalidState,
    InvalidCommand,
    MissingItem,
    ItemCapacity,
    IdentityExhausted,
    RevisionExhausted,
}

impl TodoRefusal {
    pub const fn detail(&self) -> u16 {
        match self {
            Self::InvalidTitle => 1,
            Self::InvalidText => 2,
            Self::InvalidId => 3,
            Self::InvalidState => 4,
            Self::InvalidCommand => 5,
            Self::MissingItem => 6,
            Self::ItemCapacity => 7,
            Self::IdentityExhausted => 8,
            Self::RevisionExhausted => 9,
        }
    }
}

impl TodoState {
    pub fn new(title: String) -> Result<Self, TodoRefusal> {
        let state = Self {
            title,
            revision: 0,
            next_id: 1,
            items: Vec::new(),
        };
        state.validate()?;
        Ok(state)
    }

    pub fn validate(&self) -> Result<(), TodoRefusal> {
        FixedTodoState::from_public(self).map(|_| ())
    }

    pub fn apply(&self, command: &TodoCommand) -> Result<Self, TodoRefusal> {
        let mut fixed = FixedTodoState::from_public(self)?;
        fixed.apply(&FixedTodoCommand::from_public(command)?)?;
        Ok(fixed.into_public())
    }

    pub fn encode_info(&self) -> Result<Vec<u8>, TodoRefusal> {
        let fixed = FixedTodoState::from_public(self)?;
        let mut bytes = [0; STATE_MAX_BYTES];
        let len = fixed.encode_into(&mut bytes);
        Ok(bytes[..len].to_vec())
    }

    pub fn decode_info(bytes: &[u8]) -> Result<Self, TodoRefusal> {
        Ok(FixedTodoState::decode(bytes)?.into_public())
    }
}

impl TodoCommand {
    pub fn encode_info(&self) -> Result<Vec<u8>, TodoRefusal> {
        let fixed = FixedTodoCommand::from_public(self)?;
        let mut bytes = [0; COMMAND_MAX_BYTES];
        let len = fixed.encode_into(&mut bytes);
        Ok(bytes[..len].to_vec())
    }

    pub fn decode_info(bytes: &[u8]) -> Result<Self, TodoRefusal> {
        Ok(FixedTodoCommand::decode(bytes)?.into_public())
    }
}
