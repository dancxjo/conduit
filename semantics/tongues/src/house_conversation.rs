//! Address-gated projection of explicitly wired House context into portable model input.

use conduit_ai::{
    build_house_model_request, HouseContextProvenanceClass, HouseContextRefusal,
    WiredHouseContextItem, GENERATION_REQUEST_VALUE_KIND,
};
use conduit_core::{
    kind_id, port_id, CapabilityLimits, Kind, KindId, KindIdentity, PortDescriptor, PortDirection,
    PortTemporal,
};
use conduit_form::{KindProjection, KindSignature, ProfileCatalog, StartupCatalog};
use conduit_text::{AddressDetection, ADDRESS_DETECTION_VALUE_KIND};
use serde::{Deserialize, Serialize};

pub const HOUSE_CONTEXT_TO_PROMPT_KIND: &str = "house/context-to-prompt";
pub const HOUSE_CONTEXT_TO_PROMPT_REVISION: &str = "conduit.house/context-to-prompt@1";
pub const HOUSE_CONVERSATION_FORM_KIND: &str = "house-conversation";
pub const HOUSE_CONVERSATION_FORM_REVISION: &str = "conduit.house/conversation-form@1";
pub const WIRED_HOUSE_CONTEXT_VALUE_KIND: &str = "house/wired-context@1";
pub const MAXIMUM_HOUSE_PROMPT_BYTES: usize = 131_072;
pub const MAXIMUM_ADDRESS_DETECTION_VALUE_BYTES: usize =
    conduit_text::MAX_ADDRESS_DETECTION_VALUE_BYTES;
pub const MAXIMUM_WIRED_HOUSE_CONTEXT_VALUE_BYTES: usize = 131_072;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HouseConversationValueError {
    Encoding,
    BoundExceeded,
    Malformed,
    WrongSchema,
    InvalidValue,
    NonCanonical,
}

#[derive(Serialize, Deserialize)]
struct WiredHouseContextValue {
    schema: String,
    items: Vec<WiredHouseContextItem>,
}

pub fn encode_address_detection(
    detection: &AddressDetection,
) -> Result<Vec<u8>, HouseConversationValueError> {
    conduit_text::encode_address_detection(detection).map_err(map_address_value_error)
}

pub fn decode_address_detection(
    bytes: &[u8],
) -> Result<AddressDetection, HouseConversationValueError> {
    conduit_text::decode_address_detection(bytes).map_err(map_address_value_error)
}

pub fn encode_wired_house_context(
    items: &[WiredHouseContextItem],
) -> Result<Vec<u8>, HouseConversationValueError> {
    validate_context(items)?;
    bounded_json(
        &WiredHouseContextValue {
            schema: "conduit.house/wired-context-value@1".into(),
            items: items.to_vec(),
        },
        MAXIMUM_WIRED_HOUSE_CONTEXT_VALUE_BYTES,
    )
}

pub fn decode_wired_house_context(
    bytes: &[u8],
) -> Result<Vec<WiredHouseContextItem>, HouseConversationValueError> {
    if bytes.len() > MAXIMUM_WIRED_HOUSE_CONTEXT_VALUE_BYTES {
        return Err(HouseConversationValueError::BoundExceeded);
    }
    let value: WiredHouseContextValue =
        serde_json::from_slice(bytes).map_err(|_| HouseConversationValueError::Malformed)?;
    if value.schema != "conduit.house/wired-context-value@1" {
        return Err(HouseConversationValueError::WrongSchema);
    }
    validate_context(&value.items)?;
    require_canonical(bytes, &value, MAXIMUM_WIRED_HOUSE_CONTEXT_VALUE_BYTES)?;
    Ok(value.items)
}

fn map_address_value_error(error: conduit_text::AddressValueError) -> HouseConversationValueError {
    match error {
        conduit_text::AddressValueError::BoundExceeded => {
            HouseConversationValueError::BoundExceeded
        }
        conduit_text::AddressValueError::Malformed => HouseConversationValueError::Malformed,
        conduit_text::AddressValueError::NonCanonical => HouseConversationValueError::NonCanonical,
        conduit_text::AddressValueError::InvalidValue => HouseConversationValueError::InvalidValue,
    }
}

fn validate_context(items: &[WiredHouseContextItem]) -> Result<(), HouseConversationValueError> {
    build_house_model_request("validation", items, 1)
        .map(|_| ())
        .map_err(|_| HouseConversationValueError::InvalidValue)
}

fn bounded_json<T: Serialize>(
    value: &T,
    maximum: usize,
) -> Result<Vec<u8>, HouseConversationValueError> {
    let bytes = serde_json::to_vec(value).map_err(|_| HouseConversationValueError::Encoding)?;
    if bytes.len() > maximum {
        return Err(HouseConversationValueError::BoundExceeded);
    }
    Ok(bytes)
}

fn require_canonical<T: Serialize>(
    bytes: &[u8],
    value: &T,
    maximum: usize,
) -> Result<(), HouseConversationValueError> {
    if bounded_json(value, maximum)? != bytes {
        return Err(HouseConversationValueError::NonCanonical);
    }
    Ok(())
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HousePromptContract {
    pub kind_id: KindId,
    pub kind_contract_revision: KindIdentity,
    pub inputs: Vec<PortDescriptor>,
    pub outputs: Vec<PortDescriptor>,
    pub limits: CapabilityLimits,
}

impl HousePromptContract {
    pub fn into_semantic_capability_contract(self) -> Kind {
        Kind {
            startup_parameters: Vec::new(),
            shorthand: None,
            kind_id: self.kind_id,
            kind_contract_revision: self.kind_contract_revision,
            inputs: self.inputs,
            outputs: self.outputs,
            configuration: Default::default(),
            semantic_laws: Default::default(),
            limits: self.limits,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HouseGenerationRequest {
    pub request_identity: String,
    pub encoded_request: String,
    pub maximum_output_bytes: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HousePromptRefusal {
    NotAddressed,
    EmptyAddressedUtterance,
    Context(HouseContextRefusal),
    ContextValueIsNotText,
    PromptEncoding,
    PromptBoundExceeded,
}

#[derive(Serialize)]
struct PromptProjection<'a> {
    schema: &'static str,
    request_identity: &'a str,
    response_instruction: &'static str,
    addressed_utterance: &'a str,
    context: Vec<PromptContextItem<'a>>,
}

#[derive(Serialize)]
struct PromptContextItem<'a> {
    item_identity: &'a str,
    value_kind: &'a str,
    value: &'a str,
    provenance: &'static str,
    source_identity: &'a str,
}

pub fn house_prompt_contract() -> HousePromptContract {
    HousePromptContract {
        kind_id: kind_id(HOUSE_CONTEXT_TO_PROMPT_KIND),
        kind_contract_revision: KindIdentity::from(HOUSE_CONTEXT_TO_PROMPT_REVISION),
        inputs: vec![
            port(
                "detection",
                ADDRESS_DETECTION_VALUE_KIND,
                PortDirection::Input,
            ),
            port(
                "context",
                WIRED_HOUSE_CONTEXT_VALUE_KIND,
                PortDirection::Input,
            ),
        ],
        outputs: vec![port(
            "request",
            GENERATION_REQUEST_VALUE_KIND,
            PortDirection::Output,
        )],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: MAXIMUM_HOUSE_PROMPT_BYTES as u32,
        },
    }
}

pub fn prepare_house_generation_request(
    detection: &AddressDetection,
    explicitly_wired: &[WiredHouseContextItem],
    maximum_output_bytes: u64,
) -> Result<HouseGenerationRequest, HousePromptRefusal> {
    let AddressDetection::Addressed { utterance, .. } = detection else {
        return Err(HousePromptRefusal::NotAddressed);
    };
    if utterance.is_empty() {
        return Err(HousePromptRefusal::EmptyAddressedUtterance);
    }
    let request = build_house_model_request(utterance, explicitly_wired, maximum_output_bytes)
        .map_err(HousePromptRefusal::Context)?;
    let mut context = Vec::with_capacity(request.context.len());
    for item in &request.context {
        let value = core::str::from_utf8(&item.canonical_value)
            .map_err(|_| HousePromptRefusal::ContextValueIsNotText)?;
        context.push(PromptContextItem {
            item_identity: &item.item_identity,
            value_kind: &item.value_kind,
            value,
            provenance: provenance_name(item.provenance),
            source_identity: &item.source_identity,
        });
    }
    let prompt = serde_json::to_string(&PromptProjection {
        schema: "conduit.house/model-context-presentation@1",
        request_identity: &request.request_identity,
        response_instruction:
            "Answer briefly using only the wired context. Say temperature units in full, including degrees Celsius.",
        addressed_utterance: &request.addressed_utterance,
        context,
    })
    .map_err(|_| HousePromptRefusal::PromptEncoding)?;
    if prompt.len() > MAXIMUM_HOUSE_PROMPT_BYTES {
        return Err(HousePromptRefusal::PromptBoundExceeded);
    }
    Ok(HouseGenerationRequest {
        request_identity: request.request_identity,
        encoded_request: prompt,
        maximum_output_bytes: request.maximum_output_bytes,
    })
}

pub fn install_house_conversation_catalog(
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
) -> Result<(), String> {
    startup.insert_value_kind_alias("HouseContext", kind_id(WIRED_HOUSE_CONTEXT_VALUE_KIND))?;
    startup.insert(KindSignature {
        kind: HOUSE_CONTEXT_TO_PROMPT_KIND.into(),
        startup_parameters: vec![],
    })?;
    let contract = house_prompt_contract();
    profile
        .insert(KindProjection {
            kind_id: contract.kind_id,
            kind_contract_revision: contract.kind_contract_revision,
            inputs: contract.inputs,
            outputs: contract.outputs,
            configuration: vec![],
        })
        .map_err(|error| error.to_string())
}

pub fn install_house_conversation_form_catalog(
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
) -> Result<(), String> {
    startup.insert(KindSignature {
        kind: HOUSE_CONVERSATION_FORM_KIND.into(),
        startup_parameters: vec![],
    })?;
    profile
        .insert(KindProjection {
            kind_id: kind_id(HOUSE_CONVERSATION_FORM_KIND),
            kind_contract_revision: KindIdentity::from(HOUSE_CONVERSATION_FORM_REVISION),
            inputs: vec![
                port(
                    "detection",
                    ADDRESS_DETECTION_VALUE_KIND,
                    PortDirection::Input,
                ),
                port(
                    "context",
                    WIRED_HOUSE_CONTEXT_VALUE_KIND,
                    PortDirection::Input,
                ),
            ],
            outputs: vec![port(
                "response",
                conduit_ai::TEXT_VALUE_KIND,
                PortDirection::Output,
            )],
            configuration: vec![],
        })
        .map_err(|error| error.to_string())
}

fn provenance_name(value: HouseContextProvenanceClass) -> &'static str {
    match value {
        HouseContextProvenanceClass::ObservedSign => "observed-sign",
        HouseContextProvenanceClass::DeclaredConfiguration => "declared-configuration",
        HouseContextProvenanceClass::ModelDerivedHistory => "model-derived-history",
    }
}

fn port(name: &str, value_kind: &str, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(value_kind),
        direction,
        temporal: PortTemporal::Value,
    }
}
