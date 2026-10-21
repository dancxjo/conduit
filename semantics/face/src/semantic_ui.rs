//! Semantic application-face mechanisms and deterministic lowering.
//!
//! These are face concepts, not semantic Gears and not DOM widgets.
//! Applications own their state and choose these descriptions; Hosts own the
//! concrete manifestation of the resulting finite `ApplicationView`.

use crate::{
    AdmittedNavigationDestination, ApplicationAction, ApplicationComponent, ApplicationEventKind,
    ApplicationNodeState, ApplicationView, ApplicationViewNode, ApplicationViewRefusal,
    ChoiceMultiplicity, EvidenceDisposition, FaceMechanismKind, StatusKind,
};
use alloc::{string::String, vec::Vec};

mod choice_lowering;
mod evidence_lowering;
mod form_lowering;
mod mechanism_lowering;
mod structure_lowering;
use choice_lowering::lower_choice_group;
use evidence_lowering::{
    code_node, definition_node, evidence_component, push_definition, push_evidence_state,
    validate_artifact, validate_evidence,
};
use form_lowering::{lower_plot_field, progress_node};
use mechanism_lowering::{action_node, titled};
use structure_lowering::structural_node;

impl FaceMechanismKind {
    pub const fn identity(self) -> &'static str {
        match self {
            Self::Shell => "conduit.face/shell@1",
            Self::Workbench => "conduit.face/workbench@1",
            Self::Panel => "conduit.face/panel@1",
            Self::Heading => "conduit.face/heading@1",
            Self::Separator => "conduit.face/separator@1",
            Self::Grid => "conduit.face/grid@1",
            Self::ActionGroup => "conduit.face/action-group@1",
            Self::Action => "conduit.face/action@1",
            Self::Status => "conduit.face/status@1",
            Self::Disclosure => "conduit.face/disclosure@1",
            Self::Evidence => "conduit.face/evidence@1",
            Self::DefinitionTable => "conduit.face/definition-table@1",
            Self::Definition => "conduit.face/definition@1",
            Self::CodeBlock => "conduit.face/code-block@1",
            Self::FormField => "conduit.face/form-field@1",
            Self::ChoiceGroup => "conduit.face/choice-group@1",
            Self::Navigation => "conduit.face/navigation@1",
            Self::NavigationLink => "conduit.face/navigation-link@1",
            Self::Link => "conduit.face/link@1",
            Self::Stepper => "conduit.face/stepper@1",
            Self::Progress => "conduit.face/progress@1",
            Self::Artifact => "conduit.face/artifact@1",
            Self::Download => "conduit.face/download@1",
            Self::DeviceChoice => "conduit.face/device-choice@1",
            Self::PatchbayCanvas => "conduit.face/patchbay-canvas@1",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct EvidenceFace {
    pub title: String,
    pub disposition: EvidenceDisposition,
    pub identity: String,
    pub provenance: String,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ArtifactFace {
    pub title: String,
    pub kind: String,
    pub detail: String,
    pub identity: String,
    pub provenance: String,
    pub disposition: EvidenceDisposition,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum ActionAvailability {
    Available,
    Busy { detail: String },
    Unavailable { detail: String },
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SemanticAction {
    pub identity: String,
    pub event: ApplicationEventKind,
    pub label: String,
    pub availability: ActionAvailability,
}

mod form_field;
pub use form_field::{FieldKind, FormField, SelectOption};

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ChoiceOption {
    pub identity: String,
    pub label: String,
    pub selected: bool,
    pub change_action: SemanticAction,
}

impl AdmittedNavigationDestination {
    pub const fn identity(self) -> &'static str {
        match self {
            Self::Home => "home",
            Self::Tour => "tour",
            Self::Creche => "creche",
            Self::Patchbay => "patchbay",
            Self::Source => "source",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub enum FaceMechanism {
    Shell,
    Workbench,
    Panel {
        title: String,
    },
    Heading {
        text: String,
    },
    Separator {
        label: String,
    },
    Grid,
    ActionGroup {
        label: String,
    },
    Action(SemanticAction),
    Status {
        kind: StatusKind,
        title: String,
        detail: String,
    },
    Disclosure {
        summary: String,
    },
    Evidence(EvidenceFace),
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
    ChoiceGroup {
        label: String,
        multiplicity: ChoiceMultiplicity,
        options: Vec<ChoiceOption>,
    },
    Navigation {
        label: String,
        current: String,
    },
    NavigationLink {
        label: String,
        destination: AdmittedNavigationDestination,
    },
    Link {
        label: String,
        destination: String,
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
    Artifact(ArtifactFace),
    Download(SemanticAction),
    DeviceChoice(FormField),
    PatchbayCanvas {
        label: String,
    },
}

impl FaceMechanism {
    pub const fn kind(&self) -> FaceMechanismKind {
        match self {
            Self::Shell => FaceMechanismKind::Shell,
            Self::Workbench => FaceMechanismKind::Workbench,
            Self::Panel { .. } => FaceMechanismKind::Panel,
            Self::Heading { .. } => FaceMechanismKind::Heading,
            Self::Separator { .. } => FaceMechanismKind::Separator,
            Self::Grid => FaceMechanismKind::Grid,
            Self::ActionGroup { .. } => FaceMechanismKind::ActionGroup,
            Self::Action(_) => FaceMechanismKind::Action,
            Self::Status { .. } => FaceMechanismKind::Status,
            Self::Disclosure { .. } => FaceMechanismKind::Disclosure,
            Self::Evidence(_) => FaceMechanismKind::Evidence,
            Self::DefinitionTable { .. } => FaceMechanismKind::DefinitionTable,
            Self::Definition { .. } => FaceMechanismKind::Definition,
            Self::CodeBlock { .. } => FaceMechanismKind::CodeBlock,
            Self::FormField(_) => FaceMechanismKind::FormField,
            Self::ChoiceGroup { .. } => FaceMechanismKind::ChoiceGroup,
            Self::Navigation { .. } => FaceMechanismKind::Navigation,
            Self::NavigationLink { .. } => FaceMechanismKind::NavigationLink,
            Self::Link { .. } => FaceMechanismKind::Link,
            Self::Stepper { .. } => FaceMechanismKind::Stepper,
            Self::Progress { .. } => FaceMechanismKind::Progress,
            Self::Artifact(_) => FaceMechanismKind::Artifact,
            Self::Download(_) => FaceMechanismKind::Download,
            Self::DeviceChoice(_) => FaceMechanismKind::DeviceChoice,
            Self::PatchbayCanvas { .. } => FaceMechanismKind::PatchbayCanvas,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SemanticFaceNode {
    pub key: String,
    pub mechanism: FaceMechanism,
    pub children: Vec<Self>,
}

#[derive(Clone, Debug, Eq, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct SemanticApplicationView {
    pub revision: u32,
    pub root: SemanticFaceNode,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SemanticFaceRefusal {
    InvalidActionAvailability,
    InvalidActionGroup,
    InvalidDeviceChoice,
    InvalidField,
    InvalidChoiceGroup,
    InvalidNavigation,
    InvalidProgress,
    InvalidEvidence,
    InvalidDefinition,
    InvalidCodeBlock,
    InvalidArtifact,
    ApplicationView(ApplicationViewRefusal),
}

impl From<ApplicationViewRefusal> for SemanticFaceRefusal {
    fn from(value: ApplicationViewRefusal) -> Self {
        Self::ApplicationView(value)
    }
}

impl SemanticApplicationView {
    /// Lowers the semantic vocabulary without consulting a host, DOM, or
    /// application-global registry. The resulting low-level view performs the
    /// existing finite node/depth/text/action validation.
    pub fn lower(&self) -> Result<ApplicationView, SemanticFaceRefusal> {
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
    source: &SemanticFaceNode,
    parent: Option<u8>,
    nodes: &mut Vec<ApplicationViewNode>,
    actions: &mut Vec<ApplicationAction>,
) -> Result<(), SemanticFaceRefusal> {
    let index = u8::try_from(nodes.len())
        .map_err(|_| SemanticFaceRefusal::ApplicationView(ApplicationViewRefusal::TooManyNodes))?;
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
        FaceMechanism::Evidence(evidence) => {
            push_definition(index, nodes, "Identity", &evidence.identity)?;
            push_definition(index, nodes, "Provenance", &evidence.provenance)?;
        }
        FaceMechanism::Artifact(artifact) => {
            push_evidence_state(index, nodes, artifact.disposition)?;
            push_definition(index, nodes, "Kind", &artifact.kind)?;
            push_definition(index, nodes, "Identity", &artifact.identity)?;
            push_definition(index, nodes, "Provenance", &artifact.provenance)?;
            push_definition(index, nodes, "Detail", &artifact.detail)?;
        }
        _ => {}
    }
    if let FaceMechanism::FormField(field) = &source.mechanism {
        lower_plot_field(index, &source.key, field, nodes, actions, false)?;
    } else if let FaceMechanism::DeviceChoice(field) = &source.mechanism {
        lower_plot_field(index, &source.key, field, nodes, actions, true)?;
    } else if let FaceMechanism::ChoiceGroup {
        label,
        multiplicity,
        options,
    } = &source.mechanism
    {
        if !source.children.is_empty() {
            return Err(SemanticFaceRefusal::InvalidChoiceGroup);
        }
        lower_choice_group(
            index,
            &source.key,
            label,
            *multiplicity,
            options,
            nodes,
            actions,
        )?;
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
    mechanism: &FaceMechanism,
    actions: &mut Vec<ApplicationAction>,
) -> Result<LoweredMechanism, SemanticFaceRefusal> {
    if let Some((component, text, value, value_capacity, action, state)) =
        structural_node(mechanism)
    {
        return Ok(LoweredMechanism {
            component,
            text,
            value,
            value_capacity,
            action,
            state,
        });
    }
    let empty = String::new();
    let (component, text, value, value_capacity, action, state) = match mechanism {
        FaceMechanism::Shell
        | FaceMechanism::Workbench
        | FaceMechanism::Panel { .. }
        | FaceMechanism::Heading { .. }
        | FaceMechanism::Separator { .. }
        | FaceMechanism::Grid => unreachable!("structural mechanisms return above"),
        FaceMechanism::ActionGroup { label } => {
            if label.is_empty() {
                return Err(SemanticFaceRefusal::InvalidActionGroup);
            }
            (
                ApplicationComponent::ActionGroup,
                label.clone(),
                String::new(),
                0,
                None,
                ApplicationNodeState::Ready,
            )
        }
        FaceMechanism::Action(action) => {
            action_node(action, ApplicationComponent::Button, actions)?
        }
        FaceMechanism::Status {
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
        FaceMechanism::Disclosure { summary } => (
            ApplicationComponent::Disclosure,
            summary.clone(),
            String::new(),
            0,
            None,
            ApplicationNodeState::Ready,
        ),
        FaceMechanism::Evidence(evidence) => {
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
        FaceMechanism::DefinitionTable { title } => (
            ApplicationComponent::DefinitionTable,
            title.clone(),
            String::new(),
            0,
            None,
            ApplicationNodeState::Ready,
        ),
        FaceMechanism::Definition { term, value } => definition_node(term, value)?,
        FaceMechanism::CodeBlock { language, code } => code_node(language, code)?,
        FaceMechanism::FormField(_) | FaceMechanism::DeviceChoice(_) => (
            ApplicationComponent::FormField,
            empty,
            String::new(),
            0,
            None,
            ApplicationNodeState::Ready,
        ),
        FaceMechanism::ChoiceGroup { .. } => (
            ApplicationComponent::ChoiceGroup,
            empty,
            String::new(),
            0,
            None,
            ApplicationNodeState::Ready,
        ),
        FaceMechanism::Navigation { label, current } => {
            if label.is_empty() || current.is_empty() {
                return Err(SemanticFaceRefusal::InvalidNavigation);
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
        FaceMechanism::NavigationLink { label, destination } => {
            if label.is_empty() {
                return Err(SemanticFaceRefusal::InvalidNavigation);
            }
            let identity = destination.identity();
            (
                ApplicationComponent::NavigationLink,
                label.clone(),
                identity.into(),
                u32::try_from(identity.len()).unwrap_or(u32::MAX),
                None,
                ApplicationNodeState::Ready,
            )
        }
        FaceMechanism::Link { label, destination } => {
            if label.is_empty() || !crate::application_view::valid_same_site_link(destination) {
                return Err(SemanticFaceRefusal::InvalidNavigation);
            }
            (
                ApplicationComponent::Link,
                label.clone(),
                destination.clone(),
                u32::try_from(destination.len()).unwrap_or(u32::MAX),
                None,
                ApplicationNodeState::Ready,
            )
        }
        FaceMechanism::Stepper {
            label,
            current,
            total,
        } => {
            if label.is_empty() || *current == 0 || *total == 0 || current > total {
                return Err(SemanticFaceRefusal::InvalidProgress);
            }
            progress_node(ApplicationComponent::Stepper, label, *current, *total)
        }
        FaceMechanism::Progress {
            title,
            current,
            total,
        } => {
            if title.is_empty() || *total == 0 || current > total {
                return Err(SemanticFaceRefusal::InvalidProgress);
            }
            progress_node(ApplicationComponent::Progress, title, *current, *total)
        }
        FaceMechanism::Artifact(artifact) => {
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
        FaceMechanism::Download(action) => {
            action_node(action, ApplicationComponent::Button, actions)?
        }
        FaceMechanism::PatchbayCanvas { label } => (
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
