#![no_std]

//! Bounded Todo meaning owned by the authored Todo Plot.
//!
//! This adapter validates application commands and state. The ordinary JSON
//! collection transition performs the item edit; neither owner nor Mask does.

extern crate alloc;

#[cfg(feature = "kernel-step")]
mod packet_back;
#[cfg(feature = "kernel-step")]
pub use packet_back::TodoPacketBack;

use alloc::{format, string::String, vec, vec::Vec};
use conduit_core::{
    kind_id, port_id, CapabilityLimits, Kind, KindIdentity, KindSemanticLaw, KindTerminalBehavior,
    PortDescriptor, PortDirection, PortTemporal, Scalar,
};
use conduit_web::{json_collection_combine, JsonCollectionRefusal, JsonRefusal, JsonValue};

pub const MAX_TODO_ITEMS: usize = 20;
pub const MAX_TODO_TEXT_BYTES: usize = 72;
pub const MAX_TODO_TITLE_BYTES: usize = 64;
pub const TODO_STATE_INFO_ID: &str = "conduit.todo/state@1";
pub const TODO_COMMAND_INFO_ID: &str = "conduit.todo/command@1";
pub const TODO_COMBINE_KIND: &str = "todo/combine";
pub const TODO_COMBINE_REVISION: &str = "conduit.todo/combine@1";
pub const TODO_TRANSITION_INFO_ID: &str = "conduit.todo/transition@1";
pub const TODO_PACK_KIND: &str = "todo/pack-command";
pub const TODO_APPLY_KIND: &str = "todo/apply";
pub const TODO_TRANSITION_MAX_BYTES: usize = 2 * conduit_web::JSON_MAXIMUM_ENCODED_BYTES + 4;

/// A preallocated packet builder for the two exact values consumed by a
/// one-input Host Call. It does not parse or silently reinterpret either port.
pub struct PreparedTodoPacket {
    bytes: Vec<u8>,
}

impl PreparedTodoPacket {
    pub fn new() -> Self {
        Self {
            bytes: Vec::with_capacity(TODO_TRANSITION_MAX_BYTES),
        }
    }

    pub fn pack(&mut self, state: &[u8], command: &[u8]) -> Result<&[u8], TodoRefusal> {
        if state.len() > conduit_web::JSON_MAXIMUM_ENCODED_BYTES
            || command.len() > conduit_web::JSON_MAXIMUM_ENCODED_BYTES
        {
            return Err(TodoRefusal::InvalidTransition);
        }
        self.bytes.clear();
        self.bytes
            .extend_from_slice(&(state.len() as u16).to_le_bytes());
        self.bytes.extend_from_slice(state);
        self.bytes
            .extend_from_slice(&(command.len() as u16).to_le_bytes());
        self.bytes.extend_from_slice(command);
        Ok(&self.bytes)
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn allocation_capacity(&self) -> usize {
        self.bytes.capacity()
    }
}

impl Default for PreparedTodoPacket {
    fn default() -> Self {
        Self::new()
    }
}

/// The unary Host Call owns validation and invokes the Plot's pure transition.
/// A malformed packet cannot alter retained `scan` state.
pub fn apply_transition_packet(packet: &[u8]) -> Result<Vec<u8>, TodoRefusal> {
    let state_len = packet
        .get(..2)
        .map(|bytes| u16::from_le_bytes([bytes[0], bytes[1]]) as usize)
        .ok_or(TodoRefusal::InvalidTransition)?;
    let command_offset = 2_usize
        .checked_add(state_len)
        .ok_or(TodoRefusal::InvalidTransition)?;
    let command_len = packet
        .get(command_offset..command_offset + 2)
        .map(|bytes| u16::from_le_bytes([bytes[0], bytes[1]]) as usize)
        .ok_or(TodoRefusal::InvalidTransition)?;
    if state_len > conduit_web::JSON_MAXIMUM_ENCODED_BYTES
        || command_len > conduit_web::JSON_MAXIMUM_ENCODED_BYTES
        || command_offset + 2 + command_len != packet.len()
    {
        return Err(TodoRefusal::InvalidTransition);
    }
    let state = TodoState::decode_info(&packet[2..command_offset])?;
    let command = TodoCommand::decode_info(&packet[command_offset + 2..])?;
    state.apply(&command)?.encode_info()
}

/// The exact two-input combine Kind selected by the ordinary bounded `scan`
/// activation. It owns no scheduler or retained state; `scan` retains state.
pub fn todo_combine_kind() -> Kind {
    Kind {
        startup_parameters: Vec::new(),
        shorthand: None,
        kind_id: kind_id(TODO_COMBINE_KIND),
        kind_contract_revision: KindIdentity::from(TODO_COMBINE_REVISION),
        inputs: vec![
            todo_port("accumulator", TODO_STATE_INFO_ID, PortDirection::Input),
            todo_port("item", TODO_COMMAND_INFO_ID, PortDirection::Input),
        ],
        outputs: vec![todo_port(
            "combined",
            TODO_STATE_INFO_ID,
            PortDirection::Output,
        )],
        configuration: Vec::new(),
        semantic_laws: vec![KindSemanticLaw::Terminal(
            KindTerminalBehavior::CompletesWhenInputsClose,
        )],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 3,
            max_queue_bytes: (3 * conduit_web::JSON_MAXIMUM_ENCODED_BYTES) as u32,
        },
    }
}

pub fn todo_pack_kind() -> Kind {
    Kind {
        startup_parameters: Vec::new(),
        shorthand: None,
        kind_id: kind_id(TODO_PACK_KIND),
        kind_contract_revision: KindIdentity::from("conduit.todo/pack-command@1"),
        inputs: vec![
            todo_port("accumulator", TODO_STATE_INFO_ID, PortDirection::Input),
            todo_port("command", TODO_COMMAND_INFO_ID, PortDirection::Input),
        ],
        outputs: vec![todo_port(
            "request",
            TODO_TRANSITION_INFO_ID,
            PortDirection::Output,
        )],
        configuration: Vec::new(),
        semantic_laws: vec![KindSemanticLaw::Terminal(
            KindTerminalBehavior::CompletesWhenInputsClose,
        )],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 3,
            max_queue_bytes: (2 * conduit_web::JSON_MAXIMUM_ENCODED_BYTES
                + TODO_TRANSITION_MAX_BYTES) as u32,
        },
    }
}

pub fn todo_apply_kind() -> Kind {
    Kind {
        startup_parameters: Vec::new(),
        shorthand: None,
        kind_id: kind_id(TODO_APPLY_KIND),
        kind_contract_revision: KindIdentity::from("conduit.todo/apply@1"),
        inputs: vec![todo_port(
            "request",
            TODO_TRANSITION_INFO_ID,
            PortDirection::Input,
        )],
        outputs: vec![todo_port(
            "state",
            TODO_STATE_INFO_ID,
            PortDirection::Output,
        )],
        configuration: Vec::new(),
        semantic_laws: vec![KindSemanticLaw::Terminal(
            KindTerminalBehavior::MirrorsInputTerminal,
        )],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 2,
            max_queue_bytes: (TODO_TRANSITION_MAX_BYTES + conduit_web::JSON_MAXIMUM_ENCODED_BYTES)
                as u32,
        },
    }
}

fn todo_port(name: &str, value_kind: &str, direction: PortDirection) -> PortDescriptor {
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
    InvalidTransition,
    MissingItem,
    ItemCapacity,
    IdentityExhausted,
    RevisionExhausted,
    Json(JsonRefusal),
    Collection(JsonCollectionRefusal),
}

impl TodoCommand {
    pub fn encode_info(&self) -> Result<Vec<u8>, TodoRefusal> {
        let value = match self {
            Self::Add { text } => {
                if !valid_text(text, MAX_TODO_TEXT_BYTES) {
                    return Err(TodoRefusal::InvalidText);
                }
                object(vec![
                    ("op", JsonValue::String("add".into())),
                    ("text", JsonValue::String(text.clone())),
                ])
            }
            Self::SetComplete { id, complete } => {
                if !valid_id(id) {
                    return Err(TodoRefusal::InvalidId);
                }
                object(vec![
                    ("complete", JsonValue::Bool(*complete)),
                    ("id", JsonValue::String(id.clone())),
                    ("op", JsonValue::String("set-complete".into())),
                ])
            }
            Self::Remove { id } => {
                if !valid_id(id) {
                    return Err(TodoRefusal::InvalidId);
                }
                object(vec![
                    ("id", JsonValue::String(id.clone())),
                    ("op", JsonValue::String("remove".into())),
                ])
            }
        };
        value.encode_info().map_err(TodoRefusal::Json)
    }

    pub fn decode_info(bytes: &[u8]) -> Result<Self, TodoRefusal> {
        let value = JsonValue::decode_info(bytes).map_err(TodoRefusal::Json)?;
        let JsonValue::Object(fields) = value else {
            return Err(TodoRefusal::InvalidCommand);
        };
        let command = match fields.as_slice() {
            [(op, JsonValue::String(kind)), (text, JsonValue::String(value))]
                if op == "op" && kind == "add" && text == "text" =>
            {
                Self::Add {
                    text: value.clone(),
                }
            }
            [(complete, JsonValue::Bool(value)), (id, JsonValue::String(target)), (op, JsonValue::String(kind))]
                if complete == "complete" && id == "id" && op == "op" && kind == "set-complete" =>
            {
                Self::SetComplete {
                    id: target.clone(),
                    complete: *value,
                }
            }
            [(id, JsonValue::String(target)), (op, JsonValue::String(kind))]
                if id == "id" && op == "op" && kind == "remove" =>
            {
                Self::Remove { id: target.clone() }
            }
            _ => return Err(TodoRefusal::InvalidCommand),
        };
        // One validation path for locally constructed and decoded commands.
        command.encode_info()?;
        Ok(command)
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
        if !valid_text(&self.title, MAX_TODO_TITLE_BYTES) {
            return Err(TodoRefusal::InvalidTitle);
        }
        if self.next_id == 0 || self.items.len() > MAX_TODO_ITEMS {
            return Err(TodoRefusal::InvalidState);
        }
        for (index, item) in self.items.iter().enumerate() {
            if !valid_text(&item.text, MAX_TODO_TEXT_BYTES) {
                return Err(TodoRefusal::InvalidText);
            }
            if !valid_id(&item.id)
                || self.items[..index]
                    .iter()
                    .any(|previous| previous.id == item.id)
            {
                return Err(TodoRefusal::InvalidId);
            }
            let number = item.id[5..]
                .parse::<u32>()
                .map_err(|_| TodoRefusal::InvalidId)?;
            if number >= self.next_id {
                return Err(TodoRefusal::InvalidState);
            }
        }
        self.to_json().encode_info().map_err(TodoRefusal::Json)?;
        Ok(())
    }

    pub fn apply(&self, command: &TodoCommand) -> Result<Self, TodoRefusal> {
        self.validate()?;
        let (edit, next_id) = match command {
            TodoCommand::Add { text } => {
                if !valid_text(text, MAX_TODO_TEXT_BYTES) {
                    return Err(TodoRefusal::InvalidText);
                }
                if self.items.len() == MAX_TODO_ITEMS {
                    return Err(TodoRefusal::ItemCapacity);
                }
                let next_id = self
                    .next_id
                    .checked_add(1)
                    .ok_or(TodoRefusal::IdentityExhausted)?;
                let item = TodoItem {
                    id: format!("task-{}", self.next_id),
                    text: text.clone(),
                    complete: false,
                };
                (
                    object(vec![
                        ("key", JsonValue::String("id".into())),
                        ("op", JsonValue::String("append-unique".into())),
                        ("value", item.to_json()),
                    ]),
                    next_id,
                )
            }
            TodoCommand::SetComplete { id, complete } => {
                if !valid_id(id) {
                    return Err(TodoRefusal::InvalidId);
                }
                let item = self
                    .items
                    .iter()
                    .find(|item| item.id == *id)
                    .ok_or(TodoRefusal::MissingItem)?;
                if item.complete == *complete {
                    return Ok(self.clone());
                }
                (
                    object(vec![
                        ("field", JsonValue::String("complete".into())),
                        ("key", JsonValue::String("id".into())),
                        ("match", JsonValue::String(id.clone())),
                        ("op", JsonValue::String("set-field-by-key".into())),
                        ("value", JsonValue::Bool(*complete)),
                    ]),
                    self.next_id,
                )
            }
            TodoCommand::Remove { id } => {
                if !valid_id(id) {
                    return Err(TodoRefusal::InvalidId);
                }
                if !self.items.iter().any(|item| item.id == *id) {
                    return Err(TodoRefusal::MissingItem);
                }
                (
                    object(vec![
                        ("key", JsonValue::String("id".into())),
                        ("match", JsonValue::String(id.clone())),
                        ("op", JsonValue::String("remove-by-key".into())),
                    ]),
                    self.next_id,
                )
            }
        };
        let collection = JsonValue::Array(self.items.iter().map(TodoItem::to_json).collect());
        let JsonValue::Array(items) =
            json_collection_combine(&collection, &edit).map_err(TodoRefusal::Collection)?
        else {
            return Err(TodoRefusal::InvalidState);
        };
        let revision = self
            .revision
            .checked_add(1)
            .ok_or(TodoRefusal::RevisionExhausted)?;
        let next = Self {
            title: self.title.clone(),
            revision,
            next_id,
            items: items
                .iter()
                .map(TodoItem::from_json)
                .collect::<Result<Vec<_>, _>>()?,
        };
        next.validate()?;
        Ok(next)
    }

    pub fn to_json(&self) -> JsonValue {
        object(vec![
            (
                "items",
                JsonValue::Array(self.items.iter().map(TodoItem::to_json).collect()),
            ),
            ("next_id", number(self.next_id)),
            ("revision", number(self.revision)),
            ("title", JsonValue::String(self.title.clone())),
        ])
    }

    pub fn from_json(value: &JsonValue) -> Result<Self, TodoRefusal> {
        let JsonValue::Object(fields) = value else {
            return Err(TodoRefusal::InvalidState);
        };
        if fields.len() != 4
            || fields[0].0 != "items"
            || fields[1].0 != "next_id"
            || fields[2].0 != "revision"
            || fields[3].0 != "title"
        {
            return Err(TodoRefusal::InvalidState);
        }
        let JsonValue::Array(items) = &fields[0].1 else {
            return Err(TodoRefusal::InvalidState);
        };
        let JsonValue::String(title) = &fields[3].1 else {
            return Err(TodoRefusal::InvalidState);
        };
        let state = Self {
            title: title.clone(),
            revision: unsigned(&fields[2].1)?,
            next_id: unsigned(&fields[1].1)?,
            items: items
                .iter()
                .map(TodoItem::from_json)
                .collect::<Result<Vec<_>, _>>()?,
        };
        state.validate()?;
        Ok(state)
    }

    pub fn encode_info(&self) -> Result<Vec<u8>, TodoRefusal> {
        self.validate()?;
        self.to_json().encode_info().map_err(TodoRefusal::Json)
    }

    pub fn decode_info(bytes: &[u8]) -> Result<Self, TodoRefusal> {
        let value = JsonValue::decode_info(bytes).map_err(TodoRefusal::Json)?;
        Self::from_json(&value)
    }
}

impl TodoItem {
    fn to_json(&self) -> JsonValue {
        object(vec![
            ("complete", JsonValue::Bool(self.complete)),
            ("id", JsonValue::String(self.id.clone())),
            ("text", JsonValue::String(self.text.clone())),
        ])
    }

    fn from_json(value: &JsonValue) -> Result<Self, TodoRefusal> {
        let JsonValue::Object(fields) = value else {
            return Err(TodoRefusal::InvalidState);
        };
        if fields.len() != 3
            || fields[0].0 != "complete"
            || fields[1].0 != "id"
            || fields[2].0 != "text"
        {
            return Err(TodoRefusal::InvalidState);
        }
        let (JsonValue::Bool(complete), JsonValue::String(id), JsonValue::String(text)) =
            (&fields[0].1, &fields[1].1, &fields[2].1)
        else {
            return Err(TodoRefusal::InvalidState);
        };
        Ok(Self {
            id: id.clone(),
            text: text.clone(),
            complete: *complete,
        })
    }
}

fn object(fields: Vec<(&str, JsonValue)>) -> JsonValue {
    JsonValue::Object(
        fields
            .into_iter()
            .map(|(name, value)| (name.into(), value))
            .collect(),
    )
}

fn number(value: u32) -> JsonValue {
    JsonValue::Number(Scalar::from_raw_microunits(i64::from(value) * 1_000_000))
}

fn unsigned(value: &JsonValue) -> Result<u32, TodoRefusal> {
    let JsonValue::Number(value) = value else {
        return Err(TodoRefusal::InvalidState);
    };
    let raw = value.raw_microunits();
    if raw < 0 || raw % 1_000_000 != 0 {
        return Err(TodoRefusal::InvalidState);
    }
    u32::try_from(raw / 1_000_000).map_err(|_| TodoRefusal::InvalidState)
}

fn valid_text(value: &str, maximum: usize) -> bool {
    !value.trim().is_empty() && value.len() <= maximum && !value.chars().any(char::is_control)
}

fn valid_id(value: &str) -> bool {
    value.starts_with("task-")
        && value.len() <= 15
        && value[5..].parse::<u32>().is_ok_and(|number| number > 0)
}
