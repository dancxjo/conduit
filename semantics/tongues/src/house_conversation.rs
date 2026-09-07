//! Address-gated projection of explicitly wired House context into portable model input.

use conduit_ai::{
    build_house_model_request, HouseContextProvenanceClass, HouseContextRefusal,
    WiredHouseContextItem, TEXT_VALUE_KIND,
};
use conduit_core::{
    kind_id, port_id, CapabilityLimits, KindContractRevision, KindId, PortDescriptor,
    PortDirection, PortTemporal,
};
use conduit_form::{KindDefinition, KindSignature, ProfileCatalog, StartupCatalog};
use conduit_text::{AddressDetection, ADDRESS_DETECTION_VALUE_KIND};
use serde::Serialize;

pub const HOUSE_CONTEXT_TO_PROMPT_KIND: &str = "house/context-to-prompt";
pub const HOUSE_CONTEXT_TO_PROMPT_REVISION: &str = "conduit.house/context-to-prompt@1";
pub const WIRED_HOUSE_CONTEXT_VALUE_KIND: &str = "house/wired-context@1";
pub const MAXIMUM_HOUSE_PROMPT_BYTES: usize = 131_072;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HousePromptContract {
    pub kind_id: KindId,
    pub kind_contract_revision: KindContractRevision,
    pub inputs: Vec<PortDescriptor>,
    pub outputs: Vec<PortDescriptor>,
    pub limits: CapabilityLimits,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HouseGenerationPrompt {
    pub request_identity: String,
    pub prompt: String,
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
        outputs: vec![port("prompt", TEXT_VALUE_KIND, PortDirection::Output)],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: MAXIMUM_HOUSE_PROMPT_BYTES as u32,
        },
    }
}

pub fn prepare_house_generation_prompt(
    detection: &AddressDetection,
    explicitly_wired: &[WiredHouseContextItem],
    maximum_output_bytes: u64,
) -> Result<HouseGenerationPrompt, HousePromptRefusal> {
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
    Ok(HouseGenerationPrompt {
        request_identity: request.request_identity,
        prompt,
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
