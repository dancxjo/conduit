//! Exact finite Todo Form and allocation-free state transition for Play.

use core::str;

use crate::{
    TodoCommand, TodoItem, TodoRefusal, TodoState, MAX_TODO_ITEMS, MAX_TODO_TEXT_BYTES,
    MAX_TODO_TITLE_BYTES,
};

pub const STATE_VERSION: u8 = 1;
pub const COMMAND_VERSION: u8 = 1;
pub const STATE_MAX_BYTES: usize =
    11 + MAX_TODO_TITLE_BYTES + MAX_TODO_ITEMS * (6 + MAX_TODO_TEXT_BYTES);
pub const COMMAND_MAX_BYTES: usize = 3 + MAX_TODO_TEXT_BYTES;

#[derive(Clone, Copy)]
struct FixedItem {
    id: u32,
    complete: bool,
    text_len: u8,
    text: [u8; MAX_TODO_TEXT_BYTES],
}

impl FixedItem {
    const EMPTY: Self = Self {
        id: 0,
        complete: false,
        text_len: 0,
        text: [0; MAX_TODO_TEXT_BYTES],
    };
}

#[derive(Clone)]
pub struct FixedTodoState {
    title_len: u8,
    title: [u8; MAX_TODO_TITLE_BYTES],
    revision: u32,
    next_id: u32,
    count: u8,
    items: [FixedItem; MAX_TODO_ITEMS],
}

impl FixedTodoState {
    pub fn new() -> Self {
        Self {
            title_len: 0,
            title: [0; MAX_TODO_TITLE_BYTES],
            revision: 0,
            next_id: 1,
            count: 0,
            items: [FixedItem::EMPTY; MAX_TODO_ITEMS],
        }
    }

    pub fn from_public(state: &TodoState) -> Result<Self, TodoRefusal> {
        let mut fixed = Self::new();
        copy_text(
            &mut fixed.title,
            &mut fixed.title_len,
            &state.title,
            TodoRefusal::InvalidTitle,
        )?;
        if state.items.len() > MAX_TODO_ITEMS || state.next_id == 0 {
            return Err(TodoRefusal::InvalidState);
        }
        fixed.revision = state.revision;
        fixed.next_id = state.next_id;
        for item in &state.items {
            let id = parse_id(&item.id)?;
            if id >= fixed.next_id
                || fixed.items[..usize::from(fixed.count)]
                    .iter()
                    .any(|prior| prior.id == id)
            {
                return Err(TodoRefusal::InvalidState);
            }
            let slot = &mut fixed.items[usize::from(fixed.count)];
            slot.id = id;
            slot.complete = item.complete;
            copy_text(
                &mut slot.text,
                &mut slot.text_len,
                &item.text,
                TodoRefusal::InvalidText,
            )?;
            fixed.count += 1;
        }
        Ok(fixed)
    }

    pub fn into_public(self) -> TodoState {
        TodoState {
            title: str::from_utf8(&self.title[..usize::from(self.title_len)])
                .expect("validated title")
                .into(),
            revision: self.revision,
            next_id: self.next_id,
            items: self.items[..usize::from(self.count)]
                .iter()
                .map(|item| TodoItem {
                    id: alloc::format!("task-{}", item.id),
                    text: str::from_utf8(&item.text[..usize::from(item.text_len)])
                        .expect("validated text")
                        .into(),
                    complete: item.complete,
                })
                .collect(),
        }
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, TodoRefusal> {
        let mut cursor = Cursor { bytes, offset: 0 };
        if cursor.byte()? != STATE_VERSION {
            return Err(TodoRefusal::InvalidState);
        }
        let mut state = Self::new();
        let title_len = usize::from(cursor.byte()?);
        let title = cursor.take(title_len)?;
        copy_encoded_text(
            &mut state.title,
            &mut state.title_len,
            title,
            TodoRefusal::InvalidTitle,
        )?;
        state.revision = cursor.u32()?;
        state.next_id = cursor.u32()?;
        if state.next_id == 0 {
            return Err(TodoRefusal::InvalidState);
        }
        let count = usize::from(cursor.byte()?);
        if count > MAX_TODO_ITEMS {
            return Err(TodoRefusal::ItemCapacity);
        }
        for index in 0..count {
            let id = cursor.u32()?;
            let complete = match cursor.byte()? {
                0 => false,
                1 => true,
                _ => return Err(TodoRefusal::InvalidState),
            };
            let len = usize::from(cursor.byte()?);
            let text = cursor.take(len)?;
            if id == 0
                || id >= state.next_id
                || state.items[..index].iter().any(|prior| prior.id == id)
            {
                return Err(TodoRefusal::InvalidState);
            }
            let item = &mut state.items[index];
            item.id = id;
            item.complete = complete;
            copy_encoded_text(
                &mut item.text,
                &mut item.text_len,
                text,
                TodoRefusal::InvalidText,
            )?;
        }
        if cursor.offset != bytes.len() {
            return Err(TodoRefusal::InvalidState);
        }
        state.count = count as u8;
        Ok(state)
    }

    pub fn encode_into(&self, output: &mut [u8; STATE_MAX_BYTES]) -> usize {
        let mut offset = 0;
        output[offset] = STATE_VERSION;
        offset += 1;
        output[offset] = self.title_len;
        offset += 1;
        let title_len = usize::from(self.title_len);
        output[offset..offset + title_len].copy_from_slice(&self.title[..title_len]);
        offset += title_len;
        output[offset..offset + 4].copy_from_slice(&self.revision.to_le_bytes());
        offset += 4;
        output[offset..offset + 4].copy_from_slice(&self.next_id.to_le_bytes());
        offset += 4;
        output[offset] = self.count;
        offset += 1;
        for item in &self.items[..usize::from(self.count)] {
            output[offset..offset + 4].copy_from_slice(&item.id.to_le_bytes());
            offset += 4;
            output[offset] = u8::from(item.complete);
            offset += 1;
            output[offset] = item.text_len;
            offset += 1;
            let len = usize::from(item.text_len);
            output[offset..offset + len].copy_from_slice(&item.text[..len]);
            offset += len;
        }
        offset
    }

    pub fn apply(&mut self, command: &FixedTodoCommand) -> Result<bool, TodoRefusal> {
        match command {
            FixedTodoCommand::Add { text_len, text } => {
                if usize::from(self.count) == MAX_TODO_ITEMS {
                    return Err(TodoRefusal::ItemCapacity);
                }
                let next_id = self
                    .next_id
                    .checked_add(1)
                    .ok_or(TodoRefusal::IdentityExhausted)?;
                let revision = self
                    .revision
                    .checked_add(1)
                    .ok_or(TodoRefusal::RevisionExhausted)?;
                let slot = &mut self.items[usize::from(self.count)];
                slot.id = self.next_id;
                slot.complete = false;
                slot.text_len = *text_len;
                slot.text[..usize::from(*text_len)]
                    .copy_from_slice(&text[..usize::from(*text_len)]);
                self.count += 1;
                self.next_id = next_id;
                self.revision = revision;
                Ok(true)
            }
            FixedTodoCommand::SetComplete { id, complete } => {
                let index = self.find(*id)?;
                if self.items[index].complete == *complete {
                    return Ok(false);
                }
                let revision = self
                    .revision
                    .checked_add(1)
                    .ok_or(TodoRefusal::RevisionExhausted)?;
                self.items[index].complete = *complete;
                self.revision = revision;
                Ok(true)
            }
            FixedTodoCommand::Remove { id } => {
                let index = self.find(*id)?;
                let revision = self
                    .revision
                    .checked_add(1)
                    .ok_or(TodoRefusal::RevisionExhausted)?;
                for next in index + 1..usize::from(self.count) {
                    self.items[next - 1] = self.items[next];
                }
                self.count -= 1;
                self.items[usize::from(self.count)] = FixedItem::EMPTY;
                self.revision = revision;
                Ok(true)
            }
        }
    }

    fn find(&self, id: u32) -> Result<usize, TodoRefusal> {
        self.items[..usize::from(self.count)]
            .iter()
            .position(|item| item.id == id)
            .ok_or(TodoRefusal::MissingItem)
    }
}

impl Default for FixedTodoState {
    fn default() -> Self {
        Self::new()
    }
}

pub enum FixedTodoCommand {
    Add {
        text_len: u8,
        text: [u8; MAX_TODO_TEXT_BYTES],
    },
    SetComplete {
        id: u32,
        complete: bool,
    },
    Remove {
        id: u32,
    },
}

impl FixedTodoCommand {
    pub fn from_public(command: &TodoCommand) -> Result<Self, TodoRefusal> {
        match command {
            TodoCommand::Add { text } => {
                let mut bytes = [0; MAX_TODO_TEXT_BYTES];
                let mut len = 0;
                copy_text(&mut bytes, &mut len, text, TodoRefusal::InvalidText)?;
                Ok(Self::Add {
                    text_len: len,
                    text: bytes,
                })
            }
            TodoCommand::SetComplete { id, complete } => Ok(Self::SetComplete {
                id: parse_id(id)?,
                complete: *complete,
            }),
            TodoCommand::Remove { id } => Ok(Self::Remove { id: parse_id(id)? }),
        }
    }

    pub fn into_public(self) -> TodoCommand {
        match self {
            Self::Add { text_len, text } => TodoCommand::Add {
                text: str::from_utf8(&text[..usize::from(text_len)])
                    .expect("validated text")
                    .into(),
            },
            Self::SetComplete { id, complete } => TodoCommand::SetComplete {
                id: alloc::format!("task-{id}"),
                complete,
            },
            Self::Remove { id } => TodoCommand::Remove {
                id: alloc::format!("task-{id}"),
            },
        }
    }

    pub fn decode(bytes: &[u8]) -> Result<Self, TodoRefusal> {
        let mut cursor = Cursor { bytes, offset: 0 };
        if cursor.byte()? != COMMAND_VERSION {
            return Err(TodoRefusal::InvalidCommand);
        }
        let command = match cursor.byte()? {
            1 => {
                let len = usize::from(cursor.byte()?);
                let text = cursor.take(len)?;
                let mut bytes = [0; MAX_TODO_TEXT_BYTES];
                let mut text_len = 0;
                copy_encoded_text(&mut bytes, &mut text_len, text, TodoRefusal::InvalidText)?;
                Self::Add {
                    text_len,
                    text: bytes,
                }
            }
            2 => {
                let id = cursor.u32()?;
                let complete = match cursor.byte()? {
                    0 => false,
                    1 => true,
                    _ => return Err(TodoRefusal::InvalidCommand),
                };
                if id == 0 {
                    return Err(TodoRefusal::InvalidId);
                }
                Self::SetComplete { id, complete }
            }
            3 => {
                let id = cursor.u32()?;
                if id == 0 {
                    return Err(TodoRefusal::InvalidId);
                }
                Self::Remove { id }
            }
            _ => return Err(TodoRefusal::InvalidCommand),
        };
        if cursor.offset != bytes.len() {
            return Err(TodoRefusal::InvalidCommand);
        }
        Ok(command)
    }

    pub fn encode_into(&self, output: &mut [u8; COMMAND_MAX_BYTES]) -> usize {
        output[0] = COMMAND_VERSION;
        match self {
            Self::Add { text_len, text } => {
                output[1] = 1;
                output[2] = *text_len;
                let len = usize::from(*text_len);
                output[3..3 + len].copy_from_slice(&text[..len]);
                3 + len
            }
            Self::SetComplete { id, complete } => {
                output[1] = 2;
                output[2..6].copy_from_slice(&id.to_le_bytes());
                output[6] = u8::from(*complete);
                7
            }
            Self::Remove { id } => {
                output[1] = 3;
                output[2..6].copy_from_slice(&id.to_le_bytes());
                6
            }
        }
    }
}

struct Cursor<'a> {
    bytes: &'a [u8],
    offset: usize,
}
impl<'a> Cursor<'a> {
    fn take(&mut self, len: usize) -> Result<&'a [u8], TodoRefusal> {
        let end = self
            .offset
            .checked_add(len)
            .ok_or(TodoRefusal::InvalidState)?;
        let value = self
            .bytes
            .get(self.offset..end)
            .ok_or(TodoRefusal::InvalidState)?;
        self.offset = end;
        Ok(value)
    }
    fn byte(&mut self) -> Result<u8, TodoRefusal> {
        Ok(self.take(1)?[0])
    }
    fn u32(&mut self) -> Result<u32, TodoRefusal> {
        let bytes = self.take(4)?;
        Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }
}

fn parse_id(id: &str) -> Result<u32, TodoRefusal> {
    let digits = id.strip_prefix("task-").ok_or(TodoRefusal::InvalidId)?;
    let number = digits.parse::<u32>().map_err(|_| TodoRefusal::InvalidId)?;
    if number == 0 || alloc::format!("task-{number}") != id {
        return Err(TodoRefusal::InvalidId);
    }
    Ok(number)
}

fn copy_text<const N: usize>(
    dest: &mut [u8; N],
    len: &mut u8,
    text: &str,
    refusal: TodoRefusal,
) -> Result<(), TodoRefusal> {
    if text.trim().is_empty() || text.len() > N || text.chars().any(char::is_control) {
        return Err(refusal);
    }
    dest[..text.len()].copy_from_slice(text.as_bytes());
    *len = text.len() as u8;
    Ok(())
}
fn copy_encoded_text<const N: usize>(
    dest: &mut [u8; N],
    len: &mut u8,
    bytes: &[u8],
    refusal: TodoRefusal,
) -> Result<(), TodoRefusal> {
    let text = str::from_utf8(bytes).map_err(|_| refusal.clone())?;
    copy_text(dest, len, text, refusal)
}
