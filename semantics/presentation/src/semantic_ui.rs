//! Semantic application-presentation mechanisms and deterministic lowering.
//!
//! These are presentation concepts, not semantic Gears and not DOM widgets.
//! Applications own their state and choose these descriptions; Hosts own the
//! concrete manifestation of the resulting finite `ApplicationView`.

use crate::{
    ApplicationAction, ApplicationComponent, ApplicationEventKind, ApplicationNodeState,
    ApplicationView, ApplicationViewNode, ApplicationViewRefusal,
};
use alloc::{string::String, vec::Vec};

mod evidence_lowering;
mod form_lowering;
mod mechanism_lowering;
use evidence_lowering::{
    code_node, definition_node, evidence_component, push_definition, push_evidence_state,
    validate_artifact, validate_evidence,
};
use form_lowering::{lower_form_field, progress_node};
use mechanism_lowering::{action_node, titled};

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum EvidenceDisposition {
    Missing,
    Stale,
    Refused,
    Failed,
    Succeeded,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EvidencePresentation {
    pub title: String,
    pub disposition: EvidenceDisposition,
    pub identity: String,
    pub provenance: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ArtifactPresentation {
    pub title: String,
    pub kind: String,
    pub detail: String,
    pub identity: String,
    pub provenance: String,
    pub disposition: EvidenceDisposition,
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
    pub help: String,
    pub error: Option<String>,
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
    Evidence(EvidencePresentation),
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
        current: String,
    },
    Stepper {
        label: String,
        current: u16,
        total: u16,
    },
    Progress {
        title: String,
        current: u16,
        total: u16,
    },
    Artifact(ArtifactPresentation),
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
            Self::Evidence(_) => PresentationMechanismKind::Evidence,
            Self::DefinitionTable { .. } => PresentationMechanismKind::DefinitionTable,
            Self::Definition { .. } => PresentationMechanismKind::Definition,
            Self::CodeBlock { .. } => PresentationMechanismKind::CodeBlock,
            Self::FormField(_) => PresentationMechanismKind::FormField,
            Self::Navigation { .. } => PresentationMechanismKind::Navigation,
            Self::Stepper { .. } => PresentationMechanismKind::Stepper,
            Self::Progress { .. } => PresentationMechanismKind::Progress,
            Self::Artifact(_) => PresentationMechanismKind::Artifact,
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
    InvalidNavigation,
    InvalidProgress,
    InvalidEvidence,
    InvalidDefinition,
    InvalidCodeBlock,
    InvalidArtifact,
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
        state: lowered.state,
    });
    match &source.mechanism {
        PresentationMechanism::Evidence(evidence) => {
            push_definition(index, nodes, "Identity", &evidence.identity)?;
            push_definition(index, nodes, "Provenance", &evidence.provenance)?;
        }
        PresentationMechanism::Artifact(artifact) => {
            push_evidence_state(index, nodes, artifact.disposition)?;
            push_definition(index, nodes, "Kind", &artifact.kind)?;
            push_definition(index, nodes, "Identity", &artifact.identity)?;
            push_definition(index, nodes, "Provenance", &artifact.provenance)?;
            push_definition(index, nodes, "Detail", &artifact.detail)?;
        }
        _ => {}
    }
    if let PresentationMechanism::FormField(field) = &source.mechanism {
        lower_form_field(index, &source.key, field, nodes, actions, false)?;
    } else if let PresentationMechanism::DeviceChoice(field) = &source.mechanism {
        lower_form_field(index, &source.key, field, nodes, actions, true)?;
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
    state: ApplicationNodeState,
}

fn lower_mechanism(
    mechanism: &PresentationMechanism,
    actions: &mut Vec<ApplicationAction>,
) -> Result<LoweredMechanism, SemanticPresentationRefusal> {
    let empty = String::new();
    let (component, text, value, value_capacity, action, state) = match mechanism {
        PresentationMechanism::Shell => (
            ApplicationComponent::Shell,
            empty,
            String::new(),
            0,
            None,
            ApplicationNodeState::Ready,
        ),
        PresentationMechanism::Workbench => (
            ApplicationComponent::Grid,
            empty,
            String::new(),
            0,
            None,
            ApplicationNodeState::Ready,
        ),
        PresentationMechanism::Panel { title } => (
            ApplicationComponent::Panel,
            title.clone(),
            String::new(),
            0,
            None,
            ApplicationNodeState::Ready,
        ),
        PresentationMechanism::ActionGroup => (
            ApplicationComponent::ActionGroup,
            empty,
            String::new(),
            0,
            None,
            ApplicationNodeState::Ready,
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
                StatusKind::Ordinary => ApplicationComponent::Status,
                StatusKind::Warning => ApplicationComponent::WarningStatus,
                StatusKind::Failure => ApplicationComponent::FailureStatus,
                StatusKind::Success => ApplicationComponent::SuccessStatus,
            };
            (
                component,
                titled(title, detail),
                String::new(),
                0,
                None,
                ApplicationNodeState::Ready,
            )
        }
        PresentationMechanism::Disclosure { summary } => (
            ApplicationComponent::Disclosure,
            summary.clone(),
            String::new(),
            0,
            None,
            ApplicationNodeState::Ready,
        ),
        PresentationMechanism::Evidence(evidence) => {
            validate_evidence(evidence)?;
            (
                evidence_component(evidence.disposition),
                evidence.title.clone(),
                String::new(),
                0,
                None,
                ApplicationNodeState::Ready,
            )
        }
        PresentationMechanism::DefinitionTable { title } => (
            ApplicationComponent::DefinitionTable,
            title.clone(),
            String::new(),
            0,
            None,
            ApplicationNodeState::Ready,
        ),
        PresentationMechanism::Definition { term, value } => definition_node(term, value)?,
        PresentationMechanism::CodeBlock { language, code } => code_node(language, code)?,
        PresentationMechanism::FormField(_) | PresentationMechanism::DeviceChoice(_) => (
            ApplicationComponent::FormField,
            empty,
            String::new(),
            0,
            None,
            ApplicationNodeState::Ready,
        ),
        PresentationMechanism::Navigation { label, current } => {
            if label.is_empty() || current.is_empty() {
                return Err(SemanticPresentationRefusal::InvalidNavigation);
            }
            (
                ApplicationComponent::Navigation,
                label.clone(),
                current.clone(),
                u32::try_from(current.len()).unwrap_or(u32::MAX).max(1),
                None,
                ApplicationNodeState::Ready,
            )
        }
        PresentationMechanism::Stepper {
            label,
            current,
            total,
        } => {
            if label.is_empty() || *current == 0 || *total == 0 || current > total {
                return Err(SemanticPresentationRefusal::InvalidProgress);
            }
            progress_node(ApplicationComponent::Stepper, label, *current, *total)
        }
        PresentationMechanism::Progress {
            title,
            current,
            total,
        } => {
            if title.is_empty() || *total == 0 || current > total {
                return Err(SemanticPresentationRefusal::InvalidProgress);
            }
            progress_node(ApplicationComponent::Progress, title, *current, *total)
        }
        PresentationMechanism::Artifact(artifact) => {
            validate_artifact(artifact)?;
            (
                ApplicationComponent::Artifact,
                artifact.title.clone(),
                String::new(),
                0,
                None,
                ApplicationNodeState::Ready,
            )
        }
        PresentationMechanism::Download(action) => {
            action_node(action, ApplicationComponent::Button, actions)?
        }
        PresentationMechanism::PatchbayCanvas { label } => (
            ApplicationComponent::PatchbayCanvas,
            label.clone(),
            String::new(),
            0,
            None,
            ApplicationNodeState::Ready,
        ),
    };
    Ok(LoweredMechanism {
        component,
        text,
        value,
        value_capacity,
        action,
        state,
    })
}
