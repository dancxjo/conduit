//! Bounded platform-neutral Patchbay interaction values and ordinary execution.

mod codec;
mod execution;

use conduit_core::ConfigurationValue;
use conduit_core::{
    kind_id, port_id, ActivePlayId, ArtifactId, BootId, CapabilityId, CapabilityLimits,
    CapabilityOffer, CheckedFormId, ExecutionProfileId, ExpandedFormId, FaceStartupParameter,
    HostAdvertisement, HostOperationContractId, HostOperationRequirement, HostProfileId,
    ImplementationId, KindContractRevision, OfferGeneration, PlanId, PortDescriptor, PortDirection,
    PortTemporal, SourceDocumentId, PROTOCOL_VERSION,
};
use conduit_form::{
    check_syntax_document, expand_canonical_form, parse_syntax_document, ConfigurationField,
    ConfigurationRule, KindDefinition, KindSignature, ProfileCatalog, StartupCatalog,
    StartupParameterSignature,
};
use std::collections::VecDeque;

pub const MAX_INTERACTION_ID_BYTES: usize = 384;
pub const MAX_INTERACTION_VALUE_BYTES: u32 = 512;
pub const MAX_INTERACTION_HISTORY: usize = 32;

const SELECT_KIND: &str = "interaction/select";
const INVOKE_KIND: &str = "interaction/invoke";
const APPLY_KIND: &str = "interaction/apply";
const REQUEST_VALUE_KIND: &str = "interaction/request@1";
const APPLY_HOST_OPERATION: &str = "conduit.patchbay/apply-interaction@1";
const CONTRACT_REVISION: &str = "conduit.patchbay/interaction@1";
const EXECUTION_PROFILE: &str = "conduit.patchbay/kernel-hosted@1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatchbayInteractionRequestId(String);

impl PatchbayInteractionRequestId {
    pub fn new(value: impl Into<String>) -> Result<Self, InteractionError> {
        let value = value.into();
        codec::validate_field(&value)?;
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatchbayAction {
    OpenBack,
    Save,
    ToggleLinearView,
    Birth,
    Wake,
    Lull,
    Plan,
    Play,
    Stop,
    Hold,
    PlaceGear,
    DuplicateGear,
    RemoveGear,
    ConnectPorts,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatchbayInvocation {
    pub action: PatchbayAction,
    pub target_identity: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PatchbayInteractionRequest {
    Select {
        request_id: PatchbayInteractionRequestId,
        expanded_form_id: ExpandedFormId,
        subject_identity: String,
    },
    Invoke {
        request_id: PatchbayInteractionRequestId,
        invocation: PatchbayInvocation,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatchbayRefusal {
    StalePresentation,
    UnknownSubject,
    OperationUnavailable,
    OperationRejected,
    IncompatiblePorts,
    DuplicateCord,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PatchbayInvocationOutcome {
    Succeeded,
    Refused(PatchbayRefusal),
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InteractionDisposition {
    Succeeded,
    Refused(PatchbayRefusal),
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InteractionReceipt {
    pub request: PatchbayInteractionRequest,
    pub source_document_id: SourceDocumentId,
    pub checked_form_id: CheckedFormId,
    pub expanded_form_id: ExpandedFormId,
    pub plan_id: PlanId,
    pub plan: conduit_core::Plan,
    pub active_play_id: ActivePlayId,
    pub disposition: InteractionDisposition,
    pub signs: Vec<conduit_kernel::KernelEvent>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InteractionError {
    InvalidIdentity,
    ValueTooLarge,
    MalformedValue,
    Form(String),
    Planning(String),
    Execution(String),
}

#[derive(Debug)]
pub struct PatchbayInteraction {
    host_id: conduit_core::HostId,
    boot_id: BootId,
    sequence: u64,
    play_sequence: u64,
    selected: Option<crate::PatchbaySubjectRef>,
    history: VecDeque<InteractionReceipt>,
}

impl PatchbayInteraction {
    pub fn new(host_id: conduit_core::HostId, boot_id: BootId) -> Self {
        Self {
            host_id,
            boot_id,
            sequence: 0,
            play_sequence: 0,
            selected: None,
            history: VecDeque::with_capacity(MAX_INTERACTION_HISTORY),
        }
    }

    pub fn next_request_id(
        &mut self,
        action: &str,
    ) -> Result<PatchbayInteractionRequestId, InteractionError> {
        let sequence = self.sequence;
        self.sequence = self.sequence.saturating_add(1);
        PatchbayInteractionRequestId::new(format!("patchbay/interaction/{action}/{sequence}"))
    }

    pub fn selected(&self) -> Option<&crate::PatchbaySubjectRef> {
        self.selected.as_ref()
    }

    pub fn history(&self) -> impl Iterator<Item = &InteractionReceipt> {
        self.history.iter()
    }

    pub fn lines(&self) -> Vec<String> {
        self.history
            .iter()
            .rev()
            .take(4)
            .map(|receipt| {
                let source_kind = match &receipt.request {
                    PatchbayInteractionRequest::Select { .. } => SELECT_KIND,
                    PatchbayInteractionRequest::Invoke { .. } => INVOKE_KIND,
                };
                format!(
                    "INTERACTION request={} kind={} gears=request,apply port=request:{} plan={} play={} disposition={:?} signs={}",
                    receipt.request.request_id().as_str(),
                    source_kind,
                    REQUEST_VALUE_KIND,
                    receipt.plan_id.as_str(),
                    receipt.active_play_id.as_str(),
                    receipt.disposition,
                    receipt.signs.len()
                )
            })
            .collect()
    }

    fn advertisement(&self) -> HostAdvertisement {
        HostAdvertisement {
            protocol_version: PROTOCOL_VERSION,
            host_id: self.host_id.clone(),
            boot_id: self.boot_id.clone(),
            offer_generation: OfferGeneration(1),
            profile: HostProfileId::from("patchbay-interaction"),
            resources: vec![],
            planner_capabilities: vec![],
            capabilities: vec![select_offer(), invoke_offer(), apply_offer()],
        }
    }

    fn retain(&mut self, receipt: InteractionReceipt) {
        if self.history.len() == MAX_INTERACTION_HISTORY {
            self.history.pop_front();
        }
        self.history.push_back(receipt);
    }
}

fn request_from_expanded(
    form: &conduit_form::ExpandedCanonicalForm,
) -> Result<PatchbayInteractionRequest, InteractionError> {
    let source = form
        .gears
        .iter()
        .find(|gear| matches!(gear.kind_id.as_str(), SELECT_KIND | INVOKE_KIND))
        .ok_or_else(|| InteractionError::Form("interaction source Gear is absent".into()))?;
    let text = |key: &str| {
        source
            .configuration
            .iter()
            .find_map(|entry| match (entry.key.as_str(), &entry.value) {
                (candidate, ConfigurationValue::Text(value)) if candidate == key => {
                    Some(value.clone())
                }
                _ => None,
            })
            .ok_or_else(|| InteractionError::Form(format!("interaction field '{key}' is absent")))
    };
    let request_id = PatchbayInteractionRequestId::new(text("request")?)?;
    match source.kind_id.as_str() {
        SELECT_KIND => Ok(PatchbayInteractionRequest::Select {
            request_id,
            expanded_form_id: ExpandedFormId::from(text("basis")?),
            subject_identity: text("subject")?,
        }),
        INVOKE_KIND => Ok(PatchbayInteractionRequest::Invoke {
            request_id,
            invocation: PatchbayInvocation {
                action: PatchbayAction::parse(&text("action")?)?,
                target_identity: text("target")?,
            },
        }),
        _ => Err(InteractionError::MalformedValue),
    }
}

fn expanded_request(
    request: &PatchbayInteractionRequest,
) -> Result<conduit_form::ExpandedCanonicalForm, InteractionError> {
    let (startup, profile) = interaction_catalogs()?;
    let source = request_source(request);
    let syntax = parse_syntax_document(&source);
    let checked = check_syntax_document(&syntax, &startup)
        .map_err(|error| InteractionError::Form(format!("{error:?}")))?;
    expand_canonical_form(&checked, "patchbay-interaction", &profile)
        .map_err(|error| InteractionError::Form(error.to_string()))
}

fn interaction_catalogs() -> Result<(StartupCatalog, ProfileCatalog), InteractionError> {
    let mut startup = StartupCatalog::new();
    startup
        .insert(signature(SELECT_KIND, &["request", "basis", "subject"]))
        .map_err(|error| InteractionError::Form(error.to_string()))?;
    startup
        .insert(signature(INVOKE_KIND, &["request", "action", "target"]))
        .map_err(|error| InteractionError::Form(error.to_string()))?;
    startup
        .insert(signature(APPLY_KIND, &[]))
        .map_err(|error| InteractionError::Form(error.to_string()))?;
    let mut profile = ProfileCatalog::new();
    for definition in [
        source_definition(SELECT_KIND),
        source_definition(INVOKE_KIND),
        apply_definition(),
    ] {
        profile
            .insert(definition)
            .map_err(|error| InteractionError::Form(error.to_string()))?;
    }
    Ok((startup, profile))
}

fn signature(kind: &str, fields: &[&str]) -> KindSignature {
    KindSignature {
        kind: kind.into(),
        startup_parameters: fields
            .iter()
            .map(|name| StartupParameterSignature {
                name: (*name).into(),
                value_type: "Text".into(),
                default: None,
            })
            .collect(),
    }
}

fn source_definition(kind: &str) -> KindDefinition {
    KindDefinition {
        kind_id: kind_id(kind),
        kind_contract_revision: KindContractRevision::from(CONTRACT_REVISION),
        inputs: vec![],
        outputs: vec![request_port(PortDirection::Output)],
        configuration: [
            "request",
            if kind == SELECT_KIND {
                "basis"
            } else {
                "action"
            },
            if kind == SELECT_KIND {
                "subject"
            } else {
                "target"
            },
        ]
        .into_iter()
        .map(|key| ConfigurationField {
            key: key.into(),
            default_value: ConfigurationValue::Text(String::new()),
            validation: ConfigurationRule::TextBytes {
                maximum: MAX_INTERACTION_ID_BYTES as u32,
            },
        })
        .collect(),
    }
}

fn apply_definition() -> KindDefinition {
    KindDefinition {
        kind_id: kind_id(APPLY_KIND),
        kind_contract_revision: KindContractRevision::from(CONTRACT_REVISION),
        inputs: vec![request_port(PortDirection::Input)],
        outputs: vec![],
        configuration: vec![],
    }
}

fn request_port(direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id("request"),
        value_kind: kind_id(REQUEST_VALUE_KIND),
        direction,
        temporal: PortTemporal::Value,
    }
}

fn select_offer() -> CapabilityOffer {
    source_offer(
        SELECT_KIND,
        "patchbay-select",
        "patchbay/select@1",
        &["request", "basis", "subject"],
    )
}

fn invoke_offer() -> CapabilityOffer {
    source_offer(
        INVOKE_KIND,
        "patchbay-invoke",
        "patchbay/invoke@1",
        &["request", "action", "target"],
    )
}

fn source_offer(
    kind: &str,
    capability: &str,
    implementation: &str,
    fields: &[&str],
) -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters: fields
            .iter()
            .map(|name| FaceStartupParameter {
                name: (*name).into(),
                value_type: "Text".into(),
                has_default: false,
            })
            .collect(),
        shorthand: None,
        capability_id: CapabilityId::from(capability),
        kind_id: kind_id(kind),
        kind_contract_revision: KindContractRevision::from(CONTRACT_REVISION),
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(EXECUTION_PROFILE),
            implementation_id: ImplementationId::from(implementation),
            artifact_id: ArtifactId::from(implementation),
        },
        inputs: vec![],
        outputs: vec![request_port(PortDirection::Output)],
        host_operations: vec![],
        resource_requirements: vec![],
        authority_requirements: vec![],
        limits: interaction_limits(),
    }
}

fn apply_offer() -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters: vec![],
        shorthand: None,
        capability_id: CapabilityId::from("patchbay-apply"),
        kind_id: kind_id(APPLY_KIND),
        kind_contract_revision: KindContractRevision::from(CONTRACT_REVISION),
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from(EXECUTION_PROFILE),
            implementation_id: ImplementationId::from("patchbay/apply@1"),
            artifact_id: ArtifactId::from("patchbay/apply@1"),
        },
        inputs: vec![request_port(PortDirection::Input)],
        outputs: vec![],
        host_operations: vec![HostOperationRequirement {
            contract_id: HostOperationContractId::from(APPLY_HOST_OPERATION),
            target_kind: Some(kind_id("interaction/patchbay-state")),
            maximum_in_flight: 1,
            maximum_input_bytes: MAX_INTERACTION_VALUE_BYTES,
            maximum_output_bytes: 0,
        }],
        resource_requirements: vec![],
        authority_requirements: vec![],
        limits: interaction_limits(),
    }
}

fn interaction_limits() -> CapabilityLimits {
    CapabilityLimits {
        max_active_instances: 1,
        max_queue_items: 4,
        max_queue_bytes: MAX_INTERACTION_VALUE_BYTES * 4,
    }
}

fn request_source(request: &PatchbayInteractionRequest) -> String {
    let (kind, fields) = match request {
        PatchbayInteractionRequest::Select {
            request_id,
            expanded_form_id,
            subject_identity,
        } => (
            SELECT_KIND,
            vec![
                request_id.as_str(),
                expanded_form_id.as_str(),
                subject_identity.as_str(),
            ],
        ),
        PatchbayInteractionRequest::Invoke {
            request_id,
            invocation,
        } => (
            INVOKE_KIND,
            vec![
                request_id.as_str(),
                invocation.action.as_str(),
                invocation.target_identity.as_str(),
            ],
        ),
    };
    let arguments = fields
        .into_iter()
        .map(|field| format!("\"{}\"", escape_form_text(field)))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "form patchbay-interaction {{\n    request: {kind}({arguments})\n    apply: {APPLY_KIND}\n    request > apply\n}}\n"
    )
}

fn escape_form_text(value: &str) -> String {
    value.replace('\\', "\\\\").replace('"', "\\\"")
}
