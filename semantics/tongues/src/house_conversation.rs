//! Address-gated projection of explicitly wired House context into portable model input.

use conduit_ai::{
    build_house_model_request, HouseContextProvenanceClass, HouseContextRefusal,
    WiredHouseContextItem, GENERATION_REQUEST_VALUE_KIND,
};
use conduit_core::{
    kind_id, port_id, CapabilityLimits, KindContractRevision, KindId, PortDescriptor,
    PortDirection, PortTemporal,
};
use conduit_form::{KindDefinition, KindSignature, ProfileCatalog, StartupCatalog};
use conduit_text::{AddressDetection, ADDRESS_DETECTION_VALUE_KIND};
use serde::{Deserialize, Serialize};

pub const HOUSE_CONTEXT_TO_PROMPT_KIND: &str = "house/context-to-prompt";
pub const HOUSE_CONTEXT_TO_PROMPT_REVISION: &str = "conduit.house/context-to-prompt@1";
pub const WIRED_HOUSE_CONTEXT_VALUE_KIND: &str = "house/wired-context@1";
pub const MAXIMUM_HOUSE_PROMPT_BYTES: usize = 131_072;
pub const MAXIMUM_ADDRESS_DETECTION_VALUE_BYTES: usize = 8_192;
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
struct AddressDetectionValue {
    schema: String,
    status: String,
    matched_name_index: Option<u8>,
    utterance: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct WiredHouseContextValue {
    schema: String,
    items: Vec<WiredHouseContextItem>,
}

pub fn encode_address_detection(
    detection: &AddressDetection,
) -> Result<Vec<u8>, HouseConversationValueError> {
    validate_detection(detection)?;
    let (status, matched_name_index, utterance) = match detection {
        AddressDetection::NotAddressed => ("not-addressed", None, None),
        AddressDetection::Addressed {
            matched_name_index,
            utterance,
        } => (
            "addressed",
            Some(*matched_name_index),
            Some(utterance.clone()),
        ),
    };
    bounded_json(
        &AddressDetectionValue {
            schema: "conduit.house/address-detection-value@1".into(),
            status: status.into(),
            matched_name_index,
            utterance,
        },
        MAXIMUM_ADDRESS_DETECTION_VALUE_BYTES,
    )
}

pub fn decode_address_detection(
    bytes: &[u8],
) -> Result<AddressDetection, HouseConversationValueError> {
    if bytes.len() > MAXIMUM_ADDRESS_DETECTION_VALUE_BYTES {
        return Err(HouseConversationValueError::BoundExceeded);
    }
    let value: AddressDetectionValue =
        serde_json::from_slice(bytes).map_err(|_| HouseConversationValueError::Malformed)?;
    if value.schema != "conduit.house/address-detection-value@1" {
        return Err(HouseConversationValueError::WrongSchema);
    }
    let detection = match (
        value.status.as_str(),
        value.matched_name_index,
        value.utterance.clone(),
    ) {
        ("not-addressed", None, None) => AddressDetection::NotAddressed,
        ("addressed", Some(matched_name_index), Some(utterance)) => AddressDetection::Addressed {
            matched_name_index,
            utterance,
        },
        _ => return Err(HouseConversationValueError::InvalidValue),
    };
    validate_detection(&detection)?;
    require_canonical(bytes, &value, MAXIMUM_ADDRESS_DETECTION_VALUE_BYTES)?;
    Ok(detection)
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

fn validate_detection(value: &AddressDetection) -> Result<(), HouseConversationValueError> {
    if let AddressDetection::Addressed {
        matched_name_index,
        utterance,
    } = value
    {
        if usize::from(*matched_name_index) >= conduit_text::MAX_ADDRESS_NAMES
            || utterance.len() > conduit_text::MAX_TEXT_BYTES as usize
        {
            return Err(HouseConversationValueError::InvalidValue);
        }
    }
    Ok(())
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
    pub kind_contract_revision: KindContractRevision,
    pub inputs: Vec<PortDescriptor>,
    pub outputs: Vec<PortDescriptor>,
    pub limits: CapabilityLimits,
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
        kind_contract_revision: KindContractRevision::from(HOUSE_CONTEXT_TO_PROMPT_REVISION),
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
        .insert(KindDefinition {
            kind_id: contract.kind_id,
            kind_contract_revision: contract.kind_contract_revision,
            inputs: contract.inputs,
            outputs: contract.outputs,
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
