#![no_std]

//! Concrete, allocator-free standard-node catalog.
//!
//! Entries are ordinary [`conduit_core::NodeContract`] values plus exact
//! plan-visible behavior and resource facts. This crate contains no executor,
//! registry, host framework, ambient authority, or domain profile.

use conduit_core::{
    ConfigContract, ConfigFieldContract, ConfigIdentity, ConfigMutability, ConfigRequirement,
    ConnectionCardinality, Delivery, Direction, Id, LossAcceptance, NodeContract, PortContract,
    PortFlowConstraints, Presence, SemanticHash, Sensitivity, TemporalContract, TerminalContract,
    TypeContractRef, ValueCardinality,
};

mod conformance;

pub use conformance::{
    ConformanceError, DeterministicProvider, FixtureClass, FixtureOutcome, HostedProvider,
    NormalizedEvidence, ProviderProfile, ReferenceProvider, run_catalog_fixture,
};

/// Catalog schema consumed by manifests and conformance tooling.
pub const STANDARD_CATALOG_SCHEMA_VERSION: u32 = 1;

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
    InvalidPort,
    UnboundedWork,
    MissingStateBound,
    MissingTimer,
    MissingPendingBound,
    MissingHostService,
    ImplicitTypeChange,
    MissingReferenceProvider,
}

const BYTES: TypeContractRef<'static> = type_ref("conduit.std/bytes", 1);
const BOOL: TypeContractRef<'static> = type_ref("conduit.std/bool", 2);
const U64: TypeContractRef<'static> = type_ref("conduit.std/u64", 3);
const RECORD: TypeContractRef<'static> = type_ref("conduit.std/record", 4);
const REFERENCE: TypeContractRef<'static> = type_ref("conduit.std/reference", 5);

const fn type_ref(id: &'static str, seed: u8) -> TypeContractRef<'static> {
    TypeContractRef {
        contract_id: Id(id),
        schema_version: 1,
        semantic_hash: SemanticHash::from_bytes([seed; 32]),
    }
}

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

const IN_BYTES: PortContract<'static> = port(
    "in",
    Direction::Input,
    BYTES,
    ValueCardinality::ZeroOrMore,
    TerminalContract::Either,
);
const OUT_BYTES: PortContract<'static> = port(
    "out",
    Direction::Output,
    BYTES,
    ValueCardinality::ZeroOrMore,
    TerminalContract::Either,
);
const OUT_FINITE: PortContract<'static> = port(
    "out",
    Direction::Output,
    BYTES,
    ValueCardinality::ZeroOrMore,
    TerminalContract::Finite,
);
const OUT_OPEN: PortContract<'static> = port(
    "out",
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
    "out",
    Direction::Output,
    RECORD,
    ValueCardinality::ZeroOrMore,
    TerminalContract::Either,
);
const CONTROL: PortContract<'static> = port(
    "control",
    Direction::Input,
    RECORD,
    ValueCardinality::ZeroOrMore,
    TerminalContract::Either,
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
const TRANSFORM: ConfigContract<'static> = ConfigContract {
    fields: &[
        field("implementation", REFERENCE),
        field("maximum_outputs_per_input", U64),
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
        field("grant", REFERENCE),
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

macro_rules! host_entry {
    ($id:literal, $family:ident, $service:literal) => {{
        let mut value = entry!(
            $id,
            $family,
            HOSTED,
            &[IN_BYTES],
            &[OUT_RECORD],
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
        "conduit.std/literal",
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
        "conduit.std/empty",
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
        "conduit.std/never",
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
        "conduit.std/finite-sequence",
        Source,
        BOUNDED,
        &[],
        &[OUT_FINITE],
        ProducesDeclaredType,
        None,
        BUFFERED,
        PURE
    ),
    entry!(
        "conduit.std/identity",
        Structural,
        EMPTY,
        &[IN_BYTES],
        &[OUT_BYTES],
        Preserving,
        None,
        FINITE,
        PURE
    ),
    entry!(
        "conduit.std/tee",
        Structural,
        BOUNDED,
        &[IN_BYTES],
        &[OUT_BYTES, OUT_BYTES],
        Preserving,
        None,
        BUFFERED,
        PURE
    ),
    entry!(
        "conduit.std/merge",
        Structural,
        BOUNDED,
        &[IN_BYTES, IN_BYTES],
        &[OUT_BYTES],
        Preserving,
        None,
        BUFFERED,
        PURE
    ),
    entry!(
        "conduit.std/zip",
        Structural,
        BOUNDED,
        &[IN_BYTES, IN_BYTES],
        &[OUT_RECORD],
        ExplicitAdapter,
        None,
        BUFFERED,
        PURE
    ),
    entry!(
        "conduit.std/combine-latest",
        Structural,
        BOUNDED,
        &[IN_BYTES, IN_BYTES],
        &[OUT_RECORD],
        ExplicitAdapter,
        None,
        BUFFERED,
        PURE
    ),
    entry!(
        "conduit.std/mux",
        Structural,
        BOUNDED,
        &[IN_BYTES, CONTROL],
        &[OUT_BYTES],
        Preserving,
        None,
        BUFFERED,
        PURE
    ),
    entry!(
        "conduit.std/demux",
        Structural,
        BOUNDED,
        &[IN_BYTES, CONTROL],
        &[OUT_BYTES, OUT_BYTES],
        Preserving,
        None,
        BUFFERED,
        PURE
    ),
    entry!(
        "conduit.std/keyed-dispatch",
        Structural,
        BOUNDED,
        &[IN_BYTES, CONTROL],
        &[OUT_BYTES],
        Preserving,
        None,
        BUFFERED,
        PURE
    ),
    entry!(
        "conduit.std/select",
        Structural,
        BOUNDED,
        &[IN_BYTES, IN_BYTES, CONTROL],
        &[OUT_BYTES],
        Preserving,
        None,
        BUFFERED,
        PURE
    ),
    entry!(
        "conduit.std/gate",
        Structural,
        EMPTY,
        &[IN_BYTES, CONTROL],
        &[OUT_BYTES],
        Preserving,
        None,
        FINITE,
        PURE
    ),
    entry!(
        "conduit.std/switch",
        Structural,
        BOUNDED,
        &[IN_BYTES, IN_BYTES, CONTROL],
        &[OUT_BYTES],
        Preserving,
        None,
        BUFFERED,
        PURE
    ),
    entry!(
        "conduit.std/fallback",
        Structural,
        BOUNDED,
        &[IN_BYTES, IN_BYTES, CONTROL],
        &[OUT_BYTES],
        Preserving,
        None,
        BUFFERED,
        PURE
    ),
    entry!(
        "conduit.std/feedback-delay",
        Structural,
        BOUNDED,
        &[IN_BYTES],
        &[OUT_BYTES],
        Preserving,
        Monotonic,
        TIMERS,
        PURE
    ),
    entry!(
        "conduit.std/discard",
        Sink,
        EMPTY,
        &[IN_BYTES],
        &[],
        Preserving,
        None,
        FINITE,
        PURE
    ),
    entry!(
        "conduit.std/collect",
        Sink,
        BOUNDED,
        &[IN_BYTES],
        &[OUT_RECORD],
        ExplicitAdapter,
        None,
        BUFFERED,
        PURE
    ),
    entry!(
        "conduit.std/first",
        Sink,
        BOUNDED,
        &[IN_BYTES],
        &[OUT_FINITE],
        Preserving,
        None,
        BUFFERED,
        PURE
    ),
    entry!(
        "conduit.std/last",
        Sink,
        BOUNDED,
        &[IN_BYTES],
        &[OUT_FINITE],
        Preserving,
        None,
        BUFFERED,
        PURE
    ),
    entry!(
        "conduit.std/count",
        Sink,
        BOUNDED,
        &[IN_BYTES],
        &[OUT_U64],
        ExplicitAdapter,
        None,
        BUFFERED,
        PURE
    ),
    entry!(
        "conduit.std/map",
        Transform,
        TRANSFORM,
        &[IN_BYTES],
        &[OUT_RECORD],
        ExplicitAdapter,
        None,
        BUFFERED,
        PURE
    ),
    entry!(
        "conduit.std/filter",
        Transform,
        TRANSFORM,
        &[IN_BYTES],
        &[OUT_BYTES],
        Preserving,
        None,
        BUFFERED,
        PURE
    ),
    entry!(
        "conduit.std/filter-map",
        Transform,
        TRANSFORM,
        &[IN_BYTES],
        &[OUT_RECORD],
        ExplicitAdapter,
        None,
        BUFFERED,
        PURE
    ),
    entry!(
        "conduit.std/flat-map",
        Transform,
        TRANSFORM,
        &[IN_BYTES],
        &[OUT_RECORD],
        ExplicitAdapter,
        None,
        BUFFERED,
        PURE
    ),
    entry!(
        "conduit.std/fold",
        Transform,
        STATEFUL,
        &[IN_BYTES],
        &[OUT_RECORD],
        ExplicitAdapter,
        None,
        BUFFERED,
        PURE
    ),
    entry!(
        "conduit.std/validate",
        Transform,
        TRANSFORM,
        &[IN_BYTES],
        &[OUT_BOOL],
        ExplicitAdapter,
        None,
        BUFFERED,
        PURE
    ),
    entry!(
        "conduit.std/adapter",
        Transform,
        TRANSFORM,
        &[IN_BYTES],
        &[OUT_RECORD],
        ExplicitAdapter,
        None,
        BUFFERED,
        PURE
    ),
    entry!(
        "conduit.std/encode",
        Transform,
        TRANSFORM,
        &[IN_BYTES],
        &[OUT_RECORD],
        ExplicitAdapter,
        None,
        BUFFERED,
        PURE
    ),
    entry!(
        "conduit.std/decode",
        Transform,
        TRANSFORM,
        &[IN_BYTES],
        &[OUT_RECORD],
        ExplicitAdapter,
        None,
        BUFFERED,
        PURE
    ),
    entry!(
        "conduit.std/frame",
        Transform,
        TRANSFORM,
        &[IN_BYTES],
        &[OUT_RECORD],
        ExplicitAdapter,
        None,
        BUFFERED,
        PURE
    ),
    entry!(
        "conduit.std/deframe",
        Transform,
        TRANSFORM,
        &[IN_BYTES],
        &[OUT_RECORD],
        ExplicitAdapter,
        None,
        BUFFERED,
        PURE
    ),
    entry!(
        "conduit.std/timer",
        Time,
        TIMED,
        &[],
        &[OUT_U64],
        ProducesDeclaredType,
        Monotonic,
        TIMERS,
        PURE
    ),
    entry!(
        "conduit.std/ticker",
        Time,
        TIMED,
        &[],
        &[OUT_U64],
        ProducesDeclaredType,
        Monotonic,
        TIMERS,
        PURE
    ),
    entry!(
        "conduit.std/delay",
        Time,
        TIMED,
        &[IN_BYTES],
        &[OUT_BYTES],
        Preserving,
        Monotonic,
        TIMERS,
        PURE
    ),
    entry!(
        "conduit.std/deadline",
        Time,
        TIMED,
        &[IN_BYTES],
        &[OUT_BYTES],
        Preserving,
        Monotonic,
        TIMERS,
        PURE
    ),
    entry!(
        "conduit.std/timeout",
        Time,
        TIMED,
        &[IN_BYTES],
        &[OUT_BYTES],
        Preserving,
        Monotonic,
        TIMERS,
        PURE
    ),
    entry!(
        "conduit.std/debounce",
        Time,
        TIMED,
        &[IN_BYTES],
        &[OUT_BYTES],
        Preserving,
        Monotonic,
        TIMERS,
        PURE
    ),
    entry!(
        "conduit.std/throttle",
        Time,
        TIMED,
        &[IN_BYTES],
        &[OUT_BYTES],
        Preserving,
        Monotonic,
        TIMERS,
        PURE
    ),
    entry!(
        "conduit.std/sample",
        Time,
        TIMED,
        &[IN_BYTES],
        &[OUT_BYTES],
        Preserving,
        Monotonic,
        TIMERS,
        PURE
    ),
    entry!(
        "conduit.std/rate-limit",
        Time,
        TIMED,
        &[IN_BYTES],
        &[OUT_BYTES],
        Preserving,
        Monotonic,
        TIMERS,
        PURE
    ),
    entry!(
        "conduit.std/window",
        Time,
        TIMED,
        &[IN_BYTES],
        &[OUT_RECORD],
        ExplicitAdapter,
        DomainEvent,
        TIMERS,
        PURE
    ),
    entry!(
        "conduit.std/batch",
        Time,
        TIMED,
        &[IN_BYTES],
        &[OUT_RECORD],
        ExplicitAdapter,
        Monotonic,
        TIMERS,
        PURE
    ),
    entry!(
        "conduit.std/cell",
        State,
        STATEFUL,
        &[IN_BYTES, CONTROL],
        &[OUT_BYTES],
        Preserving,
        None,
        BUFFERED,
        PURE
    ),
    entry!(
        "conduit.std/counter",
        State,
        STATEFUL,
        &[IN_BYTES, CONTROL],
        &[OUT_U64],
        ExplicitAdapter,
        None,
        BUFFERED,
        PURE
    ),
    entry!(
        "conduit.std/state-machine",
        State,
        STATEFUL,
        &[IN_BYTES, CONTROL],
        &[OUT_RECORD],
        ExplicitAdapter,
        None,
        BUFFERED,
        PURE
    ),
    entry!(
        "conduit.std/deduplicate",
        State,
        STATEFUL,
        &[IN_BYTES],
        &[OUT_BYTES],
        Preserving,
        None,
        BUFFERED,
        PURE
    ),
    entry!(
        "conduit.std/cache",
        State,
        STATEFUL,
        &[IN_BYTES, CONTROL],
        &[OUT_BYTES],
        Preserving,
        None,
        BUFFERED,
        PURE
    ),
    entry!(
        "conduit.std/history",
        State,
        STATEFUL,
        &[IN_BYTES, CONTROL],
        &[OUT_RECORD],
        ExplicitAdapter,
        None,
        BUFFERED,
        PURE
    ),
    entry!(
        "conduit.std/checkpoint",
        State,
        STATEFUL,
        &[IN_BYTES, CONTROL],
        &[OUT_RECORD],
        ExplicitAdapter,
        None,
        BUFFERED,
        PURE
    ),
    entry!(
        "conduit.std/retry",
        Supervision,
        STATEFUL,
        &[IN_BYTES, CONTROL],
        &[OUT_BYTES],
        Preserving,
        Monotonic,
        RETRY,
        PURE
    ),
    entry!(
        "conduit.std/backoff",
        Supervision,
        TIMED,
        &[IN_BYTES, CONTROL],
        &[OUT_BYTES],
        Preserving,
        Monotonic,
        RETRY,
        PURE
    ),
    entry!(
        "conduit.std/circuit-breaker",
        Supervision,
        STATEFUL,
        &[IN_BYTES, CONTROL],
        &[OUT_BYTES],
        Preserving,
        Monotonic,
        RETRY,
        PURE
    ),
    entry!(
        "conduit.std/supervisor",
        Supervision,
        STATEFUL,
        &[CONTROL],
        &[OUT_RECORD],
        ExplicitAdapter,
        Monotonic,
        RETRY,
        PURE
    ),
    entry!(
        "conduit.std/health-gate",
        Supervision,
        STATEFUL,
        &[IN_BYTES, CONTROL],
        &[OUT_BYTES],
        Preserving,
        Monotonic,
        RETRY,
        PURE
    ),
    entry!(
        "conduit.std/terminal-projection",
        Supervision,
        BOUNDED,
        &[CONTROL],
        &[OUT_RECORD],
        ExplicitAdapter,
        None,
        BUFFERED,
        PURE
    ),
    entry!(
        "conduit.std/operator-action",
        Supervision,
        BOUNDED,
        &[CONTROL],
        &[OUT_RECORD],
        ExplicitAdapter,
        None,
        BUFFERED,
        PURE
    ),
    entry!(
        "conduit.std/worker-pool",
        Supervision,
        STATEFUL,
        &[IN_BYTES, CONTROL],
        &[OUT_BYTES],
        Preserving,
        None,
        BUFFERED,
        PURE
    ),
    entry!(
        "conduit.std/probe",
        Testing,
        BOUNDED,
        &[IN_BYTES],
        &[OUT_BYTES, OUT_RECORD],
        ExplicitAdapter,
        None,
        BUFFERED,
        PURE
    ),
    entry!(
        "conduit.std/assertion",
        Testing,
        TRANSFORM,
        &[IN_BYTES],
        &[OUT_BYTES, OUT_BOOL],
        ExplicitAdapter,
        None,
        BUFFERED,
        PURE
    ),
    entry!(
        "conduit.std/fault-source",
        Testing,
        BOUNDED,
        &[],
        &[OUT_RECORD],
        ProducesDeclaredType,
        Monotonic,
        TIMERS,
        PURE
    ),
    entry!(
        "conduit.std/record",
        Testing,
        STATEFUL,
        &[IN_BYTES],
        &[OUT_BYTES, OUT_RECORD],
        ExplicitAdapter,
        None,
        BUFFERED,
        PURE
    ),
    entry!(
        "conduit.std/replay",
        Testing,
        STATEFUL,
        &[CONTROL],
        &[OUT_BYTES],
        ProducesDeclaredType,
        DomainEvent,
        TIMERS,
        PURE
    ),
    host_entry!("conduit.std/file-read", Boundary, "host/filesystem-read"),
    host_entry!("conduit.std/file-write", Boundary, "host/filesystem-write"),
    host_entry!("conduit.std/file-watch", Boundary, "host/filesystem-watch"),
    host_entry!("conduit.std/blob-read", Boundary, "host/blob-read"),
    host_entry!("conduit.std/blob-write", Boundary, "host/blob-write"),
    host_entry!("conduit.std/key-value", Boundary, "host/key-value"),
    host_entry!("conduit.std/process", Boundary, "host/process"),
    host_entry!(
        "conduit.std/process-stream",
        Boundary,
        "host/process-stream"
    ),
    host_entry!(
        "conduit.std/secret-reference",
        Boundary,
        "host/secret-resolution"
    ),
    host_entry!("conduit.std/crypto", Boundary, "host/cryptography"),
    host_entry!("conduit.std/compression", Boundary, "host/compression"),
    host_entry!("conduit.std/http-client", Boundary, "host/http-client"),
    host_entry!("conduit.std/http-server", Boundary, "host/http-server"),
    host_entry!("conduit.std/websocket", Boundary, "host/websocket"),
    host_entry!("conduit.std/sse", Boundary, "host/sse"),
    host_entry!("conduit.std/zenoh", Boundary, "host/distributed-transport"),
    host_entry!(
        "conduit.std/evidence-export",
        Boundary,
        "host/evidence-export"
    ),
    host_entry!(
        "conduit.std/network-observe",
        Network,
        "host/network-observe"
    ),
    host_entry!("conduit.std/wifi-scan", Network, "host/wifi-scan"),
    host_entry!("conduit.std/wifi-join", Network, "host/wifi-station"),
    host_entry!(
        "conduit.std/wifi-access-point",
        Network,
        "host/wifi-access-point"
    ),
    host_entry!("conduit.std/static-address", Network, "host/address-config"),
    host_entry!("conduit.std/dhcp-client", Network, "host/dhcp-client"),
    host_entry!("conduit.std/dhcp-server", Network, "host/dhcp-server"),
    host_entry!("conduit.std/dns", Network, "host/dns"),
    host_entry!("conduit.std/route", Network, "host/route"),
    host_entry!("conduit.std/bridge", Network, "host/bridge"),
    host_entry!("conduit.std/nat", Network, "host/nat"),
    host_entry!("conduit.std/reachability", Network, "host/reachability"),
    host_entry!("conduit.std/tcp", Network, "host/tcp"),
    host_entry!("conduit.std/udp", Network, "host/udp"),
];

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
