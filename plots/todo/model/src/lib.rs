#![no_std]

//! Bounded Todo meaning owned by the authored Todo Plot.
//!
//! This adapter validates application commands and state. The ordinary JSON
//! collection transition performs the item edit; neither owner nor Mask does.

extern crate alloc;

use alloc::{format, string::String, vec, vec::Vec};
use conduit_core::Scalar;
use conduit_web::{json_collection_combine, JsonCollectionRefusal, JsonRefusal, JsonValue};

pub const MAX_TODO_ITEMS: usize = 20;
pub const MAX_TODO_TEXT_BYTES: usize = 72;
pub const MAX_TODO_TITLE_BYTES: usize = 64;

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
    MissingItem,
    ItemCapacity,
    IdentityExhausted,
    RevisionExhausted,
    Json(JsonRefusal),
    Collection(JsonCollectionRefusal),
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
