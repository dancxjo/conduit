//! Finite renderer-neutral application views and browser-independent actions.

use alloc::{string::String, vec::Vec};

mod structure;

pub const APPLICATION_VIEW_VERSION: u8 = 8;
/// Version 7 omitted native grouped-choice components.
pub const RETIRED_APPLICATION_VIEW_VERSION: u8 = 7;
pub const MAX_APPLICATION_VIEW_NODES: usize = 40;
pub const MAX_APPLICATION_VIEW_DEPTH: usize = 8;
pub const MAX_APPLICATION_VIEW_KEY_BYTES: usize = 32;
pub const MAX_APPLICATION_VIEW_TEXT_BYTES: usize = 256;
pub const MAX_APPLICATION_ACTIONS: usize = 16;
pub const MAX_APPLICATION_ACTION_ID_BYTES: usize = 48;
pub const MAX_APPLICATION_CONTROL_VALUE_BYTES: usize = 65_536;
pub const MAX_APPLICATION_EVENT_BYTES: usize = MAX_APPLICATION_CONTROL_VALUE_BYTES;
pub const MAX_APPLICATION_EVENT_ENCODED_BYTES: usize =
    11 + MAX_APPLICATION_ACTION_ID_BYTES + MAX_APPLICATION_EVENT_BYTES;
pub const MAX_APPLICATION_EVENT_QUEUE: usize = 8;
pub const MAX_APPLICATION_EVENT_QUEUE_BYTES: usize = 131_072;
pub const MAX_APPLICATION_VIEW_BYTES: usize = 131_072;
/// Phase-one views admit no application-selected external resources.
pub const MAX_APPLICATION_VIEW_RESOURCES: usize = 0;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ApplicationComponent {
    Shell = 1,
    Masthead = 2,
    Main = 3,
    Stack = 4,
    Panel = 5,
    Heading = 6,
    Paragraph = 7,
    Button = 8,
    Status = 9,
    Disclosure = 10,
    PatchbayCanvas = 11,
    Navigation = 12,
    Code = 13,
    ActionGroup = 14,
    TextInput = 15,
    Select = 16,
    TextArea = 17,
    Table = 18,
    Grid = 19,
    SuccessStatus = 20,
    FailureStatus = 21,
    Option = 22,
    Summary = 23,
    WarningStatus = 24,
    MissingEvidence = 25,
    StaleEvidence = 26,
    RefusedEvidence = 27,
    FailedEvidence = 28,
    SuccessfulEvidence = 29,
    DefinitionTable = 30,
    Definition = 31,
    CodeBlock = 32,
    Artifact = 33,
    FormField = 34,
    FieldLabel = 35,
    FieldHelp = 36,
    FieldError = 37,
    Stepper = 38,
    Progress = 39,
    ChoiceGroup = 40,
    ChoiceLegend = 41,
    ChoiceLabel = 42,
    Checkbox = 43,
}

/// Renderer-neutral state for an interactive presentation node.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
#[repr(u8)]
pub enum ApplicationNodeState {
    #[default]
    Ready = 1,
    Busy = 2,
    Unavailable = 3,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum ApplicationEventKind {
    Activate = 1,
    Change = 2,
    Input = 3,
    Toggle = 4,
    Submit = 5,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApplicationAction {
    pub id: String,
    pub event: ApplicationEventKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApplicationViewNode {
    pub parent: Option<u8>,
    pub component: ApplicationComponent,
    pub key: String,
    pub text: String,
    pub value: String,
    pub value_capacity: u32,
    pub action: Option<u8>,
    pub state: ApplicationNodeState,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ApplicationView {
    pub revision: u32,
    pub nodes: Vec<ApplicationViewNode>,
    pub actions: Vec<ApplicationAction>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ApplicationViewRefusal {
    Empty,
    TooManyNodes,
    TooDeep,
    DuplicateKey,
    UnknownParent,
    TextTooLong,
    InvalidControlValue,
    TooManyActions,
    ActionIdTooLong,
    DuplicateAction,
    UnknownAction,
    InvalidNodeState,
    MalformedEncoding,
    OversizedEncoding,
    UnsupportedVersion,
    StaleRevision,
    EventTooLarge,
    QueuePressure,
}

impl ApplicationView {
    pub fn validate(&self) -> Result<(), ApplicationViewRefusal> {
        if self.nodes.is_empty() {
            return Err(ApplicationViewRefusal::Empty);
        }
        if self.nodes.len() > MAX_APPLICATION_VIEW_NODES {
            return Err(ApplicationViewRefusal::TooManyNodes);
        }
        if self.actions.len() > MAX_APPLICATION_ACTIONS {
            return Err(ApplicationViewRefusal::TooManyActions);
        }
        for (index, node) in self.nodes.iter().enumerate() {
            if node.key.is_empty()
                || node.key.len() > MAX_APPLICATION_VIEW_KEY_BYTES
                || node.text.len() > MAX_APPLICATION_VIEW_TEXT_BYTES
            {
                return Err(ApplicationViewRefusal::TextTooLong);
            }
            let has_value = matches!(
                node.component,
                ApplicationComponent::TextInput
                    | ApplicationComponent::Select
                    | ApplicationComponent::TextArea
                    | ApplicationComponent::Option
                    | ApplicationComponent::Definition
                    | ApplicationComponent::CodeBlock
                    | ApplicationComponent::Stepper
                    | ApplicationComponent::Progress
                    | ApplicationComponent::Checkbox
            );
            let value_capacity = usize::try_from(node.value_capacity)
                .map_err(|_| ApplicationViewRefusal::InvalidControlValue)?;
            if has_value
                && (value_capacity == 0
                    || value_capacity > MAX_APPLICATION_CONTROL_VALUE_BYTES
                    || node.value.len() > value_capacity)
                || (!has_value
                    && node.component != ApplicationComponent::Navigation
                    && (value_capacity != 0 || !node.value.is_empty()))
            {
                return Err(ApplicationViewRefusal::InvalidControlValue);
            }
            if node.component == ApplicationComponent::Navigation
                && ((!node.value.is_empty()
                    && (value_capacity == 0
                        || value_capacity > MAX_APPLICATION_CONTROL_VALUE_BYTES
                        || node.value.len() > value_capacity))
                    || (node.value.is_empty() && value_capacity != 0))
            {
                return Err(ApplicationViewRefusal::InvalidControlValue);
            }
            if matches!(
                node.component,
                ApplicationComponent::Stepper | ApplicationComponent::Progress
            ) && !structure::valid_progress(&node.value)
            {
                return Err(ApplicationViewRefusal::InvalidControlValue);
            }
            if node.component == ApplicationComponent::Stepper && node.value.starts_with("0/") {
                return Err(ApplicationViewRefusal::InvalidControlValue);
            }
            if node.component == ApplicationComponent::Navigation
                && !node.value.is_empty()
                && !self.nodes.iter().enumerate().any(|(child_index, child)| {
                    child_index > index
                        && child.parent == Some(index as u8)
                        && child.component == ApplicationComponent::Button
                        && child.key == node.value
                })
            {
                return Err(ApplicationViewRefusal::InvalidControlValue);
            }
            if self.nodes[..index]
                .iter()
                .any(|other| other.key == node.key)
            {
                return Err(ApplicationViewRefusal::DuplicateKey);
            }
            let depth = match node.parent {
                None if index == 0 => 1,
                None => return Err(ApplicationViewRefusal::UnknownParent),
                Some(parent) if usize::from(parent) < index => self.depth(usize::from(parent))? + 1,
                _ => return Err(ApplicationViewRefusal::UnknownParent),
            };
            if depth > MAX_APPLICATION_VIEW_DEPTH {
                return Err(ApplicationViewRefusal::TooDeep);
            }
            if node
                .action
                .is_some_and(|action| usize::from(action) >= self.actions.len())
            {
                return Err(ApplicationViewRefusal::UnknownAction);
            }
            if node.state != ApplicationNodeState::Ready
                && (node.action.is_some()
                    || !matches!(
                        node.component,
                        ApplicationComponent::Button
                            | ApplicationComponent::TextInput
                            | ApplicationComponent::Select
                            | ApplicationComponent::TextArea
                            | ApplicationComponent::Checkbox
                    ))
            {
                return Err(ApplicationViewRefusal::InvalidNodeState);
            }
        }
        for (index, action) in self.actions.iter().enumerate() {
            if action.id.is_empty() || action.id.len() > MAX_APPLICATION_ACTION_ID_BYTES {
                return Err(ApplicationViewRefusal::ActionIdTooLong);
            }
            if self.actions[..index]
                .iter()
                .any(|other| other.id == action.id)
            {
                return Err(ApplicationViewRefusal::DuplicateAction);
            }
        }
        structure::validate(&self.nodes)?;
        Ok(())
    }

    fn depth(&self, mut index: usize) -> Result<usize, ApplicationViewRefusal> {
        let mut depth = 1;
        while let Some(parent) = self
            .nodes
            .get(index)
            .ok_or(ApplicationViewRefusal::UnknownParent)?
            .parent
        {
            depth += 1;
            index = usize::from(parent);
            if depth > MAX_APPLICATION_VIEW_DEPTH {
                return Err(ApplicationViewRefusal::TooDeep);
            }
        }
        Ok(depth)
    }

    pub fn encode(&self) -> Result<Vec<u8>, ApplicationViewRefusal> {
        self.validate()?;
        let mut out = Vec::with_capacity(8 + self.nodes.len() * 16);
        out.push(APPLICATION_VIEW_VERSION);
        out.extend_from_slice(&self.revision.to_le_bytes());
        out.push(self.nodes.len() as u8);
        out.push(self.actions.len() as u8);
        for action in &self.actions {
            out.push(action.event as u8);
            out.push(action.id.len() as u8);
            out.extend_from_slice(action.id.as_bytes());
        }
        for node in &self.nodes {
            out.push(node.parent.unwrap_or(u8::MAX));
            out.push(node.component as u8);
            out.push(node.state as u8);
            out.push(node.action.unwrap_or(u8::MAX));
            out.push(node.key.len() as u8);
            out.extend_from_slice(&(node.text.len() as u16).to_le_bytes());
            out.extend_from_slice(&(node.value.len() as u32).to_le_bytes());
            out.extend_from_slice(&node.value_capacity.to_le_bytes());
            out.extend_from_slice(node.key.as_bytes());
            out.extend_from_slice(node.text.as_bytes());
            out.extend_from_slice(node.value.as_bytes());
        }
        if out.len() > MAX_APPLICATION_VIEW_BYTES {
            return Err(ApplicationViewRefusal::OversizedEncoding);
        }
        Ok(out)
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, ApplicationViewRefusal> {
        if encoded.len() > MAX_APPLICATION_VIEW_BYTES {
            return Err(ApplicationViewRefusal::OversizedEncoding);
        }
        let mut cursor = Cursor::new(encoded);
        if cursor.byte()? != APPLICATION_VIEW_VERSION {
            return Err(ApplicationViewRefusal::UnsupportedVersion);
        }
        let revision = cursor.u32()?;
        let node_count = usize::from(cursor.byte()?);
        let action_count = usize::from(cursor.byte()?);
        if node_count > MAX_APPLICATION_VIEW_NODES {
            return Err(ApplicationViewRefusal::TooManyNodes);
        }
        if action_count > MAX_APPLICATION_ACTIONS {
            return Err(ApplicationViewRefusal::TooManyActions);
        }
        let mut actions = Vec::with_capacity(action_count);
        for _ in 0..action_count {
            let event = decode_event_kind(cursor.byte()?)?;
            let length = usize::from(cursor.byte()?);
            actions.push(ApplicationAction {
                id: cursor.text(length)?.into(),
                event,
            });
        }
        let mut nodes = Vec::with_capacity(node_count);
        for _ in 0..node_count {
            let parent = match cursor.byte()? {
                u8::MAX => None,
                value => Some(value),
            };
            let component = decode_component(cursor.byte()?)?;
            let state = decode_node_state(cursor.byte()?)?;
            let action = match cursor.byte()? {
                u8::MAX => None,
                value => Some(value),
            };
            let key_length = usize::from(cursor.byte()?);
            let text_length = usize::from(cursor.u16()?);
            let value_length = usize::try_from(cursor.u32()?)
                .map_err(|_| ApplicationViewRefusal::InvalidControlValue)?;
            let value_capacity = cursor.u32()?;
            nodes.push(ApplicationViewNode {
                parent,
                component,
                action,
                state,
                key: cursor.text(key_length)?.into(),
                text: cursor.text(text_length)?.into(),
                value: cursor.text(value_length)?.into(),
                value_capacity,
            });
        }
        if !cursor.complete() {
            return Err(ApplicationViewRefusal::MalformedEncoding);
        }
        let view = Self {
            revision,
            nodes,
            actions,
        };
        view.validate()?;
        Ok(view)
    }
}

pub(super) struct Cursor<'a> {
    encoded: &'a [u8],
    offset: usize,
}

impl<'a> Cursor<'a> {
    pub(super) const fn new(encoded: &'a [u8]) -> Self {
        Self { encoded, offset: 0 }
    }
    pub(super) fn bytes(&mut self, length: usize) -> Result<&'a [u8], ApplicationViewRefusal> {
        let end = self
            .offset
            .checked_add(length)
            .ok_or(ApplicationViewRefusal::MalformedEncoding)?;
        let value = self
            .encoded
            .get(self.offset..end)
            .ok_or(ApplicationViewRefusal::MalformedEncoding)?;
        self.offset = end;
        Ok(value)
    }
    pub(super) fn byte(&mut self) -> Result<u8, ApplicationViewRefusal> {
        Ok(self.bytes(1)?[0])
    }
    pub(super) fn u16(&mut self) -> Result<u16, ApplicationViewRefusal> {
        Ok(u16::from_le_bytes(
            self.bytes(2)?
                .try_into()
                .map_err(|_| ApplicationViewRefusal::MalformedEncoding)?,
        ))
    }
    pub(super) fn u32(&mut self) -> Result<u32, ApplicationViewRefusal> {
        Ok(u32::from_le_bytes(
            self.bytes(4)?
                .try_into()
                .map_err(|_| ApplicationViewRefusal::MalformedEncoding)?,
        ))
    }
    pub(super) fn text(&mut self, length: usize) -> Result<&'a str, ApplicationViewRefusal> {
        core::str::from_utf8(self.bytes(length)?)
            .map_err(|_| ApplicationViewRefusal::MalformedEncoding)
    }
    pub(super) fn complete(&self) -> bool {
        self.offset == self.encoded.len()
    }
}

pub(super) fn decode_event_kind(value: u8) -> Result<ApplicationEventKind, ApplicationViewRefusal> {
    match value {
        1 => Ok(ApplicationEventKind::Activate),
        2 => Ok(ApplicationEventKind::Change),
        3 => Ok(ApplicationEventKind::Input),
        4 => Ok(ApplicationEventKind::Toggle),
        5 => Ok(ApplicationEventKind::Submit),
        _ => Err(ApplicationViewRefusal::MalformedEncoding),
    }
}

fn decode_component(value: u8) -> Result<ApplicationComponent, ApplicationViewRefusal> {
    match value {
        1 => Ok(ApplicationComponent::Shell),
        2 => Ok(ApplicationComponent::Masthead),
        3 => Ok(ApplicationComponent::Main),
        4 => Ok(ApplicationComponent::Stack),
        5 => Ok(ApplicationComponent::Panel),
        6 => Ok(ApplicationComponent::Heading),
        7 => Ok(ApplicationComponent::Paragraph),
        8 => Ok(ApplicationComponent::Button),
        9 => Ok(ApplicationComponent::Status),
        10 => Ok(ApplicationComponent::Disclosure),
        11 => Ok(ApplicationComponent::PatchbayCanvas),
        12 => Ok(ApplicationComponent::Navigation),
        13 => Ok(ApplicationComponent::Code),
        14 => Ok(ApplicationComponent::ActionGroup),
        15 => Ok(ApplicationComponent::TextInput),
        16 => Ok(ApplicationComponent::Select),
        17 => Ok(ApplicationComponent::TextArea),
        18 => Ok(ApplicationComponent::Table),
        19 => Ok(ApplicationComponent::Grid),
        20 => Ok(ApplicationComponent::SuccessStatus),
        21 => Ok(ApplicationComponent::FailureStatus),
        22 => Ok(ApplicationComponent::Option),
        23 => Ok(ApplicationComponent::Summary),
        24 => Ok(ApplicationComponent::WarningStatus),
        25 => Ok(ApplicationComponent::MissingEvidence),
        26 => Ok(ApplicationComponent::StaleEvidence),
        27 => Ok(ApplicationComponent::RefusedEvidence),
        28 => Ok(ApplicationComponent::FailedEvidence),
        29 => Ok(ApplicationComponent::SuccessfulEvidence),
        30 => Ok(ApplicationComponent::DefinitionTable),
        31 => Ok(ApplicationComponent::Definition),
        32 => Ok(ApplicationComponent::CodeBlock),
        33 => Ok(ApplicationComponent::Artifact),
        34 => Ok(ApplicationComponent::FormField),
        35 => Ok(ApplicationComponent::FieldLabel),
        36 => Ok(ApplicationComponent::FieldHelp),
        37 => Ok(ApplicationComponent::FieldError),
        38 => Ok(ApplicationComponent::Stepper),
        39 => Ok(ApplicationComponent::Progress),
        40 => Ok(ApplicationComponent::ChoiceGroup),
        41 => Ok(ApplicationComponent::ChoiceLegend),
        42 => Ok(ApplicationComponent::ChoiceLabel),
        43 => Ok(ApplicationComponent::Checkbox),
        _ => Err(ApplicationViewRefusal::MalformedEncoding),
    }
}

fn decode_node_state(value: u8) -> Result<ApplicationNodeState, ApplicationViewRefusal> {
    match value {
        1 => Ok(ApplicationNodeState::Ready),
        2 => Ok(ApplicationNodeState::Busy),
        3 => Ok(ApplicationNodeState::Unavailable),
        _ => Err(ApplicationViewRefusal::MalformedEncoding),
    }
}
