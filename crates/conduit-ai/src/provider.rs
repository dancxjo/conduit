//! One bounded provider-protocol realization below portable AI meaning.

use alloc::{
    format,
    string::{String, ToString},
    vec,
    vec::Vec,
};
use conduit_core::{
    kind_id, port_id, protected_resource_requirement, resource_requirement, ArtifactId,
    AuthorityContractId, AuthorityRequirement, CapabilityId, CapabilityLimits, CapabilityOffer,
    ExecutionProfileId, HostOperationContractId, HostOperationRequirement, ImplementationId,
    ImplementationOffer, JsonRefusal, JsonValue, KindContractRevision, PortDescriptor,
    PortDirection, PortTemporal,
};
use conduit_form::{
    check_syntax_document, parse_syntax_document, CanonicalBackCatalog, KindDefinition,
    KindSignature, ProfileCatalog, StartupCatalog,
};
use conduit_std_catalog::{
    HttpHeader, HttpMethod, HttpRequest, HttpResponse, HttpTarget, HttpTransactionId,
};

use crate::{GENERATE_TEXT_KIND, TEXT_VALUE_KIND};

pub const PROVIDER_REQUEST_KIND: &str = "provider/openai-compatible-request";
pub const PROVIDER_ENVELOPE_KIND: &str = "provider/openai-compatible-http-envelope";
pub const PROVIDER_RESPONSE_KIND: &str = "provider/openai-compatible-http-response";
pub const PROVIDER_RESULT_KIND: &str = "provider/openai-compatible-result";
pub const PROVIDER_HTTP_IMPLEMENTATION: &str = "provider/openai-compatible-http-client@1";
pub const PROVIDER_HTTP_OPERATION: &str = "conduit.host/http-client-exchange@1";
pub const PROVIDER_ENDPOINT_AUTHORITY: &str = "conduit.authority/provider-endpoint@1";
pub const PROVIDER_CREDENTIAL_CLASS: &str = "conduit.resource/protected-provider-credential@1";
pub const PROVIDER_CREDENTIAL_ROLE: &str = "provider-credential";
pub const PROVIDER_HTTP_RESOURCE: &str = "conduit.resource/network/http-client@1";
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

pub fn provider_request(prompt: &str) -> Result<JsonValue, ProviderFailure> {
    if prompt.len() > MAXIMUM_PROVIDER_PROMPT_BYTES {
        return Err(ProviderFailure::InputOverflow);
    }
    let value = JsonValue::Object(vec![
        ("input".into(), JsonValue::String(prompt.into())),
        ("model".into(), JsonValue::String("conduit-fixture".into())),
        ("stream".into(), JsonValue::Bool(false)),
    ]);
    value.validate().map_err(map_json)?;
    Ok(value)
}

pub fn provider_result(value: &JsonValue) -> Result<String, ProviderFailure> {
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
    Ok(output.clone())
}

pub fn provider_http_request(
    transaction_id: u64,
    authority: &str,
    path_and_query: &str,
    json: &[u8],
) -> Result<HttpRequest, ProviderFailure> {
    let request = HttpRequest {
        transaction_id: HttpTransactionId(transaction_id),
        method: HttpMethod::Post,
        target: HttpTarget {
            scheme: "https".into(),
            authority: authority.into(),
            path_and_query: path_and_query.into(),
        },
        headers: vec![HttpHeader {
            name: "content-type".into(),
            value: b"application/json".to_vec(),
        }],
        body: json.to_vec(),
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
        200..=299 => Ok(&response.body),
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
    conduit_std_catalog::install_json_catalogs(startup, profile)?;
    conduit_std_catalog::install_http_catalogs(startup, profile)?;
    for definition in provider_definitions() {
        startup.insert(KindSignature {
            kind: definition.kind_id.as_str().into(),
            startup_parameters: Vec::new(),
        })?;
        profile
            .insert(definition)
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
        "form {GENERATE_TEXT_KIND} (\n prompt: {TEXT_VALUE_KIND} > text: {TEXT_VALUE_KIND}\n) {{\n request: {PROVIDER_REQUEST_KIND}\n encode: {}\n envelope: {PROVIDER_ENVELOPE_KIND}\n http: {}\n response: {PROVIDER_RESPONSE_KIND}\n decode: {}\n result: {PROVIDER_RESULT_KIND}\n prompt > request.prompt\n request.value > encode.value\n encode.value > envelope.json\n envelope.request > http.request\n http.response > response.response\n response.json > decode.value\n decode.value > result.value\n result.text > text\n}}\n",
        conduit_std_catalog::JSON_ENCODE_KIND,
        conduit_std_catalog::HTTP_CLIENT_KIND,
        conduit_std_catalog::JSON_DECODE_KIND,
    );
    let checked = check_syntax_document(&parse_syntax_document(&source), startup)
        .map_err(|error| format!("provider Back check: {} {}", error.code, error.message))?;
    let high = profile
        .get(&kind_id(GENERATE_TEXT_KIND))
        .ok_or_else(|| "portable generate-text definition missing".to_string())?;
    backs
        .insert(high, &checked, GENERATE_TEXT_KIND)
        .map_err(|error| format!("provider Back catalog: {error:?}"))
}

pub fn provider_offers() -> Vec<CapabilityOffer> {
    provider_definitions()
        .into_iter()
        .map(adapter_offer)
        .collect()
}

pub fn provider_http_offer() -> CapabilityOffer {
    let contract = conduit_std_catalog::http_client_contract();
    let operation = HostOperationRequirement {
        contract_id: HostOperationContractId::from(PROVIDER_HTTP_OPERATION),
        target_kind: Some(kind_id(conduit_std_catalog::HTTP_CLIENT_KIND)),
        maximum_in_flight: 1,
        maximum_input_bytes: conduit_std_catalog::HTTP_MAXIMUM_ENCODED_REQUEST_BYTES,
        maximum_output_bytes: conduit_std_catalog::HTTP_MAXIMUM_ENCODED_RESPONSE_BYTES,
    };
    CapabilityOffer {
        startup_parameters: Vec::new(),
        shorthand: None,
        capability_id: CapabilityId::from("provider-http-client-v1"),
        kind_id: contract.kind_id,
        kind_contract_revision: KindContractRevision::from(
            conduit_std_catalog::HTTP_CLIENT_REVISION,
        ),
        inputs: contract.inputs,
        outputs: contract.outputs,
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from("provider/http-hosted@1"),
            implementation_id: ImplementationId::from(PROVIDER_HTTP_IMPLEMENTATION),
            artifact_id: ArtifactId::from("conduit-ai/provider-http-adapter@1"),
        },
        host_operations: vec![operation.clone()],
        resource_requirements: vec![
            resource_requirement(PROVIDER_HTTP_RESOURCE, 1),
            protected_resource_requirement(PROVIDER_CREDENTIAL_ROLE, PROVIDER_CREDENTIAL_CLASS, 1),
        ],
        authority_requirements: vec![AuthorityRequirement {
            contract_id: AuthorityContractId::from(PROVIDER_ENDPOINT_AUTHORITY),
            host_operation_contract_id: operation.contract_id,
            subject_kind: kind_id(conduit_std_catalog::HTTP_CLIENT_KIND),
        }],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: conduit_std_catalog::HTTP_MAXIMUM_ENCODED_REQUEST_BYTES
                + conduit_std_catalog::HTTP_MAXIMUM_ENCODED_RESPONSE_BYTES,
        },
    }
}

fn provider_definitions() -> Vec<KindDefinition> {
    vec![
        definition(
            PROVIDER_REQUEST_KIND,
            "prompt",
            TEXT_VALUE_KIND,
            PortTemporal::Value,
            "value",
            conduit_core::JSON_INFO_ID,
            PortTemporal::Value,
        ),
        definition(
            PROVIDER_ENVELOPE_KIND,
            "json",
            conduit_core::JSON_TEXT_INFO_ID,
            PortTemporal::Value,
            "request",
            conduit_std_catalog::HTTP_REQUEST_INFO_ID,
            PortTemporal::Flow { closes: true },
        ),
        definition(
            PROVIDER_RESPONSE_KIND,
            "response",
            conduit_std_catalog::HTTP_RESPONSE_INFO_ID,
            PortTemporal::Flow { closes: true },
            "json",
            conduit_core::JSON_TEXT_INFO_ID,
            PortTemporal::Value,
        ),
        definition(
            PROVIDER_RESULT_KIND,
            "value",
            conduit_core::JSON_INFO_ID,
            PortTemporal::Value,
            "text",
            TEXT_VALUE_KIND,
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
) -> KindDefinition {
    KindDefinition {
        kind_id: kind_id(kind),
        kind_contract_revision: KindContractRevision::from(format!("{kind}@1")),
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
        configuration: Vec::new(),
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
    }
}

fn adapter_offer(definition: KindDefinition) -> CapabilityOffer {
    let slug = definition.kind_id.as_str().replace('/', "-");
    CapabilityOffer {
        startup_parameters: Vec::new(),
        shorthand: None,
        capability_id: CapabilityId::from(format!("provider-{slug}")),
        kind_id: definition.kind_id,
        kind_contract_revision: definition.kind_contract_revision,
        inputs: definition.inputs,
        outputs: definition.outputs,
        implementation: ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from("provider/bounded-protocol@1"),
            implementation_id: ImplementationId::from(format!("provider/{slug}@1")),
            artifact_id: ArtifactId::from("conduit-ai/provider-protocol@1"),
        },
        host_operations: Vec::new(),
        resource_requirements: Vec::new(),
        authority_requirements: Vec::new(),
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 1,
            max_queue_bytes: conduit_core::JSON_MAXIMUM_ENCODED_BYTES as u32,
        },
    }
}

#[cfg(test)]
#[path = "provider/tests.rs"]
mod tests;
