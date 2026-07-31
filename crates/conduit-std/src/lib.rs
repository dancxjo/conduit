#![no_std]

//! Concrete, allocator-free standard-node catalog.
//!
//! Entries are ordinary [`conduit_core::NodeContract`] values plus exact
//! plan-visible behavior and resource facts. This crate contains no executor,
//! registry, host framework, ambient authority, or domain profile.

#[cfg(test)]
extern crate std;

use conduit_core::{
    ConfigContract, ConfigFieldContract, ConfigIdentity, ConfigMutability, ConfigRequirement,
    ConnectionCardinality, Delivery, Direction, Id, LossAcceptance, NodeContract, PortContract,
    PortFlowConstraints, Presence, SemanticHash, Sensitivity, TemporalContract, TerminalContract,
    TypeContractRef, ValueCardinality,
};

mod conformance;
mod data_boundaries;
mod filesystem;
mod process_exec;
mod socket;
mod state;
mod storage_cache;
mod supervision;
mod text_format;
mod text_lines_join;
mod time;
mod types;

pub use conformance::{
    ConformanceError, DeterministicProvider, FixtureClass, FixtureOutcome, HostedProvider,
    NormalizedEvidence, ProviderProfile, ReferenceProvider, run_catalog_fixture,
};
pub use data_boundaries::{
    DATA_MAX_FIELD_NAME_BYTES, DATA_MAX_FIELD_VALUE_BYTES, DATA_MAX_FRAME_BYTES,
    DATA_MAX_RECORD_BYTES, DATA_MAX_RECORD_FIELDS, DataBoundaryError, LENGTH_U32BE_PREFIX_BYTES,
    LengthU32BeDecoder, RequiredField, StructuralDecision, StructuralField, StructuralRejection,
    decode_utf8, encode_closed_record, encode_length_u32be, encode_utf8, validate_closed_record,
    validate_closed_record_bytes,
};
pub use filesystem::{
    FILESYSTEM_MAX_FILE_BYTES, FILESYSTEM_MAX_FILES, FILESYSTEM_MAX_WATCH_EVENTS, FileHandle,
    FileSlot, FilesystemError, FlushClaim, MemoryFilesystem, PartialWritePolicy, ReadConsistency,
    ReadRequest, ReadResult, WatchCoalescing, WatchEvent, WatchEventKind, WatchOverflow,
    WatchRequest, WriteMode, WriteRequest, WriteResult,
};
pub use process_exec::{
    EnvironmentAddition, ExecCommand, ExecError, ExecEvent, ExecEventKind, ExecLimits, ExecRequest,
    ExecResult, ExecTerminal, FakeExecBuffers, FakeExecControl, FakeProgram, OutputStream,
    PROCESS_MAX_ARGUMENT_BYTES, PROCESS_MAX_ARGUMENTS, PROCESS_MAX_CHUNK_BYTES,
    PROCESS_MAX_ENVIRONMENT, PROCESS_MAX_ENVIRONMENT_NAME_BYTES,
    PROCESS_MAX_ENVIRONMENT_VALUE_BYTES, PROCESS_MAX_EVIDENCE_EVENTS, PROCESS_MAX_STREAM_BYTES,
    StdinClosePolicy, TerminationPolicy, run_fake_exec, validate_exec_request,
};
pub use socket::{
    SOCKET_MAX_DATAGRAMS, SOCKET_MAX_EVIDENCE_EVENTS, SOCKET_MAX_MESSAGE_BYTES,
    SOCKET_MAX_SESSIONS, SOCKET_MAX_STREAM_BYTES, SocketAddress, SocketBuffers, SocketControl,
    SocketError, SocketEvent, SocketEventKind, SocketLimits, SocketResult, SocketSession,
    SocketTerminal, TcpOperation, TcpRequest, UdpOperation, UdpRequest, run_tcp_fixture,
    run_udp_fixture, validate_tcp_request, validate_udp_request,
};
pub use state::{
    CacheEntry, CacheInsert, CacheState, CellState, DeduplicateDecision, DeduplicateState,
    STATE_MAX_ENTRIES, STATE_MAX_VALUE_BYTES, StateError, StateIdentity,
};
pub use storage_cache::{
    BlobIdentity, CACHE_MAX_BLOB_BYTES, CACHE_MAX_ENTRIES, CacheError, CacheHandle,
    CachePersistence, CacheRequirement, CacheSensitivity, CacheStore, GetOutcome, GetRequest,
    GetResult, PutRequest, PutResult, RemoveOutcome,
};
pub use supervision::{
    AttemptOutcome, BackoffMode, BackoffPolicy, BreakerAdmission, BreakerOutcome, BreakerState,
    CircuitBreakerState, RetryDecision, RetryPermission, RetryState, SUPERVISION_MAX_ATTEMPTS,
    SUPERVISION_MAX_DURATION_TICKS, SUPERVISION_MAX_OBSERVATIONS, SupervisionError,
};
pub use text_format::{
    FORMAT_MAX_NAME_BYTES, FORMAT_MAX_OUTPUT_BYTES, FORMAT_MAX_RETAINED_BYTES,
    FORMAT_MAX_SCALAR_BYTES, FORMAT_MAX_TEMPLATE_BYTES, FORMAT_MAX_VALUES, FORMAT_MAX_WORK,
    FORMAT_VALUES_MAX_ENCODED_BYTES, FormatError, FormatScalarRef, FormatValueRef,
    format_text_into, validate_format_values,
};
pub use text_lines_join::{
    JOIN_MAX_ITEM_BYTES, JOIN_MAX_ITEMS, JOIN_MAX_OUTPUT_BYTES, JOIN_MAX_SEPARATOR_BYTES,
    LINES_MAX_LINE_BYTES, LINES_MAX_RETAINED_PREFIX_BYTES, LineError, LinesState, Utf8State,
    join_text_into,
};
pub use time::{
    Admission, DebounceMode, DebounceState, OneShotTimer, TIME_MAX_DURATION_TICKS,
    TIME_MAX_RETAINED_VALUES, TerminalPendingPolicy, ThrottleMode, ThrottleState, TimeError,
    exact_deadline,
};
pub use types::{
    STANDARD_TYPE_CATALOG, StandardRepresentation, StandardTypeDefinition, StandardTypeFamily,
    TypeRepresentationSupport, standard_type, standard_type_descriptor, standard_type_reference,
};

/// Catalog schema consumed by manifests and conformance tooling.
pub const STANDARD_CATALOG_SCHEMA_VERSION: u32 = 0;

/// Broad ownership family; this is catalog metadata, not a runtime node kind.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StandardFamily {
    Source,
    Sink,
    Structural,
    Transform,
    Time,
    State,
    Supervision,
    Testing,
    Boundary,
    Network,
}

/// Whether execution changes value type.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TypeBehavior {
    Preserving,
    ExplicitAdapter,
    ProducesDeclaredType,
}

/// A value type in a generic standard-node definition.
///
/// Generic expressions describe stable relationships between ports. They are
/// not concrete [`TypeContractRef`] values and must be specialized before an
/// exact plan is emitted.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CatalogTypeExpression {
    /// One parameter declared by the enclosing signature.
    Parameter(Id<'static>),
    /// One concrete standard type.
    Named(Id<'static>),
    /// An application of a standard generic type constructor.
    Apply {
        constructor: Id<'static>,
        arguments: &'static [CatalogTypeExpression],
    },
}

/// The generic value type assigned to one node port.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GenericPortType {
    pub direction: Direction,
    pub port_index: u16,
    pub value_type: CatalogTypeExpression,
}

/// Type parameters and port relationships for a polymorphic node definition.
///
/// Ports omitted from `ports` retain the concrete type in [`NodeContract`].
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GenericNodeSignature {
    pub parameters: &'static [Id<'static>],
    pub ports: &'static [GenericPortType],
}

/// Time basis required by the semantic contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TimeBasis {
    None,
    Monotonic,
    Wall,
    DomainEvent,
}

/// Provider profiles for which conformance evidence is required.
///
/// This is a requirement, not a claim that an implementation is installed.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProviderRequirement {
    pub deterministic: bool,
    pub hosted: bool,
    pub constrained: bool,
}

/// Finite resources retained in the exact plan.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CatalogLimits {
    pub retained_values: u32,
    pub retained_bytes: u64,
    pub pending_operations: u16,
    pub timers: u16,
    pub retries: u16,
    pub work_per_step: u32,
    pub evidence_events: u32,
}

/// One concrete published standard contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CatalogEntry {
    pub contract: NodeContract<'static>,
    pub family: StandardFamily,
    pub type_behavior: TypeBehavior,
    /// Authoritative polymorphic relationships, when this is a generic node.
    ///
    /// The ordinary `NodeContract` remains the concrete reference-provider
    /// specialization used by allocator-free conformance fixtures.
    pub generic_signature: Option<&'static GenericNodeSignature>,
    pub time_basis: TimeBasis,
    pub ordering_policy: Id<'static>,
    pub terminal_policy: Id<'static>,
    pub cancellation_policy: Id<'static>,
    pub pressure_policy: Id<'static>,
    pub provider: Id<'static>,
    pub host_service: Option<Id<'static>>,
    pub limits: CatalogLimits,
    pub required_support: ProviderRequirement,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CatalogError {
    InvalidIdentifier,
    DuplicateContract,
    DuplicatePort,
    InvalidPort,
    UnboundedWork,
    MissingStateBound,
    MissingTimer,
    MissingPendingBound,
    MissingHostService,
    ImplicitTypeChange,
    MissingReferenceProvider,
    InvalidGenericSignature,
}

const BYTES: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("std/bytes"),
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([
        0xf8, 0x55, 0x1a, 0x62, 0x9e, 0x94, 0xf0, 0xd3, 0x66, 0x2f, 0x02, 0x78, 0x1d, 0x17, 0x63,
        0xdb, 0x29, 0xdf, 0x21, 0xce, 0x97, 0x7a, 0x90, 0xf5, 0xc7, 0x43, 0x76, 0x59, 0x9b, 0x21,
        0x90, 0x74,
    ]),
};
const BOOL: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("std/bool"),
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([
        0x5c, 0xae, 0xd2, 0xca, 0xbd, 0x6d, 0x64, 0xf0, 0xba, 0x75, 0xa2, 0x43, 0xc6, 0x25, 0xb0,
        0x4a, 0xf0, 0xa9, 0x67, 0xbb, 0x92, 0x39, 0xca, 0x10, 0x2e, 0x51, 0x4b, 0xbb, 0xb7, 0x27,
        0x3e, 0x92,
    ]),
};
const TEXT: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("std/text"),
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([
        0x94, 0xdf, 0xe2, 0x55, 0x09, 0xfe, 0x62, 0x4d, 0x89, 0x74, 0xb1, 0xdd, 0x44, 0x2e, 0xb7,
        0xf9, 0x6f, 0x7e, 0x62, 0x1e, 0x6e, 0x71, 0xf0, 0x35, 0xac, 0x6f, 0x08, 0x04, 0x63, 0x61,
        0x80, 0x72,
    ]),
};
const FORMAT_VALUES: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("std/format-values"),
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([
        0xb6, 0x77, 0x82, 0xbd, 0x64, 0xf1, 0x19, 0x95, 0x15, 0xf7, 0x93, 0x1f, 0xd3, 0x9d, 0x9b,
        0xea, 0xca, 0xda, 0xb9, 0x1c, 0x78, 0xfe, 0x66, 0x75, 0x27, 0x12, 0x02, 0x4b, 0xa1, 0x5b,
        0xeb, 0x2e,
    ]),
};
const U64: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("std/u64"),
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([
        0xf9, 0xba, 0xd3, 0xea, 0x53, 0xd3, 0xca, 0x01, 0xa0, 0xa4, 0xd6, 0x9f, 0x86, 0xc8, 0x25,
        0x65, 0x17, 0x07, 0x16, 0x45, 0xea, 0x7d, 0x68, 0xef, 0x63, 0x6b, 0x6d, 0x94, 0x87, 0x70,
        0xf0, 0xec,
    ]),
};
const RECORD: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("std/record"),
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([
        0xde, 0x0b, 0x25, 0xed, 0xf4, 0x15, 0xc7, 0x2c, 0x7d, 0xbb, 0xe1, 0x1d, 0xc7, 0x78, 0xbd,
        0x12, 0xe6, 0x8e, 0x5f, 0xc7, 0x3a, 0xb2, 0xe3, 0x8f, 0x61, 0x07, 0x2e, 0x1d, 0x29, 0x5f,
        0x22, 0xfa,
    ]),
};
const VALIDATION_DECISION: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("std/validation-decision"),
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([
        0xf9, 0x59, 0x03, 0x96, 0xa8, 0x2c, 0x69, 0xd3, 0x7e, 0xd0, 0xf2, 0x57, 0x46, 0x0e, 0x3c,
        0xee, 0x36, 0x5e, 0x79, 0x84, 0x6e, 0x1d, 0xd2, 0xa0, 0x0c, 0x83, 0xee, 0x8e, 0x86, 0x67,
        0x2a, 0xff,
    ]),
};
const REFERENCE: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("std/reference/any"),
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([
        0x73, 0x02, 0x02, 0xbc, 0x0e, 0x9f, 0x52, 0x0c, 0x30, 0x74, 0x26, 0x51, 0x50, 0x3d, 0x16,
        0x68, 0x72, 0xbf, 0x79, 0xdf, 0x7d, 0xd5, 0x25, 0x22, 0x0f, 0xa8, 0xc8, 0x31, 0x76, 0xfc,
        0x7b, 0xfb,
    ]),
};
const SOCKET_ADDRESS: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("net/socket/address"),
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([
        0x07, 0x3d, 0xb6, 0x72, 0x6d, 0x1c, 0x2a, 0xa9, 0xdf, 0xf7, 0x70, 0xe9, 0x96, 0xe1, 0x6c,
        0x7c, 0x44, 0x9a, 0x8b, 0x5a, 0x39, 0xb9, 0x27, 0xc9, 0x37, 0xe0, 0x94, 0x60, 0xb5, 0x3c,
        0xe4, 0xd6,
    ]),
};
const SOCKET_SESSION: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("net/socket/session"),
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([
        178, 60, 218, 192, 43, 170, 112, 86, 148, 49, 73, 97, 141, 183, 118, 14, 133, 185, 11, 93,
        13, 102, 175, 97, 105, 158, 141, 106, 66, 100, 232, 4,
    ]),
};
const UDP_DATAGRAM: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("net/udp/datagram"),
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([
        213, 245, 189, 184, 1, 110, 75, 40, 182, 135, 129, 111, 68, 187, 205, 170, 78, 50, 103,
        188, 128, 69, 175, 34, 31, 179, 15, 3, 238, 55, 239, 225,
    ]),
};
const SOCKET_RESULT: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("net/socket/result"),
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([
        102, 244, 215, 109, 149, 65, 19, 239, 21, 155, 174, 56, 57, 204, 103, 12, 99, 73, 43, 32,
        206, 227, 137, 90, 206, 191, 170, 102, 163, 123, 111, 42,
    ]),
};
const HTTP_REQUEST: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("net/http/request"),
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([
        0x3f, 0xd5, 0x41, 0xf4, 0x0b, 0xab, 0x96, 0x20, 0x4f, 0xfb, 0x99, 0x3e, 0x4c, 0x76, 0xa4,
        0xa9, 0x54, 0x0d, 0x0e, 0xab, 0x57, 0xd1, 0xea, 0x71, 0x72, 0x37, 0xad, 0xf2, 0x4f, 0xbe,
        0x47, 0x0c,
    ]),
};
const HTTP_RESPONSE: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("net/http/response"),
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([
        0x60, 0x61, 0xa8, 0x2b, 0x39, 0xac, 0x26, 0xc9, 0xe7, 0x25, 0xc0, 0x11, 0xeb, 0x8b, 0x79,
        0x2d, 0xff, 0x12, 0xdd, 0xde, 0xed, 0xf9, 0x9f, 0x10, 0xa2, 0x78, 0xa9, 0xf3, 0xb5, 0x0e,
        0x86, 0xa1,
    ]),
};
const FS_RESOURCE: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("fs/resource"),
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([
        0x29, 0x1a, 0x75, 0xbb, 0x09, 0x2b, 0x1e, 0x8e, 0xa4, 0x8f, 0xd4, 0xc0, 0x69, 0xf7, 0x5c,
        0x18, 0xc3, 0xaf, 0xe4, 0x3e, 0xe9, 0x2d, 0x9c, 0xcf, 0xdc, 0x33, 0x04, 0x48, 0x61, 0x7e,
        0x1c, 0xfc,
    ]),
};
const FS_CHUNK: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("fs/chunk"),
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([
        0x32, 0xa7, 0x02, 0xbb, 0x48, 0x7b, 0xc1, 0x20, 0x5d, 0xa6, 0x91, 0x4d, 0x5b, 0xb7, 0x9a,
        0x17, 0x65, 0xe3, 0x89, 0xd8, 0xa7, 0xaf, 0x19, 0x4b, 0x7d, 0xbc, 0x67, 0x4c, 0xf6, 0x4f,
        0x5f, 0x21,
    ]),
};
const FS_READ_RESULT: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("fs/read-result"),
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([
        0x20, 0x29, 0x67, 0xff, 0x2f, 0x91, 0x38, 0xb7, 0xde, 0xf7, 0xb1, 0xf1, 0x85, 0x5c, 0x39,
        0xd7, 0xde, 0x49, 0xfa, 0x10, 0xae, 0xa0, 0x08, 0x45, 0xc8, 0x8a, 0x85, 0x52, 0xe7, 0xd8,
        0x76, 0xb9,
    ]),
};
const FS_WRITE_RESULT: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("fs/write-result"),
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([
        0x7c, 0x68, 0xdf, 0x17, 0x52, 0x52, 0x84, 0xfa, 0xf7, 0x52, 0xd4, 0xda, 0x14, 0x56, 0x94,
        0xe1, 0x1c, 0x5e, 0x3b, 0xc8, 0xf9, 0x83, 0xc2, 0xa9, 0x6d, 0x7f, 0x3b, 0x65, 0x2c, 0x09,
        0x04, 0x9c,
    ]),
};
const FS_EVENT: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("fs/event"),
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([
        0x28, 0x99, 0x14, 0x95, 0x74, 0x29, 0x64, 0x9c, 0x34, 0xec, 0xcc, 0x42, 0x73, 0x72, 0xb6,
        0xc4, 0x9d, 0x87, 0x99, 0xc6, 0x98, 0xe6, 0x8d, 0xc9, 0x38, 0xf3, 0x2a, 0x92, 0xf6, 0x87,
        0x19, 0xa6,
    ]),
};
const STORAGE_BLOB: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("storage/blob"),
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([
        96, 173, 77, 1, 167, 191, 66, 199, 206, 22, 192, 212, 34, 118, 151, 25, 71, 127, 32, 35,
        71, 104, 235, 72, 250, 35, 84, 123, 8, 94, 49, 63,
    ]),
};
const STORAGE_CACHE_HANDLE: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("storage/cache-handle"),
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([
        220, 59, 69, 20, 176, 217, 205, 14, 252, 2, 189, 253, 165, 49, 57, 11, 236, 223, 255, 94,
        9, 220, 89, 182, 35, 116, 57, 96, 255, 87, 65, 75,
    ]),
};
const STORAGE_CACHE_PUT_RESULT: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("storage/cache-put-result"),
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([
        25, 173, 190, 203, 158, 243, 60, 161, 250, 213, 193, 252, 29, 197, 194, 38, 95, 2, 12, 115,
        163, 200, 185, 175, 122, 31, 106, 124, 183, 206, 121, 0,
    ]),
};
const STORAGE_CACHE_GET_RESULT: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("storage/cache-get-result"),
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([
        169, 18, 33, 166, 225, 52, 239, 240, 167, 94, 129, 196, 253, 213, 8, 228, 69, 55, 246, 200,
        49, 169, 223, 176, 252, 168, 53, 121, 230, 22, 203, 190,
    ]),
};
const STORAGE_CACHE_REMOVE_RESULT: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("storage/cache-remove-result"),
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([
        228, 128, 85, 30, 56, 195, 0, 24, 27, 231, 165, 14, 158, 38, 223, 218, 248, 132, 43, 113,
        158, 140, 249, 199, 222, 73, 51, 234, 197, 212, 113, 154,
    ]),
};
const PROCESS_STDIN: PortContract<'static> = port(
    "stdin",
    Direction::Input,
    BYTES,
    ValueCardinality::ZeroOrMore,
    TerminalContract::Finite,
);
const PROCESS_STDOUT: PortContract<'static> = port(
    "stdout",
    Direction::Output,
    BYTES,
    ValueCardinality::ZeroOrMore,
    TerminalContract::Finite,
);
const PROCESS_STDERR: PortContract<'static> = port(
    "stderr",
    Direction::Output,
    BYTES,
    ValueCardinality::ZeroOrMore,
    TerminalContract::Finite,
);
const TCP_TRANSMIT: PortContract<'static> = port(
    "transmit",
    Direction::Input,
    BYTES,
    ValueCardinality::ZeroOrMore,
    TerminalContract::Finite,
);
const TCP_RECEIVED: PortContract<'static> = port(
    "received",
    Direction::Output,
    BYTES,
    ValueCardinality::ZeroOrMore,
    TerminalContract::Finite,
);
const SOCKET_SESSION_OUTPUT: PortContract<'static> = optional_output(port(
    "session",
    Direction::Output,
    SOCKET_SESSION,
    ValueCardinality::ZeroOrMore,
    TerminalContract::Finite,
));
const SOCKET_RESULT_OUTPUT: PortContract<'static> = optional_output(port(
    "result",
    Direction::Output,
    SOCKET_RESULT,
    ValueCardinality::ZeroOrMore,
    TerminalContract::Finite,
));
const UDP_DATAGRAM_INPUT: PortContract<'static> = optional_input(port(
    "datagram",
    Direction::Input,
    UDP_DATAGRAM,
    ValueCardinality::ZeroOrMore,
    TerminalContract::Finite,
));
const UDP_DATAGRAM_OUTPUT: PortContract<'static> = optional_output(port(
    "datagram",
    Direction::Output,
    UDP_DATAGRAM,
    ValueCardinality::ZeroOrMore,
    TerminalContract::Finite,
));

const VALUE_PARAMETER: CatalogTypeExpression = CatalogTypeExpression::Parameter(Id("value"));
const NATURAL_EXPRESSION: CatalogTypeExpression = CatalogTypeExpression::Named(Id("std/natural"));
static VALUE_PARAMETERS: &[Id<'static>] = &[Id("value")];
static OPTION_VALUE_ARGUMENTS: &[CatalogTypeExpression] = &[VALUE_PARAMETER];
const OPTION_VALUE: CatalogTypeExpression = CatalogTypeExpression::Apply {
    constructor: Id("std/option"),
    arguments: OPTION_VALUE_ARGUMENTS,
};

static IDENTITY_GENERIC_PORTS: &[GenericPortType] = &[
    GenericPortType {
        direction: Direction::Input,
        port_index: 0,
        value_type: VALUE_PARAMETER,
    },
    GenericPortType {
        direction: Direction::Output,
        port_index: 0,
        value_type: VALUE_PARAMETER,
    },
];
static IDENTITY_GENERIC: GenericNodeSignature = GenericNodeSignature {
    parameters: VALUE_PARAMETERS,
    ports: IDENTITY_GENERIC_PORTS,
};

static TEE_GENERIC_PORTS: &[GenericPortType] = &[
    GenericPortType {
        direction: Direction::Input,
        port_index: 0,
        value_type: VALUE_PARAMETER,
    },
    GenericPortType {
        direction: Direction::Output,
        port_index: 0,
        value_type: VALUE_PARAMETER,
    },
    GenericPortType {
        direction: Direction::Output,
        port_index: 1,
        value_type: VALUE_PARAMETER,
    },
];
static TEE_GENERIC: GenericNodeSignature = GenericNodeSignature {
    parameters: VALUE_PARAMETERS,
    ports: TEE_GENERIC_PORTS,
};

static MERGE_GENERIC_PORTS: &[GenericPortType] = &[
    GenericPortType {
        direction: Direction::Input,
        port_index: 0,
        value_type: VALUE_PARAMETER,
    },
    GenericPortType {
        direction: Direction::Input,
        port_index: 1,
        value_type: VALUE_PARAMETER,
    },
    GenericPortType {
        direction: Direction::Output,
        port_index: 0,
        value_type: VALUE_PARAMETER,
    },
];
static MERGE_GENERIC: GenericNodeSignature = GenericNodeSignature {
    parameters: VALUE_PARAMETERS,
    ports: MERGE_GENERIC_PORTS,
};

static FIRST_GENERIC_PORTS: &[GenericPortType] = &[
    GenericPortType {
        direction: Direction::Input,
        port_index: 0,
        value_type: VALUE_PARAMETER,
    },
    GenericPortType {
        direction: Direction::Output,
        port_index: 0,
        value_type: OPTION_VALUE,
    },
];
static FIRST_GENERIC: GenericNodeSignature = GenericNodeSignature {
    parameters: VALUE_PARAMETERS,
    ports: FIRST_GENERIC_PORTS,
};

static COUNT_GENERIC_PORTS: &[GenericPortType] = &[
    GenericPortType {
        direction: Direction::Input,
        port_index: 0,
        value_type: VALUE_PARAMETER,
    },
    GenericPortType {
        direction: Direction::Output,
        port_index: 0,
        value_type: NATURAL_EXPRESSION,
    },
];
static COUNT_GENERIC: GenericNodeSignature = GenericNodeSignature {
    parameters: VALUE_PARAMETERS,
    ports: COUNT_GENERIC_PORTS,
};

static CELL_GENERIC_PORTS: &[GenericPortType] = &[
    GenericPortType {
        direction: Direction::Input,
        port_index: 0,
        value_type: VALUE_PARAMETER,
    },
    GenericPortType {
        direction: Direction::Output,
        port_index: 0,
        value_type: VALUE_PARAMETER,
    },
];
static CELL_GENERIC: GenericNodeSignature = GenericNodeSignature {
    parameters: VALUE_PARAMETERS,
    ports: CELL_GENERIC_PORTS,
};

const fn port(
    id: &'static str,
    direction: Direction,
    value_type: TypeContractRef<'static>,
    values: ValueCardinality,
    terminal: TerminalContract,
) -> PortContract<'static> {
    PortContract {
        id: Id(id),
        direction,
        value_type,
        presence: Presence::Required,
        connections: ConnectionCardinality::ExactlyOne,
        values,
        delivery: Delivery::Stream,
        temporal: TemporalContract::Committed,
        terminal,
        sensitivity: Sensitivity::Restricted,
        flow: PortFlowConstraints {
            loss: LossAcceptance::LosslessOnly,
        },
    }
}

const fn named(id: &'static str, template: PortContract<'static>) -> PortContract<'static> {
    PortContract {
        id: Id(id),
        ..template
    }
}

const fn batch_text_port(id: &'static str, direction: Direction) -> PortContract<'static> {
    PortContract {
        id: Id(id),
        direction,
        value_type: TEXT,
        presence: Presence::Required,
        connections: if matches!(direction, Direction::Input) {
            ConnectionCardinality::ExactlyOne
        } else {
            ConnectionCardinality::OneOrMore
        },
        values: ValueCardinality::ExactlyOne,
        delivery: Delivery::FiniteBatch,
        temporal: TemporalContract::Atemporal,
        terminal: TerminalContract::Finite,
        sensitivity: Sensitivity::Public,
        flow: PortFlowConstraints {
            loss: LossAcceptance::LosslessOnly,
        },
    }
}

const IN_BYTES: PortContract<'static> = port(
    "value",
    Direction::Input,
    BYTES,
    ValueCardinality::ZeroOrMore,
    TerminalContract::Either,
);
const IN_BYTES_1: PortContract<'static> = port(
    "left",
    Direction::Input,
    BYTES,
    ValueCardinality::ZeroOrMore,
    TerminalContract::Either,
);
const IN_BYTES_2: PortContract<'static> = port(
    "right",
    Direction::Input,
    BYTES,
    ValueCardinality::ZeroOrMore,
    TerminalContract::Either,
);
const OUT_BYTES: PortContract<'static> = port(
    "value",
    Direction::Output,
    BYTES,
    ValueCardinality::ZeroOrMore,
    TerminalContract::Either,
);
const OUT_BYTES_1: PortContract<'static> = port(
    "left",
    Direction::Output,
    BYTES,
    ValueCardinality::ZeroOrMore,
    TerminalContract::Either,
);
const OUT_BYTES_2: PortContract<'static> = port(
    "right",
    Direction::Output,
    BYTES,
    ValueCardinality::ZeroOrMore,
    TerminalContract::Either,
);
const OUT_FINITE: PortContract<'static> = port(
    "value",
    Direction::Output,
    BYTES,
    ValueCardinality::ZeroOrMore,
    TerminalContract::Finite,
);
const OUT_OPEN: PortContract<'static> = port(
    "value",
    Direction::Output,
    BYTES,
    ValueCardinality::ZeroOrMore,
    TerminalContract::OpenEnded,
);
const OUT_BOOL: PortContract<'static> = port(
    "valid",
    Direction::Output,
    BOOL,
    ValueCardinality::ZeroOrMore,
    TerminalContract::Either,
);
const OUT_U64: PortContract<'static> = port(
    "count",
    Direction::Output,
    U64,
    ValueCardinality::ExactlyOne,
    TerminalContract::Finite,
);
const OUT_RECORD: PortContract<'static> = port(
    "result",
    Direction::Output,
    RECORD,
    ValueCardinality::ZeroOrMore,
    TerminalContract::Either,
);
const OUT_EVIDENCE_RECORD: PortContract<'static> = port(
    "evidence",
    Direction::Output,
    RECORD,
    ValueCardinality::ZeroOrMore,
    TerminalContract::Either,
);
const FS_CHUNK_INPUT: PortContract<'static> = port(
    "chunk",
    Direction::Input,
    FS_CHUNK,
    ValueCardinality::ZeroOrMore,
    TerminalContract::Finite,
);
const FS_CHUNK_OUTPUT: PortContract<'static> = port(
    "chunk",
    Direction::Output,
    FS_CHUNK,
    ValueCardinality::ZeroOrMore,
    TerminalContract::Finite,
);
const FS_READ_RESULT_OUTPUT: PortContract<'static> = port(
    "result",
    Direction::Output,
    FS_READ_RESULT,
    ValueCardinality::ExactlyOne,
    TerminalContract::Finite,
);
const FS_WRITE_RESULT_OUTPUT: PortContract<'static> = port(
    "result",
    Direction::Output,
    FS_WRITE_RESULT,
    ValueCardinality::ExactlyOne,
    TerminalContract::Finite,
);
const FS_EVENT_OUTPUT: PortContract<'static> = port(
    "event",
    Direction::Output,
    FS_EVENT,
    ValueCardinality::ZeroOrMore,
    TerminalContract::Finite,
);
const STORAGE_BLOB_INPUT: PortContract<'static> = port(
    "blob",
    Direction::Input,
    STORAGE_BLOB,
    ValueCardinality::ExactlyOne,
    TerminalContract::Finite,
);
const STORAGE_BLOB_OUTPUT: PortContract<'static> = port(
    "blob",
    Direction::Output,
    STORAGE_BLOB,
    ValueCardinality::ExactlyOne,
    TerminalContract::Finite,
);
const STORAGE_CACHE_HANDLE_INPUT: PortContract<'static> = port(
    "handle",
    Direction::Input,
    STORAGE_CACHE_HANDLE,
    ValueCardinality::ExactlyOne,
    TerminalContract::Finite,
);
const STORAGE_CACHE_HANDLE_OUTPUT: PortContract<'static> = port(
    "handle",
    Direction::Output,
    STORAGE_CACHE_HANDLE,
    ValueCardinality::ExactlyOne,
    TerminalContract::Finite,
);
const STORAGE_CACHE_PUT_RESULT_OUTPUT: PortContract<'static> = port(
    "result",
    Direction::Output,
    STORAGE_CACHE_PUT_RESULT,
    ValueCardinality::ExactlyOne,
    TerminalContract::Finite,
);
const STORAGE_CACHE_GET_RESULT_OUTPUT: PortContract<'static> = port(
    "result",
    Direction::Output,
    STORAGE_CACHE_GET_RESULT,
    ValueCardinality::ExactlyOne,
    TerminalContract::Finite,
);
const STORAGE_CACHE_REMOVE_RESULT_OUTPUT: PortContract<'static> = port(
    "result",
    Direction::Output,
    STORAGE_CACHE_REMOVE_RESULT,
    ValueCardinality::ExactlyOne,
    TerminalContract::Finite,
);
const CONTROL: PortContract<'static> = port(
    "command",
    Direction::Input,
    RECORD,
    ValueCardinality::ZeroOrMore,
    TerminalContract::Either,
);
const IN_HTTP_REQUEST: PortContract<'static> = port(
    "request",
    Direction::Input,
    HTTP_REQUEST,
    ValueCardinality::ZeroOrMore,
    TerminalContract::Either,
);
const OUT_HTTP_REQUEST: PortContract<'static> = port(
    "request",
    Direction::Output,
    HTTP_REQUEST,
    ValueCardinality::ZeroOrMore,
    TerminalContract::Either,
);
const IN_HTTP_RESPONSE: PortContract<'static> = port(
    "response",
    Direction::Input,
    HTTP_RESPONSE,
    ValueCardinality::ZeroOrMore,
    TerminalContract::Either,
);
const OUT_HTTP_RESPONSE: PortContract<'static> = port(
    "response",
    Direction::Output,
    HTTP_RESPONSE,
    ValueCardinality::ZeroOrMore,
    TerminalContract::Either,
);
const FORMAT_TEMPLATE: PortContract<'static> = port(
    "template",
    Direction::Input,
    TEXT,
    ValueCardinality::ExactlyOne,
    TerminalContract::Finite,
);
const FORMAT_VALUES_INPUT: PortContract<'static> = port(
    "values",
    Direction::Input,
    FORMAT_VALUES,
    ValueCardinality::ExactlyOne,
    TerminalContract::Finite,
);
const FORMAT_TEXT_OUTPUT: PortContract<'static> = port(
    "text",
    Direction::Output,
    TEXT,
    ValueCardinality::ExactlyOne,
    TerminalContract::Finite,
);
const FORMAT_VALUES_OUTPUT: PortContract<'static> = port(
    "values",
    Direction::Output,
    FORMAT_VALUES,
    ValueCardinality::ExactlyOne,
    TerminalContract::Finite,
);
const TEXT_STREAM_INPUT: PortContract<'static> = batch_text_port("text", Direction::Input);
const TEXT_LINES_OUTPUT: PortContract<'static> = port(
    "line",
    Direction::Output,
    TEXT,
    ValueCardinality::ZeroOrMore,
    TerminalContract::Finite,
);
const TEXT_ITEMS_INPUT: PortContract<'static> = port(
    "item",
    Direction::Input,
    TEXT,
    ValueCardinality::ZeroOrMore,
    TerminalContract::Finite,
);
const TEXT_JOIN_OUTPUT: PortContract<'static> = batch_text_port("text", Direction::Output);
const DATA_TEXT_INPUT: PortContract<'static> = batch_text_port("text", Direction::Input);
const DATA_BYTES_INPUT: PortContract<'static> = PortContract {
    value_type: BYTES,
    ..batch_text_port("bytes", Direction::Input)
};
const DATA_BYTES_OUTPUT: PortContract<'static> = PortContract {
    value_type: BYTES,
    ..batch_text_port("bytes", Direction::Output)
};
const DATA_TEXT_OUTPUT: PortContract<'static> = batch_text_port("text", Direction::Output);
const DATA_PAYLOAD_INPUT: PortContract<'static> = PortContract {
    value_type: BYTES,
    ..batch_text_port("payload", Direction::Input)
};
const DATA_PAYLOAD_OUTPUT: PortContract<'static> = PortContract {
    value_type: BYTES,
    ..batch_text_port("payload", Direction::Output)
};
const DATA_CHUNK_INPUT: PortContract<'static> = PortContract {
    value_type: BYTES,
    ..batch_text_port("chunk", Direction::Input)
};
const DATA_CANDIDATE_INPUT: PortContract<'static> = port(
    "candidate",
    Direction::Input,
    RECORD,
    ValueCardinality::ZeroOrMore,
    TerminalContract::Either,
);
const DATA_CANDIDATE_OUTPUT: PortContract<'static> = port(
    "candidate",
    Direction::Output,
    RECORD,
    ValueCardinality::ZeroOrMore,
    TerminalContract::Finite,
);
const fn optional_output(mut port: PortContract<'static>) -> PortContract<'static> {
    port.presence = Presence::Optional;
    port.connections = ConnectionCardinality::ZeroOrMore;
    port
}
const fn optional_input(mut port: PortContract<'static>) -> PortContract<'static> {
    port.presence = Presence::Optional;
    port.connections = ConnectionCardinality::ZeroOrOne;
    port
}
const DATA_OPTIONAL_CANDIDATE_OUTPUT: PortContract<'static> =
    optional_output(DATA_CANDIDATE_OUTPUT);
const DATA_DECISION_OUTPUT: PortContract<'static> = port(
    "decision",
    Direction::Output,
    VALIDATION_DECISION,
    ValueCardinality::ZeroOrMore,
    TerminalContract::Finite,
);
const DATA_DECISION_INPUT: PortContract<'static> = port(
    "decision",
    Direction::Input,
    VALIDATION_DECISION,
    ValueCardinality::ZeroOrMore,
    TerminalContract::Finite,
);
const DATA_RECORD_OUTPUT: PortContract<'static> = port(
    "record",
    Direction::Output,
    RECORD,
    ValueCardinality::ExactlyOne,
    TerminalContract::Finite,
);

const EMPTY: ConfigContract<'static> = ConfigContract { fields: &[] };
const BOUNDED: ConfigContract<'static> = ConfigContract {
    fields: &[field("maximum_values", U64), field("maximum_bytes", U64)],
};
const TIMED: ConfigContract<'static> = ConfigContract {
    fields: &[
        field("duration_ticks", U64),
        field("time_basis", REFERENCE),
        field("maximum_pending", U64),
    ],
};
const TIME_DELAY: ConfigContract<'static> = ConfigContract {
    fields: &[
        field("clock", REFERENCE),
        field("clock_schema_version", U64),
        field("clock_hash", BYTES),
        field("resolution_ticks", U64),
        field("duration_ticks", U64),
        field("maximum_pending", U64),
        field("terminal", TEXT),
        field("discontinuity", TEXT),
    ],
};
const TIME_TIMEOUT: ConfigContract<'static> = ConfigContract {
    fields: &[
        field("clock", REFERENCE),
        field("clock_schema_version", U64),
        field("clock_hash", BYTES),
        field("resolution_ticks", U64),
        field("duration_ticks", U64),
        field("condition", TEXT),
        field("reset", TEXT),
        field("late", TEXT),
        field("discontinuity", TEXT),
    ],
};
const TIME_DEBOUNCE: ConfigContract<'static> = ConfigContract {
    fields: &[
        field("clock", REFERENCE),
        field("clock_schema_version", U64),
        field("clock_hash", BYTES),
        field("resolution_ticks", U64),
        field("duration_ticks", U64),
        field("mode", TEXT),
        field("loss", TEXT),
        field("terminal", TEXT),
        field("maximum_retained", U64),
        field("discontinuity", TEXT),
    ],
};
const TIME_THROTTLE: ConfigContract<'static> = ConfigContract {
    fields: &[
        field("clock", REFERENCE),
        field("clock_schema_version", U64),
        field("clock_hash", BYTES),
        field("resolution_ticks", U64),
        field("duration_ticks", U64),
        field("mode", TEXT),
        field("overflow", TEXT),
        field("terminal", TEXT),
        field("maximum_retained", U64),
        field("discontinuity", TEXT),
    ],
};
const STATE_CELL: ConfigContract<'static> = ConfigContract {
    fields: &[
        field("state_schema", REFERENCE),
        field("state_schema_version", U64),
        field("state_schema_hash", BYTES),
        field("initialization", TEXT),
        field("initial", TEXT),
        field("maximum_value_bytes", U64),
        field("emission", TEXT),
        field("reset", TEXT),
        field("terminal", TEXT),
        field("restart", TEXT),
        field("checkpoint", TEXT),
    ],
};
const STATE_DEDUPLICATE: ConfigContract<'static> = ConfigContract {
    fields: &[
        field("equality", REFERENCE),
        field("equality_schema_version", U64),
        field("equality_hash", BYTES),
        field("maximum_entries", U64),
        field("maximum_bytes", U64),
        field("eviction", TEXT),
        field("duplicate", TEXT),
        field("reset", TEXT),
        field("terminal", TEXT),
        field("restart", TEXT),
        field("checkpoint", TEXT),
    ],
};
const STATE_CACHE: ConfigContract<'static> = ConfigContract {
    fields: &[
        field("request_schema", REFERENCE),
        field("request_schema_version", U64),
        field("request_schema_hash", BYTES),
        field("key_equality", REFERENCE),
        field("key_equality_schema_version", U64),
        field("key_equality_hash", BYTES),
        field("maximum_entries", U64),
        field("maximum_key_bytes", U64),
        field("maximum_value_bytes", U64),
        field("maximum_total_bytes", U64),
        field("eviction", TEXT),
        field("ttl", TEXT),
        field("sensitivity", TEXT),
        field("restart", TEXT),
        field("checkpoint", TEXT),
    ],
};
const SUPERVISION_RETRY: ConfigContract<'static> = ConfigContract {
    fields: &[
        field("terminal_schema", REFERENCE),
        field("terminal_schema_version", U64),
        field("terminal_schema_hash", BYTES),
        field("clock", REFERENCE),
        field("clock_schema_version", U64),
        field("clock_hash", BYTES),
        field("maximum_attempts", U64),
        field("deadline_ticks", U64),
        field("idempotency", TEXT),
        field("committed_replay", TEXT),
        field("backoff", TEXT),
        field("initial_backoff_ticks", U64),
        field("maximum_backoff_ticks", U64),
        field("jitter", TEXT),
        field("jitter_ticks", U64),
        field("entropy", REFERENCE),
        field("entropy_schema_version", U64),
        field("entropy_hash", BYTES),
        field("maximum_pending", U64),
        field("cancellation", TEXT),
        field("exhaustion", TEXT),
        field("restart", TEXT),
        field("checkpoint", TEXT),
    ],
};
const SUPERVISION_CIRCUIT_BREAKER: ConfigContract<'static> = ConfigContract {
    fields: &[
        field("terminal_schema", REFERENCE),
        field("terminal_schema_version", U64),
        field("terminal_schema_hash", BYTES),
        field("clock", REFERENCE),
        field("clock_schema_version", U64),
        field("clock_hash", BYTES),
        field("counted_outcomes", TEXT),
        field("maximum_observations", U64),
        field("failure_threshold", U64),
        field("cooldown_ticks", U64),
        field("maximum_half_open_probes", U64),
        field("maximum_pending", U64),
        field("reset", TEXT),
        field("terminal", TEXT),
        field("restart", TEXT),
        field("checkpoint", TEXT),
    ],
};
const TRANSFORM: ConfigContract<'static> = ConfigContract {
    fields: &[
        field("implementation", REFERENCE),
        field("maximum_outputs_per_input", U64),
    ],
};
const FORMAT_VALUES_LITERAL: ConfigContract<'static> = ConfigContract {
    fields: &[field("values", FORMAT_VALUES)],
};
const TEXT_LINES: ConfigContract<'static> = ConfigContract {
    fields: &[
        field("maximum_line_bytes", U64),
        field("maximum_retained_prefix_bytes", U64),
    ],
};
const TEXT_JOIN: ConfigContract<'static> = ConfigContract {
    fields: &[
        field("separator", TEXT),
        field("maximum_items", U64),
        field("maximum_item_bytes", U64),
        field("maximum_separator_bytes", U64),
        field("maximum_output_bytes", U64),
    ],
};
const DATA_CODEC: ConfigContract<'static> = ConfigContract {
    fields: &[
        field("codec", REFERENCE),
        field("codec_schema_version", U64),
        field("codec_hash", BYTES),
        field("maximum_input_bytes", U64),
        field("maximum_output_bytes", U64),
    ],
};
const DATA_FRAMING: ConfigContract<'static> = ConfigContract {
    fields: &[
        field("framing", REFERENCE),
        field("framing_schema_version", U64),
        field("framing_hash", BYTES),
        field("maximum_frame_bytes", U64),
        field("maximum_partial_bytes", U64),
        field("maximum_output_bytes", U64),
    ],
};
const DATA_VALIDATION: ConfigContract<'static> = ConfigContract {
    fields: &[
        field("schema", REFERENCE),
        field("schema_version", U64),
        field("schema_hash", BYTES),
        field("maximum_fields", U64),
        field("maximum_field_name_bytes", U64),
        field("maximum_field_value_bytes", U64),
        field("maximum_work", U64),
    ],
};
const DATA_RECORD_LITERAL: ConfigContract<'static> = ConfigContract {
    fields: &[
        field("fields", RECORD),
        field("maximum_fields", U64),
        field("maximum_field_name_bytes", U64),
        field("maximum_field_value_bytes", U64),
        field("maximum_work", U64),
    ],
};
const DATA_VALIDATION_ASSERT: ConfigContract<'static> = ConfigContract {
    fields: &[field("expected", TEXT)],
};
const STATEFUL: ConfigContract<'static> = ConfigContract {
    fields: &[
        field("maximum_values", U64),
        field("maximum_bytes", U64),
        field("compatibility", REFERENCE),
    ],
};
const HOSTED: ConfigContract<'static> = ConfigContract {
    fields: &[
        field("resource", REFERENCE),
        protected_field("grant", REFERENCE),
        field("maximum_request_bytes", U64),
        field("maximum_response_bytes", U64),
        field("maximum_pending", U64),
    ],
};
const FILE_READ: ConfigContract<'static> = ConfigContract {
    fields: &[
        protected_field("resource", FS_RESOURCE),
        protected_field("grant", REFERENCE),
        field("offset", U64),
        field("maximum_bytes", U64),
        field("chunk_bytes", U64),
        field("consistency", TEXT),
        field("eof", TEXT),
        field("cancellation", TEXT),
    ],
};
const FILE_CHUNK_LITERAL: ConfigContract<'static> = ConfigContract {
    fields: &[field("value", BYTES), field("maximum_bytes", U64)],
};
const FILE_WRITE: ConfigContract<'static> = ConfigContract {
    fields: &[
        protected_field("resource", FS_RESOURCE),
        protected_field("grant", REFERENCE),
        field("mode", TEXT),
        field("maximum_bytes", U64),
        field("partial", TEXT),
        field("flush", TEXT),
        field("cleanup", TEXT),
        field("cancellation", TEXT),
    ],
};
const FILE_WATCH: ConfigContract<'static> = ConfigContract {
    fields: &[
        protected_field("resource", FS_RESOURCE),
        protected_field("grant", REFERENCE),
        field("clock", REFERENCE),
        field("clock_schema_version", U64),
        field("clock_hash", BYTES),
        field("event_kinds", TEXT),
        field("emit_initial", BOOL),
        field("coalescing", TEXT),
        field("loss", TEXT),
        field("queue_capacity", U64),
        field("maximum_events", U64),
        field("overflow", TEXT),
        field("rename_identity", TEXT),
        field("cancellation", TEXT),
    ],
};
const STORAGE_BLOB_LITERAL: ConfigContract<'static> = ConfigContract {
    fields: &[field("value", BYTES), field("maximum_bytes", U64)],
};
const STORAGE_CACHE_PUT: ConfigContract<'static> = ConfigContract {
    fields: &[
        protected_field("resource", REFERENCE),
        protected_field("grant", REFERENCE),
        field("descriptor", REFERENCE),
        field("run_epoch", U64),
        field("now_tick", U64),
        field("retention_ticks", U64),
        field("maximum_blob_bytes", U64),
        field("persistence", TEXT),
        field("eviction", TEXT),
        field("sensitivity", TEXT),
        field("cancellation", TEXT),
    ],
};
const STORAGE_CACHE_GET: ConfigContract<'static> = ConfigContract {
    fields: &[
        protected_field("resource", REFERENCE),
        protected_field("grant", REFERENCE),
        field("descriptor", REFERENCE),
        field("run_epoch", U64),
        field("now_tick", U64),
        field("maximum_blob_bytes", U64),
        field("integrity", TEXT),
        field("cancellation", TEXT),
    ],
};
const STORAGE_CACHE_REMOVE: ConfigContract<'static> = ConfigContract {
    fields: &[
        protected_field("resource", REFERENCE),
        protected_field("grant", REFERENCE),
        field("descriptor", REFERENCE),
        field("run_epoch", U64),
        field("cancellation", TEXT),
    ],
};
const PROCESS_EXEC: ConfigContract<'static> = ConfigContract {
    fields: &[
        field("program", REFERENCE),
        field("argv", RECORD),
        field("environment", RECORD),
        protected_field("working_resource", REFERENCE),
        protected_field("grant", REFERENCE),
        field("stdin_close", TEXT),
        field("maximum_stdin_bytes", U64),
        field("maximum_stdout_bytes", U64),
        field("maximum_stderr_bytes", U64),
        field("maximum_chunk_bytes", U64),
        field("maximum_pending_operations", U64),
        field("maximum_processes", U64),
        field("maximum_child_processes", U64),
        field("maximum_descriptors", U64),
        field("maximum_environment_bytes", U64),
        field("maximum_work", U64),
        field("maximum_evidence_events", U64),
        field("deadline_ticks", U64),
        field("graceful_signal", U64),
        field("graceful_ticks", U64),
        field("forced_ticks", U64),
        field("cancellation", TEXT),
    ],
};
const TCP_CONNECT: ConfigContract<'static> = ConfigContract {
    fields: &[
        protected_field("network_resource", REFERENCE),
        protected_field("target_grant", REFERENCE),
        field("local", SOCKET_ADDRESS),
        field("peer", SOCKET_ADDRESS),
        field("maximum_sessions", U64),
        field("maximum_pending_operations", U64),
        field("maximum_send_bytes", U64),
        field("maximum_receive_bytes", U64),
        field("maximum_chunk_bytes", U64),
        field("maximum_queue_bytes", U64),
        field("maximum_timers", U64),
        field("maximum_work", U64),
        field("maximum_evidence_events", U64),
        field("deadline_ticks", U64),
        field("cleanup_ticks", U64),
        field("cancellation", TEXT),
    ],
};
const TCP_LISTEN: ConfigContract<'static> = ConfigContract {
    fields: &[
        protected_field("network_resource", REFERENCE),
        protected_field("bind_grant", REFERENCE),
        field("local", SOCKET_ADDRESS),
        field("backlog", U64),
        field("maximum_sessions", U64),
        field("maximum_accepts", U64),
        field("maximum_pending_operations", U64),
        field("maximum_send_bytes", U64),
        field("maximum_receive_bytes", U64),
        field("maximum_chunk_bytes", U64),
        field("maximum_queue_bytes", U64),
        field("maximum_timers", U64),
        field("maximum_work", U64),
        field("maximum_evidence_events", U64),
        field("deadline_ticks", U64),
        field("cleanup_ticks", U64),
        field("cancellation", TEXT),
    ],
};
const UDP_CONNECTED: ConfigContract<'static> = ConfigContract {
    fields: &[
        protected_field("network_resource", REFERENCE),
        protected_field("target_grant", REFERENCE),
        field("local", SOCKET_ADDRESS),
        field("peer", SOCKET_ADDRESS),
        field("path_mtu_bytes", U64),
        field("fragmentation", BOOL),
        field("maximum_pending_operations", U64),
        field("maximum_send_bytes", U64),
        field("maximum_receive_bytes", U64),
        field("maximum_message_bytes", U64),
        field("maximum_queued_messages", U64),
        field("maximum_queue_bytes", U64),
        field("maximum_timers", U64),
        field("maximum_work", U64),
        field("maximum_evidence_events", U64),
        field("deadline_ticks", U64),
        field("cleanup_ticks", U64),
        field("cancellation", TEXT),
    ],
};
const UDP_UNCONNECTED: ConfigContract<'static> = ConfigContract {
    fields: &[
        protected_field("network_resource", REFERENCE),
        protected_field("bind_grant", REFERENCE),
        field("local", SOCKET_ADDRESS),
        field("path_mtu_bytes", U64),
        field("fragmentation", BOOL),
        field("maximum_pending_operations", U64),
        field("maximum_send_bytes", U64),
        field("maximum_receive_bytes", U64),
        field("maximum_message_bytes", U64),
        field("maximum_queued_messages", U64),
        field("maximum_queue_bytes", U64),
        field("maximum_timers", U64),
        field("maximum_work", U64),
        field("maximum_evidence_events", U64),
        field("deadline_ticks", U64),
        field("cleanup_ticks", U64),
        field("cancellation", TEXT),
    ],
};
const HTTP_FETCH: ConfigContract<'static> = ConfigContract {
    fields: &[
        protected_field("grant", REFERENCE),
        field("maximum_request_bytes", U64),
        field("maximum_response_bytes", U64),
        field("maximum_pending", U64),
    ],
};
const HTTP_SERVE: ConfigContract<'static> = ConfigContract {
    fields: &[
        field("listen", SOCKET_ADDRESS),
        protected_field("grant", REFERENCE),
        field("maximum_request_bytes", U64),
        field("maximum_response_bytes", U64),
        field("maximum_pending", U64),
    ],
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
        identity: ConfigIdentity::Plan,
    }
}

const fn protected_field(
    key: &'static str,
    value_type: TypeContractRef<'static>,
) -> ConfigFieldContract<'static> {
    ConfigFieldContract {
        key: Id(key),
        value_type,
        requirement: ConfigRequirement::Required,
        sensitivity: Sensitivity::Restricted,
        mutability: ConfigMutability::PreStart,
        identity: ConfigIdentity::Plan,
    }
}

const PURE: ProviderRequirement = ProviderRequirement {
    deterministic: true,
    hosted: true,
    constrained: true,
};
const HOST_SUPPORT: ProviderRequirement = ProviderRequirement {
    deterministic: true,
    hosted: true,
    constrained: false,
};
const FORMAT_SUPPORT: ProviderRequirement = ProviderRequirement {
    deterministic: true,
    hosted: true,
    constrained: false,
};
const FINITE: CatalogLimits = CatalogLimits {
    retained_values: 0,
    retained_bytes: 0,
    pending_operations: 0,
    timers: 0,
    retries: 0,
    work_per_step: 1,
    evidence_events: 4,
};
const BUFFERED: CatalogLimits = CatalogLimits {
    retained_values: 16,
    retained_bytes: 65_536,
    pending_operations: 1,
    timers: 0,
    retries: 0,
    work_per_step: 16,
    evidence_events: 32,
};
const TIMERS: CatalogLimits = CatalogLimits {
    retained_values: 16,
    retained_bytes: 65_536,
    pending_operations: 1,
    timers: 4,
    retries: 0,
    work_per_step: 16,
    evidence_events: 32,
};
const TIME_FAMILY_LIMITS: CatalogLimits = CatalogLimits {
    retained_values: TIME_MAX_RETAINED_VALUES as u32,
    retained_bytes: 65_536,
    pending_operations: 1,
    timers: 1,
    retries: 0,
    work_per_step: 4,
    evidence_events: 128,
};
const RETRY: CatalogLimits = CatalogLimits {
    retained_values: 4,
    retained_bytes: 16_384,
    pending_operations: 1,
    timers: 2,
    retries: 4,
    work_per_step: 8,
    evidence_events: 32,
};
const HOST_LIMITS: CatalogLimits = CatalogLimits {
    retained_values: 8,
    retained_bytes: 65_536,
    pending_operations: 4,
    timers: 2,
    retries: 0,
    work_per_step: 8,
    evidence_events: 32,
};
const FILE_READ_LIMITS: CatalogLimits = CatalogLimits {
    retained_values: 2,
    retained_bytes: FILESYSTEM_MAX_FILE_BYTES as u64,
    pending_operations: 1,
    timers: 0,
    retries: 0,
    work_per_step: 16,
    evidence_events: 64,
};
const FILE_WRITE_LIMITS: CatalogLimits = CatalogLimits {
    retained_values: 2,
    retained_bytes: FILESYSTEM_MAX_FILE_BYTES as u64,
    pending_operations: 1,
    timers: 0,
    retries: 0,
    work_per_step: 16,
    evidence_events: 64,
};
const FILE_WATCH_LIMITS: CatalogLimits = CatalogLimits {
    retained_values: FILESYSTEM_MAX_WATCH_EVENTS as u32,
    retained_bytes: 16_384,
    pending_operations: 1,
    timers: 1,
    retries: 0,
    work_per_step: 8,
    evidence_events: 128,
};
const STORAGE_CACHE_LIMITS: CatalogLimits = CatalogLimits {
    retained_values: CACHE_MAX_ENTRIES as u32,
    retained_bytes: (CACHE_MAX_ENTRIES * CACHE_MAX_BLOB_BYTES) as u64,
    pending_operations: 1,
    timers: 1,
    retries: 0,
    work_per_step: 16,
    evidence_events: 128,
};
const PROCESS_EXEC_LIMITS: CatalogLimits = CatalogLimits {
    retained_values: 3,
    retained_bytes: (PROCESS_MAX_STREAM_BYTES * 3) as u64,
    pending_operations: 3,
    timers: 2,
    retries: 0,
    work_per_step: PROCESS_MAX_CHUNK_BYTES as u32,
    evidence_events: PROCESS_MAX_EVIDENCE_EVENTS as u32,
};
const SOCKET_LIMITS: CatalogLimits = CatalogLimits {
    retained_values: (SOCKET_MAX_SESSIONS + SOCKET_MAX_DATAGRAMS) as u32,
    retained_bytes: SOCKET_MAX_STREAM_BYTES as u64,
    pending_operations: 4,
    timers: 2,
    retries: 0,
    work_per_step: SOCKET_MAX_MESSAGE_BYTES as u32,
    evidence_events: SOCKET_MAX_EVIDENCE_EVENTS as u32,
};
const FORMAT_LIMITS: CatalogLimits = CatalogLimits {
    retained_values: 3,
    retained_bytes: FORMAT_MAX_RETAINED_BYTES as u64,
    pending_operations: 0,
    timers: 0,
    retries: 0,
    work_per_step: FORMAT_MAX_WORK as u32,
    evidence_events: 32,
};
const FORMAT_VALUES_LITERAL_LIMITS: CatalogLimits = CatalogLimits {
    retained_values: 0,
    retained_bytes: 0,
    pending_operations: 0,
    timers: 0,
    retries: 0,
    work_per_step: FORMAT_VALUES_MAX_ENCODED_BYTES as u32,
    evidence_events: 4,
};
const TEXT_LINES_LIMITS: CatalogLimits = CatalogLimits {
    retained_values: 1,
    retained_bytes: LINES_MAX_RETAINED_PREFIX_BYTES as u64,
    pending_operations: 1,
    timers: 0,
    retries: 0,
    work_per_step: 4,
    evidence_events: 4096,
};
const TEXT_JOIN_LIMITS: CatalogLimits = CatalogLimits {
    retained_values: JOIN_MAX_ITEMS as u32,
    retained_bytes: (JOIN_MAX_ITEMS * JOIN_MAX_ITEM_BYTES) as u64,
    pending_operations: 1,
    timers: 0,
    retries: 0,
    work_per_step: JOIN_MAX_ITEMS as u32,
    evidence_events: 128,
};
const DATA_CODEC_LIMITS: CatalogLimits = CatalogLimits {
    retained_values: 1,
    retained_bytes: DATA_MAX_FRAME_BYTES as u64,
    pending_operations: 1,
    timers: 0,
    retries: 0,
    work_per_step: 16,
    evidence_events: 64,
};
const DATA_FRAMING_LIMITS: CatalogLimits = CatalogLimits {
    retained_values: 1,
    retained_bytes: DATA_MAX_FRAME_BYTES as u64 + LENGTH_U32BE_PREFIX_BYTES as u64,
    pending_operations: 1,
    timers: 0,
    retries: 0,
    work_per_step: 16,
    evidence_events: 128,
};
const DATA_VALIDATION_LIMITS: CatalogLimits = CatalogLimits {
    retained_values: 1,
    retained_bytes: (DATA_MAX_RECORD_FIELDS * DATA_MAX_FIELD_VALUE_BYTES) as u64,
    pending_operations: 1,
    timers: 0,
    retries: 0,
    work_per_step: DATA_MAX_RECORD_FIELDS as u32,
    evidence_events: 64,
};

const fn node(
    id: &'static str,
    config: ConfigContract<'static>,
    inputs: &'static [PortContract<'static>],
    outputs: &'static [PortContract<'static>],
) -> NodeContract<'static> {
    NodeContract {
        id: Id(id),
        config,
        inputs,
        outputs,
    }
}

macro_rules! entry {
    ($id:literal, $family:ident, $config:ident, $inputs:expr, $outputs:expr,
     $type_behavior:ident, $time:ident, $limits:ident, $support:ident) => {
        CatalogEntry {
            contract: node($id, $config, $inputs, $outputs),
            family: StandardFamily::$family,
            type_behavior: TypeBehavior::$type_behavior,
            generic_signature: None,
            time_basis: TimeBasis::$time,
            ordering_policy: Id("ordering/input-sequence"),
            terminal_policy: Id("terminal/explicit"),
            cancellation_policy: Id("cancellation/bounded"),
            pressure_policy: Id("pressure/cord-policy"),
            provider: Id(concat!($id, ".reference")),
            host_service: None,
            limits: $limits,
            required_support: $support,
        }
    };
}

macro_rules! generic_entry {
    ($id:literal, $family:ident, $config:ident, $inputs:expr, $outputs:expr,
     $type_behavior:ident, $time:ident, $limits:ident, $support:ident, $signature:ident) => {
        CatalogEntry {
            generic_signature: Some(&$signature),
            ..entry!(
                $id,
                $family,
                $config,
                $inputs,
                $outputs,
                $type_behavior,
                $time,
                $limits,
                $support
            )
        }
    };
}

macro_rules! host_entry {
    ($id:literal, $family:ident, $request:literal, $result:literal, $service:literal) => {{
        let mut value = entry!(
            $id,
            $family,
            HOSTED,
            &[named($request, IN_BYTES)],
            &[named($result, OUT_RECORD)],
            ExplicitAdapter,
            None,
            HOST_LIMITS,
            HOST_SUPPORT
        );
        value.host_service = Some(Id($service));
        value
    }};
}

macro_rules! typed_host_entry {
    ($id:literal, $config:ident, $inputs:expr, $outputs:expr, $service:literal) => {{
        let mut value = entry!(
            $id,
            Boundary,
            $config,
            $inputs,
            $outputs,
            ExplicitAdapter,
            None,
            HOST_LIMITS,
            HOST_SUPPORT
        );
        value.host_service = Some(Id($service));
        value
    }};
}

/// Version-one concrete catalog. Port/config arrays and entries are all
/// borrowed static data and require no allocator.
pub static STANDARD_CATALOG: &[CatalogEntry] = &[
    entry!(
        "std/literal",
        Source,
        BOUNDED,
        &[],
        &[OUT_FINITE],
        ProducesDeclaredType,
        None,
        FINITE,
        PURE
    ),
    entry!(
        "std/text/format",
        Transform,
        EMPTY,
        &[FORMAT_TEMPLATE, FORMAT_VALUES_INPUT],
        &[FORMAT_TEXT_OUTPUT],
        ExplicitAdapter,
        None,
        FORMAT_LIMITS,
        FORMAT_SUPPORT
    ),
    entry!(
        "std/format-values/literal",
        Source,
        FORMAT_VALUES_LITERAL,
        &[],
        &[FORMAT_VALUES_OUTPUT],
        ProducesDeclaredType,
        None,
        FORMAT_VALUES_LITERAL_LIMITS,
        PURE
    ),
    entry!(
        "std/text/lines",
        Transform,
        TEXT_LINES,
        &[TEXT_STREAM_INPUT],
        &[TEXT_LINES_OUTPUT],
        Preserving,
        None,
        TEXT_LINES_LIMITS,
        FORMAT_SUPPORT
    ),
    entry!(
        "std/text/join",
        Transform,
        TEXT_JOIN,
        &[TEXT_ITEMS_INPUT],
        &[TEXT_JOIN_OUTPUT],
        Preserving,
        None,
        TEXT_JOIN_LIMITS,
        FORMAT_SUPPORT
    ),
    entry!(
        "std/empty",
        Source,
        EMPTY,
        &[],
        &[OUT_FINITE],
        ProducesDeclaredType,
        None,
        FINITE,
        PURE
    ),
    entry!(
        "std/never",
        Source,
        EMPTY,
        &[],
        &[OUT_OPEN],
        ProducesDeclaredType,
        None,
        FINITE,
        PURE
    ),
    entry!(
        "std/finite-sequence",
        Source,
        BOUNDED,
        &[],
        &[named("item", OUT_FINITE)],
        ProducesDeclaredType,
        None,
        BUFFERED,
        PURE
    ),
    generic_entry!(
        "flow/identity",
        Structural,
        EMPTY,
        &[IN_BYTES],
        &[OUT_BYTES],
        Preserving,
        None,
        FINITE,
        PURE,
        IDENTITY_GENERIC
    ),
    generic_entry!(
        "conduit.std/tee",
        Structural,
        BOUNDED,
        &[IN_BYTES],
        &[OUT_BYTES_1, OUT_BYTES_2],
        Preserving,
        None,
        BUFFERED,
        PURE,
        TEE_GENERIC
    ),
    generic_entry!(
        "conduit.std/merge",
        Structural,
        BOUNDED,
        &[IN_BYTES_1, IN_BYTES_2],
        &[OUT_BYTES],
        Preserving,
        None,
        BUFFERED,
        PURE,
        MERGE_GENERIC
    ),
    entry!(
        "conduit.std/zip",
        Structural,
        BOUNDED,
        &[IN_BYTES_1, IN_BYTES_2],
        &[OUT_RECORD],
        ExplicitAdapter,
        None,
        BUFFERED,
        PURE
    ),
    entry!(
        "flow/combine-latest",
        Structural,
        BOUNDED,
        &[IN_BYTES_1, IN_BYTES_2],
        &[named("snapshot", OUT_RECORD)],
        ExplicitAdapter,
        None,
        BUFFERED,
        PURE
    ),
    entry!(
        "flow/mux",
        Structural,
        BOUNDED,
        &[named("candidate", IN_BYTES), named("selector", CONTROL)],
        &[named("selected", OUT_BYTES)],
        Preserving,
        None,
        BUFFERED,
        PURE
    ),
    entry!(
        "flow/demux",
        Structural,
        BOUNDED,
        &[named("item", IN_BYTES), named("selector", CONTROL)],
        &[OUT_BYTES_1, OUT_BYTES_2],
        Preserving,
        None,
        BUFFERED,
        PURE
    ),
    entry!(
        "flow/keyed-dispatch",
        Structural,
        BOUNDED,
        &[named("item", IN_BYTES), named("key", CONTROL)],
        &[named("dispatched", OUT_BYTES)],
        Preserving,
        None,
        BUFFERED,
        PURE
    ),
    entry!(
        "conduit.std/select",
        Structural,
        BOUNDED,
        &[IN_BYTES_1, IN_BYTES_2, named("selector", CONTROL)],
        &[named("selected", OUT_BYTES)],
        Preserving,
        None,
        BUFFERED,
        PURE
    ),
    entry!(
        "conduit.std/gate",
        Structural,
        EMPTY,
        &[IN_BYTES, named("permit", CONTROL)],
        &[OUT_BYTES],
        Preserving,
        None,
        FINITE,
        PURE
    ),
    entry!(
        "flow/switch",
        Structural,
        BOUNDED,
        &[IN_BYTES_1, IN_BYTES_2, named("selector", CONTROL)],
        &[named("selected", OUT_BYTES)],
        Preserving,
        None,
        BUFFERED,
        PURE
    ),
    entry!(
        "flow/fallback",
        Structural,
        BOUNDED,
        &[
            named("primary", IN_BYTES_1),
            named("fallback", IN_BYTES_2),
            named("availability", CONTROL),
        ],
        &[named("selected", OUT_BYTES)],
        Preserving,
        None,
        BUFFERED,
        PURE
    ),
    entry!(
        "flow/feedback-delay",
        Structural,
        BOUNDED,
        &[named("feedback", IN_BYTES)],
        &[named("delayed", OUT_BYTES)],
        Preserving,
        Monotonic,
        TIMERS,
        PURE
    ),
    entry!(
        "flow/discard",
        Sink,
        EMPTY,
        &[named("item", IN_BYTES)],
        &[],
        Preserving,
        None,
        FINITE,
        PURE
    ),
    entry!(
        "flow/collect",
        Sink,
        BOUNDED,
        &[named("item", IN_BYTES)],
        &[named("collection", OUT_RECORD)],
        ExplicitAdapter,
        None,
        BUFFERED,
        PURE
    ),
    generic_entry!(
        "flow/first",
        Sink,
        BOUNDED,
        &[named("item", IN_BYTES)],
        &[named("first", OUT_FINITE)],
        ExplicitAdapter,
        None,
        BUFFERED,
        PURE,
        FIRST_GENERIC
    ),
    entry!(
        "flow/last",
        Sink,
        BOUNDED,
        &[named("item", IN_BYTES)],
        &[named("last", OUT_FINITE)],
        Preserving,
        None,
        BUFFERED,
        PURE
    ),
    generic_entry!(
        "flow/count",
        Sink,
        BOUNDED,
        &[named("item", IN_BYTES)],
        &[OUT_U64],
        ExplicitAdapter,
        None,
        BUFFERED,
        PURE,
        COUNT_GENERIC
    ),
    entry!(
        "flow/map",
        Transform,
        TRANSFORM,
        &[named("item", IN_BYTES)],
        &[named("mapped", OUT_RECORD)],
        ExplicitAdapter,
        None,
        BUFFERED,
        PURE
    ),
    entry!(
        "flow/filter",
        Transform,
        TRANSFORM,
        &[named("candidate", IN_BYTES)],
        &[named("accepted", OUT_BYTES)],
        Preserving,
        None,
        BUFFERED,
        PURE
    ),
    entry!(
        "flow/filter-map",
        Transform,
        TRANSFORM,
        &[named("candidate", IN_BYTES)],
        &[named("mapped", OUT_RECORD)],
        ExplicitAdapter,
        None,
        BUFFERED,
        PURE
    ),
    entry!(
        "flow/flat-map",
        Transform,
        TRANSFORM,
        &[named("item", IN_BYTES)],
        &[named("mapped", OUT_RECORD)],
        ExplicitAdapter,
        None,
        BUFFERED,
        PURE
    ),
    entry!(
        "flow/fold",
        Transform,
        STATEFUL,
        &[named("item", IN_BYTES)],
        &[named("aggregate", OUT_RECORD)],
        ExplicitAdapter,
        None,
        BUFFERED,
        PURE
    ),
    entry!(
        "std/record/literal",
        Source,
        DATA_RECORD_LITERAL,
        &[],
        &[DATA_RECORD_OUTPUT],
        ProducesDeclaredType,
        None,
        DATA_VALIDATION_LIMITS,
        PURE
    ),
    entry!(
        "std/data/validate-closed-record",
        Transform,
        DATA_VALIDATION,
        &[DATA_CANDIDATE_INPUT],
        &[DATA_OPTIONAL_CANDIDATE_OUTPUT, DATA_DECISION_OUTPUT],
        ExplicitAdapter,
        None,
        DATA_VALIDATION_LIMITS,
        PURE
    ),
    entry!(
        "std/testing/assert-validation-decision",
        Testing,
        DATA_VALIDATION_ASSERT,
        &[DATA_DECISION_INPUT],
        &[],
        Preserving,
        None,
        DATA_VALIDATION_LIMITS,
        PURE
    ),
    entry!(
        "flow/adapter",
        Transform,
        TRANSFORM,
        &[named("source", IN_BYTES)],
        &[named("adapted", OUT_RECORD)],
        ExplicitAdapter,
        None,
        BUFFERED,
        PURE
    ),
    entry!(
        "std/data/encode-utf8",
        Transform,
        DATA_CODEC,
        &[DATA_TEXT_INPUT],
        &[DATA_BYTES_OUTPUT],
        ExplicitAdapter,
        None,
        DATA_CODEC_LIMITS,
        PURE
    ),
    entry!(
        "std/data/decode-utf8",
        Transform,
        DATA_CODEC,
        &[DATA_BYTES_INPUT],
        &[DATA_TEXT_OUTPUT],
        ExplicitAdapter,
        None,
        DATA_CODEC_LIMITS,
        PURE
    ),
    entry!(
        "std/data/frame-length-u32be",
        Transform,
        DATA_FRAMING,
        &[DATA_PAYLOAD_INPUT],
        &[DATA_BYTES_OUTPUT],
        ExplicitAdapter,
        None,
        DATA_FRAMING_LIMITS,
        PURE
    ),
    entry!(
        "std/data/deframe-length-u32be",
        Transform,
        DATA_FRAMING,
        &[DATA_CHUNK_INPUT],
        &[DATA_PAYLOAD_OUTPUT],
        ExplicitAdapter,
        None,
        DATA_FRAMING_LIMITS,
        PURE
    ),
    entry!(
        "time/timer",
        Time,
        TIMED,
        &[],
        &[named("elapsed", OUT_U64)],
        ProducesDeclaredType,
        Monotonic,
        TIMERS,
        PURE
    ),
    entry!(
        "time/ticker",
        Time,
        TIMED,
        &[],
        &[named("tick", OUT_U64)],
        ProducesDeclaredType,
        Monotonic,
        TIMERS,
        PURE
    ),
    generic_entry!(
        "time/delay",
        Time,
        TIME_DELAY,
        &[named("value", TEXT_ITEMS_INPUT)],
        &[named("value", TEXT_LINES_OUTPUT)],
        Preserving,
        Monotonic,
        TIME_FAMILY_LIMITS,
        PURE,
        IDENTITY_GENERIC
    ),
    entry!(
        "time/deadline",
        Time,
        TIMED,
        &[named("item", IN_BYTES)],
        &[named("before_deadline", OUT_BYTES)],
        Preserving,
        Monotonic,
        TIMERS,
        PURE
    ),
    generic_entry!(
        "time/timeout",
        Time,
        TIME_TIMEOUT,
        &[named("item", TEXT_ITEMS_INPUT)],
        &[named("completed", TEXT_LINES_OUTPUT)],
        Preserving,
        Monotonic,
        TIME_FAMILY_LIMITS,
        PURE,
        IDENTITY_GENERIC
    ),
    generic_entry!(
        "time/debounce",
        Time,
        TIME_DEBOUNCE,
        &[named("event", TEXT_ITEMS_INPUT)],
        &[named("settled", TEXT_LINES_OUTPUT)],
        Preserving,
        Monotonic,
        TIME_FAMILY_LIMITS,
        PURE,
        IDENTITY_GENERIC
    ),
    generic_entry!(
        "time/throttle",
        Time,
        TIME_THROTTLE,
        &[named("request", TEXT_ITEMS_INPUT)],
        &[named("admitted", TEXT_LINES_OUTPUT)],
        Preserving,
        Monotonic,
        TIME_FAMILY_LIMITS,
        PURE,
        IDENTITY_GENERIC
    ),
    entry!(
        "time/sample",
        Time,
        TIMED,
        &[named("observation", IN_BYTES)],
        &[named("sample", OUT_BYTES)],
        Preserving,
        Monotonic,
        TIMERS,
        PURE
    ),
    entry!(
        "time/rate-limit",
        Time,
        TIMED,
        &[named("request", IN_BYTES)],
        &[named("admitted", OUT_BYTES)],
        Preserving,
        Monotonic,
        TIMERS,
        PURE
    ),
    entry!(
        "time/window",
        Time,
        TIMED,
        &[named("item", IN_BYTES)],
        &[named("window", OUT_RECORD)],
        ExplicitAdapter,
        DomainEvent,
        TIMERS,
        PURE
    ),
    entry!(
        "time/batch",
        Time,
        TIMED,
        &[named("item", IN_BYTES)],
        &[named("batch", OUT_RECORD)],
        ExplicitAdapter,
        Monotonic,
        TIMERS,
        PURE
    ),
    generic_entry!(
        "state/cell",
        State,
        STATE_CELL,
        &[
            named("update", TEXT_ITEMS_INPUT),
            optional_input(named("command", TEXT_ITEMS_INPUT))
        ],
        &[named("current", TEXT_LINES_OUTPUT)],
        Preserving,
        None,
        BUFFERED,
        FORMAT_SUPPORT,
        CELL_GENERIC
    ),
    entry!(
        "state/counter",
        State,
        STATEFUL,
        &[named("event", IN_BYTES), named("command", CONTROL)],
        &[OUT_U64],
        ExplicitAdapter,
        None,
        BUFFERED,
        PURE
    ),
    entry!(
        "state/machine",
        State,
        STATEFUL,
        &[named("event", IN_BYTES), named("command", CONTROL)],
        &[named("state", OUT_RECORD)],
        ExplicitAdapter,
        None,
        BUFFERED,
        PURE
    ),
    generic_entry!(
        "state/deduplicate",
        State,
        STATE_DEDUPLICATE,
        &[named("candidate", TEXT_ITEMS_INPUT)],
        &[named("unique", TEXT_LINES_OUTPUT)],
        Preserving,
        None,
        BUFFERED,
        FORMAT_SUPPORT,
        IDENTITY_GENERIC
    ),
    entry!(
        "state/cache",
        State,
        STATE_CACHE,
        &[named("request", TEXT_ITEMS_INPUT)],
        &[named("response", TEXT_LINES_OUTPUT)],
        ExplicitAdapter,
        None,
        BUFFERED,
        FORMAT_SUPPORT
    ),
    entry!(
        "state/history",
        State,
        STATEFUL,
        &[named("event", IN_BYTES), named("command", CONTROL)],
        &[named("history", OUT_RECORD)],
        ExplicitAdapter,
        None,
        BUFFERED,
        PURE
    ),
    entry!(
        "state/checkpoint",
        State,
        STATEFUL,
        &[named("state", IN_BYTES), named("command", CONTROL)],
        &[named("checkpoint", OUT_RECORD)],
        ExplicitAdapter,
        None,
        BUFFERED,
        PURE
    ),
    generic_entry!(
        "supervision/retry",
        Supervision,
        SUPERVISION_RETRY,
        &[
            named("request", TEXT_ITEMS_INPUT),
            named("terminal", TEXT_ITEMS_INPUT),
            optional_input(named("entropy", TEXT_ITEMS_INPUT))
        ],
        &[named("attempt", TEXT_LINES_OUTPUT)],
        Preserving,
        Monotonic,
        RETRY,
        FORMAT_SUPPORT,
        IDENTITY_GENERIC
    ),
    generic_entry!(
        "supervision/circuit-breaker",
        Supervision,
        SUPERVISION_CIRCUIT_BREAKER,
        &[
            named("request", TEXT_ITEMS_INPUT),
            named("terminal", TEXT_ITEMS_INPUT)
        ],
        &[named("admitted", TEXT_LINES_OUTPUT)],
        Preserving,
        Monotonic,
        RETRY,
        FORMAT_SUPPORT,
        IDENTITY_GENERIC
    ),
    entry!(
        "supervision/supervisor",
        Supervision,
        STATEFUL,
        &[named("terminal", CONTROL)],
        &[named("decision", OUT_RECORD)],
        ExplicitAdapter,
        Monotonic,
        RETRY,
        PURE
    ),
    entry!(
        "supervision/health-gate",
        Supervision,
        STATEFUL,
        &[named("observation", IN_BYTES), named("command", CONTROL)],
        &[named("healthy", OUT_BYTES)],
        Preserving,
        Monotonic,
        RETRY,
        PURE
    ),
    entry!(
        "supervision/terminal-projection",
        Supervision,
        BOUNDED,
        &[named("terminal", CONTROL)],
        &[named("projection", OUT_RECORD)],
        ExplicitAdapter,
        None,
        BUFFERED,
        PURE
    ),
    entry!(
        "supervision/operator-action",
        Supervision,
        BOUNDED,
        &[named("request", CONTROL)],
        &[named("decision", OUT_RECORD)],
        ExplicitAdapter,
        None,
        BUFFERED,
        PURE
    ),
    entry!(
        "supervision/worker-pool",
        Supervision,
        STATEFUL,
        &[named("job", IN_BYTES), named("command", CONTROL)],
        &[named("completion", OUT_BYTES)],
        Preserving,
        None,
        BUFFERED,
        PURE
    ),
    entry!(
        "test/probe",
        Testing,
        BOUNDED,
        &[named("observed", IN_BYTES)],
        &[named("forwarded", OUT_BYTES), OUT_EVIDENCE_RECORD],
        ExplicitAdapter,
        None,
        BUFFERED,
        PURE
    ),
    entry!(
        "test/assertion",
        Testing,
        TRANSFORM,
        &[named("candidate", IN_BYTES)],
        &[named("verified", OUT_BYTES), OUT_BOOL],
        ExplicitAdapter,
        None,
        BUFFERED,
        PURE
    ),
    entry!(
        "test/fault-source",
        Testing,
        BOUNDED,
        &[],
        &[named("failure", OUT_RECORD)],
        ProducesDeclaredType,
        Monotonic,
        TIMERS,
        PURE
    ),
    entry!(
        "test/record",
        Testing,
        STATEFUL,
        &[named("observed", IN_BYTES)],
        &[named("recorded", OUT_BYTES), OUT_EVIDENCE_RECORD],
        ExplicitAdapter,
        None,
        BUFFERED,
        PURE
    ),
    entry!(
        "test/replay",
        Testing,
        STATEFUL,
        &[named("command", CONTROL)],
        &[named("recorded", OUT_BYTES)],
        ProducesDeclaredType,
        DomainEvent,
        TIMERS,
        PURE
    ),
    entry!(
        "fs/chunk/literal",
        Source,
        FILE_CHUNK_LITERAL,
        &[],
        &[FS_CHUNK_OUTPUT],
        ProducesDeclaredType,
        None,
        FINITE,
        PURE
    ),
    {
        let mut value = entry!(
            "fs/read",
            Boundary,
            FILE_READ,
            &[],
            &[
                optional_output(FS_CHUNK_OUTPUT),
                optional_output(FS_READ_RESULT_OUTPUT),
            ],
            ProducesDeclaredType,
            None,
            FILE_READ_LIMITS,
            HOST_SUPPORT
        );
        value.host_service = Some(Id("host/filesystem-read"));
        value
    },
    {
        let mut value = entry!(
            "fs/write",
            Boundary,
            FILE_WRITE,
            &[FS_CHUNK_INPUT],
            &[optional_output(FS_WRITE_RESULT_OUTPUT)],
            ProducesDeclaredType,
            None,
            FILE_WRITE_LIMITS,
            HOST_SUPPORT
        );
        value.host_service = Some(Id("host/filesystem-write"));
        value
    },
    {
        let mut value = entry!(
            "fs/watch",
            Boundary,
            FILE_WATCH,
            &[],
            &[optional_output(FS_EVENT_OUTPUT)],
            ProducesDeclaredType,
            Monotonic,
            FILE_WATCH_LIMITS,
            HOST_SUPPORT
        );
        value.host_service = Some(Id("host/filesystem-watch"));
        value
    },
    entry!(
        "storage/blob/literal",
        Source,
        STORAGE_BLOB_LITERAL,
        &[],
        &[STORAGE_BLOB_OUTPUT],
        ProducesDeclaredType,
        None,
        FINITE,
        PURE
    ),
    {
        let mut value = entry!(
            "storage/cache/put",
            Boundary,
            STORAGE_CACHE_PUT,
            &[STORAGE_BLOB_INPUT],
            &[
                STORAGE_CACHE_HANDLE_OUTPUT,
                optional_output(STORAGE_CACHE_PUT_RESULT_OUTPUT),
            ],
            ProducesDeclaredType,
            None,
            STORAGE_CACHE_LIMITS,
            HOST_SUPPORT
        );
        value.host_service = Some(Id("host/storage-cache-put"));
        value
    },
    {
        let mut value = entry!(
            "storage/cache/get",
            Boundary,
            STORAGE_CACHE_GET,
            &[STORAGE_CACHE_HANDLE_INPUT],
            &[
                optional_output(STORAGE_BLOB_OUTPUT),
                optional_output(STORAGE_CACHE_GET_RESULT_OUTPUT),
            ],
            ProducesDeclaredType,
            None,
            STORAGE_CACHE_LIMITS,
            HOST_SUPPORT
        );
        value.host_service = Some(Id("host/storage-cache-get"));
        value
    },
    {
        let mut value = entry!(
            "storage/cache/remove",
            Boundary,
            STORAGE_CACHE_REMOVE,
            &[STORAGE_CACHE_HANDLE_INPUT],
            &[optional_output(STORAGE_CACHE_REMOVE_RESULT_OUTPUT)],
            ProducesDeclaredType,
            None,
            STORAGE_CACHE_LIMITS,
            HOST_SUPPORT
        );
        value.host_service = Some(Id("host/storage-cache-remove"));
        value
    },
    {
        let mut value = entry!(
            "conduit.host/process/exec",
            Boundary,
            PROCESS_EXEC,
            &[PROCESS_STDIN],
            &[PROCESS_STDOUT, PROCESS_STDERR],
            Preserving,
            Monotonic,
            PROCESS_EXEC_LIMITS,
            HOST_SUPPORT
        );
        value.host_service = Some(Id("host/process-exec"));
        value
    },
    host_entry!(
        "secret/reference",
        Boundary,
        "reference",
        "secret",
        "host/secret-resolution"
    ),
    host_entry!(
        "crypto/operation",
        Boundary,
        "operation",
        "result",
        "host/cryptography"
    ),
    host_entry!(
        "data/compress",
        Boundary,
        "uncompressed",
        "compressed",
        "host/compression"
    ),
    typed_host_entry!(
        "net/http/fetch",
        HTTP_FETCH,
        &[IN_HTTP_REQUEST],
        &[OUT_HTTP_RESPONSE],
        "host/http-client"
    ),
    typed_host_entry!(
        "net/http/serve",
        HTTP_SERVE,
        &[IN_HTTP_RESPONSE],
        &[OUT_HTTP_REQUEST],
        "host/http-server"
    ),
    host_entry!(
        "net/websocket",
        Boundary,
        "frame",
        "frame",
        "host/websocket"
    ),
    host_entry!("net/sse", Boundary, "request", "event", "host/sse"),
    host_entry!(
        "transport/zenoh",
        Boundary,
        "sample",
        "sample",
        "host/distributed-transport"
    ),
    host_entry!(
        "evidence/export",
        Boundary,
        "evidence",
        "receipt",
        "host/evidence-export"
    ),
    host_entry!(
        "net/observe",
        Network,
        "request",
        "observation",
        "host/network-observe"
    ),
    host_entry!(
        "net/wifi/scan",
        Network,
        "request",
        "network",
        "host/wifi-scan"
    ),
    host_entry!(
        "net/wifi/join",
        Network,
        "configuration",
        "state",
        "host/wifi-station"
    ),
    host_entry!(
        "net/wifi/access-point",
        Network,
        "configuration",
        "state",
        "host/wifi-access-point"
    ),
    host_entry!(
        "net/ip/configure-static",
        Network,
        "configuration",
        "state",
        "host/address-config"
    ),
    host_entry!(
        "net/dhcp/client",
        Network,
        "request",
        "lease",
        "host/dhcp-client"
    ),
    host_entry!(
        "net/dhcp/server",
        Network,
        "configuration",
        "lease",
        "host/dhcp-server"
    ),
    host_entry!("net/dns/resolve", Network, "name", "addresses", "host/dns"),
    host_entry!("net/route", Network, "route", "state", "host/route"),
    host_entry!("net/bridge", Network, "frame", "frame", "host/bridge"),
    host_entry!("net/nat", Network, "packet", "packet", "host/nat"),
    host_entry!(
        "net/reachability",
        Network,
        "target",
        "observation",
        "host/reachability"
    ),
    {
        let mut value = entry!(
            "conduit.host/net/tcp/connect",
            Network,
            TCP_CONNECT,
            &[TCP_TRANSMIT],
            &[TCP_RECEIVED, SOCKET_SESSION_OUTPUT, SOCKET_RESULT_OUTPUT],
            ProducesDeclaredType,
            Monotonic,
            SOCKET_LIMITS,
            HOST_SUPPORT
        );
        value.host_service = Some(Id("host/tcp-connect"));
        value
    },
    {
        let mut value = entry!(
            "conduit.host/net/tcp/listen",
            Network,
            TCP_LISTEN,
            &[TCP_TRANSMIT],
            &[TCP_RECEIVED, SOCKET_SESSION_OUTPUT, SOCKET_RESULT_OUTPUT],
            ProducesDeclaredType,
            Monotonic,
            SOCKET_LIMITS,
            HOST_SUPPORT
        );
        value.host_service = Some(Id("host/tcp-listen"));
        value
    },
    {
        let mut value = entry!(
            "conduit.host/net/udp/connected",
            Network,
            UDP_CONNECTED,
            &[UDP_DATAGRAM_INPUT],
            &[UDP_DATAGRAM_OUTPUT, SOCKET_RESULT_OUTPUT],
            ProducesDeclaredType,
            Monotonic,
            SOCKET_LIMITS,
            HOST_SUPPORT
        );
        value.host_service = Some(Id("host/udp-connected"));
        value
    },
    {
        let mut value = entry!(
            "conduit.host/net/udp/datagram",
            Network,
            UDP_UNCONNECTED,
            &[UDP_DATAGRAM_INPUT],
            &[UDP_DATAGRAM_OUTPUT, SOCKET_RESULT_OUTPUT],
            ProducesDeclaredType,
            Monotonic,
            SOCKET_LIMITS,
            HOST_SUPPORT
        );
        value.host_service = Some(Id("host/udp-datagram"));
        value
    },
];

/// Looks up one exact published standard contract without allocating.
#[must_use]
pub fn standard_node_contract(id: &str) -> Option<&'static NodeContract<'static>> {
    STANDARD_CATALOG
        .iter()
        .find(|entry| entry.contract.id.as_str() == id)
        .map(|entry| &entry.contract)
}

/// Validate the complete catalog without allocating or consulting a registry.
pub fn validate_catalog(entries: &[CatalogEntry]) -> Result<(), CatalogError> {
    for (index, entry) in entries.iter().enumerate() {
        validate_entry(entry)?;
        if entries[..index]
            .iter()
            .any(|prior| prior.contract.id == entry.contract.id)
        {
            return Err(CatalogError::DuplicateContract);
        }
    }
    Ok(())
}

pub fn validate_entry(entry: &CatalogEntry) -> Result<(), CatalogError> {
    for id in [
        entry.contract.id,
        entry.ordering_policy,
        entry.terminal_policy,
        entry.cancellation_policy,
        entry.pressure_policy,
        entry.provider,
    ] {
        Id::new(id.as_str()).map_err(|_| CatalogError::InvalidIdentifier)?;
    }
    if entry
        .contract
        .inputs
        .iter()
        .any(|port| port.direction != Direction::Input)
        || entry
            .contract
            .outputs
            .iter()
            .any(|port| port.direction != Direction::Output)
    {
        return Err(CatalogError::InvalidPort);
    }
    for ports in [entry.contract.inputs, entry.contract.outputs] {
        for (index, port) in ports.iter().enumerate() {
            Id::new(port.id.as_str()).map_err(|_| CatalogError::InvalidPort)?;
            if ports[..index].iter().any(|prior| prior.id == port.id) {
                return Err(CatalogError::DuplicatePort);
            }
        }
    }
    if let Some(signature) = entry.generic_signature {
        validate_generic_signature(entry, signature)?;
    }
    if entry.limits.work_per_step == 0 || entry.limits.evidence_events == 0 {
        return Err(CatalogError::UnboundedWork);
    }
    if matches!(
        entry.family,
        StandardFamily::State | StandardFamily::Supervision
    ) && (entry.limits.retained_values == 0 || entry.limits.retained_bytes == 0)
    {
        return Err(CatalogError::MissingStateBound);
    }
    if entry.time_basis != TimeBasis::None && entry.limits.timers == 0 {
        return Err(CatalogError::MissingTimer);
    }
    if matches!(
        entry.family,
        StandardFamily::Boundary | StandardFamily::Network
    ) {
        if entry.host_service.is_none() {
            return Err(CatalogError::MissingHostService);
        }
        if entry.limits.pending_operations == 0 {
            return Err(CatalogError::MissingPendingBound);
        }
    }
    if entry.contract.outputs.iter().any(|output| {
        !entry
            .contract
            .inputs
            .iter()
            .any(|input| input.value_type == output.value_type)
    }) && entry.type_behavior == TypeBehavior::Preserving
    {
        return Err(CatalogError::ImplicitTypeChange);
    }
    if !entry.required_support.deterministic
        || (!entry.required_support.hosted && !entry.required_support.constrained)
    {
        return Err(CatalogError::MissingReferenceProvider);
    }
    Ok(())
}

fn validate_generic_signature(
    entry: &CatalogEntry,
    signature: &GenericNodeSignature,
) -> Result<(), CatalogError> {
    if signature.parameters.is_empty() || signature.ports.is_empty() {
        return Err(CatalogError::InvalidGenericSignature);
    }
    for (index, parameter) in signature.parameters.iter().enumerate() {
        Id::new(parameter.as_str()).map_err(|_| CatalogError::InvalidGenericSignature)?;
        if signature.parameters[..index].contains(parameter)
            || !signature
                .ports
                .iter()
                .any(|port| expression_contains_parameter(port.value_type, *parameter))
        {
            return Err(CatalogError::InvalidGenericSignature);
        }
    }
    for (index, port_type) in signature.ports.iter().enumerate() {
        let ports = match port_type.direction {
            Direction::Input => entry.contract.inputs,
            Direction::Output => entry.contract.outputs,
        };
        if usize::from(port_type.port_index) >= ports.len()
            || signature.ports[..index].iter().any(|prior| {
                prior.direction == port_type.direction && prior.port_index == port_type.port_index
            })
            || !valid_type_expression(port_type.value_type, signature.parameters)
        {
            return Err(CatalogError::InvalidGenericSignature);
        }
    }
    Ok(())
}

fn valid_type_expression(expression: CatalogTypeExpression, parameters: &[Id<'static>]) -> bool {
    match expression {
        CatalogTypeExpression::Parameter(parameter) => parameters.contains(&parameter),
        CatalogTypeExpression::Named(id) => {
            standard_type(id.as_str()).is_some_and(|definition| definition.parameters == 0)
        }
        CatalogTypeExpression::Apply {
            constructor,
            arguments,
        } => standard_type(constructor.as_str()).is_some_and(|definition| {
            usize::from(definition.parameters) == arguments.len()
                && definition.parameters != 0
                && arguments
                    .iter()
                    .all(|argument| valid_type_expression(*argument, parameters))
        }),
    }
}

fn expression_contains_parameter(
    expression: CatalogTypeExpression,
    parameter: Id<'static>,
) -> bool {
    match expression {
        CatalogTypeExpression::Parameter(candidate) => candidate == parameter,
        CatalogTypeExpression::Named(_) => false,
        CatalogTypeExpression::Apply { arguments, .. } => arguments
            .iter()
            .any(|argument| expression_contains_parameter(*argument, parameter)),
    }
}
