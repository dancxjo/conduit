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
mod text_format;
mod text_lines_join;
mod types;

pub use conformance::{
    ConformanceError, DeterministicProvider, FixtureClass, FixtureOutcome, HostedProvider,
    NormalizedEvidence, ProviderProfile, ReferenceProvider, run_catalog_fixture,
};
pub use data_boundaries::{
    DATA_MAX_FIELD_NAME_BYTES, DATA_MAX_FIELD_VALUE_BYTES, DATA_MAX_FRAME_BYTES,
    DATA_MAX_RECORD_FIELDS, DataBoundaryError, LENGTH_U32BE_PREFIX_BYTES, LengthU32BeDecoder,
    RequiredField, StructuralDecision, StructuralField, StructuralRejection, decode_utf8,
    encode_length_u32be, encode_utf8, validate_closed_record,
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
const REFERENCE: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("std/reference/any"),
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([
        0xb9, 0xbb, 0x20, 0x9f, 0x2c, 0xec, 0x4e, 0xaa, 0x98, 0x5d, 0x81, 0xcc, 0x69, 0xf5, 0x23,
        0x7e, 0x40, 0x35, 0x6f, 0x88, 0x33, 0x36, 0x00, 0xb8, 0x33, 0x04, 0x86, 0x9b, 0x93, 0x57,
        0x1b, 0xc7,
    ]),
};
const SOCKET_ADDRESS: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("net/socket/address"),
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([
        0x9e, 0xef, 0x61, 0xf8, 0x0f, 0x02, 0x36, 0x17, 0xe4, 0x68, 0x8d, 0x0c, 0xca, 0xf9, 0x54,
        0xec, 0x2a, 0x12, 0xfb, 0xe8, 0xca, 0x25, 0xec, 0xc5, 0x40, 0xee, 0x35, 0x2a, 0xea, 0xa1,
        0x3a, 0x1f,
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
        "flow/validate",
        Transform,
        TRANSFORM,
        &[named("candidate", IN_BYTES)],
        &[OUT_BOOL],
        ExplicitAdapter,
        None,
        BUFFERED,
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
        "flow/encode",
        Transform,
        TRANSFORM,
        &[named("value", IN_BYTES)],
        &[named("encoded", OUT_RECORD)],
        ExplicitAdapter,
        None,
        BUFFERED,
        PURE
    ),
    entry!(
        "flow/decode",
        Transform,
        TRANSFORM,
        &[named("encoded", IN_BYTES)],
        &[named("decoded", OUT_RECORD)],
        ExplicitAdapter,
        None,
        BUFFERED,
        PURE
    ),
    entry!(
        "flow/frame",
        Transform,
        TRANSFORM,
        &[named("bytes", IN_BYTES)],
        &[named("frame", OUT_RECORD)],
        ExplicitAdapter,
        None,
        BUFFERED,
        PURE
    ),
    entry!(
        "flow/deframe",
        Transform,
        TRANSFORM,
        &[named("frame", IN_BYTES)],
        &[named("bytes", OUT_RECORD)],
        ExplicitAdapter,
        None,
        BUFFERED,
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
        TIMED,
        &[IN_BYTES],
        &[OUT_BYTES],
        Preserving,
        Monotonic,
        TIMERS,
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
    entry!(
        "time/timeout",
        Time,
        TIMED,
        &[named("item", IN_BYTES)],
        &[named("completed", OUT_BYTES)],
        Preserving,
        Monotonic,
        TIMERS,
        PURE
    ),
    entry!(
        "time/debounce",
        Time,
        TIMED,
        &[named("event", IN_BYTES)],
        &[named("settled", OUT_BYTES)],
        Preserving,
        Monotonic,
        TIMERS,
        PURE
    ),
    entry!(
        "time/throttle",
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
        STATEFUL,
        &[named("update", IN_BYTES), named("command", CONTROL)],
        &[named("current", OUT_BYTES)],
        Preserving,
        None,
        BUFFERED,
        PURE,
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
    entry!(
        "state/deduplicate",
        State,
        STATEFUL,
        &[named("candidate", IN_BYTES)],
        &[named("unique", OUT_BYTES)],
        Preserving,
        None,
        BUFFERED,
        PURE
    ),
    entry!(
        "state/cache",
        State,
        STATEFUL,
        &[named("request", IN_BYTES), named("command", CONTROL)],
        &[named("response", OUT_BYTES)],
        Preserving,
        None,
        BUFFERED,
        PURE
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
    entry!(
        "supervision/retry",
        Supervision,
        STATEFUL,
        &[named("request", IN_BYTES), named("command", CONTROL)],
        &[named("attempt", OUT_BYTES)],
        Preserving,
        Monotonic,
        RETRY,
        PURE
    ),
    entry!(
        "supervision/backoff",
        Supervision,
        TIMED,
        &[named("request", IN_BYTES), named("command", CONTROL)],
        &[named("ready", OUT_BYTES)],
        Preserving,
        Monotonic,
        RETRY,
        PURE
    ),
    entry!(
        "supervision/circuit-breaker",
        Supervision,
        STATEFUL,
        &[named("request", IN_BYTES), named("command", CONTROL)],
        &[named("admitted", OUT_BYTES)],
        Preserving,
        Monotonic,
        RETRY,
        PURE
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
    host_entry!(
        "fs/read",
        Boundary,
        "path",
        "contents",
        "host/filesystem-read"
    ),
    host_entry!(
        "fs/write",
        Boundary,
        "write",
        "receipt",
        "host/filesystem-write"
    ),
    host_entry!(
        "fs/watch",
        Boundary,
        "path",
        "event",
        "host/filesystem-watch"
    ),
    host_entry!(
        "storage/blob/read",
        Boundary,
        "reference",
        "blob",
        "host/blob-read"
    ),
    host_entry!(
        "storage/blob/write",
        Boundary,
        "blob",
        "reference",
        "host/blob-write"
    ),
    host_entry!(
        "storage/key-value",
        Boundary,
        "operation",
        "result",
        "host/key-value"
    ),
    host_entry!(
        "process/run",
        Boundary,
        "invocation",
        "completion",
        "host/process"
    ),
    host_entry!(
        "process/stream",
        Boundary,
        "stdin",
        "event",
        "host/process-stream"
    ),
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
    host_entry!(
        "net/tcp/socket",
        Network,
        "transmit",
        "received",
        "host/tcp"
    ),
    host_entry!(
        "net/udp/socket",
        Network,
        "datagram",
        "datagram",
        "host/udp"
    ),
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
