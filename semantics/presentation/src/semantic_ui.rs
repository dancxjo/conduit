//! Semantic application-presentation mechanisms and deterministic lowering.
//!
//! These are presentation concepts, not semantic Gears and not DOM widgets.
//! Applications own their state and choose these descriptions; Hosts own the
//! concrete manifestation of the resulting finite `ApplicationView`.

use crate::{
    ApplicationAction, ApplicationComponent, ApplicationEventKind, ApplicationView,
    ApplicationViewNode, ApplicationViewRefusal,
};
use alloc::{format, string::String, vec::Vec};

/// Stable identities for the shared application-presentation vocabulary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PresentationMechanismKind {
    Shell,
    Workbench,
    Panel,
    ActionGroup,
    Action,
    Status,
    Disclosure,
    Evidence,
    DefinitionTable,
    Definition,
    CodeBlock,
    FormField,
    Navigation,
    Stepper,
    Progress,
    Artifact,
    Download,
    DeviceChoice,
    PatchbayCanvas,
}

impl PresentationMechanismKind {
    pub const fn identity(self) -> &'static str {
        match self {
            Self::Shell => "conduit.presentation/shell@1",
            Self::Workbench => "conduit.presentation/workbench@1",
            Self::Panel => "conduit.presentation/panel@1",
            Self::ActionGroup => "conduit.presentation/action-group@1",
            Self::Action => "conduit.presentation/action@1",
            Self::Status => "conduit.presentation/status@1",
            Self::Disclosure => "conduit.presentation/disclosure@1",
            Self::Evidence => "conduit.presentation/evidence@1",
            Self::DefinitionTable => "conduit.presentation/definition-table@1",
            Self::Definition => "conduit.presentation/definition@1",
            Self::CodeBlock => "conduit.presentation/code-block@1",
            Self::FormField => "conduit.presentation/form-field@1",
            Self::Navigation => "conduit.presentation/navigation@1",
            Self::Stepper => "conduit.presentation/stepper@1",
            Self::Progress => "conduit.presentation/progress@1",
            Self::Artifact => "conduit.presentation/artifact@1",
            Self::Download => "conduit.presentation/download@1",
            Self::DeviceChoice => "conduit.presentation/device-choice@1",
            Self::PatchbayCanvas => "conduit.presentation/patchbay-canvas@1",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StatusKind {
    Ordinary,
    Warning,
    Failure,
    Success,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ActionAvailability {
    Available,
    Busy { detail: String },
    Unavailable { detail: String },
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticAction {
    pub identity: String,
    pub event: ApplicationEventKind,
    pub label: String,
    pub availability: ActionAvailability,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum FieldKind {
    Text,
    Select { options: Vec<String> },
    TextArea,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct FormField {
    pub label: String,
    pub value: String,
    pub value_capacity: u32,
    pub input_action: SemanticAction,
    pub kind: FieldKind,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PresentationMechanism {
    Shell,
    Workbench,
    Panel {
        title: String,
    },
    ActionGroup,
    Action(SemanticAction),
    Status {
        kind: StatusKind,
        title: String,
        detail: String,
    },
    Disclosure {
        summary: String,
    },
    Evidence {
        title: String,
    },
    DefinitionTable {
        title: String,
    },
    Definition {
        term: String,
        value: String,
    },
    CodeBlock {
        language: String,
        code: String,
    },
    FormField(FormField),
    Navigation {
        label: String,
    },
    Stepper {
        label: String,
    },
    Progress {
        title: String,
        current: u16,
        total: u16,
    },
    Artifact {
        kind: String,
        title: String,
        detail: String,
    },
    Download(SemanticAction),
    DeviceChoice(FormField),
    PatchbayCanvas {
        label: String,
    },
}

impl PresentationMechanism {
    pub const fn kind(&self) -> PresentationMechanismKind {
        match self {
            Self::Shell => PresentationMechanismKind::Shell,
            Self::Workbench => PresentationMechanismKind::Workbench,
            Self::Panel { .. } => PresentationMechanismKind::Panel,
            Self::ActionGroup => PresentationMechanismKind::ActionGroup,
            Self::Action(_) => PresentationMechanismKind::Action,
            Self::Status { .. } => PresentationMechanismKind::Status,
            Self::Disclosure { .. } => PresentationMechanismKind::Disclosure,
            Self::Evidence { .. } => PresentationMechanismKind::Evidence,
            Self::DefinitionTable { .. } => PresentationMechanismKind::DefinitionTable,
            Self::Definition { .. } => PresentationMechanismKind::Definition,
            Self::CodeBlock { .. } => PresentationMechanismKind::CodeBlock,
            Self::FormField(_) => PresentationMechanismKind::FormField,
            Self::Navigation { .. } => PresentationMechanismKind::Navigation,
            Self::Stepper { .. } => PresentationMechanismKind::Stepper,
            Self::Progress { .. } => PresentationMechanismKind::Progress,
            Self::Artifact { .. } => PresentationMechanismKind::Artifact,
            Self::Download(_) => PresentationMechanismKind::Download,
            Self::DeviceChoice(_) => PresentationMechanismKind::DeviceChoice,
            Self::PatchbayCanvas { .. } => PresentationMechanismKind::PatchbayCanvas,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticPresentationNode {
    pub key: String,
    pub mechanism: PresentationMechanism,
    pub children: Vec<Self>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SemanticApplicationView {
    pub revision: u32,
    pub root: SemanticPresentationNode,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SemanticPresentationRefusal {
    InvalidActionAvailability,
    InvalidDeviceChoice,
    InvalidField,
    InvalidProgress,
    ApplicationView(ApplicationViewRefusal),
}

impl From<ApplicationViewRefusal> for SemanticPresentationRefusal {
    fn from(value: ApplicationViewRefusal) -> Self {
        Self::ApplicationView(value)
    }
}

impl SemanticApplicationView {
    /// Lowers the semantic vocabulary without consulting a Host, DOM, or
    /// application-global registry. The resulting low-level view performs the
    /// existing finite node/depth/text/action validation.
    pub fn lower(&self) -> Result<ApplicationView, SemanticPresentationRefusal> {
        let mut nodes = Vec::new();
        let mut actions = Vec::new();
        lower_node(&self.root, None, &mut nodes, &mut actions)?;
        let view = ApplicationView {
            revision: self.revision,
            nodes,
            actions,
        };
        view.validate()?;
        Ok(view)
    }
}

fn lower_node(
    source: &SemanticPresentationNode,
    parent: Option<u8>,
    nodes: &mut Vec<ApplicationViewNode>,
    actions: &mut Vec<ApplicationAction>,
) -> Result<(), SemanticPresentationRefusal> {
    let index = u8::try_from(nodes.len()).map_err(|_| {
        SemanticPresentationRefusal::ApplicationView(ApplicationViewRefusal::TooManyNodes)
    })?;
    let lowered = lower_mechanism(&source.mechanism, actions)?;
    nodes.push(ApplicationViewNode {
        parent,
        component: lowered.component,
        key: source.key.clone(),
        text: lowered.text,
        value: lowered.value,
        value_capacity: lowered.value_capacity,
        action: lowered.action,
    });
    if let PresentationMechanism::FormField(FormField {
        kind: FieldKind::Select { options },
        ..
    })
    | PresentationMechanism::DeviceChoice(FormField {
        kind: FieldKind::Select { options },
        ..
    }) = &source.mechanism
    {
        for (option_index, option) in options.iter().enumerate() {
            nodes.push(ApplicationViewNode {
                parent: Some(index),
                component: ApplicationComponent::Option,
                key: format!("{}-o{option_index}", source.key),
                text: option.clone(),
                value: option.clone(),
                value_capacity: u32::try_from(option.len()).unwrap_or(u32::MAX).max(1),
                action: None,
            });
        }
    }
    for child in &source.children {
        lower_node(child, Some(index), nodes, actions)?;
    }
    Ok(())
}

struct LoweredMechanism {
    component: ApplicationComponent,
    text: String,
    value: String,
    value_capacity: u32,
    action: Option<u8>,
}

fn lower_mechanism(
    mechanism: &PresentationMechanism,
    actions: &mut Vec<ApplicationAction>,
) -> Result<LoweredMechanism, SemanticPresentationRefusal> {
    let empty = String::new();
    let (component, text, value, value_capacity, action) = match mechanism {
        PresentationMechanism::Shell => {
            (ApplicationComponent::Shell, empty, String::new(), 0, None)
        }
        PresentationMechanism::Workbench => {
            (ApplicationComponent::Grid, empty, String::new(), 0, None)
        }
        PresentationMechanism::Panel { title } => (
            ApplicationComponent::Panel,
            title.clone(),
            String::new(),
            0,
            None,
        ),
        PresentationMechanism::ActionGroup => (
            ApplicationComponent::ActionGroup,
            empty,
            String::new(),
            0,
            None,
        ),
        PresentationMechanism::Action(action) => {
            action_node(action, ApplicationComponent::Button, actions)?
        }
        PresentationMechanism::Status {
            kind,
            title,
            detail,
        } => {
            let component = match kind {
                StatusKind::Ordinary | StatusKind::Warning => ApplicationComponent::Status,
                StatusKind::Failure => ApplicationComponent::FailureStatus,
                StatusKind::Success => ApplicationComponent::SuccessStatus,
            };
            (component, titled(title, detail), String::new(), 0, None)
        }
        PresentationMechanism::Disclosure { summary } => (
            ApplicationComponent::Disclosure,
            summary.clone(),
            String::new(),
            0,
            None,
        ),
        PresentationMechanism::Evidence { title }
        | PresentationMechanism::DefinitionTable { title } => (
            ApplicationComponent::Table,
            title.clone(),
            String::new(),
            0,
            None,
        ),
        PresentationMechanism::Definition { term, value } => (
            ApplicationComponent::Grid,
            format!("{term}: {value}"),
            String::new(),
            0,
            None,
        ),
        PresentationMechanism::CodeBlock { language, code } => (
            ApplicationComponent::Code,
            format!("{language}\n{code}"),
            String::new(),
            0,
            None,
        ),
        PresentationMechanism::FormField(field) => field_node(field, actions, false)?,
        PresentationMechanism::Navigation { label } | PresentationMechanism::Stepper { label } => (
            ApplicationComponent::Navigation,
            label.clone(),
            String::new(),
            0,
            None,
        ),
        PresentationMechanism::Progress {
            title,
            current,
            total,
        } => {
            if *total == 0 || current > total {
                return Err(SemanticPresentationRefusal::InvalidProgress);
            }
            (
                ApplicationComponent::Status,
                format!("{title}: {current} of {total}"),
                String::new(),
                0,
                None,
            )
        }
        PresentationMechanism::Artifact {
            kind,
            title,
            detail,
        } => (
            ApplicationComponent::Panel,
            format!("{title} [{kind}] — {detail}"),
            String::new(),
            0,
            None,
        ),
        PresentationMechanism::Download(action) => {
            action_node(action, ApplicationComponent::Button, actions)?
        }
        PresentationMechanism::DeviceChoice(field) => field_node(field, actions, true)?,
        PresentationMechanism::PatchbayCanvas { label } => (
            ApplicationComponent::PatchbayCanvas,
            label.clone(),
            String::new(),
            0,
            None,
        ),
    };
    Ok(LoweredMechanism {
        component,
        text,
        value,
        value_capacity,
        action,
    })
}

fn action_node(
    action: &SemanticAction,
    component: ApplicationComponent,
    actions: &mut Vec<ApplicationAction>,
) -> Result<(ApplicationComponent, String, String, u32, Option<u8>), SemanticPresentationRefusal> {
    let (label, enabled) = availability_label(action)?;
    let index = enabled.then(|| admit_action(action, actions)).transpose()?;
    Ok((component, label, String::new(), 0, index))
}

fn field_node(
    field: &FormField,
    actions: &mut Vec<ApplicationAction>,
    device: bool,
) -> Result<(ApplicationComponent, String, String, u32, Option<u8>), SemanticPresentationRefusal> {
    let component = match &field.kind {
        FieldKind::Text if !device => ApplicationComponent::TextInput,
        FieldKind::TextArea if !device => ApplicationComponent::TextArea,
        FieldKind::Select { options } if !options.is_empty() => ApplicationComponent::Select,
        _ if device => return Err(SemanticPresentationRefusal::InvalidDeviceChoice),
        _ => return Err(SemanticPresentationRefusal::InvalidField),
    };
    let (label, enabled) = availability_label(&field.input_action)?;
    let text = titled(&field.label, &label);
    let index = enabled
        .then(|| admit_action(&field.input_action, actions))
        .transpose()?;
    Ok((
        component,
        text,
        field.value.clone(),
        field.value_capacity,
        index,
    ))
}

fn availability_label(
    action: &SemanticAction,
) -> Result<(String, bool), SemanticPresentationRefusal> {
    match &action.availability {
        ActionAvailability::Available => Ok((action.label.clone(), true)),
        ActionAvailability::Busy { detail } if !detail.is_empty() => {
            Ok((format!("{} — busy: {detail}", action.label), false))
        }
        ActionAvailability::Unavailable { detail } if !detail.is_empty() => {
            Ok((format!("{} — unavailable: {detail}", action.label), false))
        }
        _ => Err(SemanticPresentationRefusal::InvalidActionAvailability),
    }
}

fn admit_action(
    action: &SemanticAction,
    actions: &mut Vec<ApplicationAction>,
) -> Result<u8, SemanticPresentationRefusal> {
    if let Some(index) = actions
        .iter()
        .position(|candidate| candidate.id == action.identity && candidate.event == action.event)
    {
        return Ok(index as u8);
    }
    let index = u8::try_from(actions.len()).map_err(|_| {
        SemanticPresentationRefusal::ApplicationView(ApplicationViewRefusal::TooManyActions)
    })?;
    actions.push(ApplicationAction {
        id: action.identity.clone(),
        event: action.event,
    });
    Ok(index)
}

fn titled(title: &str, detail: &str) -> String {
    if title.is_empty() {
        detail.into()
    } else if detail.is_empty() {
        title.into()
    } else {
        format!("{title} — {detail}")
    }
}
