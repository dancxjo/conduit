//! Exact bounded learned-model contracts and a deterministic first proof.
//!
//! Model purpose, artifact bytes, schemas, runtime, device, resource, and
//! provider identity remain separate. Registering these contracts does not
//! install a provider, download a model, or expose a universal tensor type to
//! product domains.

pub mod lifecycle;

use conduit_core::{
    ConfigContract, ConfigFieldContract, ConfigIdentity, ConfigMutability, ConfigRequirement,
    ConnectionCardinality, Delivery, Direction, Id, LossAcceptance, NodeContract, PortContract,
    PortFlowConstraints, Presence, SemanticHash, Sensitivity, TemporalContract, TerminalContract,
    TypeContractRef, ValueCardinality,
};
use conduit_panel::{Node, SourceValue};
use conduit_runtime::{
    CompiledInHostService, Handler, Registry, RegistryError, ResolutionError, RunIo, RuntimeError,
    Value,
};

pub const MODEL_PURPOSE_IDENTITY: &str =
    "sha256:1e76e306f83ff89bf3770153145f271a0aadbb8ad536cd0bba28041f31c0ff25";
pub const MODEL_ARTIFACT_IDENTITY: &str =
    "sha256:65ecb31b3f30690325f1e29e6bf34e4ca73118b103ccd1d21c32e4db47d7d29c";
pub const MODEL_GRAPH_IDENTITY: &str =
    "sha256:604a0e059d430170f5d94f7c5275e133b1b4b8ee7f3a890e5b43fdc514c53eb9";
pub const INPUT_SCHEMA_IDENTITY: &str =
    "sha256:753084a8a46eceeed56c50ea01ff84c88c9d6a53217be0a4bbc36e15af956988";
pub const OUTPUT_SCHEMA_IDENTITY: &str =
    "sha256:9fa614c446b8791315cee5d015192640aaecbfbb385823dde094a37f19f50ae3";
pub const PROVENANCE_IDENTITY: &str =
    "sha256:0124a373ef4fbfda15e53e63c791b4763887659b211e41ca1fd400399a992e59";
pub const POLICY_IDENTITY: &str =
    "sha256:46fca87089fe51d208ece004b58cb443861ca2a7f2714f0089c387c0f23f3fb0";
pub const PROVIDER_PROFILE_IDENTITY: &str =
    "sha256:feec98311a5b83c749b82f6ccaadcf5557e7791c94c3f247fc1a3cea7b1c78c9";
pub const EMPTY_STATE_IDENTITY: &str =
    "sha256:e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

pub const MODEL_ARTIFACT_BYTES: &[u8] = &[
    b'C', b'L', b'M', b'0', 1, 0, 1, 0, 2, 0, 3, 0, 4, 0, 0xff, 0xff, 1, 0, 0xff, 0xff, 1, 0, 5, 0,
    0xfb, 0xff,
];
pub const INPUT_TENSOR_BYTES: &[u8] = &[
    b'C', b'L', b'T', b'0', 1, 2, 1, 0, 4, 0, 1, 0, 2, 0, 3, 0, 4, 0,
];
pub const OUTPUT_TENSOR_BYTES: &[u8] =
    &[b'C', b'L', b'T', b'0', 1, 2, 1, 0, 2, 0, 35, 0, 0xfd, 0xff];

pub const MODEL_ARTIFACT_DESCRIPTOR: &str = "conduit.learned/model-artifact|0|purpose,artifact,format,graph,provenance,policy,license,schemas|finite-bytes";
pub const TENSOR_DESCRIPTOR: &str =
    "conduit.learned/tensor|0|finite-rank|i16|exact-shape-layout-bytes-sensitivity";

pub const MODEL_ARTIFACT_TYPE: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("learned/model-artifact"),
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([
        0xe3, 0xad, 0x0d, 0xf2, 0x9b, 0x87, 0x7c, 0x2b, 0x3a, 0xc1, 0xa0, 0xdc, 0xad, 0x6f, 0xc0,
        0xd7, 0x90, 0x46, 0x87, 0xe0, 0x31, 0x50, 0x68, 0xb7, 0x8d, 0x80, 0x4e, 0x3f, 0x63, 0x58,
        0x04, 0xfb,
    ]),
};
pub const TENSOR_TYPE: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("learned/tensor"),
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([
        0xc9, 0x76, 0xd5, 0x4c, 0x5b, 0xb2, 0x14, 0x81, 0xe8, 0x27, 0x26, 0xd9, 0x4d, 0x5f, 0x40,
        0xb6, 0xfd, 0xea, 0xdb, 0x4c, 0x93, 0xa5, 0x5f, 0xca, 0xae, 0x1a, 0x03, 0x7e, 0x35, 0x16,
        0x50, 0x7b,
    ]),
};
const TEXT_TYPE: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("std/text"),
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([
        0x94, 0xdf, 0xe2, 0x55, 0x09, 0xfe, 0x62, 0x4d, 0x89, 0x74, 0xb1, 0xdd, 0x44, 0x2e, 0xb7,
        0xf9, 0x6f, 0x7e, 0x62, 0x1e, 0x6e, 0x71, 0xf0, 0x35, 0xac, 0x6f, 0x08, 0x04, 0x63, 0x61,
        0x80, 0x72,
    ]),
};
const U64_TYPE: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("std/u64"),
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([
        0xf9, 0xba, 0xd3, 0xea, 0x53, 0xd3, 0xca, 0x01, 0xa0, 0xa4, 0xd6, 0x9f, 0x86, 0xc8, 0x25,
        0x65, 0x17, 0x07, 0x16, 0x45, 0xea, 0x7d, 0x68, 0xef, 0x63, 0x6b, 0x6d, 0x94, 0x87, 0x70,
        0xf0, 0xec,
    ]),
};

const fn field(
    key: &'static str,
    value_type: TypeContractRef<'static>,
) -> ConfigFieldContract<'static> {
    ConfigFieldContract {
        key: Id(key),
        value_type,
        requirement: ConfigRequirement::Required,
        sensitivity: Sensitivity::Public,
        mutability: ConfigMutability::PreStart,
        identity: ConfigIdentity::Semantic,
    }
}

const MODEL_FIELDS: [ConfigFieldContract<'static>; 12] = [
    field("fixture", TEXT_TYPE),
    field("semantic_model_identity", TEXT_TYPE),
    field("artifact_identity", TEXT_TYPE),
    field("format", TEXT_TYPE),
    field("opset", U64_TYPE),
    field("graph_identity", TEXT_TYPE),
    field("provenance_identity", TEXT_TYPE),
    field("policy_identity", TEXT_TYPE),
    field("license", TEXT_TYPE),
    field("input_schema_identity", TEXT_TYPE),
    field("output_schema_identity", TEXT_TYPE),
    field("maximum_artifact_bytes", U64_TYPE),
];
const TENSOR_FIELDS: [ConfigFieldContract<'static>; 7] = [
    field("fixture", TEXT_TYPE),
    field("schema_identity", TEXT_TYPE),
    field("dtype", TEXT_TYPE),
    field("shape", TEXT_TYPE),
    field("layout", TEXT_TYPE),
    field("maximum_output_bytes", U64_TYPE),
    field("sensitivity", TEXT_TYPE),
];
const INFERENCE_FIELDS: [ConfigFieldContract<'static>; 24] = [
    field("semantic_model_identity", TEXT_TYPE),
    field("artifact_identity", TEXT_TYPE),
    field("format", TEXT_TYPE),
    field("opset", U64_TYPE),
    field("graph_identity", TEXT_TYPE),
    field("input_schema_identity", TEXT_TYPE),
    field("output_schema_identity", TEXT_TYPE),
    field("runtime_identity", TEXT_TYPE),
    field("device_identity", TEXT_TYPE),
    field("resource_identity", TEXT_TYPE),
    field("provider_profile_identity", TEXT_TYPE),
    field("dtype", TEXT_TYPE),
    field("input_shape", TEXT_TYPE),
    field("output_shape", TEXT_TYPE),
    field("layout", TEXT_TYPE),
    field("maximum_batch", U64_TYPE),
    field("maximum_input_bytes", U64_TYPE),
    field("maximum_output_bytes", U64_TYPE),
    field("maximum_retained_bytes", U64_TYPE),
    field("maximum_state_bytes", U64_TYPE),
    field("maximum_work", U64_TYPE),
    field("state_identity", TEXT_TYPE),
    field("determinism", TEXT_TYPE),
    field("tolerance", TEXT_TYPE),
];

const fn port(
    id: &'static str,
    direction: Direction,
    value_type: TypeContractRef<'static>,
    connections: ConnectionCardinality,
) -> PortContract<'static> {
    PortContract {
        id: Id(id),
        direction,
        value_type,
        presence: Presence::Required,
        connections,
        values: ValueCardinality::ExactlyOne,
        delivery: Delivery::Stream,
        temporal: TemporalContract::Committed,
        terminal: TerminalContract::Finite,
        sensitivity: Sensitivity::Public,
        flow: PortFlowConstraints {
            loss: LossAcceptance::LosslessOnly,
        },
    }
}

const MODEL_OUTPUT: [PortContract<'static>; 1] = [port(
    "model",
    Direction::Output,
    MODEL_ARTIFACT_TYPE,
    ConnectionCardinality::OneOrMore,
)];
const TENSOR_OUTPUT: [PortContract<'static>; 1] = [port(
    "tensor",
    Direction::Output,
    TENSOR_TYPE,
    ConnectionCardinality::OneOrMore,
)];
const INFERENCE_INPUTS: [PortContract<'static>; 2] = [
    port(
        "model",
        Direction::Input,
        MODEL_ARTIFACT_TYPE,
        ConnectionCardinality::ExactlyOne,
    ),
    port(
        "tensor",
        Direction::Input,
        TENSOR_TYPE,
        ConnectionCardinality::ExactlyOne,
    ),
];
const TENSOR_INPUT: [PortContract<'static>; 1] = [port(
    "tensor",
    Direction::Input,
    TENSOR_TYPE,
    ConnectionCardinality::ExactlyOne,
)];
const SUMMARY_OUTPUT: [PortContract<'static>; 1] = [PortContract {
    id: Id("summary"),
    direction: Direction::Output,
    value_type: TEXT_TYPE,
    presence: Presence::Required,
    connections: ConnectionCardinality::OneOrMore,
    values: ValueCardinality::ExactlyOne,
    delivery: Delivery::FiniteBatch,
    temporal: TemporalContract::Atemporal,
    terminal: TerminalContract::Finite,
    sensitivity: Sensitivity::Public,
    flow: PortFlowConstraints {
        loss: LossAcceptance::LosslessOnly,
    },
}];

pub const MODEL_LITERAL_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("learned/model/literal"),
    config: ConfigContract {
        fields: &MODEL_FIELDS,
    },
    inputs: &[],
    outputs: &MODEL_OUTPUT,
};
pub const TENSOR_LITERAL_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("learned/tensor/literal"),
    config: ConfigContract {
        fields: &TENSOR_FIELDS,
    },
    inputs: &[],
    outputs: &TENSOR_OUTPUT,
};
pub const INFER_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("learned/infer"),
    config: ConfigContract {
        fields: &INFERENCE_FIELDS,
    },
    inputs: &INFERENCE_INPUTS,
    outputs: &TENSOR_OUTPUT,
};
pub const TENSOR_INSPECT_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("learned/tensor/inspect"),
    config: ConfigContract { fields: &[] },
    inputs: &TENSOR_INPUT,
    outputs: &SUMMARY_OUTPUT,
};

pub const LEARNED_CONTRACTS: [&NodeContract<'static>; 4] = [
    &MODEL_LITERAL_CONTRACT,
    &TENSOR_LITERAL_CONTRACT,
    &INFER_CONTRACT,
    &TENSOR_INSPECT_CONTRACT,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InferenceReason {
    WrongType,
    WrongArtifact,
    WrongFormat,
    SchemaMismatch,
    ShapeMismatch,
    DtypeMismatch,
    LayoutMismatch,
    BatchOverflow,
    InputOverflow,
    OutputOverflow,
    RetainedOverflow,
    StateMismatch,
    StateOverflow,
    WorkOverflow,
    UnsupportedRuntime,
    UnsupportedDevice,
    UnsupportedOpset,
    SensitivityDenied,
    Cancelled,
    ProviderLost,
}

impl InferenceReason {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::WrongType => "CND-LEARN-001",
            Self::WrongArtifact | Self::WrongFormat | Self::UnsupportedOpset => "CND-LEARN-002",
            Self::SchemaMismatch
            | Self::ShapeMismatch
            | Self::DtypeMismatch
            | Self::LayoutMismatch => "CND-LEARN-003",
            Self::BatchOverflow
            | Self::InputOverflow
            | Self::OutputOverflow
            | Self::RetainedOverflow
            | Self::StateOverflow
            | Self::WorkOverflow => "CND-LEARN-004",
            Self::StateMismatch => "CND-LEARN-005",
            Self::UnsupportedRuntime | Self::UnsupportedDevice => "CND-LEARN-006",
            Self::SensitivityDenied => "CND-LEARN-007",
            Self::Cancelled => "CND-LEARN-008",
            Self::ProviderLost => "CND-LEARN-009",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InferenceBounds {
    pub maximum_batch: usize,
    pub maximum_input_bytes: usize,
    pub maximum_output_bytes: usize,
    pub maximum_retained_bytes: usize,
    pub maximum_state_bytes: usize,
    pub maximum_work: usize,
}

impl InferenceBounds {
    pub const FIRST_PROOF: Self = Self {
        maximum_batch: 1,
        maximum_input_bytes: 64,
        maximum_output_bytes: 64,
        maximum_retained_bytes: 128,
        maximum_state_bytes: 0,
        maximum_work: 256,
    };

    fn validate(self) -> Result<(), InferenceReason> {
        if self.maximum_batch != 1 {
            return Err(InferenceReason::BatchOverflow);
        }
        if self.maximum_input_bytes < INPUT_TENSOR_BYTES.len() {
            return Err(InferenceReason::InputOverflow);
        }
        if self.maximum_output_bytes < OUTPUT_TENSOR_BYTES.len() {
            return Err(InferenceReason::OutputOverflow);
        }
        if self.maximum_retained_bytes < MODEL_ARTIFACT_BYTES.len() + INPUT_TENSOR_BYTES.len() {
            return Err(InferenceReason::RetainedOverflow);
        }
        if self.maximum_state_bytes != 0 {
            return Err(InferenceReason::StateMismatch);
        }
        if self.maximum_work < 64 {
            return Err(InferenceReason::WorkOverflow);
        }
        Ok(())
    }
}

fn read_i16(bytes: &[u8], offset: usize) -> Result<i16, InferenceReason> {
    let value = bytes
        .get(offset..offset + 2)
        .ok_or(InferenceReason::ShapeMismatch)?;
    Ok(i16::from_le_bytes([value[0], value[1]]))
}

fn validate_tensor(bytes: &[u8], shape: [u16; 2], maximum: usize) -> Result<(), InferenceReason> {
    if bytes.len() > maximum {
        return Err(InferenceReason::InputOverflow);
    }
    if bytes.get(..4) != Some(b"CLT0") {
        return Err(InferenceReason::WrongType);
    }
    if bytes.get(4) != Some(&1) {
        return Err(InferenceReason::DtypeMismatch);
    }
    if bytes.get(5) != Some(&2) {
        return Err(InferenceReason::ShapeMismatch);
    }
    if bytes.get(6..10) != Some(&[shape[0] as u8, 0, shape[1] as u8, 0]) {
        return Err(InferenceReason::ShapeMismatch);
    }
    let expected = 10_usize
        .checked_add(usize::from(shape[0]) * usize::from(shape[1]) * 2)
        .ok_or(InferenceReason::InputOverflow)?;
    if bytes.len() != expected {
        return Err(InferenceReason::ShapeMismatch);
    }
    Ok(())
}

pub fn infer_fixed_linear(
    artifact: &[u8],
    input: &[u8],
    bounds: InferenceBounds,
) -> Result<Vec<u8>, InferenceReason> {
    bounds.validate()?;
    if artifact.len() + input.len() > bounds.maximum_retained_bytes {
        return Err(InferenceReason::RetainedOverflow);
    }
    if artifact != MODEL_ARTIFACT_BYTES {
        return Err(InferenceReason::WrongArtifact);
    }
    validate_tensor(input, [1, 4], bounds.maximum_input_bytes)?;
    let work = artifact
        .len()
        .checked_add(input.len())
        .and_then(|value| value.checked_add(8 * 4))
        .ok_or(InferenceReason::WorkOverflow)?;
    if work > bounds.maximum_work {
        return Err(InferenceReason::WorkOverflow);
    }
    let mut output = Vec::from(&OUTPUT_TENSOR_BYTES[..10]);
    for row in 0..2 {
        let mut accumulator = i32::from(read_i16(artifact, 22 + row * 2)?);
        for column in 0..4 {
            let weight = i32::from(read_i16(artifact, 6 + (row * 4 + column) * 2)?);
            let value = i32::from(read_i16(input, 10 + column * 2)?);
            accumulator = accumulator
                .checked_add(
                    weight
                        .checked_mul(value)
                        .ok_or(InferenceReason::WorkOverflow)?,
                )
                .ok_or(InferenceReason::WorkOverflow)?;
        }
        let value = i16::try_from(accumulator).map_err(|_| InferenceReason::OutputOverflow)?;
        output.extend_from_slice(&value.to_le_bytes());
    }
    if output.len() > bounds.maximum_output_bytes {
        return Err(InferenceReason::OutputOverflow);
    }
    Ok(output)
}

fn exact_u64(node: &Node, key: &str) -> Option<u64> {
    match node.config_value(key) {
        Some(SourceValue::Integer(value)) => u64::try_from(*value).ok(),
        _ => None,
    }
}

fn validate_model_config(node: &Node) -> Result<(), ResolutionError> {
    (node.config.len() == MODEL_FIELDS.len()
        && node.config("fixture") == Some("fixed-linear-i16-2x4")
        && node.config("semantic_model_identity") == Some(MODEL_PURPOSE_IDENTITY)
        && node.config("artifact_identity") == Some(MODEL_ARTIFACT_IDENTITY)
        && node.config("format") == Some("conduit-fixed-linear")
        && exact_u64(node, "opset") == Some(0)
        && node.config("graph_identity") == Some(MODEL_GRAPH_IDENTITY)
        && node.config("provenance_identity") == Some(PROVENANCE_IDENTITY)
        && node.config("policy_identity") == Some(POLICY_IDENTITY)
        && node.config("license") == Some("CC0-1.0")
        && node.config("input_schema_identity") == Some(INPUT_SCHEMA_IDENTITY)
        && node.config("output_schema_identity") == Some(OUTPUT_SCHEMA_IDENTITY)
        && exact_u64(node, "maximum_artifact_bytes") == Some(64))
    .then_some(())
    .ok_or_else(|| {
        ResolutionError::new(
            "CND-LEARN-002",
            "model literal requires the exact content-addressed fixture",
        )
    })
}

fn validate_tensor_config(node: &Node) -> Result<(), ResolutionError> {
    (node.config.len() == TENSOR_FIELDS.len()
        && node.config("fixture") == Some("input-i16-1x4")
        && node.config("schema_identity") == Some(INPUT_SCHEMA_IDENTITY)
        && node.config("dtype") == Some("i16le")
        && node.config("shape") == Some("1x4")
        && node.config("layout") == Some("row-major")
        && exact_u64(node, "maximum_output_bytes") == Some(64)
        && node.config("sensitivity") == Some("public"))
    .then_some(())
    .ok_or_else(|| {
        ResolutionError::new(
            "CND-LEARN-003",
            "tensor literal requires the exact finite input schema",
        )
    })
}

fn validate_inference_config(node: &Node) -> Result<(), ResolutionError> {
    let exact = node.config.len() == INFERENCE_FIELDS.len()
        && node.config("semantic_model_identity") == Some(MODEL_PURPOSE_IDENTITY)
        && node.config("artifact_identity") == Some(MODEL_ARTIFACT_IDENTITY)
        && node.config("format") == Some("conduit-fixed-linear")
        && exact_u64(node, "opset") == Some(0)
        && node.config("graph_identity") == Some(MODEL_GRAPH_IDENTITY)
        && node.config("input_schema_identity") == Some(INPUT_SCHEMA_IDENTITY)
        && node.config("output_schema_identity") == Some(OUTPUT_SCHEMA_IDENTITY)
        && node.config("runtime_identity") == Some("conduit.learned/runtime/rust-fixed-linear")
        && node.config("device_identity") == Some("conduit.learned/device/cpu-reference")
        && node.config("resource_identity") == Some("conduit.learned/resource/cpu-reference-0")
        && node.config("provider_profile_identity") == Some(PROVIDER_PROFILE_IDENTITY)
        && node.config("dtype") == Some("i16le")
        && node.config("input_shape") == Some("1x4")
        && node.config("output_shape") == Some("1x2")
        && node.config("layout") == Some("row-major")
        && exact_u64(node, "maximum_batch") == Some(1)
        && exact_u64(node, "maximum_input_bytes") == Some(64)
        && exact_u64(node, "maximum_output_bytes") == Some(64)
        && exact_u64(node, "maximum_retained_bytes") == Some(128)
        && exact_u64(node, "maximum_state_bytes") == Some(0)
        && exact_u64(node, "maximum_work") == Some(256)
        && node.config("state_identity") == Some(EMPTY_STATE_IDENTITY)
        && node.config("determinism") == Some("exact")
        && node.config("tolerance") == Some("none");
    exact.then_some(()).ok_or_else(|| {
        ResolutionError::new("CND-LEARN-006", "inference requires the exact runtime, device, resource, schemas, state, and finite limits")
    })
}

fn runtime(reason: InferenceReason) -> RuntimeError {
    RuntimeError::new(
        reason.code(),
        format!("bounded inference failed: {reason:?}"),
    )
}

struct ModelLiteral;
impl Handler for ModelLiteral {
    fn run(
        &mut self,
        _node: &Node,
        inputs: &[Value],
        _io: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        if !inputs.is_empty() {
            return Err(runtime(InferenceReason::WrongType));
        }
        Ok(vec![Value {
            value_type: MODEL_ARTIFACT_TYPE,
            bytes: MODEL_ARTIFACT_BYTES.to_vec(),
        }])
    }
}
struct TensorLiteral;
impl Handler for TensorLiteral {
    fn run(
        &mut self,
        _node: &Node,
        inputs: &[Value],
        _io: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        if !inputs.is_empty() {
            return Err(runtime(InferenceReason::WrongType));
        }
        Ok(vec![Value {
            value_type: TENSOR_TYPE,
            bytes: INPUT_TENSOR_BYTES.to_vec(),
        }])
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InferenceProviderFault {
    None,
    ProviderLost,
}

struct Infer {
    fault: InferenceProviderFault,
}

impl Handler for Infer {
    fn run(
        &mut self,
        _node: &Node,
        inputs: &[Value],
        _io: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        if self.fault == InferenceProviderFault::ProviderLost {
            return Err(runtime(InferenceReason::ProviderLost));
        }
        let [model, tensor] = inputs else {
            return Err(runtime(InferenceReason::WrongType));
        };
        if model.value_type != MODEL_ARTIFACT_TYPE || tensor.value_type != TENSOR_TYPE {
            return Err(runtime(InferenceReason::WrongType));
        }
        Ok(vec![Value {
            value_type: TENSOR_TYPE,
            bytes: infer_fixed_linear(&model.bytes, &tensor.bytes, InferenceBounds::FIRST_PROOF)
                .map_err(runtime)?,
        }])
    }
}
struct TensorInspect;
impl Handler for TensorInspect {
    fn run(
        &mut self,
        _node: &Node,
        inputs: &[Value],
        _io: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        let [tensor] = inputs else {
            return Err(runtime(InferenceReason::WrongType));
        };
        if tensor.value_type != TENSOR_TYPE {
            return Err(runtime(InferenceReason::WrongType));
        }
        validate_tensor(&tensor.bytes, [1, 2], 64).map_err(runtime)?;
        let first = read_i16(&tensor.bytes, 10).map_err(runtime)?;
        let second = read_i16(&tensor.bytes, 12).map_err(runtime)?;
        Ok(vec![Value {
            value_type: TEXT_TYPE,
            bytes: format!("learned:i16:1x2:[{first},{second}]").into_bytes(),
        }])
    }
}

pub fn register_learned_contracts(registry: &mut Registry) {
    for contract in LEARNED_CONTRACTS {
        registry.register_contract_only(contract);
    }
}

pub fn register_deterministic_inference_provider(
    registry: &mut Registry,
) -> Result<(), RegistryError> {
    register_deterministic_inference_provider_with_fault(registry, InferenceProviderFault::None)
}

fn inference_provider() -> Box<dyn Handler> {
    Box::new(Infer {
        fault: InferenceProviderFault::None,
    })
}

fn lost_inference_provider() -> Box<dyn Handler> {
    Box::new(Infer {
        fault: InferenceProviderFault::ProviderLost,
    })
}

pub fn register_deterministic_inference_provider_with_fault(
    registry: &mut Registry,
    fault: InferenceProviderFault,
) -> Result<(), RegistryError> {
    register_learned_contracts(registry);
    static NO_AUTHORITIES: [SemanticHash; 0] = [];
    for (contract, implementation_id, artifact_id, entrypoint, factory, validator) in [
        (
            &MODEL_LITERAL_CONTRACT,
            "conduit.learned/model-literal-deterministic",
            "conduit.learned/model-literal-artifact",
            "learned-model-literal",
            (|| Box::new(ModelLiteral) as Box<dyn Handler>) as conduit_runtime::HandlerFactory,
            validate_model_config as conduit_runtime::ConfigValidator,
        ),
        (
            &TENSOR_LITERAL_CONTRACT,
            "conduit.learned/tensor-literal-deterministic",
            "conduit.learned/tensor-literal-artifact",
            "learned-tensor-literal",
            (|| Box::new(TensorLiteral) as Box<dyn Handler>) as conduit_runtime::HandlerFactory,
            validate_tensor_config as conduit_runtime::ConfigValidator,
        ),
        (
            &INFER_CONTRACT,
            "conduit.learned/fixed-linear-rust",
            "conduit.learned/fixed-linear-rust-artifact",
            "learned-fixed-linear-infer",
            match fault {
                InferenceProviderFault::None => inference_provider,
                InferenceProviderFault::ProviderLost => lost_inference_provider,
            },
            validate_inference_config as conduit_runtime::ConfigValidator,
        ),
        (
            &TENSOR_INSPECT_CONTRACT,
            "conduit.learned/tensor-inspect-deterministic",
            "conduit.learned/tensor-inspect-artifact",
            "learned-tensor-inspect",
            (|| Box::new(TensorInspect) as Box<dyn Handler>) as conduit_runtime::HandlerFactory,
            (|node: &Node| {
                node.config.is_empty().then_some(()).ok_or_else(|| {
                    ResolutionError::new("CND-LEARN-003", "tensor inspector has no configuration")
                })
            }) as conduit_runtime::ConfigValidator,
        ),
    ] {
        registry.register_compiled_in_host_service(CompiledInHostService {
            contract,
            implementation_id,
            artifact_id,
            entrypoint,
            source_bytes: include_bytes!("lib.rs"),
            required_authorities: &NO_AUTHORITIES,
            factory,
            validate_config: validator,
        })?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

    const FIXTURE: &str = include_str!("../../../conformance/c4/learned-inference.json");

    #[test]
    fn fixed_inference_is_exact_and_content_addressed() {
        assert_eq!(
            format!("sha256:{:x}", Sha256::digest(MODEL_ARTIFACT_BYTES)),
            MODEL_ARTIFACT_IDENTITY
        );
        assert_eq!(
            infer_fixed_linear(
                MODEL_ARTIFACT_BYTES,
                INPUT_TENSOR_BYTES,
                InferenceBounds::FIRST_PROOF
            )
            .unwrap(),
            OUTPUT_TENSOR_BYTES
        );
    }

    #[test]
    fn artifact_schema_and_every_finite_bound_fail_closed() {
        let mut artifact = MODEL_ARTIFACT_BYTES.to_vec();
        artifact[0] ^= 1;
        assert_eq!(
            infer_fixed_linear(&artifact, INPUT_TENSOR_BYTES, InferenceBounds::FIRST_PROOF),
            Err(InferenceReason::WrongArtifact)
        );
        let mut tensor = INPUT_TENSOR_BYTES.to_vec();
        tensor[4] = 2;
        assert_eq!(
            infer_fixed_linear(MODEL_ARTIFACT_BYTES, &tensor, InferenceBounds::FIRST_PROOF),
            Err(InferenceReason::DtypeMismatch)
        );
        let mut tensor = INPUT_TENSOR_BYTES.to_vec();
        tensor[8] = 5;
        assert_eq!(
            infer_fixed_linear(MODEL_ARTIFACT_BYTES, &tensor, InferenceBounds::FIRST_PROOF),
            Err(InferenceReason::ShapeMismatch)
        );
        for (bounds, expected) in [
            (
                InferenceBounds {
                    maximum_batch: 2,
                    ..InferenceBounds::FIRST_PROOF
                },
                InferenceReason::BatchOverflow,
            ),
            (
                InferenceBounds {
                    maximum_input_bytes: 1,
                    ..InferenceBounds::FIRST_PROOF
                },
                InferenceReason::InputOverflow,
            ),
            (
                InferenceBounds {
                    maximum_output_bytes: 1,
                    ..InferenceBounds::FIRST_PROOF
                },
                InferenceReason::OutputOverflow,
            ),
            (
                InferenceBounds {
                    maximum_retained_bytes: 1,
                    ..InferenceBounds::FIRST_PROOF
                },
                InferenceReason::RetainedOverflow,
            ),
            (
                InferenceBounds {
                    maximum_state_bytes: 1,
                    ..InferenceBounds::FIRST_PROOF
                },
                InferenceReason::StateMismatch,
            ),
            (
                InferenceBounds {
                    maximum_work: 1,
                    ..InferenceBounds::FIRST_PROOF
                },
                InferenceReason::WorkOverflow,
            ),
        ] {
            assert_eq!(
                infer_fixed_linear(MODEL_ARTIFACT_BYTES, INPUT_TENSOR_BYTES, bounds),
                Err(expected)
            );
        }
    }

    #[test]
    fn contracts_do_not_install_an_inference_provider() {
        let mut registry = Registry::default();
        register_learned_contracts(&mut registry);
        assert!(
            registry
                .installed_providers()
                .iter()
                .all(|provider| !LEARNED_CONTRACTS.contains(&provider.contract))
        );
    }

    #[test]
    fn conformance_fixture_owns_the_complete_first_inference_matrix() {
        let fixture: serde_json::Value = serde_json::from_str(FIXTURE).unwrap();
        assert_eq!(fixture["schema"], "conduit.learned-inference-conformance");
        assert_eq!(fixture["schema_version"], 0);
        assert_eq!(fixture["semantic_model_identity"], MODEL_PURPOSE_IDENTITY);
        assert_eq!(fixture["artifact_identity"], MODEL_ARTIFACT_IDENTITY);
        assert_eq!(fixture["input_schema_identity"], INPUT_SCHEMA_IDENTITY);
        assert_eq!(fixture["output_schema_identity"], OUTPUT_SCHEMA_IDENTITY);
        assert_eq!(fixture["positive"].as_array().unwrap().len(), 7);
        assert_eq!(fixture["negative"].as_array().unwrap().len(), 16);
        assert_ne!(
            InferenceReason::Cancelled.code(),
            InferenceReason::ProviderLost.code()
        );
        assert_ne!(
            InferenceReason::UnsupportedDevice.code(),
            InferenceReason::SchemaMismatch.code()
        );
    }
}
