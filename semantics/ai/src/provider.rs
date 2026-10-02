//! One bounded provider-protocol realization below portable AI meaning.

use alloc::{
    format,
    string::{String, ToString},
    vec,
    vec::Vec,
};
use conduit_core::{
    kind_id, port_id, protected_resource_requirement, resource_requirement, ArtifactId,
    AuthorityContractId, AuthorityRequirement, Back, BackOfferBuilder, CapabilityId,
    CapabilityLimits, CapabilityOffer, ExecutionProfileId, HostCallContractId, HostCallRequirement,
    ImplementationId, Kind, KindIdentity, PortDescriptor, PortDirection, PortTemporal,
};
use conduit_plot::{
    check_syntax_document, parse_syntax_document, CanonicalBackCatalog, KindProjection,
    KindSignature, ProfileCatalog, StartupCatalog,
};
use conduit_web::{
    HttpHeader, HttpMethod, HttpRequest, HttpResponse, HttpTarget, HttpTransactionId, JsonRefusal,
    JsonValue,
};

use crate::{
    llm_contract, LlmDeterminismProfile, ModelDerivedResult, ModelResultDisposition,
    ModelResultProvenance, ModelWorkAccounting, GENERATED_RESULT_VALUE_KIND,
    GENERATION_REQUEST_VALUE_KIND, LLM_GENERATE_KIND,
};

pub const PROVIDER_REQUEST_KIND: &str = "provider/openai-compatible-request";
pub const PROVIDER_ENVELOPE_KIND: &str = "provider/openai-compatible-http-envelope";
pub const PROVIDER_RESPONSE_KIND: &str = "provider/openai-compatible-http-response";
pub const PROVIDER_RESULT_KIND: &str = "provider/openai-compatible-result";
pub const PROVIDER_HTTP_IMPLEMENTATION: &str = "provider/openai-compatible-http-client@1";
pub const PROVIDER_HTTP_OPERATION: &str = "conduit.host/http-client-exchange";
pub const PROVIDER_ENDPOINT_AUTHORITY: &str = "conduit.authority/provider-endpoint@1";
pub const PROVIDER_CREDENTIAL_CLASS: &str = "conduit.resource/protected-provider-credential@1";
pub const PROVIDER_CREDENTIAL_ROLE: &str = "provider-credential";
pub const PROVIDER_HTTP_RESOURCE: &str = "conduit.resource/network/http-client";
pub const MAXIMUM_PROVIDER_PROMPT_BYTES: usize = 1_024;
pub const MAXIMUM_PROVIDER_OUTPUT_BYTES: usize = 1_024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u16)]
pub enum ProviderFailure {
    HttpTransport = 1,
    HttpStatus = 2,
    ProviderProtocol = 3,
    CredentialRefused = 4,
    ProviderCapacity = 5,
    MalformedJson = 6,
    SemanticValidation = 7,
    Pressure = 8,
    Cancelled = 9,
    PartOrLineLost = 10,
    InputOverflow = 11,
    OutputOverflow = 12,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderEvidence {
    pub request_sequence: u64,
    pub credential_present: bool,
    pub credential_value: Option<String>,
    pub terminal: Result<(), ProviderFailure>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderResultContext {
    pub implementation_identity: String,
    pub request_identity: String,
    pub run_identity: String,
    pub input_bytes: u64,
    pub context_items: u64,
    pub work_units: u64,
    pub history_items: u64,
    pub determinism: LlmDeterminismProfile,
}

impl ProviderEvidence {
    pub fn redacted(request_sequence: u64, terminal: Result<(), ProviderFailure>) -> Self {
        Self {
            request_sequence,
            credential_present: true,
            credential_value: None,
            terminal,
        }
    }
}

pub fn provider_request(request: &[u8]) -> Result<JsonValue, ProviderFailure> {
    if request.len() > MAXIMUM_PROVIDER_PROMPT_BYTES {
        return Err(ProviderFailure::InputOverflow);
    }
    let prompt = core::str::from_utf8(request).map_err(|_| ProviderFailure::SemanticValidation)?;
    let value = JsonValue::Object(vec![
        ("input".into(), JsonValue::String(prompt.into())),
        ("model".into(), JsonValue::String("conduit-fixture".into())),
        ("stream".into(), JsonValue::Bool(false)),
    ]);
    value.validate().map_err(map_json)?;
    Ok(value)
}

pub fn provider_result(
    value: &JsonValue,
    context: ProviderResultContext,
) -> Result<Vec<u8>, ProviderFailure> {
    let JsonValue::Object(members) = value else {
        return Err(ProviderFailure::ProviderProtocol);
    };
    if members.iter().any(|(key, _)| key == "error") {
        return Err(ProviderFailure::ProviderProtocol);
    }
    let Some(JsonValue::String(output)) = members
        .iter()
        .find(|(key, _)| key == "output")
        .map(|(_, value)| value)
    else {
        return Err(ProviderFailure::SemanticValidation);
    };
    if output.len() > MAXIMUM_PROVIDER_OUTPUT_BYTES {
        return Err(ProviderFailure::OutputOverflow);
    }
    let contract = llm_contract(LLM_GENERATE_KIND).expect("generation contract is catalogued");
    let result = ModelDerivedResult {
        provenance: ModelResultProvenance::ModelDerived,
        payload_kind: contract.result_payload_kind.as_str().into(),
        payload: output.as_bytes().to_vec(),
        implementation_identity: context.implementation_identity,
        request_identity: context.request_identity,
        run_identity: context.run_identity,
        confidence: None,
        disposition: ModelResultDisposition::Produced,
        determinism: context.determinism,
        accounting: ModelWorkAccounting {
            input_bytes: context.input_bytes,
            context_items: context.context_items,
            output_bytes: output.len() as u64,
            work_units: context.work_units,
            history_items: context.history_items,
        },
    };
    result
        .validate(&contract)
        .map_err(|_| ProviderFailure::SemanticValidation)?;
    serde_json::to_vec(&result).map_err(|_| ProviderFailure::OutputOverflow)
}

pub fn provider_http_request(
    transaction_id: u64,
    authority: &str,
    path_and_query: &str,
    json: &[u8],
) -> Result<HttpRequest, ProviderFailure> {
    let request = HttpRequest {
        transaction_id: HttpTransactionId::new(transaction_id)
            .map_err(|_| ProviderFailure::ProviderProtocol)?,
        method: HttpMethod::Post,
        target: HttpTarget::new(
            authority.into(),
            path_and_query.into(),
            conduit_web::HttpScheme::Https,
        )
        .map_err(|_| ProviderFailure::ProviderProtocol)?,
        headers: conduit_web::http_headers([HttpHeader::new(
            "content-type".into(),
            conduit_plot::rust_binding::BoundedBytes::new(b"application/json")
                .ok_or(ProviderFailure::ProviderProtocol)?,
        )
        .map_err(|_| ProviderFailure::ProviderProtocol)?])
        .map_err(|_| ProviderFailure::ProviderProtocol)?,
        body: conduit_web::HttpBody::inline(json),
    };
    request
        .validate()
        .map_err(|_| ProviderFailure::ProviderProtocol)?;
    Ok(request)
}

pub fn provider_http_response(response: &HttpResponse) -> Result<&[u8], ProviderFailure> {
    response
        .validate()
        .map_err(|_| ProviderFailure::ProviderProtocol)?;
    match response.status {
        200..=299 => response
            .body
            .as_inline()
            .ok_or(ProviderFailure::ProviderProtocol),
        429 => Err(ProviderFailure::ProviderCapacity),
        _ => Err(ProviderFailure::HttpStatus),
    }
}

fn map_json(value: JsonRefusal) -> ProviderFailure {
    match value {
        JsonRefusal::StringByteOverflow
        | JsonRefusal::TotalStringByteOverflow
        | JsonRefusal::EncodedByteOverflow => ProviderFailure::InputOverflow,
        _ => ProviderFailure::MalformedJson,
    }
}

pub fn install_provider_catalogs(
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
) -> Result<(), String> {
    conduit_web::install_json_catalogs(startup, profile)?;
    conduit_web::install_http_catalogs(startup, profile)?;
    for definition in provider_definitions() {
        startup.insert(KindSignature {
            kind: definition.kind_id.as_str().into(),
            startup_parameters: Vec::new(),
        })?;
        profile
            .insert_kind(provider_adapter_semantic_contract(definition))
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

pub fn install_provider_back(
    startup: &StartupCatalog,
    profile: &ProfileCatalog,
    backs: &mut CanonicalBackCatalog,
) -> Result<(), String> {
    let source = format!(
        "plot {LLM_GENERATE_KIND} (\n maximum-input-bytes: Count = 262144\n maximum-context-items: Count = 128\n maximum-output-bytes: Count = 65536\n maximum-work-units: Count = 1000000\n maximum-history-items: Count = 64\n >> request: {GENERATION_REQUEST_VALUE_KIND}\n result: {GENERATED_RESULT_VALUE_KIND} >>\n) {{\n request_adapter: {PROVIDER_REQUEST_KIND}\n encode: {}\n envelope: {PROVIDER_ENVELOPE_KIND}\n http: {}\n response: {PROVIDER_RESPONSE_KIND}\n decode: {}\n result_adapter: {PROVIDER_RESULT_KIND}\n request >> request_adapter.request\n request_adapter.value >> encode.source\n encode.value >> envelope.json\n envelope.request >> http.request\n http.response >> response.response\n response.json >> decode.source\n decode.value >> result_adapter.value\n result_adapter.result >> result\n}}\n",
        conduit_web::JSON_ENCODE_KIND,
        conduit_web::HTTP_CLIENT_KIND,
        conduit_web::JSON_DECODE_KIND,
    );
    let checked = check_syntax_document(&parse_syntax_document(&source), startup)
        .map_err(|error| format!("provider Back check: {} {}", error.code, error.message))?;
    let high = profile
        .canonical_kind(&kind_id(LLM_GENERATE_KIND))
        .ok_or_else(|| "portable llm/generate definition missing".to_string())?;
    backs
        .insert(high, &checked, LLM_GENERATE_KIND)
        .map_err(|error| format!("provider Back catalog: {error:?}"))
}

pub fn provider_offers() -> Vec<CapabilityOffer> {
    provider_definitions()
        .into_iter()
        .map(adapter_offer)
        .collect()
}

pub fn provider_http_offer() -> CapabilityOffer {
    let contract = conduit_web::http_client_semantics().into_semantic_contract();
    let operation = HostCallRequirement {
        contract_id: HostCallContractId::from(PROVIDER_HTTP_OPERATION),
        target_kind: Some(kind_id(conduit_web::HTTP_CLIENT_KIND)),
        maximum_in_flight: 1,
        maximum_input_bytes: conduit_web::HTTP_MAXIMUM_ENCODED_REQUEST_BYTES,
        maximum_output_bytes: conduit_web::HTTP_MAXIMUM_ENCODED_RESPONSE_BYTES,
    };
    BackOfferBuilder::new(
        contract,
        Back {
            capability_id: CapabilityId::from("provider-http-client-v1"),
            execution_profile_id: ExecutionProfileId::from("provider/http-hosted@1"),
            implementation_id: ImplementationId::from(PROVIDER_HTTP_IMPLEMENTATION),
            artifact_id: ArtifactId::from("conduit-ai/provider-http-adapter@1"),
            host_calls: vec![operation.clone()],
            resource_requirements: vec![
                resource_requirement(PROVIDER_HTTP_RESOURCE, 1),
                protected_resource_requirement(
                    PROVIDER_CREDENTIAL_ROLE,
                    PROVIDER_CREDENTIAL_CLASS,
                    1,
                ),
            ],
            authority_requirements: vec![AuthorityRequirement {
                contract_id: AuthorityContractId::from(PROVIDER_ENDPOINT_AUTHORITY),
                host_call_contract_id: operation.contract_id,
                subject_kind: kind_id(conduit_web::HTTP_CLIENT_KIND),
            }],
        },
    )
    .narrow_capacity(CapabilityLimits {
        max_active_instances: 1,
        max_queue_items: 1,
        max_queue_bytes: conduit_web::HTTP_MAXIMUM_ENCODED_REQUEST_BYTES
            + conduit_web::HTTP_MAXIMUM_ENCODED_RESPONSE_BYTES,
    })
    .expect("provider HTTP capacity narrows portable HTTP semantics")
    .build()
}

fn provider_definitions() -> Vec<KindProjection> {
    vec![
        definition(
            PROVIDER_REQUEST_KIND,
            "request",
            GENERATION_REQUEST_VALUE_KIND,
            PortTemporal::Value,
            "value",
            conduit_web::JSON_INFO_ID,
            PortTemporal::Value,
        ),
        definition(
            PROVIDER_ENVELOPE_KIND,
            "json",
            conduit_web::JSON_TEXT_INFO_ID,
            PortTemporal::Value,
            "request",
            conduit_web::http_request_type()
                .profile()
                .expect("finite HTTP request profile")
                .value_kind()
                .as_str(),
            PortTemporal::Flow { closes: true },
        ),
        definition(
            PROVIDER_RESPONSE_KIND,
            "response",
            conduit_web::http_response_type()
                .profile()
                .expect("finite HTTP response profile")
                .value_kind()
                .as_str(),
            PortTemporal::Flow { closes: true },
            "json",
            conduit_web::JSON_TEXT_INFO_ID,
            PortTemporal::Value,
        ),
        definition(
            PROVIDER_RESULT_KIND,
            "value",
            conduit_web::JSON_INFO_ID,
            PortTemporal::Value,
            "result",
            GENERATED_RESULT_VALUE_KIND,
            PortTemporal::Value,
        ),
    ]
}

#[allow(clippy::too_many_arguments)]
fn definition(
    kind: &str,
    input_name: &str,
    input_value: &str,
    input_temporal: PortTemporal,
    output_name: &str,
    output_value: &str,
    output_temporal: PortTemporal,
) -> KindProjection {
    KindProjection {
        kind_id: kind_id(kind),
        kind_contract_revision: KindIdentity::from(format!("{kind}@1")),
        inputs: vec![port(
            input_name,
            input_value,
            PortDirection::Input,
            input_temporal,
        )],
        outputs: vec![port(
            output_name,
            output_value,
            PortDirection::Output,
            output_temporal,
        )],
        configuration: Default::default(),
    }
}

fn port(
    name: &str,
    value: &str,
    direction: PortDirection,
    temporal: PortTemporal,
) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(value),
        direction,
        temporal,
        abnormal_kind: None,
    }
}

fn adapter_offer(definition: KindProjection) -> CapabilityOffer {
    let slug = definition.kind_id.as_str().replace('/', "-");
    BackOfferBuilder::new(
        provider_adapter_semantic_contract(definition),
        Back {
            capability_id: CapabilityId::from(format!("provider-{slug}")),
            execution_profile_id: ExecutionProfileId::from("provider/bounded-protocol@1"),
            implementation_id: ImplementationId::from(format!("provider/{slug}@1")),
            artifact_id: ArtifactId::from("conduit-ai/provider-protocol@1"),
            host_calls: Vec::new(),
            resource_requirements: Vec::new(),
            authority_requirements: Vec::new(),
        },
    )
    .build()
}

fn provider_adapter_semantic_contract(definition: KindProjection) -> Kind {
    Kind {
        startup_parameters: Vec::new(),
        shorthand: None,
        kind_id: definition.kind_id,
        kind_contract_revision: definition.kind_contract_revision,
        inputs: definition.inputs,
        outputs: definition.outputs,
        configuration: Default::default(),
        semantic_laws: Default::default(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: conduit_web::JSON_MAXIMUM_ENCODED_BYTES as u32,
        },
    }
}

#[cfg(test)]
#[path = "provider/tests.rs"]
mod tests;
