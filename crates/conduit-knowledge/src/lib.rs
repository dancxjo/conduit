//! Source-grounded, bounded knowledge values and a deterministic retrieval proof.
//!
//! Documents, queries, retrieval results, citations, and execution evidence are
//! deliberately separate. The first provider owns a tiny checked corpus; it
//! performs no fetch, model download, generated-text call, or ambient lookup.

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

pub const MAXIMUM_DOCUMENT_BYTES: usize = 256;
pub const MAXIMUM_QUERY_BYTES: usize = 64;
pub const MAXIMUM_RESULTS: usize = 4;
pub const MAXIMUM_RETAINED_BYTES: usize = 1024;
pub const MAXIMUM_WORK: usize = 4096;

pub const SOURCE_IDENTITY: [u8; 32] = [0x61; 32];
pub const REVISION_IDENTITY: [u8; 32] = [0x62; 32];
pub const INDEX_IDENTITY: [u8; 32] = [0x63; 32];
pub const EMBEDDING_IDENTITY: [u8; 32] = [0x64; 32];
pub const RERANKER_IDENTITY: [u8; 32] = [0x65; 32];
pub const SOURCE_IDENTITY_TEXT: &str =
    "sha256:6161616161616161616161616161616161616161616161616161616161616161";
pub const REVISION_IDENTITY_TEXT: &str =
    "sha256:6262626262626262626262626262626262626262626262626262626262626262";
pub const INDEX_IDENTITY_TEXT: &str =
    "sha256:6363636363636363636363636363636363636363636363636363636363636363";
pub const EMBEDDING_IDENTITY_TEXT: &str =
    "sha256:6464646464646464646464646464646464646464646464646464646464646464";
pub const RERANKER_IDENTITY_TEXT: &str =
    "sha256:6565656565656565656565656565656565656565656565656565656565656565";
pub const DOCUMENT_TEXT: &str =
    "Conduit keeps authored source, exact plans, run evidence, and presentation distinct.";
pub const QUERY_TEXT: &str = "exact plans";

pub const DOCUMENT_TYPE: TypeContractRef<'static> = type_ref("knowledge/document", 0x71);
pub const INDEX_TYPE: TypeContractRef<'static> = type_ref("knowledge/index-snapshot", 0x72);
pub const QUERY_TYPE: TypeContractRef<'static> = type_ref("knowledge/query", 0x73);
pub const RESULTS_TYPE: TypeContractRef<'static> = type_ref("knowledge/retrieval-results", 0x74);
pub const CITATION_TYPE: TypeContractRef<'static> = type_ref("knowledge/citation", 0x75);
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

const fn type_ref(id: &'static str, byte: u8) -> TypeContractRef<'static> {
    TypeContractRef {
        contract_id: Id(id),
        schema_version: 0,
        semantic_hash: SemanticHash::from_bytes([byte; 32]),
    }
}

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

const fn port(
    id: &'static str,
    direction: Direction,
    value_type: TypeContractRef<'static>,
) -> PortContract<'static> {
    PortContract {
        id: Id(id),
        direction,
        value_type,
        presence: Presence::Required,
        connections: if matches!(direction, Direction::Input) {
            ConnectionCardinality::ExactlyOne
        } else {
            ConnectionCardinality::OneOrMore
        },
        values: ValueCardinality::ExactlyOne,
        delivery: Delivery::FiniteBatch,
        temporal: TemporalContract::Committed,
        terminal: TerminalContract::Finite,
        sensitivity: Sensitivity::Public,
        flow: PortFlowConstraints {
            loss: LossAcceptance::LosslessOnly,
        },
    }
}

const DOCUMENT_FIELDS: [ConfigFieldContract<'static>; 5] = [
    field("fixture", TEXT_TYPE),
    field("source_identity", TEXT_TYPE),
    field("revision_identity", TEXT_TYPE),
    field("maximum_document_bytes", U64_TYPE),
    field("access_scope", TEXT_TYPE),
];
const INDEX_FIELDS: [ConfigFieldContract<'static>; 6] = [
    field("index_identity", TEXT_TYPE),
    field("embedding_identity", TEXT_TYPE),
    field("embedding_dimensions", U64_TYPE),
    field("coverage", TEXT_TYPE),
    field("maximum_retained_bytes", U64_TYPE),
    field("maximum_work", U64_TYPE),
];
const QUERY_FIELDS: [ConfigFieldContract<'static>; 2] = [
    field("fixture", TEXT_TYPE),
    field("maximum_query_bytes", U64_TYPE),
];
const RETRIEVE_FIELDS: [ConfigFieldContract<'static>; 6] = [
    field("index_identity", TEXT_TYPE),
    field("embedding_identity", TEXT_TYPE),
    field("embedding_dimensions", U64_TYPE),
    field("maximum_results", U64_TYPE),
    field("maximum_context_bytes", U64_TYPE),
    field("maximum_work", U64_TYPE),
];
const RERANK_FIELDS: [ConfigFieldContract<'static>; 3] = [
    field("reranker_identity", TEXT_TYPE),
    field("maximum_results", U64_TYPE),
    field("maximum_work", U64_TYPE),
];

const DOCUMENT_OUTPUTS: [PortContract<'static>; 1] =
    [port("document", Direction::Output, DOCUMENT_TYPE)];
const DOCUMENT_INPUTS: [PortContract<'static>; 1] =
    [port("document", Direction::Input, DOCUMENT_TYPE)];
const INDEX_OUTPUTS: [PortContract<'static>; 1] = [port("index", Direction::Output, INDEX_TYPE)];
const INDEX_QUERY_INPUTS: [PortContract<'static>; 2] = [
    port("index", Direction::Input, INDEX_TYPE),
    port("query", Direction::Input, QUERY_TYPE),
];
const QUERY_OUTPUTS: [PortContract<'static>; 1] = [port("query", Direction::Output, QUERY_TYPE)];
const RESULTS_OUTPUTS: [PortContract<'static>; 1] =
    [port("results", Direction::Output, RESULTS_TYPE)];
const RESULTS_INPUTS: [PortContract<'static>; 1] =
    [port("results", Direction::Input, RESULTS_TYPE)];
const CITATION_OUTPUTS: [PortContract<'static>; 1] =
    [port("citation", Direction::Output, CITATION_TYPE)];
const CITATION_INPUTS: [PortContract<'static>; 1] =
    [port("citation", Direction::Input, CITATION_TYPE)];
const TEXT_OUTPUTS: [PortContract<'static>; 1] = [PortContract {
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

pub const DOCUMENT_LITERAL_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("knowledge/document/literal"),
    config: ConfigContract {
        fields: &DOCUMENT_FIELDS,
    },
    inputs: &[],
    outputs: &DOCUMENT_OUTPUTS,
};
pub const INDEX_FIXTURE_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("knowledge/index/fixture"),
    config: ConfigContract {
        fields: &INDEX_FIELDS,
    },
    inputs: &DOCUMENT_INPUTS,
    outputs: &INDEX_OUTPUTS,
};
pub const QUERY_LITERAL_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("knowledge/query/literal"),
    config: ConfigContract {
        fields: &QUERY_FIELDS,
    },
    inputs: &[],
    outputs: &QUERY_OUTPUTS,
};
pub const RETRIEVE_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("knowledge/retrieve"),
    config: ConfigContract {
        fields: &RETRIEVE_FIELDS,
    },
    inputs: &INDEX_QUERY_INPUTS,
    outputs: &RESULTS_OUTPUTS,
};
pub const RERANK_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("knowledge/rerank"),
    config: ConfigContract {
        fields: &RERANK_FIELDS,
    },
    inputs: &RESULTS_INPUTS,
    outputs: &RESULTS_OUTPUTS,
};
pub const CITE_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("knowledge/citation/assemble"),
    config: ConfigContract { fields: &[] },
    inputs: &RESULTS_INPUTS,
    outputs: &CITATION_OUTPUTS,
};
pub const CITATION_INSPECT_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("knowledge/citation/inspect"),
    config: ConfigContract { fields: &[] },
    inputs: &CITATION_INPUTS,
    outputs: &TEXT_OUTPUTS,
};

pub const KNOWLEDGE_CONTRACTS: [&NodeContract<'static>; 7] = [
    &DOCUMENT_LITERAL_CONTRACT,
    &INDEX_FIXTURE_CONTRACT,
    &QUERY_LITERAL_CONTRACT,
    &RETRIEVE_CONTRACT,
    &RERANK_CONTRACT,
    &CITE_CONTRACT,
    &CITATION_INSPECT_CONTRACT,
];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KnowledgeReason {
    WrongType,
    MissingSource,
    DeletedSource,
    StaleRevision,
    PartialIndex,
    InvalidSpan,
    DocumentOverflow,
    QueryOverflow,
    ResultOverflow,
    ContextOverflow,
    RetainedOverflow,
    WorkOverflow,
    EmbeddingMismatch,
    DimensionMismatch,
    IndexMismatch,
    AccessDenied,
    ScoreTie,
    Cancellation,
    ProviderLost,
}

impl KnowledgeReason {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::WrongType | Self::InvalidSpan => "CND-KNOW-001",
            Self::MissingSource
            | Self::DeletedSource
            | Self::StaleRevision
            | Self::PartialIndex => "CND-KNOW-002",
            Self::EmbeddingMismatch | Self::DimensionMismatch | Self::IndexMismatch => {
                "CND-KNOW-003"
            }
            Self::DocumentOverflow
            | Self::QueryOverflow
            | Self::ResultOverflow
            | Self::ContextOverflow
            | Self::RetainedOverflow
            | Self::WorkOverflow => "CND-KNOW-004",
            Self::AccessDenied => "CND-KNOW-005",
            Self::ScoreTie => "CND-KNOW-006",
            Self::Cancellation => "CND-KNOW-007",
            Self::ProviderLost => "CND-KNOW-008",
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Document {
    pub source: [u8; 32],
    pub revision: [u8; 32],
    pub content: Vec<u8>,
    pub deleted: bool,
    pub access_granted: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RetrievalResult {
    pub source: [u8; 32],
    pub revision: [u8; 32],
    pub start: u16,
    pub end: u16,
    pub excerpt: Vec<u8>,
    pub score_milli: i16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RetrievalBounds {
    pub maximum_document_bytes: usize,
    pub maximum_query_bytes: usize,
    pub maximum_results: usize,
    pub maximum_context_bytes: usize,
    pub maximum_retained_bytes: usize,
    pub maximum_work: usize,
}

impl RetrievalBounds {
    pub const FIRST_PROOF: Self = Self {
        maximum_document_bytes: MAXIMUM_DOCUMENT_BYTES,
        maximum_query_bytes: MAXIMUM_QUERY_BYTES,
        maximum_results: 1,
        maximum_context_bytes: MAXIMUM_DOCUMENT_BYTES,
        maximum_retained_bytes: MAXIMUM_RETAINED_BYTES,
        maximum_work: MAXIMUM_WORK,
    };
}

pub fn retrieve(
    document: &Document,
    query: &[u8],
    index_identity: [u8; 32],
    embedding_identity: [u8; 32],
    embedding_dimensions: u16,
    coverage_complete: bool,
    bounds: RetrievalBounds,
) -> Result<RetrievalResult, KnowledgeReason> {
    if document.content.len() > bounds.maximum_document_bytes {
        return Err(KnowledgeReason::DocumentOverflow);
    }
    if query.len() > bounds.maximum_query_bytes {
        return Err(KnowledgeReason::QueryOverflow);
    }
    if bounds.maximum_results == 0 || bounds.maximum_results > MAXIMUM_RESULTS {
        return Err(KnowledgeReason::ResultOverflow);
    }
    if document.content.len() + query.len() > bounds.maximum_retained_bytes {
        return Err(KnowledgeReason::RetainedOverflow);
    }
    if document.content.len().saturating_mul(query.len()) > bounds.maximum_work {
        return Err(KnowledgeReason::WorkOverflow);
    }
    if index_identity != INDEX_IDENTITY {
        return Err(KnowledgeReason::IndexMismatch);
    }
    if embedding_identity != EMBEDDING_IDENTITY {
        return Err(KnowledgeReason::EmbeddingMismatch);
    }
    if embedding_dimensions != 4 {
        return Err(KnowledgeReason::DimensionMismatch);
    }
    if !coverage_complete {
        return Err(KnowledgeReason::PartialIndex);
    }
    if document.deleted {
        return Err(KnowledgeReason::DeletedSource);
    }
    if !document.access_granted {
        return Err(KnowledgeReason::AccessDenied);
    }
    if query.is_empty() {
        return Err(KnowledgeReason::MissingSource);
    }
    let start = document
        .content
        .windows(query.len())
        .position(|window| window.eq_ignore_ascii_case(query))
        .ok_or(KnowledgeReason::MissingSource)?;
    let end = start
        .checked_add(query.len())
        .ok_or(KnowledgeReason::InvalidSpan)?;
    if end > document.content.len() || end > bounds.maximum_context_bytes {
        return Err(KnowledgeReason::ContextOverflow);
    }
    Ok(RetrievalResult {
        source: document.source,
        revision: document.revision,
        start: u16::try_from(start).map_err(|_| KnowledgeReason::InvalidSpan)?,
        end: u16::try_from(end).map_err(|_| KnowledgeReason::InvalidSpan)?,
        excerpt: document.content[start..end].to_vec(),
        score_milli: 1000,
    })
}

pub fn assemble_citation(
    result: &RetrievalResult,
    document: &Document,
) -> Result<Vec<u8>, KnowledgeReason> {
    if result.source != document.source {
        return Err(KnowledgeReason::MissingSource);
    }
    if result.revision != document.revision {
        return Err(KnowledgeReason::StaleRevision);
    }
    let start = usize::from(result.start);
    let end = usize::from(result.end);
    if start > end || document.content.get(start..end) != Some(result.excerpt.as_slice()) {
        return Err(KnowledgeReason::InvalidSpan);
    }
    Ok(encode_result(b"KCT0", result))
}

fn encode_document(document: &Document) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(70 + document.content.len());
    bytes.extend_from_slice(b"KDO0");
    bytes.extend_from_slice(&document.source);
    bytes.extend_from_slice(&document.revision);
    bytes.push(u8::from(document.deleted));
    bytes.push(u8::from(document.access_granted));
    bytes.extend_from_slice(&document.content);
    bytes
}

fn decode_document(bytes: &[u8]) -> Result<Document, KnowledgeReason> {
    if bytes.len() < 70 || bytes.get(..4) != Some(b"KDO0") {
        return Err(KnowledgeReason::WrongType);
    }
    let mut source = [0; 32];
    source.copy_from_slice(&bytes[4..36]);
    let mut revision = [0; 32];
    revision.copy_from_slice(&bytes[36..68]);
    Ok(Document {
        source,
        revision,
        deleted: bytes[68] != 0,
        access_granted: bytes[69] != 0,
        content: bytes[70..].to_vec(),
    })
}

fn encode_index(document: &Document) -> Vec<u8> {
    let mut bytes = Vec::from(&b"KIX0"[..]);
    bytes.extend_from_slice(&INDEX_IDENTITY);
    bytes.extend_from_slice(&EMBEDDING_IDENTITY);
    bytes.extend_from_slice(&4_u16.to_le_bytes());
    bytes.push(1);
    bytes.extend_from_slice(&encode_document(document));
    bytes
}

struct DecodedIndex {
    document: Document,
    identity: [u8; 32],
    embedding: [u8; 32],
    dimensions: u16,
    coverage_complete: bool,
}

fn decode_index(bytes: &[u8]) -> Result<DecodedIndex, KnowledgeReason> {
    if bytes.len() < 71 || bytes.get(..4) != Some(b"KIX0") {
        return Err(KnowledgeReason::WrongType);
    }
    let mut index = [0; 32];
    index.copy_from_slice(&bytes[4..36]);
    let mut embedding = [0; 32];
    embedding.copy_from_slice(&bytes[36..68]);
    let dimensions = u16::from_le_bytes([bytes[68], bytes[69]]);
    Ok(DecodedIndex {
        document: decode_document(&bytes[71..])?,
        identity: index,
        embedding,
        dimensions,
        coverage_complete: bytes[70] != 0,
    })
}

fn encode_result(prefix: &[u8; 4], result: &RetrievalResult) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(74 + result.excerpt.len());
    bytes.extend_from_slice(prefix);
    bytes.extend_from_slice(&result.source);
    bytes.extend_from_slice(&result.revision);
    bytes.extend_from_slice(&result.start.to_le_bytes());
    bytes.extend_from_slice(&result.end.to_le_bytes());
    bytes.extend_from_slice(&result.score_milli.to_le_bytes());
    bytes.extend_from_slice(&result.excerpt);
    bytes
}

fn decode_result(bytes: &[u8], prefix: &[u8; 4]) -> Result<RetrievalResult, KnowledgeReason> {
    if bytes.len() < 74 || bytes.get(..4) != Some(prefix) {
        return Err(KnowledgeReason::WrongType);
    }
    let mut source = [0; 32];
    source.copy_from_slice(&bytes[4..36]);
    let mut revision = [0; 32];
    revision.copy_from_slice(&bytes[36..68]);
    Ok(RetrievalResult {
        source,
        revision,
        start: u16::from_le_bytes([bytes[68], bytes[69]]),
        end: u16::from_le_bytes([bytes[70], bytes[71]]),
        score_milli: i16::from_le_bytes([bytes[72], bytes[73]]),
        excerpt: bytes[74..].to_vec(),
    })
}

fn exact_u64(node: &Node, key: &str) -> Option<u64> {
    match node.config_value(key) {
        Some(SourceValue::Integer(value)) => u64::try_from(*value).ok(),
        _ => None,
    }
}
fn no_config(node: &Node) -> Result<(), ResolutionError> {
    node.config
        .is_empty()
        .then_some(())
        .ok_or_else(|| ResolutionError::new("CND-KNOW-001", "node has no configuration"))
}
fn validate_document(node: &Node) -> Result<(), ResolutionError> {
    (node.config.len() == DOCUMENT_FIELDS.len()
        && node.config("fixture") == Some("conduit-origin")
        && node.config("source_identity") == Some(SOURCE_IDENTITY_TEXT)
        && node.config("revision_identity") == Some(REVISION_IDENTITY_TEXT)
        && exact_u64(node, "maximum_document_bytes") == Some(MAXIMUM_DOCUMENT_BYTES as u64)
        && node.config("access_scope") == Some("public"))
    .then_some(())
    .ok_or_else(|| {
        ResolutionError::new(
            "CND-KNOW-001",
            "document requires the exact bounded fixture",
        )
    })
}
fn validate_index(node: &Node) -> Result<(), ResolutionError> {
    (node.config.len() == INDEX_FIELDS.len()
        && node.config("index_identity") == Some(INDEX_IDENTITY_TEXT)
        && node.config("embedding_identity") == Some(EMBEDDING_IDENTITY_TEXT)
        && exact_u64(node, "embedding_dimensions") == Some(4)
        && node.config("coverage") == Some("complete")
        && exact_u64(node, "maximum_retained_bytes") == Some(MAXIMUM_RETAINED_BYTES as u64)
        && exact_u64(node, "maximum_work") == Some(MAXIMUM_WORK as u64))
    .then_some(())
    .ok_or_else(|| {
        ResolutionError::new(
            "CND-KNOW-003",
            "index requires exact identity, coverage, embedding, and bounds",
        )
    })
}
fn validate_query(node: &Node) -> Result<(), ResolutionError> {
    (node.config.len() == QUERY_FIELDS.len()
        && node.config("fixture") == Some(QUERY_TEXT)
        && exact_u64(node, "maximum_query_bytes") == Some(MAXIMUM_QUERY_BYTES as u64))
    .then_some(())
    .ok_or_else(|| {
        ResolutionError::new(
            "CND-KNOW-004",
            "query exceeds the exact first-proof profile",
        )
    })
}
fn validate_retrieve(node: &Node) -> Result<(), ResolutionError> {
    (node.config.len() == RETRIEVE_FIELDS.len()
        && node.config("index_identity") == Some(INDEX_IDENTITY_TEXT)
        && node.config("embedding_identity") == Some(EMBEDDING_IDENTITY_TEXT)
        && exact_u64(node, "embedding_dimensions") == Some(4)
        && exact_u64(node, "maximum_results") == Some(1)
        && exact_u64(node, "maximum_context_bytes") == Some(MAXIMUM_DOCUMENT_BYTES as u64)
        && exact_u64(node, "maximum_work") == Some(MAXIMUM_WORK as u64))
    .then_some(())
    .ok_or_else(|| {
        ResolutionError::new(
            "CND-KNOW-003",
            "retrieve requires exact snapshot, embedding, and bounds",
        )
    })
}
fn validate_rerank(node: &Node) -> Result<(), ResolutionError> {
    (node.config.len() == RERANK_FIELDS.len()
        && node.config("reranker_identity") == Some(RERANKER_IDENTITY_TEXT)
        && exact_u64(node, "maximum_results") == Some(1)
        && exact_u64(node, "maximum_work") == Some(MAXIMUM_WORK as u64))
    .then_some(())
    .ok_or_else(|| {
        ResolutionError::new(
            "CND-KNOW-004",
            "rerank requires the bounded deterministic profile",
        )
    })
}
fn runtime(reason: KnowledgeReason) -> RuntimeError {
    RuntimeError::new(
        reason.code(),
        format!("bounded knowledge operation failed: {reason:?}"),
    )
}

struct DocumentLiteral;
impl Handler for DocumentLiteral {
    fn run(
        &mut self,
        _: &Node,
        inputs: &[Value],
        _: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        if !inputs.is_empty() {
            return Err(runtime(KnowledgeReason::WrongType));
        }
        Ok(vec![Value {
            value_type: DOCUMENT_TYPE,
            bytes: encode_document(&Document {
                source: SOURCE_IDENTITY,
                revision: REVISION_IDENTITY,
                content: DOCUMENT_TEXT.as_bytes().to_vec(),
                deleted: false,
                access_granted: true,
            }),
        }])
    }
}
struct IndexFixture;
impl Handler for IndexFixture {
    fn run(
        &mut self,
        _: &Node,
        inputs: &[Value],
        _: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        let [document] = inputs else {
            return Err(runtime(KnowledgeReason::WrongType));
        };
        if document.value_type != DOCUMENT_TYPE {
            return Err(runtime(KnowledgeReason::WrongType));
        }
        Ok(vec![Value {
            value_type: INDEX_TYPE,
            bytes: encode_index(&decode_document(&document.bytes).map_err(runtime)?),
        }])
    }
}
struct QueryLiteral;
impl Handler for QueryLiteral {
    fn run(
        &mut self,
        _: &Node,
        inputs: &[Value],
        _: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        if !inputs.is_empty() {
            return Err(runtime(KnowledgeReason::WrongType));
        }
        Ok(vec![Value {
            value_type: QUERY_TYPE,
            bytes: QUERY_TEXT.as_bytes().to_vec(),
        }])
    }
}
struct Retrieve;
impl Handler for Retrieve {
    fn run(
        &mut self,
        _: &Node,
        inputs: &[Value],
        _: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        let [index, query] = inputs else {
            return Err(runtime(KnowledgeReason::WrongType));
        };
        if index.value_type != INDEX_TYPE || query.value_type != QUERY_TYPE {
            return Err(runtime(KnowledgeReason::WrongType));
        }
        let snapshot = decode_index(&index.bytes).map_err(runtime)?;
        let result = retrieve(
            &snapshot.document,
            &query.bytes,
            snapshot.identity,
            snapshot.embedding,
            snapshot.dimensions,
            snapshot.coverage_complete,
            RetrievalBounds::FIRST_PROOF,
        )
        .map_err(runtime)?;
        Ok(vec![Value {
            value_type: RESULTS_TYPE,
            bytes: encode_result(b"KRS0", &result),
        }])
    }
}
struct Rerank;
impl Handler for Rerank {
    fn run(
        &mut self,
        _: &Node,
        inputs: &[Value],
        _: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        let [results] = inputs else {
            return Err(runtime(KnowledgeReason::WrongType));
        };
        if results.value_type != RESULTS_TYPE {
            return Err(runtime(KnowledgeReason::WrongType));
        }
        let result = decode_result(&results.bytes, b"KRS0").map_err(runtime)?;
        Ok(vec![Value {
            value_type: RESULTS_TYPE,
            bytes: encode_result(b"KRS0", &result),
        }])
    }
}
struct Cite;
impl Handler for Cite {
    fn run(
        &mut self,
        _: &Node,
        inputs: &[Value],
        _: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        let [results] = inputs else {
            return Err(runtime(KnowledgeReason::WrongType));
        };
        if results.value_type != RESULTS_TYPE {
            return Err(runtime(KnowledgeReason::WrongType));
        }
        let result = decode_result(&results.bytes, b"KRS0").map_err(runtime)?;
        let document = Document {
            source: SOURCE_IDENTITY,
            revision: REVISION_IDENTITY,
            content: DOCUMENT_TEXT.as_bytes().to_vec(),
            deleted: false,
            access_granted: true,
        };
        Ok(vec![Value {
            value_type: CITATION_TYPE,
            bytes: assemble_citation(&result, &document).map_err(runtime)?,
        }])
    }
}
struct CitationInspect;
impl Handler for CitationInspect {
    fn run(
        &mut self,
        _: &Node,
        inputs: &[Value],
        _: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        let [citation] = inputs else {
            return Err(runtime(KnowledgeReason::WrongType));
        };
        if citation.value_type != CITATION_TYPE {
            return Err(runtime(KnowledgeReason::WrongType));
        }
        let result = decode_result(&citation.bytes, b"KCT0").map_err(runtime)?;
        Ok(vec![Value {
            value_type: TEXT_TYPE,
            bytes: format!(
                "knowledge:citation:{}..{}:{}",
                result.start,
                result.end,
                String::from_utf8_lossy(&result.excerpt)
            )
            .into_bytes(),
        }])
    }
}

pub fn register_knowledge_contracts(registry: &mut Registry) {
    for contract in KNOWLEDGE_CONTRACTS {
        registry.register_contract_only(contract);
    }
}

pub fn register_deterministic_knowledge_provider(
    registry: &mut Registry,
) -> Result<(), RegistryError> {
    register_knowledge_contracts(registry);
    static NO_AUTHORITIES: [SemanticHash; 0] = [];
    for (contract, implementation_id, artifact_id, entrypoint, factory, validator) in [
        (
            &DOCUMENT_LITERAL_CONTRACT,
            "conduit.knowledge/document-literal",
            "conduit.knowledge/document-literal-artifact",
            "knowledge-document-literal",
            (|| Box::new(DocumentLiteral) as Box<dyn Handler>) as conduit_runtime::HandlerFactory,
            validate_document as conduit_runtime::ConfigValidator,
        ),
        (
            &INDEX_FIXTURE_CONTRACT,
            "conduit.knowledge/index-fixture",
            "conduit.knowledge/index-fixture-artifact",
            "knowledge-index-fixture",
            (|| Box::new(IndexFixture) as Box<dyn Handler>) as conduit_runtime::HandlerFactory,
            validate_index as conduit_runtime::ConfigValidator,
        ),
        (
            &QUERY_LITERAL_CONTRACT,
            "conduit.knowledge/query-literal",
            "conduit.knowledge/query-literal-artifact",
            "knowledge-query-literal",
            (|| Box::new(QueryLiteral) as Box<dyn Handler>) as conduit_runtime::HandlerFactory,
            validate_query as conduit_runtime::ConfigValidator,
        ),
        (
            &RETRIEVE_CONTRACT,
            "conduit.knowledge/retrieve-reference",
            "conduit.knowledge/retrieve-reference-artifact",
            "knowledge-retrieve",
            (|| Box::new(Retrieve) as Box<dyn Handler>) as conduit_runtime::HandlerFactory,
            validate_retrieve as conduit_runtime::ConfigValidator,
        ),
        (
            &RERANK_CONTRACT,
            "conduit.knowledge/rerank-reference",
            "conduit.knowledge/rerank-reference-artifact",
            "knowledge-rerank",
            (|| Box::new(Rerank) as Box<dyn Handler>) as conduit_runtime::HandlerFactory,
            validate_rerank as conduit_runtime::ConfigValidator,
        ),
        (
            &CITE_CONTRACT,
            "conduit.knowledge/cite-reference",
            "conduit.knowledge/cite-reference-artifact",
            "knowledge-cite",
            (|| Box::new(Cite) as Box<dyn Handler>) as conduit_runtime::HandlerFactory,
            no_config as conduit_runtime::ConfigValidator,
        ),
        (
            &CITATION_INSPECT_CONTRACT,
            "conduit.knowledge/citation-inspect",
            "conduit.knowledge/citation-inspect-artifact",
            "knowledge-citation-inspect",
            (|| Box::new(CitationInspect) as Box<dyn Handler>) as conduit_runtime::HandlerFactory,
            no_config as conduit_runtime::ConfigValidator,
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

    fn document() -> Document {
        Document {
            source: SOURCE_IDENTITY,
            revision: REVISION_IDENTITY,
            content: DOCUMENT_TEXT.as_bytes().to_vec(),
            deleted: false,
            access_granted: true,
        }
    }

    #[test]
    fn exact_revision_and_span_survive_retrieve_rerank_and_citation() {
        let document = document();
        let result = retrieve(
            &document,
            QUERY_TEXT.as_bytes(),
            INDEX_IDENTITY,
            EMBEDDING_IDENTITY,
            4,
            true,
            RetrievalBounds::FIRST_PROOF,
        )
        .unwrap();
        assert_eq!(result.source, SOURCE_IDENTITY);
        assert_eq!(result.revision, REVISION_IDENTITY);
        assert_eq!(
            &document.content[usize::from(result.start)..usize::from(result.end)],
            result.excerpt
        );
        let citation = assemble_citation(&result, &document).unwrap();
        assert_eq!(decode_result(&citation, b"KCT0").unwrap(), result);
    }

    #[test]
    fn source_index_embedding_access_and_bounds_fail_closed() {
        let base = document();
        let mut deleted = base.clone();
        deleted.deleted = true;
        let mut denied = base.clone();
        denied.access_granted = false;
        for (document, index, embedding, dimensions, coverage, bounds, expected) in [
            (
                deleted,
                INDEX_IDENTITY,
                EMBEDDING_IDENTITY,
                4,
                true,
                RetrievalBounds::FIRST_PROOF,
                KnowledgeReason::DeletedSource,
            ),
            (
                denied,
                INDEX_IDENTITY,
                EMBEDDING_IDENTITY,
                4,
                true,
                RetrievalBounds::FIRST_PROOF,
                KnowledgeReason::AccessDenied,
            ),
            (
                base.clone(),
                [0; 32],
                EMBEDDING_IDENTITY,
                4,
                true,
                RetrievalBounds::FIRST_PROOF,
                KnowledgeReason::IndexMismatch,
            ),
            (
                base.clone(),
                INDEX_IDENTITY,
                [0; 32],
                4,
                true,
                RetrievalBounds::FIRST_PROOF,
                KnowledgeReason::EmbeddingMismatch,
            ),
            (
                base.clone(),
                INDEX_IDENTITY,
                EMBEDDING_IDENTITY,
                3,
                true,
                RetrievalBounds::FIRST_PROOF,
                KnowledgeReason::DimensionMismatch,
            ),
            (
                base.clone(),
                INDEX_IDENTITY,
                EMBEDDING_IDENTITY,
                4,
                false,
                RetrievalBounds::FIRST_PROOF,
                KnowledgeReason::PartialIndex,
            ),
            (
                base.clone(),
                INDEX_IDENTITY,
                EMBEDDING_IDENTITY,
                4,
                true,
                RetrievalBounds {
                    maximum_results: 0,
                    ..RetrievalBounds::FIRST_PROOF
                },
                KnowledgeReason::ResultOverflow,
            ),
            (
                base,
                INDEX_IDENTITY,
                EMBEDDING_IDENTITY,
                4,
                true,
                RetrievalBounds {
                    maximum_work: 1,
                    ..RetrievalBounds::FIRST_PROOF
                },
                KnowledgeReason::WorkOverflow,
            ),
        ] {
            assert_eq!(
                retrieve(
                    &document,
                    QUERY_TEXT.as_bytes(),
                    index,
                    embedding,
                    dimensions,
                    coverage,
                    bounds
                ),
                Err(expected)
            );
        }
    }

    #[test]
    fn stale_or_mutated_citation_never_becomes_grounded() {
        let document = document();
        let mut result = retrieve(
            &document,
            QUERY_TEXT.as_bytes(),
            INDEX_IDENTITY,
            EMBEDDING_IDENTITY,
            4,
            true,
            RetrievalBounds::FIRST_PROOF,
        )
        .unwrap();
        result.revision = [0; 32];
        assert_eq!(
            assemble_citation(&result, &document),
            Err(KnowledgeReason::StaleRevision)
        );
        result.revision = REVISION_IDENTITY;
        result.excerpt[0] ^= 1;
        assert_eq!(
            assemble_citation(&result, &document),
            Err(KnowledgeReason::InvalidSpan)
        );
    }

    #[test]
    fn contracts_do_not_install_a_provider() {
        let mut registry = Registry::default();
        register_knowledge_contracts(&mut registry);
        assert!(
            registry
                .installed_providers()
                .iter()
                .all(|provider| !KNOWLEDGE_CONTRACTS.contains(&provider.contract))
        );
    }

    #[test]
    fn conformance_fixture_names_the_complete_first_matrix() {
        let fixture: serde_json::Value = serde_json::from_str(include_str!(
            "../../../conformance/c4/knowledge-retrieval.json"
        ))
        .unwrap();
        assert_eq!(fixture["schema"], "conduit.knowledge-retrieval-conformance");
        assert_eq!(fixture["positive"].as_array().unwrap().len(), 6);
        assert_eq!(fixture["negative"].as_array().unwrap().len(), 15);
        assert_ne!(
            KnowledgeReason::Cancellation.code(),
            KnowledgeReason::ProviderLost.code()
        );
        for case in fixture["negative"].as_array().unwrap() {
            let reason = match case.as_str().unwrap() {
                "empty-corpus" => KnowledgeReason::MissingSource,
                "duplicate-or-revised-source" | "citation-to-wrong-revision" => {
                    KnowledgeReason::StaleRevision
                }
                "deleted-source" => KnowledgeReason::DeletedSource,
                "invalid-span" => KnowledgeReason::InvalidSpan,
                "document-or-query-overflow" => KnowledgeReason::DocumentOverflow,
                "top-k-overflow" => KnowledgeReason::ResultOverflow,
                "embedding-dimension-mismatch" => KnowledgeReason::DimensionMismatch,
                "embedding-model-mismatch" => KnowledgeReason::EmbeddingMismatch,
                "stale-or-partial-index" => KnowledgeReason::PartialIndex,
                "access-denial" => KnowledgeReason::AccessDenied,
                "score-tie" | "reranker-reorder" => KnowledgeReason::ScoreTie,
                "cancellation" => KnowledgeReason::Cancellation,
                "provider-loss" => KnowledgeReason::ProviderLost,
                unknown => panic!("unowned knowledge conformance case {unknown}"),
            };
            assert!(reason.code().starts_with("CND-KNOW-"));
        }
    }
}
