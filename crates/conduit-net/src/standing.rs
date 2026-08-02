//! Standing packet, service, and observation node contracts.

use conduit_core::{
    ArtifactDigest, ConfigContract, ConfigFieldContract, ConfigIdentity, ConfigMutability,
    ConfigRequirement, ConnectionCardinality, Delivery, Direction, ExecutorKind, Id,
    LossAcceptance, NodeContract, PinnedDescriptor, PlanResourceBudget, PortContract,
    PortFlowConstraints, Presence, SemanticHash, Sensitivity, StopPolicy, TemporalContract,
    TerminalContract, TypeContractRef, ValueCardinality,
};
use conduit_panel::{Node, SourceValue};
use conduit_runtime::{
    CompiledInHostService, ExactHostedServiceBinding, Handler, HostedServiceCleanup,
    HostedServiceInterest, HostedServiceStep, HostedServiceStepContext,
    InstalledArtifactRegistration, InstalledCapabilityRequirement,
    InstalledImplementationRegistration, Registry, RegistryError, ResolutionError, RunIo,
    RuntimeError, Value,
};
use sha2::{Digest as _, Sha256};

use crate::{
    ADDRESS_STATE_TYPE, AddressFamily, AddressReadiness, BYTE_STREAM_TYPE, ByteStreamChunk,
    CONTROL_EVENT_TYPE, DATAGRAM_TYPE, DHCP_LEASE_TYPE, DatagramDelivery, EvidenceKind,
    EvidenceLog, FRAME_TYPE, Ipv4Address, LINK_OBSERVATION_TYPE, LeasePhase, LinkKind,
    LinkObservation, MAXIMUM_DATAGRAM_BYTES, MAXIMUM_EVIDENCE_EVENTS, MAXIMUM_FRAME_BYTES,
    MAXIMUM_PACKET_BYTES, MAXIMUM_ROUTES, MAXIMUM_SESSIONS, MAXIMUM_STREAM_CHUNK_BYTES,
    NetworkAddressState, NetworkAvailability, NetworkControlEvent, NetworkControlKind,
    NetworkControlOutcome, NetworkDatagram, NetworkDhcpLease, NetworkDirection, NetworkFrame,
    NetworkPacket, NetworkReachabilityObservation, NetworkReason, NetworkServiceRegistration,
    PACKET_TYPE, PacketDisposition, REACHABILITY_OBSERVATION_TYPE, RETAINED_NETWORK_STATE_TYPE,
    ROUTE_STATE_TYPE, ReachabilityOutcome, ReachabilityScope, RetainedNetworkState,
    RetainedStatePolicy, RouteEntry, RouteTable, SERVICE_REGISTRATION_TYPE, SESSION_TYPE,
    SessionAdmission, SessionLifecycle, SessionTable, StreamPressure, TransportProtocol,
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

const fn config_field(
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

const fn stream_port(
    id: &'static str,
    direction: Direction,
    value_type: TypeContractRef<'static>,
    sensitivity: Sensitivity,
    loss: LossAcceptance,
) -> PortContract<'static> {
    PortContract {
        id: Id(id),
        direction,
        value_type,
        presence: Presence::Required,
        connections: ConnectionCardinality::ExactlyOne,
        values: ValueCardinality::ZeroOrMore,
        delivery: Delivery::Stream,
        temporal: TemporalContract::Committed,
        terminal: if matches!(direction, Direction::Input) {
            TerminalContract::Either
        } else {
            TerminalContract::OpenEnded
        },
        sensitivity,
        flow: PortFlowConstraints { loss },
    }
}

const fn state_port(
    id: &'static str,
    value_type: TypeContractRef<'static>,
    sensitivity: Sensitivity,
) -> PortContract<'static> {
    PortContract {
        id: Id(id),
        direction: Direction::Output,
        value_type,
        presence: Presence::Optional,
        connections: ConnectionCardinality::ZeroOrMore,
        values: ValueCardinality::ZeroOrMore,
        delivery: Delivery::LatestState,
        temporal: TemporalContract::RetainedState,
        terminal: TerminalContract::OpenEnded,
        sensitivity,
        flow: PortFlowConstraints {
            loss: LossAcceptance::TypeContractDefined,
        },
    }
}

const fn optional_stream_port(
    id: &'static str,
    direction: Direction,
    value_type: TypeContractRef<'static>,
    sensitivity: Sensitivity,
    loss: LossAcceptance,
) -> PortContract<'static> {
    PortContract {
        id: Id(id),
        direction,
        value_type,
        presence: Presence::Optional,
        connections: ConnectionCardinality::ZeroOrMore,
        values: ValueCardinality::ZeroOrMore,
        delivery: Delivery::Stream,
        temporal: TemporalContract::Committed,
        terminal: if matches!(direction, Direction::Input) {
            TerminalContract::Either
        } else {
            TerminalContract::OpenEnded
        },
        sensitivity,
        flow: PortFlowConstraints { loss },
    }
}

const fn state_input_port(
    id: &'static str,
    value_type: TypeContractRef<'static>,
    sensitivity: Sensitivity,
) -> PortContract<'static> {
    PortContract {
        id: Id(id),
        direction: Direction::Input,
        value_type,
        presence: Presence::Required,
        connections: ConnectionCardinality::ExactlyOne,
        values: ValueCardinality::ZeroOrMore,
        delivery: Delivery::LatestState,
        temporal: TemporalContract::RetainedState,
        terminal: TerminalContract::Either,
        sensitivity,
        flow: PortFlowConstraints {
            loss: LossAcceptance::TypeContractDefined,
        },
    }
}

const LINK_FIELDS: [ConfigFieldContract<'static>; 9] = [
    config_field("lifecycle", TEXT_TYPE),
    config_field("interface", U64_TYPE),
    config_field("kind", TEXT_TYPE),
    config_field("carrier", TEXT_TYPE),
    config_field("address", TEXT_TYPE),
    config_field("mtu", U64_TYPE),
    config_field("period_ticks", U64_TYPE),
    config_field("freshness_ticks", U64_TYPE),
    config_field("maximum_evidence_events", U64_TYPE),
];
const PACKET_SOURCE_FIELDS: [ConfigFieldContract<'static>; 9] = [
    config_field("lifecycle", TEXT_TYPE),
    config_field("source", TEXT_TYPE),
    config_field("destination", TEXT_TYPE),
    config_field("hop_limit", U64_TYPE),
    config_field("payload_bytes", U64_TYPE),
    config_field("period_ticks", U64_TYPE),
    config_field("maximum_packets_per_step", U64_TYPE),
    config_field("maximum_packet_bytes", U64_TYPE),
    config_field("maximum_evidence_events", U64_TYPE),
];
const FRAME_SOURCE_FIELDS: [ConfigFieldContract<'static>; 6] = [
    config_field("lifecycle", TEXT_TYPE),
    config_field("interface", U64_TYPE),
    config_field("period_ticks", U64_TYPE),
    config_field("payload_bytes", U64_TYPE),
    config_field("maximum_frame_bytes", U64_TYPE),
    config_field("maximum_evidence_events", U64_TYPE),
];
const FRAME_SINK_FIELDS: [ConfigFieldContract<'static>; 4] = [
    config_field("lifecycle", TEXT_TYPE),
    config_field("maximum_frames_per_step", U64_TYPE),
    config_field("maximum_retained_items", U64_TYPE),
    config_field("maximum_evidence_events", U64_TYPE),
];
const DATAGRAM_SOURCE_FIELDS: [ConfigFieldContract<'static>; 8] = [
    config_field("lifecycle", TEXT_TYPE),
    config_field("source_port", U64_TYPE),
    config_field("destination_port", U64_TYPE),
    config_field("period_ticks", U64_TYPE),
    config_field("payload_bytes", U64_TYPE),
    config_field("maximum_datagram_bytes", U64_TYPE),
    config_field("maximum_datagrams_per_step", U64_TYPE),
    config_field("maximum_evidence_events", U64_TYPE),
];
const DATAGRAM_IMPAIR_FIELDS: [ConfigFieldContract<'static>; 4] = [
    config_field("lifecycle", TEXT_TYPE),
    config_field("pattern", TEXT_TYPE),
    config_field("maximum_datagram_bytes", U64_TYPE),
    config_field("maximum_evidence_events", U64_TYPE),
];
const DATAGRAM_SINK_FIELDS: [ConfigFieldContract<'static>; 4] = [
    config_field("lifecycle", TEXT_TYPE),
    config_field("maximum_datagrams_per_step", U64_TYPE),
    config_field("maximum_retained_items", U64_TYPE),
    config_field("maximum_evidence_events", U64_TYPE),
];
const STREAM_SOURCE_FIELDS: [ConfigFieldContract<'static>; 6] = [
    config_field("lifecycle", TEXT_TYPE),
    config_field("session", U64_TYPE),
    config_field("period_ticks", U64_TYPE),
    config_field("chunk_bytes", U64_TYPE),
    config_field("maximum_chunk_bytes", U64_TYPE),
    config_field("maximum_evidence_events", U64_TYPE),
];
const STREAM_SINK_FIELDS: [ConfigFieldContract<'static>; 4] = [
    config_field("lifecycle", TEXT_TYPE),
    config_field("maximum_chunks_per_step", U64_TYPE),
    config_field("maximum_retained_items", U64_TYPE),
    config_field("maximum_evidence_events", U64_TYPE),
];
const CLASSIFY_FIELDS: [ConfigFieldContract<'static>; 5] = [
    config_field("lifecycle", TEXT_TYPE),
    config_field("admitted_prefix", TEXT_TYPE),
    config_field("prefix_length", U64_TYPE),
    config_field("maximum_packet_bytes", U64_TYPE),
    config_field("maximum_evidence_events", U64_TYPE),
];
const ROUTE_FIELDS: [ConfigFieldContract<'static>; 9] = [
    config_field("lifecycle", TEXT_TYPE),
    config_field("prefix", TEXT_TYPE),
    config_field("prefix_length", U64_TYPE),
    config_field("egress_interface", U64_TYPE),
    config_field("mtu", U64_TYPE),
    config_field("forwarding", TEXT_TYPE),
    config_field("maximum_routes", U64_TYPE),
    config_field("maximum_packet_bytes", U64_TYPE),
    config_field("maximum_evidence_events", U64_TYPE),
];
const SINK_FIELDS: [ConfigFieldContract<'static>; 4] = [
    config_field("lifecycle", TEXT_TYPE),
    config_field("maximum_packets_per_step", U64_TYPE),
    config_field("maximum_retained_items", U64_TYPE),
    config_field("maximum_evidence_events", U64_TYPE),
];
const LISTENER_FIELDS: [ConfigFieldContract<'static>; 8] = [
    config_field("lifecycle", TEXT_TYPE),
    config_field("transport", TEXT_TYPE),
    config_field("local_port", U64_TYPE),
    config_field("period_ticks", U64_TYPE),
    config_field("session_timeout_ticks", U64_TYPE),
    config_field("maximum_sessions", U64_TYPE),
    config_field("maximum_retained_items", U64_TYPE),
    config_field("maximum_evidence_events", U64_TYPE),
];
const METER_FIELDS: [ConfigFieldContract<'static>; 4] = [
    config_field("lifecycle", TEXT_TYPE),
    config_field("maximum_packets_per_step", U64_TYPE),
    config_field("maximum_retained_items", U64_TYPE),
    config_field("maximum_evidence_events", U64_TYPE),
];
const SERVICE_OBSERVE_FIELDS: [ConfigFieldContract<'static>; 3] = [
    config_field("lifecycle", TEXT_TYPE),
    config_field("maximum_retained_items", U64_TYPE),
    config_field("maximum_evidence_events", U64_TYPE),
];

// Effect contracts remain contract-only here. Their configuration authors a
// requested effect; it is never a provider observation, capability, grant, or
// proof that the effect happened. An executable provider must separately bind
// and revalidate its exact authority at use time.
const LINK_EFFECT_FIELDS: [ConfigFieldContract<'static>; 6] = [
    config_field("lifecycle", TEXT_TYPE),
    config_field("interface", U64_TYPE),
    config_field("profile", TEXT_TYPE),
    config_field("mtu", U64_TYPE),
    config_field("maximum_events_per_step", U64_TYPE),
    config_field("maximum_evidence_events", U64_TYPE),
];
const ADDRESS_EFFECT_FIELDS: [ConfigFieldContract<'static>; 7] = [
    config_field("lifecycle", TEXT_TYPE),
    config_field("interface", U64_TYPE),
    config_field("family", TEXT_TYPE),
    config_field("address", TEXT_TYPE),
    config_field("prefix_length", U64_TYPE),
    config_field("maximum_states", U64_TYPE),
    config_field("maximum_evidence_events", U64_TYPE),
];
const DHCP_CLIENT_EFFECT_FIELDS: [ConfigFieldContract<'static>; 5] = [
    config_field("lifecycle", TEXT_TYPE),
    config_field("interface", U64_TYPE),
    config_field("maximum_leases", U64_TYPE),
    config_field("maximum_packet_bytes", U64_TYPE),
    config_field("maximum_evidence_events", U64_TYPE),
];
const ROUTE_EFFECT_FIELDS: [ConfigFieldContract<'static>; 9] = [
    config_field("lifecycle", TEXT_TYPE),
    config_field("family", TEXT_TYPE),
    config_field("prefix", TEXT_TYPE),
    config_field("prefix_length", U64_TYPE),
    config_field("next_hop", TEXT_TYPE),
    config_field("interface", U64_TYPE),
    config_field("mtu", U64_TYPE),
    config_field("policy", TEXT_TYPE),
    config_field("maximum_evidence_events", U64_TYPE),
];
const FRAME_EFFECT_FIELDS: [ConfigFieldContract<'static>; 5] = [
    config_field("lifecycle", TEXT_TYPE),
    config_field("left_interface", U64_TYPE),
    config_field("right_interface", U64_TYPE),
    config_field("maximum_frame_bytes", U64_TYPE),
    config_field("maximum_evidence_events", U64_TYPE),
];
const PACKET_EFFECT_FIELDS: [ConfigFieldContract<'static>; 5] = [
    config_field("lifecycle", TEXT_TYPE),
    config_field("policy", TEXT_TYPE),
    config_field("maximum_packet_bytes", U64_TYPE),
    config_field("maximum_packets_per_step", U64_TYPE),
    config_field("maximum_evidence_events", U64_TYPE),
];
const DNS_EFFECT_FIELDS: [ConfigFieldContract<'static>; 6] = [
    config_field("lifecycle", TEXT_TYPE),
    config_field("name", TEXT_TYPE),
    config_field("family", TEXT_TYPE),
    config_field("transport", TEXT_TYPE),
    config_field("maximum_results", U64_TYPE),
    config_field("maximum_evidence_events", U64_TYPE),
];
const INTERNET_EFFECT_FIELDS: [ConfigFieldContract<'static>; 5] = [
    config_field("lifecycle", TEXT_TYPE),
    config_field("interface", U64_TYPE),
    config_field("scope", TEXT_TYPE),
    config_field("maximum_observations", U64_TYPE),
    config_field("maximum_evidence_events", U64_TYPE),
];

const LINK_OUTPUTS: [PortContract<'static>; 2] = [
    stream_port(
        "observation",
        Direction::Output,
        LINK_OBSERVATION_TYPE,
        Sensitivity::Public,
        LossAcceptance::TypeContractDefined,
    ),
    stream_port(
        "event",
        Direction::Output,
        CONTROL_EVENT_TYPE,
        Sensitivity::Restricted,
        LossAcceptance::LosslessOnly,
    ),
];
const PACKET_INPUT: [PortContract<'static>; 1] = [stream_port(
    "packet",
    Direction::Input,
    PACKET_TYPE,
    Sensitivity::Restricted,
    LossAcceptance::TypeContractDefined,
)];
const FRAME_INPUT: [PortContract<'static>; 1] = [stream_port(
    "frame",
    Direction::Input,
    FRAME_TYPE,
    Sensitivity::Restricted,
    LossAcceptance::TypeContractDefined,
)];
const FRAME_OUTPUT: [PortContract<'static>; 1] = [stream_port(
    "frame",
    Direction::Output,
    FRAME_TYPE,
    Sensitivity::Restricted,
    LossAcceptance::TypeContractDefined,
)];
const DATAGRAM_INPUT: [PortContract<'static>; 1] = [stream_port(
    "datagram",
    Direction::Input,
    DATAGRAM_TYPE,
    Sensitivity::Restricted,
    LossAcceptance::TypeContractDefined,
)];
const DATAGRAM_OUTPUT: [PortContract<'static>; 1] = [stream_port(
    "datagram",
    Direction::Output,
    DATAGRAM_TYPE,
    Sensitivity::Restricted,
    LossAcceptance::TypeContractDefined,
)];
const STREAM_INPUT: [PortContract<'static>; 1] = [stream_port(
    "chunk",
    Direction::Input,
    BYTE_STREAM_TYPE,
    Sensitivity::Restricted,
    LossAcceptance::LosslessOnly,
)];
const STREAM_OUTPUT: [PortContract<'static>; 1] = [stream_port(
    "chunk",
    Direction::Output,
    BYTE_STREAM_TYPE,
    Sensitivity::Restricted,
    LossAcceptance::LosslessOnly,
)];
const PACKET_OUTPUT: [PortContract<'static>; 1] = [stream_port(
    "packet",
    Direction::Output,
    PACKET_TYPE,
    Sensitivity::Restricted,
    LossAcceptance::TypeContractDefined,
)];
const PACKET_STATE_OUTPUT: [PortContract<'static>; 1] = [state_port(
    "state",
    RETAINED_NETWORK_STATE_TYPE,
    Sensitivity::Restricted,
)];
const SESSION_OUTPUTS: [PortContract<'static>; 3] = [
    stream_port(
        "session",
        Direction::Output,
        SESSION_TYPE,
        Sensitivity::Restricted,
        LossAcceptance::LosslessOnly,
    ),
    stream_port(
        "event",
        Direction::Output,
        CONTROL_EVENT_TYPE,
        Sensitivity::Restricted,
        LossAcceptance::LosslessOnly,
    ),
    state_port(
        "state",
        RETAINED_NETWORK_STATE_TYPE,
        Sensitivity::Restricted,
    ),
];
const SERVICE_OBSERVE_INPUTS: [PortContract<'static>; 3] = [
    stream_port(
        "session",
        Direction::Input,
        SESSION_TYPE,
        Sensitivity::Restricted,
        LossAcceptance::LosslessOnly,
    ),
    stream_port(
        "event",
        Direction::Input,
        CONTROL_EVENT_TYPE,
        Sensitivity::Restricted,
        LossAcceptance::LosslessOnly,
    ),
    state_input_port(
        "state",
        RETAINED_NETWORK_STATE_TYPE,
        Sensitivity::Restricted,
    ),
];
const EFFECT_REQUEST_INPUT: [PortContract<'static>; 1] = [optional_stream_port(
    "request",
    Direction::Input,
    CONTROL_EVENT_TYPE,
    Sensitivity::Restricted,
    LossAcceptance::LosslessOnly,
)];
const LINK_EFFECT_OUTPUTS: [PortContract<'static>; 2] = [
    optional_stream_port(
        "observation",
        Direction::Output,
        LINK_OBSERVATION_TYPE,
        Sensitivity::Public,
        LossAcceptance::TypeContractDefined,
    ),
    optional_stream_port(
        "event",
        Direction::Output,
        CONTROL_EVENT_TYPE,
        Sensitivity::Restricted,
        LossAcceptance::LosslessOnly,
    ),
];
const ADDRESS_EFFECT_OUTPUTS: [PortContract<'static>; 2] = [
    state_port("state", ADDRESS_STATE_TYPE, Sensitivity::Restricted),
    optional_stream_port(
        "event",
        Direction::Output,
        CONTROL_EVENT_TYPE,
        Sensitivity::Restricted,
        LossAcceptance::LosslessOnly,
    ),
];
const DHCP_CLIENT_EFFECT_OUTPUTS: [PortContract<'static>; 3] = [
    optional_stream_port(
        "lease",
        Direction::Output,
        DHCP_LEASE_TYPE,
        Sensitivity::Restricted,
        LossAcceptance::LosslessOnly,
    ),
    state_port(
        "state",
        RETAINED_NETWORK_STATE_TYPE,
        Sensitivity::Restricted,
    ),
    optional_stream_port(
        "event",
        Direction::Output,
        CONTROL_EVENT_TYPE,
        Sensitivity::Restricted,
        LossAcceptance::LosslessOnly,
    ),
];
const ROUTE_EFFECT_OUTPUTS: [PortContract<'static>; 3] = [
    state_port("route", ROUTE_STATE_TYPE, Sensitivity::Restricted),
    state_port(
        "state",
        RETAINED_NETWORK_STATE_TYPE,
        Sensitivity::Restricted,
    ),
    optional_stream_port(
        "event",
        Direction::Output,
        CONTROL_EVENT_TYPE,
        Sensitivity::Restricted,
        LossAcceptance::LosslessOnly,
    ),
];
const FRAME_EFFECT_OUTPUTS: [PortContract<'static>; 2] = [
    optional_stream_port(
        "frame",
        Direction::Output,
        FRAME_TYPE,
        Sensitivity::Restricted,
        LossAcceptance::TypeContractDefined,
    ),
    state_port(
        "state",
        RETAINED_NETWORK_STATE_TYPE,
        Sensitivity::Restricted,
    ),
];
const PACKET_EFFECT_OUTPUTS: [PortContract<'static>; 2] = [
    optional_stream_port(
        "packet",
        Direction::Output,
        PACKET_TYPE,
        Sensitivity::Restricted,
        LossAcceptance::TypeContractDefined,
    ),
    state_port(
        "state",
        RETAINED_NETWORK_STATE_TYPE,
        Sensitivity::Restricted,
    ),
];
const DNS_EFFECT_OUTPUTS: [PortContract<'static>; 2] = [
    optional_stream_port(
        "result",
        Direction::Output,
        SERVICE_REGISTRATION_TYPE,
        Sensitivity::Restricted,
        LossAcceptance::LosslessOnly,
    ),
    optional_stream_port(
        "event",
        Direction::Output,
        CONTROL_EVENT_TYPE,
        Sensitivity::Restricted,
        LossAcceptance::LosslessOnly,
    ),
];
const INTERNET_EFFECT_OUTPUTS: [PortContract<'static>; 2] = [
    optional_stream_port(
        "observation",
        Direction::Output,
        REACHABILITY_OBSERVATION_TYPE,
        Sensitivity::Restricted,
        LossAcceptance::TypeContractDefined,
    ),
    optional_stream_port(
        "event",
        Direction::Output,
        CONTROL_EVENT_TYPE,
        Sensitivity::Restricted,
        LossAcceptance::LosslessOnly,
    ),
];

pub const LINK_OBSERVE_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("net/link/observe"),
    config: ConfigContract {
        fields: &LINK_FIELDS,
    },
    inputs: &[],
    outputs: &LINK_EFFECT_OUTPUTS,
};
pub const FRAME_SOURCE_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("net/frame/source"),
    config: ConfigContract {
        fields: &FRAME_SOURCE_FIELDS,
    },
    inputs: &[],
    outputs: &FRAME_OUTPUT,
};
pub const FRAME_SINK_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("net/frame/sink"),
    config: ConfigContract {
        fields: &FRAME_SINK_FIELDS,
    },
    inputs: &FRAME_INPUT,
    outputs: &PACKET_STATE_OUTPUT,
};
pub const PACKET_SOURCE_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("net/packet/source"),
    config: ConfigContract {
        fields: &PACKET_SOURCE_FIELDS,
    },
    inputs: &[],
    outputs: &PACKET_OUTPUT,
};
pub const PACKET_CLASSIFY_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("net/packet/classify"),
    config: ConfigContract {
        fields: &CLASSIFY_FIELDS,
    },
    inputs: &PACKET_INPUT,
    outputs: &PACKET_OUTPUT,
};
pub const PACKET_ROUTE_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("net/packet/route"),
    config: ConfigContract {
        fields: &ROUTE_FIELDS,
    },
    inputs: &PACKET_INPUT,
    outputs: &PACKET_OUTPUT,
};
pub const PACKET_SINK_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("net/packet/sink"),
    config: ConfigContract {
        fields: &SINK_FIELDS,
    },
    inputs: &PACKET_INPUT,
    outputs: &PACKET_STATE_OUTPUT,
};
pub const DATAGRAM_SOURCE_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("net/datagram/source"),
    config: ConfigContract {
        fields: &DATAGRAM_SOURCE_FIELDS,
    },
    inputs: &[],
    outputs: &DATAGRAM_OUTPUT,
};
pub const DATAGRAM_IMPAIR_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("net/datagram/impair"),
    config: ConfigContract {
        fields: &DATAGRAM_IMPAIR_FIELDS,
    },
    inputs: &DATAGRAM_INPUT,
    outputs: &DATAGRAM_OUTPUT,
};
pub const DATAGRAM_SINK_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("net/datagram/sink"),
    config: ConfigContract {
        fields: &DATAGRAM_SINK_FIELDS,
    },
    inputs: &DATAGRAM_INPUT,
    outputs: &PACKET_STATE_OUTPUT,
};
pub const STREAM_SOURCE_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("net/stream/source"),
    config: ConfigContract {
        fields: &STREAM_SOURCE_FIELDS,
    },
    inputs: &[],
    outputs: &STREAM_OUTPUT,
};
pub const STREAM_SINK_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("net/stream/sink"),
    config: ConfigContract {
        fields: &STREAM_SINK_FIELDS,
    },
    inputs: &STREAM_INPUT,
    outputs: &PACKET_STATE_OUTPUT,
};
pub const SESSION_LISTEN_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("net/session/listen"),
    config: ConfigContract {
        fields: &LISTENER_FIELDS,
    },
    inputs: &[],
    outputs: &SESSION_OUTPUTS,
};
pub const NETWORK_METER_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("net/observe/meter"),
    config: ConfigContract {
        fields: &METER_FIELDS,
    },
    inputs: &PACKET_INPUT,
    outputs: &PACKET_STATE_OUTPUT,
};
pub const SERVICE_OBSERVE_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("net/observe/service"),
    config: ConfigContract {
        fields: &SERVICE_OBSERVE_FIELDS,
    },
    inputs: &SERVICE_OBSERVE_INPUTS,
    outputs: &[],
};

pub const WIFI_JOIN_EFFECT_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("net/wifi/join"),
    config: ConfigContract {
        fields: &LINK_EFFECT_FIELDS,
    },
    inputs: &EFFECT_REQUEST_INPUT,
    outputs: &LINK_EFFECT_OUTPUTS,
};
pub const WIRED_LINK_EFFECT_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("net/link/wired"),
    config: ConfigContract {
        fields: &LINK_EFFECT_FIELDS,
    },
    inputs: &EFFECT_REQUEST_INPUT,
    outputs: &LINK_EFFECT_OUTPUTS,
};
pub const VIRTUAL_LINK_EFFECT_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("net/link/virtual"),
    config: ConfigContract {
        fields: &LINK_EFFECT_FIELDS,
    },
    inputs: &EFFECT_REQUEST_INPUT,
    outputs: &LINK_OUTPUTS,
};
pub const ADDRESS_ASSIGN_EFFECT_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("net/address/assign"),
    config: ConfigContract {
        fields: &ADDRESS_EFFECT_FIELDS,
    },
    inputs: &EFFECT_REQUEST_INPUT,
    outputs: &ADDRESS_EFFECT_OUTPUTS,
};
pub const DHCP_CLIENT_EFFECT_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("net/dhcp/client"),
    config: ConfigContract {
        fields: &DHCP_CLIENT_EFFECT_FIELDS,
    },
    inputs: &EFFECT_REQUEST_INPUT,
    outputs: &DHCP_CLIENT_EFFECT_OUTPUTS,
};
pub const ROUTE_INSTALL_EFFECT_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("net/route/install"),
    config: ConfigContract {
        fields: &ROUTE_EFFECT_FIELDS,
    },
    inputs: &EFFECT_REQUEST_INPUT,
    outputs: &ROUTE_EFFECT_OUTPUTS,
};
pub const BRIDGE_EFFECT_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("net/bridge"),
    config: ConfigContract {
        fields: &FRAME_EFFECT_FIELDS,
    },
    inputs: &FRAME_INPUT,
    outputs: &FRAME_EFFECT_OUTPUTS,
};
pub const FORWARD_EFFECT_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("net/forward"),
    config: ConfigContract {
        fields: &PACKET_EFFECT_FIELDS,
    },
    inputs: &PACKET_INPUT,
    outputs: &PACKET_EFFECT_OUTPUTS,
};
pub const NAT_EFFECT_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("net/nat"),
    config: ConfigContract {
        fields: &PACKET_EFFECT_FIELDS,
    },
    inputs: &PACKET_INPUT,
    outputs: &PACKET_EFFECT_OUTPUTS,
};
pub const FIREWALL_EFFECT_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("net/firewall"),
    config: ConfigContract {
        fields: &PACKET_EFFECT_FIELDS,
    },
    inputs: &PACKET_INPUT,
    outputs: &PACKET_EFFECT_OUTPUTS,
};
pub const DNS_RESOLVE_EFFECT_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("net/dns/resolve"),
    config: ConfigContract {
        fields: &DNS_EFFECT_FIELDS,
    },
    inputs: &EFFECT_REQUEST_INPUT,
    outputs: &DNS_EFFECT_OUTPUTS,
};
pub const INTERNET_ACCESS_EFFECT_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("net/internet/access"),
    config: ConfigContract {
        fields: &INTERNET_EFFECT_FIELDS,
    },
    inputs: &EFFECT_REQUEST_INPUT,
    outputs: &INTERNET_EFFECT_OUTPUTS,
};

pub const EXECUTABLE_STANDING_NETWORK_CONTRACTS: [&NodeContract<'static>; 15] = [
    &LINK_OBSERVE_CONTRACT,
    &FRAME_SOURCE_CONTRACT,
    &FRAME_SINK_CONTRACT,
    &PACKET_SOURCE_CONTRACT,
    &PACKET_CLASSIFY_CONTRACT,
    &PACKET_ROUTE_CONTRACT,
    &PACKET_SINK_CONTRACT,
    &DATAGRAM_SOURCE_CONTRACT,
    &DATAGRAM_IMPAIR_CONTRACT,
    &DATAGRAM_SINK_CONTRACT,
    &STREAM_SOURCE_CONTRACT,
    &STREAM_SINK_CONTRACT,
    &SESSION_LISTEN_CONTRACT,
    &NETWORK_METER_CONTRACT,
    &SERVICE_OBSERVE_CONTRACT,
];

pub const NETWORK_EFFECT_CONTRACTS: [&NodeContract<'static>; 12] = [
    &WIFI_JOIN_EFFECT_CONTRACT,
    &WIRED_LINK_EFFECT_CONTRACT,
    &VIRTUAL_LINK_EFFECT_CONTRACT,
    &ADDRESS_ASSIGN_EFFECT_CONTRACT,
    &DHCP_CLIENT_EFFECT_CONTRACT,
    &ROUTE_INSTALL_EFFECT_CONTRACT,
    &BRIDGE_EFFECT_CONTRACT,
    &FORWARD_EFFECT_CONTRACT,
    &NAT_EFFECT_CONTRACT,
    &FIREWALL_EFFECT_CONTRACT,
    &DNS_RESOLVE_EFFECT_CONTRACT,
    &INTERNET_ACCESS_EFFECT_CONTRACT,
];

pub const STANDING_NETWORK_CONTRACTS: [&NodeContract<'static>; 27] = [
    &LINK_OBSERVE_CONTRACT,
    &FRAME_SOURCE_CONTRACT,
    &FRAME_SINK_CONTRACT,
    &PACKET_SOURCE_CONTRACT,
    &PACKET_CLASSIFY_CONTRACT,
    &PACKET_ROUTE_CONTRACT,
    &PACKET_SINK_CONTRACT,
    &DATAGRAM_SOURCE_CONTRACT,
    &DATAGRAM_IMPAIR_CONTRACT,
    &DATAGRAM_SINK_CONTRACT,
    &STREAM_SOURCE_CONTRACT,
    &STREAM_SINK_CONTRACT,
    &SESSION_LISTEN_CONTRACT,
    &NETWORK_METER_CONTRACT,
    &SERVICE_OBSERVE_CONTRACT,
    &WIFI_JOIN_EFFECT_CONTRACT,
    &WIRED_LINK_EFFECT_CONTRACT,
    &VIRTUAL_LINK_EFFECT_CONTRACT,
    &ADDRESS_ASSIGN_EFFECT_CONTRACT,
    &DHCP_CLIENT_EFFECT_CONTRACT,
    &ROUTE_INSTALL_EFFECT_CONTRACT,
    &BRIDGE_EFFECT_CONTRACT,
    &FORWARD_EFFECT_CONTRACT,
    &NAT_EFFECT_CONTRACT,
    &FIREWALL_EFFECT_CONTRACT,
    &DNS_RESOLVE_EFFECT_CONTRACT,
    &INTERNET_ACCESS_EFFECT_CONTRACT,
];

fn integer(node: &Node, key: &str) -> Option<u64> {
    match node.config_value(key) {
        Some(SourceValue::Integer(value)) => u64::try_from(*value).ok(),
        _ => None,
    }
}

fn validate_base(node: &Node, field_count: usize) -> Result<(), ResolutionError> {
    for forbidden in [
        "resource",
        "grant",
        "authority",
        "provider",
        "interface_observation",
        "authenticated",
        "member",
        "internet",
    ] {
        if node.config_value(forbidden).is_some() {
            return Err(ResolutionError::new(
                "CND-SRC-002",
                "network source cannot manufacture host facts, authority, identity, or reachability",
            ));
        }
    }
    if node.config.len() != field_count || node.config("lifecycle") != Some("standing") {
        return Err(ResolutionError::new(
            NetworkReason::Bounds.code(),
            "standing network node requires the exact current bounded profile",
        ));
    }
    Ok(())
}

fn validate_link(node: &Node) -> Result<(), ResolutionError> {
    validate_base(node, LINK_FIELDS.len())?;
    if integer(node, "interface") != Some(1)
        || node.config("kind") != Some("virtual")
        || node.config("carrier") != Some("present")
        || node.config("address") != Some("ready")
        || integer(node, "mtu") != Some(1_500)
        || integer(node, "period_ticks") != Some(10)
        || integer(node, "freshness_ticks") != Some(20)
        || integer(node, "maximum_evidence_events") != Some(MAXIMUM_EVIDENCE_EVENTS as u64)
    {
        return Err(ResolutionError::new(
            NetworkReason::Bounds.code(),
            "link observer does not match the deterministic virtual-link profile",
        ));
    }
    Ok(())
}

fn validate_frame_source(node: &Node) -> Result<(), ResolutionError> {
    validate_base(node, FRAME_SOURCE_FIELDS.len())?;
    if integer(node, "interface") != Some(1)
        || integer(node, "period_ticks") != Some(1_000)
        || integer(node, "payload_bytes") != Some(64)
        || integer(node, "maximum_frame_bytes") != Some(MAXIMUM_FRAME_BYTES as u64)
        || integer(node, "maximum_evidence_events") != Some(MAXIMUM_EVIDENCE_EVENTS as u64)
    {
        return Err(ResolutionError::new(
            NetworkReason::Bounds.code(),
            "frame source does not match the bounded deterministic profile",
        ));
    }
    Ok(())
}

fn validate_frame_sink(node: &Node) -> Result<(), ResolutionError> {
    validate_base(node, FRAME_SINK_FIELDS.len())?;
    if integer(node, "maximum_frames_per_step") != Some(1)
        || integer(node, "maximum_retained_items") != Some(1)
        || integer(node, "maximum_evidence_events") != Some(MAXIMUM_EVIDENCE_EVENTS as u64)
    {
        return Err(ResolutionError::new(
            NetworkReason::Bounds.code(),
            "frame sink does not match its finite counter profile",
        ));
    }
    Ok(())
}

fn validate_packet_source(node: &Node) -> Result<(), ResolutionError> {
    validate_base(node, PACKET_SOURCE_FIELDS.len())?;
    if node.config("source") != Some("10.0.0.2")
        || node.config("destination") != Some("10.1.0.2")
        || integer(node, "hop_limit") != Some(4)
        || integer(node, "payload_bytes") != Some(64)
        || integer(node, "period_ticks") != Some(10)
        || integer(node, "maximum_packets_per_step") != Some(1)
        || integer(node, "maximum_packet_bytes") != Some(MAXIMUM_PACKET_BYTES as u64)
        || integer(node, "maximum_evidence_events") != Some(MAXIMUM_EVIDENCE_EVENTS as u64)
    {
        return Err(ResolutionError::new(
            NetworkReason::Bounds.code(),
            "packet source does not match the bounded deterministic profile",
        ));
    }
    Ok(())
}

fn validate_classify(node: &Node) -> Result<(), ResolutionError> {
    validate_base(node, CLASSIFY_FIELDS.len())?;
    if node.config("admitted_prefix") != Some("10.0.0.0")
        || integer(node, "prefix_length") != Some(8)
        || integer(node, "maximum_packet_bytes") != Some(MAXIMUM_PACKET_BYTES as u64)
        || integer(node, "maximum_evidence_events") != Some(MAXIMUM_EVIDENCE_EVENTS as u64)
    {
        return Err(ResolutionError::new(
            NetworkReason::Bounds.code(),
            "packet classifier does not match the exact admitted prefix",
        ));
    }
    Ok(())
}

fn validate_route(node: &Node) -> Result<(), ResolutionError> {
    validate_base(node, ROUTE_FIELDS.len())?;
    if node.config("prefix") != Some("10.1.0.0")
        || integer(node, "prefix_length") != Some(16)
        || integer(node, "egress_interface") != Some(2)
        || integer(node, "mtu") != Some(1_500)
        || node.config("forwarding") != Some("admitted")
        || integer(node, "maximum_routes") != Some(MAXIMUM_ROUTES as u64)
        || integer(node, "maximum_packet_bytes") != Some(MAXIMUM_PACKET_BYTES as u64)
        || integer(node, "maximum_evidence_events") != Some(MAXIMUM_EVIDENCE_EVENTS as u64)
    {
        return Err(ResolutionError::new(
            NetworkReason::RouteInvalid.code(),
            "router does not match the exact finite route-table profile",
        ));
    }
    Ok(())
}

fn validate_sink(node: &Node) -> Result<(), ResolutionError> {
    validate_base(node, SINK_FIELDS.len())?;
    if integer(node, "maximum_packets_per_step") != Some(1)
        || integer(node, "maximum_retained_items") != Some(1)
        || integer(node, "maximum_evidence_events") != Some(MAXIMUM_EVIDENCE_EVENTS as u64)
    {
        return Err(ResolutionError::new(
            NetworkReason::Bounds.code(),
            "packet sink does not match its finite counter and evidence bounds",
        ));
    }
    Ok(())
}

fn validate_datagram_source(node: &Node) -> Result<(), ResolutionError> {
    validate_base(node, DATAGRAM_SOURCE_FIELDS.len())?;
    if integer(node, "source_port") != Some(30_000)
        || integer(node, "destination_port") != Some(30_001)
        || integer(node, "period_ticks") != Some(1_000)
        || integer(node, "payload_bytes") != Some(64)
        || integer(node, "maximum_datagram_bytes") != Some(MAXIMUM_DATAGRAM_BYTES as u64)
        || integer(node, "maximum_datagrams_per_step") != Some(1)
        || integer(node, "maximum_evidence_events") != Some(MAXIMUM_EVIDENCE_EVENTS as u64)
    {
        return Err(ResolutionError::new(
            NetworkReason::Bounds.code(),
            "datagram source does not match the bounded deterministic profile",
        ));
    }
    Ok(())
}

fn validate_datagram_impair(node: &Node) -> Result<(), ResolutionError> {
    validate_base(node, DATAGRAM_IMPAIR_FIELDS.len())?;
    if node.config("pattern") != Some("deliver,loss,duplicate,reorder")
        || integer(node, "maximum_datagram_bytes") != Some(MAXIMUM_DATAGRAM_BYTES as u64)
        || integer(node, "maximum_evidence_events") != Some(MAXIMUM_EVIDENCE_EVENTS as u64)
    {
        return Err(ResolutionError::new(
            NetworkReason::Bounds.code(),
            "datagram impairment requires the exact deterministic outcome pattern",
        ));
    }
    Ok(())
}

fn validate_datagram_sink(node: &Node) -> Result<(), ResolutionError> {
    validate_base(node, DATAGRAM_SINK_FIELDS.len())?;
    if integer(node, "maximum_datagrams_per_step") != Some(1)
        || integer(node, "maximum_retained_items") != Some(1)
        || integer(node, "maximum_evidence_events") != Some(MAXIMUM_EVIDENCE_EVENTS as u64)
    {
        return Err(ResolutionError::new(
            NetworkReason::Bounds.code(),
            "datagram sink does not match its finite counter profile",
        ));
    }
    Ok(())
}

fn validate_stream_source(node: &Node) -> Result<(), ResolutionError> {
    validate_base(node, STREAM_SOURCE_FIELDS.len())?;
    if integer(node, "session") != Some(1)
        || integer(node, "period_ticks") != Some(1_000)
        || integer(node, "chunk_bytes") != Some(64)
        || integer(node, "maximum_chunk_bytes") != Some(MAXIMUM_STREAM_CHUNK_BYTES as u64)
        || integer(node, "maximum_evidence_events") != Some(MAXIMUM_EVIDENCE_EVENTS as u64)
    {
        return Err(ResolutionError::new(
            NetworkReason::Bounds.code(),
            "stream source does not match the bounded deterministic profile",
        ));
    }
    Ok(())
}

fn validate_stream_sink(node: &Node) -> Result<(), ResolutionError> {
    validate_base(node, STREAM_SINK_FIELDS.len())?;
    if integer(node, "maximum_chunks_per_step") != Some(1)
        || integer(node, "maximum_retained_items") != Some(1)
        || integer(node, "maximum_evidence_events") != Some(MAXIMUM_EVIDENCE_EVENTS as u64)
    {
        return Err(ResolutionError::new(
            NetworkReason::Bounds.code(),
            "stream sink does not match its finite counter profile",
        ));
    }
    Ok(())
}

fn validate_listener(node: &Node) -> Result<(), ResolutionError> {
    validate_base(node, LISTENER_FIELDS.len())?;
    if node.config("transport") != Some("tcp-reference")
        || integer(node, "local_port") != Some(8080)
        || integer(node, "period_ticks") != Some(10)
        || integer(node, "session_timeout_ticks") != Some(25)
        || integer(node, "maximum_sessions") != Some(MAXIMUM_SESSIONS as u64)
        || integer(node, "maximum_retained_items") != Some(MAXIMUM_SESSIONS as u64)
        || integer(node, "maximum_evidence_events") != Some(MAXIMUM_EVIDENCE_EVENTS as u64)
    {
        return Err(ResolutionError::new(
            NetworkReason::Bounds.code(),
            "listener does not match the bounded multi-session reference profile",
        ));
    }
    Ok(())
}

fn validate_service_observe(node: &Node) -> Result<(), ResolutionError> {
    validate_base(node, SERVICE_OBSERVE_FIELDS.len())?;
    if integer(node, "maximum_retained_items") != Some(0)
        || integer(node, "maximum_evidence_events") != Some(MAXIMUM_EVIDENCE_EVENTS as u64)
    {
        return Err(ResolutionError::new(
            NetworkReason::Bounds.code(),
            "service observer must declare zero content retention and finite evidence",
        ));
    }
    Ok(())
}

fn runtime_error(reason: NetworkReason, detail: &'static str) -> RuntimeError {
    RuntimeError::new(reason.code(), detail)
}

struct RecordedHandler<H> {
    inner: H,
    evidence: EvidenceLog,
}

impl<H> RecordedHandler<H> {
    const fn new(inner: H) -> Self {
        Self {
            inner,
            evidence: EvidenceLog::new(),
        }
    }
}

impl<H: Handler> Handler for RecordedHandler<H> {
    fn bind_exact(&mut self, binding: ExactHostedServiceBinding) -> Result<(), RuntimeError> {
        self.inner.bind_exact(binding)
    }

    fn prepare(
        &mut self,
        node: &Node,
        binding: ExactHostedServiceBinding,
    ) -> Result<(), RuntimeError> {
        self.inner.prepare(node, binding)
    }

    fn start(&mut self, node: &Node) -> Result<(), RuntimeError> {
        self.inner.start(node)
    }

    fn step(
        &mut self,
        node: &Node,
        inputs: &[Value],
        context: HostedServiceStepContext,
        io: &mut RunIo<'_>,
    ) -> Result<HostedServiceStep, RuntimeError> {
        match self.inner.step(node, inputs, context, io) {
            Ok(step) => {
                let kind = match &step {
                    HostedServiceStep::Produced { .. } => Some(EvidenceKind::Accepted),
                    HostedServiceStep::Completed { .. } => Some(EvidenceKind::Terminal),
                    HostedServiceStep::Waiting { .. } => None,
                };
                if let Some(kind) = kind {
                    self.evidence
                        .push(context.tick, kind, None)
                        .map_err(|reason| {
                            runtime_error(reason, "provider evidence sequence exhausted")
                        })?;
                }
                Ok(step)
            }
            Err(error) => {
                let _ = self
                    .evidence
                    .push(context.tick, EvidenceKind::Rejected, None);
                Err(error)
            }
        }
    }

    fn cancel(&mut self, node: &Node, stop: StopPolicy) -> Result<(), RuntimeError> {
        self.inner.cancel(node, stop)
    }

    fn cleanup(
        &mut self,
        node: &Node,
        context: HostedServiceStepContext,
    ) -> Result<HostedServiceCleanup, RuntimeError> {
        self.inner.cleanup(node, context)
    }
}

pub(crate) fn recorded_handler<H: Handler + 'static>(inner: H) -> Box<dyn Handler> {
    Box::new(RecordedHandler::new(inner))
}

fn push_u16(bytes: &mut Vec<u8>, value: u16) {
    bytes.extend_from_slice(&value.to_be_bytes());
}

fn push_u32(bytes: &mut Vec<u8>, value: u32) {
    bytes.extend_from_slice(&value.to_be_bytes());
}

fn push_u64(bytes: &mut Vec<u8>, value: u64) {
    bytes.extend_from_slice(&value.to_be_bytes());
}

fn read_u16(bytes: &[u8], offset: usize) -> Result<u16, RuntimeError> {
    bytes
        .get(offset..offset + 2)
        .and_then(|bytes| bytes.try_into().ok())
        .map(u16::from_be_bytes)
        .ok_or_else(|| runtime_error(NetworkReason::MalformedPacket, "truncated network value"))
}

fn read_u32(bytes: &[u8], offset: usize) -> Result<u32, RuntimeError> {
    bytes
        .get(offset..offset + 4)
        .and_then(|bytes| bytes.try_into().ok())
        .map(u32::from_be_bytes)
        .ok_or_else(|| runtime_error(NetworkReason::MalformedPacket, "truncated network value"))
}

fn read_u64(bytes: &[u8], offset: usize) -> Result<u64, RuntimeError> {
    bytes
        .get(offset..offset + 8)
        .and_then(|bytes| bytes.try_into().ok())
        .map(u64::from_be_bytes)
        .ok_or_else(|| runtime_error(NetworkReason::MalformedPacket, "truncated network value"))
}

fn disposition_tag(disposition: PacketDisposition) -> u8 {
    match disposition {
        PacketDisposition::Pending => 0,
        PacketDisposition::Forwarded => 1,
        PacketDisposition::LocalDelivery => 2,
        PacketDisposition::Dropped => 3,
        PacketDisposition::Rejected => 4,
        PacketDisposition::NoRoute => 5,
        PacketDisposition::HopExhausted => 6,
        PacketDisposition::MtuExceeded => 7,
        PacketDisposition::PolicyDenied => 8,
        PacketDisposition::ProviderLost => 9,
    }
}

fn disposition_from_tag(tag: u8) -> Result<PacketDisposition, RuntimeError> {
    match tag {
        0 => Ok(PacketDisposition::Pending),
        1 => Ok(PacketDisposition::Forwarded),
        2 => Ok(PacketDisposition::LocalDelivery),
        3 => Ok(PacketDisposition::Dropped),
        4 => Ok(PacketDisposition::Rejected),
        5 => Ok(PacketDisposition::NoRoute),
        6 => Ok(PacketDisposition::HopExhausted),
        7 => Ok(PacketDisposition::MtuExceeded),
        8 => Ok(PacketDisposition::PolicyDenied),
        9 => Ok(PacketDisposition::ProviderLost),
        _ => Err(runtime_error(
            NetworkReason::MalformedPacket,
            "unknown packet disposition",
        )),
    }
}

fn packet_value(packet: &NetworkPacket) -> Result<Value, RuntimeError> {
    packet
        .validate()
        .map_err(|reason| runtime_error(reason, "invalid bounded packet"))?;
    let payload_len = u16::try_from(packet.payload.len())
        .map_err(|_| runtime_error(NetworkReason::PacketTooLarge, "packet payload is too large"))?;
    let mut bytes = Vec::with_capacity(51 + packet.payload.len());
    bytes.extend_from_slice(b"CNP0");
    push_u64(&mut bytes, packet.sequence);
    bytes.push(match packet.family {
        AddressFamily::Ipv4 => 4,
        AddressFamily::Ipv6 => 6,
    });
    bytes.extend_from_slice(&packet.source);
    bytes.extend_from_slice(&packet.destination);
    bytes.push(packet.hop_limit);
    bytes.push(u8::from(packet.fragmented));
    push_u16(&mut bytes, packet.egress_interface.unwrap_or(0));
    bytes.push(disposition_tag(packet.disposition));
    push_u16(&mut bytes, payload_len);
    bytes.extend_from_slice(&packet.payload);
    Ok(Value {
        value_type: PACKET_TYPE,
        bytes,
    })
}

fn parse_packet(value: &Value) -> Result<NetworkPacket, RuntimeError> {
    if value.value_type != PACKET_TYPE || !value.bytes.starts_with(b"CNP0") {
        return Err(runtime_error(
            NetworkReason::MalformedPacket,
            "value is not a network packet",
        ));
    }
    let bytes = &value.bytes;
    let family = match bytes.get(12) {
        Some(4) => AddressFamily::Ipv4,
        Some(6) => AddressFamily::Ipv6,
        _ => {
            return Err(runtime_error(
                NetworkReason::MalformedPacket,
                "unknown address family",
            ));
        }
    };
    let source: [u8; 16] = bytes
        .get(13..29)
        .and_then(|value| value.try_into().ok())
        .ok_or_else(|| runtime_error(NetworkReason::MalformedPacket, "truncated source address"))?;
    let destination: [u8; 16] = bytes
        .get(29..45)
        .and_then(|value| value.try_into().ok())
        .ok_or_else(|| {
            runtime_error(
                NetworkReason::MalformedPacket,
                "truncated destination address",
            )
        })?;
    let hop_limit = *bytes
        .get(45)
        .ok_or_else(|| runtime_error(NetworkReason::MalformedPacket, "missing hop limit"))?;
    let fragmented = *bytes
        .get(46)
        .ok_or_else(|| runtime_error(NetworkReason::MalformedPacket, "missing fragment fact"))?
        != 0;
    let egress = read_u16(bytes, 47)?;
    let disposition = disposition_from_tag(*bytes.get(49).ok_or_else(|| {
        runtime_error(NetworkReason::MalformedPacket, "missing packet disposition")
    })?)?;
    let payload_len = usize::from(read_u16(bytes, 50)?);
    let payload = bytes
        .get(52..52 + payload_len)
        .ok_or_else(|| runtime_error(NetworkReason::MalformedPacket, "truncated packet payload"))?
        .to_vec();
    if bytes.len() != 52 + payload_len {
        return Err(runtime_error(
            NetworkReason::MalformedPacket,
            "packet has trailing bytes",
        ));
    }
    let packet = NetworkPacket {
        sequence: read_u64(bytes, 4)?,
        family,
        source,
        destination,
        hop_limit,
        fragmented,
        egress_interface: (egress != 0).then_some(egress),
        disposition,
        payload,
    };
    packet
        .validate()
        .map_err(|reason| runtime_error(reason, "invalid bounded packet"))?;
    Ok(packet)
}

fn frame_value(frame: &NetworkFrame) -> Result<Value, RuntimeError> {
    frame
        .validate()
        .map_err(|reason| runtime_error(reason, "invalid bounded frame"))?;
    let payload_len = u16::try_from(frame.payload.len())
        .map_err(|_| runtime_error(NetworkReason::Bounds, "frame payload is too large"))?;
    let mut bytes = Vec::with_capacity(19 + frame.payload.len());
    bytes.extend_from_slice(b"CNF0");
    push_u16(&mut bytes, frame.interface);
    bytes.push(match frame.direction {
        NetworkDirection::Ingress => 0,
        NetworkDirection::Egress => 1,
    });
    push_u16(&mut bytes, frame.protocol.unwrap_or(0));
    push_u64(&mut bytes, frame.observed_at_tick);
    push_u16(&mut bytes, payload_len);
    bytes.extend_from_slice(&frame.payload);
    Ok(Value {
        value_type: FRAME_TYPE,
        bytes,
    })
}

fn parse_frame(value: &Value) -> Result<NetworkFrame, RuntimeError> {
    if value.value_type != FRAME_TYPE || !value.bytes.starts_with(b"CNF0") {
        return Err(runtime_error(
            NetworkReason::MalformedPacket,
            "value is not a network frame",
        ));
    }
    let direction = match value.bytes.get(6) {
        Some(0) => NetworkDirection::Ingress,
        Some(1) => NetworkDirection::Egress,
        _ => {
            return Err(runtime_error(
                NetworkReason::MalformedPacket,
                "unknown frame direction",
            ));
        }
    };
    let protocol = read_u16(&value.bytes, 7)?;
    let payload_len = usize::from(read_u16(&value.bytes, 17)?);
    let payload = value
        .bytes
        .get(19..19 + payload_len)
        .ok_or_else(|| runtime_error(NetworkReason::MalformedPacket, "truncated frame payload"))?
        .to_vec();
    if value.bytes.len() != 19 + payload_len {
        return Err(runtime_error(
            NetworkReason::MalformedPacket,
            "frame has trailing bytes",
        ));
    }
    let frame = NetworkFrame {
        interface: read_u16(&value.bytes, 4)?,
        direction,
        protocol: (protocol != 0).then_some(protocol),
        observed_at_tick: read_u64(&value.bytes, 9)?,
        payload,
    };
    frame
        .validate()
        .map_err(|reason| runtime_error(reason, "invalid bounded frame"))?;
    Ok(frame)
}

fn datagram_delivery_tag(delivery: DatagramDelivery) -> u8 {
    match delivery {
        DatagramDelivery::Pending => 0,
        DatagramDelivery::Delivered => 1,
        DatagramDelivery::Lost => 2,
        DatagramDelivery::Duplicated => 3,
        DatagramDelivery::Reordered => 4,
        DatagramDelivery::Rejected => 5,
        DatagramDelivery::Cancelled => 6,
        DatagramDelivery::ProviderLost => 7,
    }
}

fn datagram_delivery_from_tag(tag: u8) -> Result<DatagramDelivery, RuntimeError> {
    match tag {
        0 => Ok(DatagramDelivery::Pending),
        1 => Ok(DatagramDelivery::Delivered),
        2 => Ok(DatagramDelivery::Lost),
        3 => Ok(DatagramDelivery::Duplicated),
        4 => Ok(DatagramDelivery::Reordered),
        5 => Ok(DatagramDelivery::Rejected),
        6 => Ok(DatagramDelivery::Cancelled),
        7 => Ok(DatagramDelivery::ProviderLost),
        _ => Err(runtime_error(
            NetworkReason::MalformedPacket,
            "unknown datagram delivery outcome",
        )),
    }
}

fn datagram_value(datagram: &NetworkDatagram) -> Result<Value, RuntimeError> {
    datagram
        .validate()
        .map_err(|reason| runtime_error(reason, "invalid bounded datagram"))?;
    let payload_len = u16::try_from(datagram.payload.len())
        .map_err(|_| runtime_error(NetworkReason::Bounds, "datagram payload is too large"))?;
    let mut bytes = Vec::with_capacity(60 + datagram.payload.len());
    bytes.extend_from_slice(b"CND0");
    push_u64(&mut bytes, datagram.session.unwrap_or(0));
    push_u64(&mut bytes, datagram.sequence);
    bytes.push(family_tag(datagram.family));
    bytes.extend_from_slice(&datagram.source);
    push_u16(&mut bytes, datagram.source_port);
    bytes.extend_from_slice(&datagram.destination);
    push_u16(&mut bytes, datagram.destination_port);
    bytes.push(datagram_delivery_tag(datagram.delivery));
    push_u16(&mut bytes, payload_len);
    bytes.extend_from_slice(&datagram.payload);
    Ok(Value {
        value_type: DATAGRAM_TYPE,
        bytes,
    })
}

fn parse_datagram(value: &Value) -> Result<NetworkDatagram, RuntimeError> {
    if value.value_type != DATAGRAM_TYPE || !value.bytes.starts_with(b"CND0") {
        return Err(runtime_error(
            NetworkReason::MalformedPacket,
            "value is not a network datagram",
        ));
    }
    let payload_len = usize::from(read_u16(&value.bytes, 58)?);
    let payload = value
        .bytes
        .get(60..60 + payload_len)
        .ok_or_else(|| runtime_error(NetworkReason::MalformedPacket, "truncated datagram payload"))?
        .to_vec();
    if value.bytes.len() != 60 + payload_len {
        return Err(runtime_error(
            NetworkReason::MalformedPacket,
            "datagram has trailing bytes",
        ));
    }
    let session = read_u64(&value.bytes, 4)?;
    let datagram = NetworkDatagram {
        session: (session != 0).then_some(session),
        sequence: read_u64(&value.bytes, 12)?,
        family: family_from_tag(value.bytes[20])?,
        source: value.bytes[21..37]
            .try_into()
            .expect("exact datagram width"),
        source_port: read_u16(&value.bytes, 37)?,
        destination: value.bytes[39..55]
            .try_into()
            .expect("exact datagram width"),
        destination_port: read_u16(&value.bytes, 55)?,
        delivery: datagram_delivery_from_tag(value.bytes[57])?,
        payload,
    };
    datagram
        .validate()
        .map_err(|reason| runtime_error(reason, "invalid bounded datagram"))?;
    Ok(datagram)
}

fn pressure_tag(pressure: StreamPressure) -> u8 {
    match pressure {
        StreamPressure::Ready => 0,
        StreamPressure::Backpressured => 1,
        StreamPressure::Draining => 2,
    }
}

fn pressure_from_tag(tag: u8) -> Result<StreamPressure, RuntimeError> {
    match tag {
        0 => Ok(StreamPressure::Ready),
        1 => Ok(StreamPressure::Backpressured),
        2 => Ok(StreamPressure::Draining),
        _ => Err(runtime_error(
            NetworkReason::MalformedPacket,
            "unknown byte-stream pressure state",
        )),
    }
}

fn stream_value(chunk: &ByteStreamChunk) -> Result<Value, RuntimeError> {
    chunk
        .validate()
        .map_err(|reason| runtime_error(reason, "invalid bounded stream chunk"))?;
    let length = u16::try_from(chunk.bytes.len())
        .map_err(|_| runtime_error(NetworkReason::Bounds, "stream chunk is too large"))?;
    let mut bytes = Vec::with_capacity(24 + chunk.bytes.len());
    bytes.extend_from_slice(b"CNB0");
    push_u64(&mut bytes, chunk.session);
    push_u64(&mut bytes, chunk.offset);
    bytes.push(
        u8::from(chunk.eof)
            | (u8::from(chunk.read_half_closed) << 1)
            | (u8::from(chunk.write_half_closed) << 2),
    );
    bytes.push(pressure_tag(chunk.pressure));
    push_u16(&mut bytes, length);
    bytes.extend_from_slice(&chunk.bytes);
    Ok(Value {
        value_type: BYTE_STREAM_TYPE,
        bytes,
    })
}

fn parse_stream(value: &Value) -> Result<ByteStreamChunk, RuntimeError> {
    if value.value_type != BYTE_STREAM_TYPE || !value.bytes.starts_with(b"CNB0") {
        return Err(runtime_error(
            NetworkReason::MalformedPacket,
            "value is not a byte-stream chunk",
        ));
    }
    let flags = *value
        .bytes
        .get(20)
        .ok_or_else(|| runtime_error(NetworkReason::MalformedPacket, "missing stream flags"))?;
    if flags & !0b111 != 0 {
        return Err(runtime_error(
            NetworkReason::MalformedPacket,
            "stream chunk has unknown flags",
        ));
    }
    let length = usize::from(read_u16(&value.bytes, 22)?);
    let bytes = value
        .bytes
        .get(24..24 + length)
        .ok_or_else(|| runtime_error(NetworkReason::MalformedPacket, "truncated stream bytes"))?
        .to_vec();
    if value.bytes.len() != 24 + length {
        return Err(runtime_error(
            NetworkReason::MalformedPacket,
            "stream chunk has trailing bytes",
        ));
    }
    let chunk = ByteStreamChunk {
        session: read_u64(&value.bytes, 4)?,
        offset: read_u64(&value.bytes, 12)?,
        eof: flags & 1 != 0,
        read_half_closed: flags & 2 != 0,
        write_half_closed: flags & 4 != 0,
        pressure: pressure_from_tag(value.bytes[21])?,
        bytes,
    };
    chunk
        .validate()
        .map_err(|reason| runtime_error(reason, "invalid bounded stream chunk"))?;
    Ok(chunk)
}

pub(crate) fn link_value(observation: LinkObservation) -> Value {
    let mut bytes = Vec::with_capacity(34);
    bytes.extend_from_slice(b"CNL0");
    push_u16(&mut bytes, observation.interface);
    push_u32(&mut bytes, observation.generation);
    bytes.push(match observation.kind {
        LinkKind::Ethernet => 1,
        LinkKind::WifiStation => 2,
        LinkKind::WifiAccessPoint => 3,
        LinkKind::Usb => 4,
        LinkKind::Loopback => 5,
        LinkKind::Virtual => 6,
        LinkKind::Embedded => 7,
    });
    bytes.push(u8::from(observation.carrier));
    push_u16(&mut bytes, observation.mtu);
    bytes.push(u8::from(observation.address_ready));
    bytes.push(match observation.availability {
        NetworkAvailability::Unsupported => 0,
        NetworkAvailability::Active => 1,
        NetworkAvailability::Waiting => 2,
        NetworkAvailability::Degraded => 3,
        NetworkAvailability::Draining => 4,
        NetworkAvailability::Stopped => 5,
        NetworkAvailability::Failed => 6,
    });
    push_u64(&mut bytes, observation.observed_at_tick);
    push_u64(&mut bytes, observation.valid_until_tick);
    Value {
        value_type: LINK_OBSERVATION_TYPE,
        bytes,
    }
}

fn control_kind_tag(kind: NetworkControlKind) -> u8 {
    match kind {
        NetworkControlKind::Link => 0,
        NetworkControlKind::Lease => 1,
        NetworkControlKind::Neighbor => 2,
        NetworkControlKind::Route => 3,
        NetworkControlKind::Session => 4,
        NetworkControlKind::Timeout => 5,
        NetworkControlKind::Loss => 6,
        NetworkControlKind::Reset => 7,
        NetworkControlKind::Policy => 8,
        NetworkControlKind::Provider => 9,
    }
}

fn control_kind_from_tag(tag: u8) -> Result<NetworkControlKind, RuntimeError> {
    match tag {
        0 => Ok(NetworkControlKind::Link),
        1 => Ok(NetworkControlKind::Lease),
        2 => Ok(NetworkControlKind::Neighbor),
        3 => Ok(NetworkControlKind::Route),
        4 => Ok(NetworkControlKind::Session),
        5 => Ok(NetworkControlKind::Timeout),
        6 => Ok(NetworkControlKind::Loss),
        7 => Ok(NetworkControlKind::Reset),
        8 => Ok(NetworkControlKind::Policy),
        9 => Ok(NetworkControlKind::Provider),
        _ => Err(runtime_error(
            NetworkReason::MalformedPacket,
            "unknown network control kind",
        )),
    }
}

fn control_outcome_tag(outcome: NetworkControlOutcome) -> u8 {
    match outcome {
        NetworkControlOutcome::Observed => 0,
        NetworkControlOutcome::Admitted => 1,
        NetworkControlOutcome::Applied => 2,
        NetworkControlOutcome::Rejected => 3,
        NetworkControlOutcome::Expired => 4,
        NetworkControlOutcome::Cancelled => 5,
        NetworkControlOutcome::Failed => 6,
    }
}

fn control_outcome_from_tag(tag: u8) -> Result<NetworkControlOutcome, RuntimeError> {
    match tag {
        0 => Ok(NetworkControlOutcome::Observed),
        1 => Ok(NetworkControlOutcome::Admitted),
        2 => Ok(NetworkControlOutcome::Applied),
        3 => Ok(NetworkControlOutcome::Rejected),
        4 => Ok(NetworkControlOutcome::Expired),
        5 => Ok(NetworkControlOutcome::Cancelled),
        6 => Ok(NetworkControlOutcome::Failed),
        _ => Err(runtime_error(
            NetworkReason::MalformedPacket,
            "unknown network control outcome",
        )),
    }
}

pub(crate) fn control_value(event: NetworkControlEvent) -> Value {
    let mut bytes = Vec::with_capacity(26);
    bytes.extend_from_slice(b"CNC0");
    bytes.push(control_kind_tag(event.kind));
    bytes.push(control_outcome_tag(event.outcome));
    push_u64(&mut bytes, event.identity);
    push_u32(&mut bytes, event.generation);
    push_u64(&mut bytes, event.tick);
    Value {
        value_type: CONTROL_EVENT_TYPE,
        bytes,
    }
}

fn parse_control(value: &Value) -> Result<NetworkControlEvent, RuntimeError> {
    if value.value_type != CONTROL_EVENT_TYPE
        || !value.bytes.starts_with(b"CNC0")
        || value.bytes.len() != 26
    {
        return Err(runtime_error(
            NetworkReason::MalformedPacket,
            "value is not an exact network control event",
        ));
    }
    Ok(NetworkControlEvent {
        kind: control_kind_from_tag(value.bytes[4])?,
        outcome: control_outcome_from_tag(value.bytes[5])?,
        identity: read_u64(&value.bytes, 6)?,
        generation: read_u32(&value.bytes, 14)?,
        tick: read_u64(&value.bytes, 18)?,
    })
}

fn retained_policy_tag(policy: RetainedStatePolicy) -> u8 {
    match policy {
        RetainedStatePolicy::ReplaceLatest => 0,
        RetainedStatePolicy::Expiring => 1,
        RetainedStatePolicy::GenerationFenced => 2,
    }
}

fn retained_policy_from_tag(tag: u8) -> Result<RetainedStatePolicy, RuntimeError> {
    match tag {
        0 => Ok(RetainedStatePolicy::ReplaceLatest),
        1 => Ok(RetainedStatePolicy::Expiring),
        2 => Ok(RetainedStatePolicy::GenerationFenced),
        _ => Err(runtime_error(
            NetworkReason::MalformedPacket,
            "unknown retained-state policy",
        )),
    }
}

pub(crate) fn state_value(state: RetainedNetworkState) -> Value {
    let mut bytes = Vec::with_capacity(32);
    bytes.extend_from_slice(b"CNR0");
    bytes.push(state.table);
    push_u32(&mut bytes, state.generation);
    push_u16(&mut bytes, state.items);
    push_u32(&mut bytes, state.bytes);
    push_u64(&mut bytes, state.observed_at_tick);
    push_u64(&mut bytes, state.expires_at_tick.unwrap_or(u64::MAX));
    bytes.push(retained_policy_tag(state.policy));
    Value {
        value_type: RETAINED_NETWORK_STATE_TYPE,
        bytes,
    }
}

pub(crate) fn parse_state(value: &Value) -> Result<RetainedNetworkState, RuntimeError> {
    if value.value_type != RETAINED_NETWORK_STATE_TYPE
        || !value.bytes.starts_with(b"CNR0")
        || value.bytes.len() != 32
    {
        return Err(runtime_error(
            NetworkReason::MalformedPacket,
            "value is not an exact retained network state",
        ));
    }
    let expires_at_tick = read_u64(&value.bytes, 23)?;
    Ok(RetainedNetworkState {
        table: value.bytes[4],
        generation: read_u32(&value.bytes, 5)?,
        items: read_u16(&value.bytes, 9)?,
        bytes: read_u32(&value.bytes, 11)?,
        observed_at_tick: read_u64(&value.bytes, 15)?,
        expires_at_tick: (expires_at_tick != u64::MAX).then_some(expires_at_tick),
        policy: retained_policy_from_tag(value.bytes[31])?,
    })
}

fn address_readiness_tag(readiness: AddressReadiness) -> u8 {
    match readiness {
        AddressReadiness::Tentative => 0,
        AddressReadiness::Ready => 1,
        AddressReadiness::Duplicate => 2,
        AddressReadiness::Expired => 3,
        AddressReadiness::Removed => 4,
    }
}

fn address_readiness_from_tag(tag: u8) -> Result<AddressReadiness, RuntimeError> {
    match tag {
        0 => Ok(AddressReadiness::Tentative),
        1 => Ok(AddressReadiness::Ready),
        2 => Ok(AddressReadiness::Duplicate),
        3 => Ok(AddressReadiness::Expired),
        4 => Ok(AddressReadiness::Removed),
        _ => Err(runtime_error(
            NetworkReason::MalformedPacket,
            "unknown address readiness",
        )),
    }
}

pub(crate) fn address_state_value(state: NetworkAddressState) -> Result<Value, RuntimeError> {
    state
        .validate()
        .map_err(|reason| runtime_error(reason, "invalid address state"))?;
    let mut bytes = Vec::with_capacity(37);
    bytes.extend_from_slice(b"CNA0");
    push_u16(&mut bytes, state.interface);
    push_u32(&mut bytes, state.generation);
    bytes.push(family_tag(state.family));
    bytes.extend_from_slice(&state.address);
    bytes.push(state.prefix_length);
    bytes.push(address_readiness_tag(state.readiness));
    push_u64(&mut bytes, state.valid_until_tick.unwrap_or(u64::MAX));
    Ok(Value {
        value_type: ADDRESS_STATE_TYPE,
        bytes,
    })
}

pub(crate) fn parse_address_state(value: &Value) -> Result<NetworkAddressState, RuntimeError> {
    if value.value_type != ADDRESS_STATE_TYPE
        || !value.bytes.starts_with(b"CNA0")
        || value.bytes.len() != 37
    {
        return Err(runtime_error(
            NetworkReason::MalformedPacket,
            "value is not an exact address state",
        ));
    }
    let valid_until_tick = read_u64(&value.bytes, 29)?;
    let state = NetworkAddressState {
        interface: read_u16(&value.bytes, 4)?,
        generation: read_u32(&value.bytes, 6)?,
        family: family_from_tag(value.bytes[10])?,
        address: value.bytes[11..27]
            .try_into()
            .expect("exact address-state width"),
        prefix_length: value.bytes[27],
        readiness: address_readiness_from_tag(value.bytes[28])?,
        valid_until_tick: (valid_until_tick != u64::MAX).then_some(valid_until_tick),
    };
    state
        .validate()
        .map_err(|reason| runtime_error(reason, "invalid address state"))?;
    Ok(state)
}

fn lease_phase_tag(phase: LeasePhase) -> u8 {
    match phase {
        LeasePhase::Offered => 0,
        LeasePhase::Bound => 1,
        LeasePhase::Renewed => 2,
        LeasePhase::Rebinding => 3,
        LeasePhase::Released => 4,
        LeasePhase::Expired => 5,
        LeasePhase::Rejected => 6,
    }
}

fn lease_phase_from_tag(tag: u8) -> Result<LeasePhase, RuntimeError> {
    match tag {
        0 => Ok(LeasePhase::Offered),
        1 => Ok(LeasePhase::Bound),
        2 => Ok(LeasePhase::Renewed),
        3 => Ok(LeasePhase::Rebinding),
        4 => Ok(LeasePhase::Released),
        5 => Ok(LeasePhase::Expired),
        6 => Ok(LeasePhase::Rejected),
        _ => Err(runtime_error(
            NetworkReason::MalformedPacket,
            "unknown DHCP lease phase",
        )),
    }
}

pub(crate) fn dhcp_lease_value(lease: NetworkDhcpLease) -> Result<Value, RuntimeError> {
    lease
        .validate()
        .map_err(|reason| runtime_error(reason, "invalid DHCP lease"))?;
    let mut bytes = Vec::with_capacity(58);
    bytes.extend_from_slice(b"CNDH");
    push_u64(&mut bytes, lease.client);
    bytes.push(family_tag(lease.family));
    bytes.extend_from_slice(&lease.address);
    push_u32(&mut bytes, lease.generation);
    bytes.push(lease_phase_tag(lease.phase));
    push_u64(&mut bytes, lease.expires_at_tick.unwrap_or(u64::MAX));
    bytes.extend_from_slice(&lease.server);
    Ok(Value {
        value_type: DHCP_LEASE_TYPE,
        bytes,
    })
}

pub(crate) fn parse_dhcp_lease(value: &Value) -> Result<NetworkDhcpLease, RuntimeError> {
    if value.value_type != DHCP_LEASE_TYPE
        || !value.bytes.starts_with(b"CNDH")
        || value.bytes.len() != 58
    {
        return Err(runtime_error(
            NetworkReason::MalformedPacket,
            "value is not an exact DHCP lease",
        ));
    }
    let expires_at_tick = read_u64(&value.bytes, 34)?;
    let lease = NetworkDhcpLease {
        client: read_u64(&value.bytes, 4)?,
        family: family_from_tag(value.bytes[12])?,
        address: value.bytes[13..29]
            .try_into()
            .expect("exact DHCP lease width"),
        generation: read_u32(&value.bytes, 29)?,
        phase: lease_phase_from_tag(value.bytes[33])?,
        expires_at_tick: (expires_at_tick != u64::MAX).then_some(expires_at_tick),
        server: value.bytes[42..58]
            .try_into()
            .expect("exact DHCP lease width"),
    };
    lease
        .validate()
        .map_err(|reason| runtime_error(reason, "invalid DHCP lease"))?;
    Ok(lease)
}

pub(crate) fn service_registration_value(
    registration: NetworkServiceRegistration,
) -> Result<Value, RuntimeError> {
    registration
        .validate()
        .map_err(|reason| runtime_error(reason, "invalid service registration"))?;
    let mut bytes = Vec::with_capacity(100);
    bytes.extend_from_slice(b"CNRS");
    bytes.push(registration.name_bytes);
    bytes.extend_from_slice(&registration.name);
    bytes.push(family_tag(registration.family));
    bytes.extend_from_slice(&registration.address);
    push_u16(&mut bytes, registration.port);
    bytes.push(transport_tag(registration.protocol));
    push_u32(&mut bytes, registration.generation);
    push_u64(&mut bytes, registration.expires_at_tick);
    Ok(Value {
        value_type: SERVICE_REGISTRATION_TYPE,
        bytes,
    })
}

pub(crate) fn parse_service_registration(
    value: &Value,
) -> Result<NetworkServiceRegistration, RuntimeError> {
    if value.value_type != SERVICE_REGISTRATION_TYPE
        || !value.bytes.starts_with(b"CNRS")
        || value.bytes.len() != 100
    {
        return Err(runtime_error(
            NetworkReason::MalformedPacket,
            "value is not an exact service registration",
        ));
    }
    let registration = NetworkServiceRegistration {
        name_bytes: value.bytes[4],
        name: value.bytes[5..68]
            .try_into()
            .expect("exact service-registration width"),
        family: family_from_tag(value.bytes[68])?,
        address: value.bytes[69..85]
            .try_into()
            .expect("exact service-registration width"),
        port: read_u16(&value.bytes, 85)?,
        protocol: transport_from_tag(value.bytes[87])?,
        generation: read_u32(&value.bytes, 88)?,
        expires_at_tick: read_u64(&value.bytes, 92)?,
    };
    registration
        .validate()
        .map_err(|reason| runtime_error(reason, "invalid service registration"))?;
    Ok(registration)
}

fn reachability_scope_tag(scope: ReachabilityScope) -> u8 {
    match scope {
        ReachabilityScope::LinkLocal => 0,
        ReachabilityScope::LocalNetwork => 1,
        ReachabilityScope::Routed => 2,
        ReachabilityScope::Internet => 3,
    }
}

#[cfg(test)]
fn reachability_scope_from_tag(tag: u8) -> Result<ReachabilityScope, RuntimeError> {
    match tag {
        0 => Ok(ReachabilityScope::LinkLocal),
        1 => Ok(ReachabilityScope::LocalNetwork),
        2 => Ok(ReachabilityScope::Routed),
        3 => Ok(ReachabilityScope::Internet),
        _ => Err(runtime_error(
            NetworkReason::MalformedPacket,
            "unknown reachability scope",
        )),
    }
}

fn reachability_outcome_tag(outcome: ReachabilityOutcome) -> u8 {
    match outcome {
        ReachabilityOutcome::Reachable => 0,
        ReachabilityOutcome::Unreachable => 1,
        ReachabilityOutcome::TimedOut => 2,
        ReachabilityOutcome::RateLimited => 3,
        ReachabilityOutcome::Unsupported => 4,
        ReachabilityOutcome::ProviderLost => 5,
    }
}

#[cfg(test)]
fn reachability_outcome_from_tag(tag: u8) -> Result<ReachabilityOutcome, RuntimeError> {
    match tag {
        0 => Ok(ReachabilityOutcome::Reachable),
        1 => Ok(ReachabilityOutcome::Unreachable),
        2 => Ok(ReachabilityOutcome::TimedOut),
        3 => Ok(ReachabilityOutcome::RateLimited),
        4 => Ok(ReachabilityOutcome::Unsupported),
        5 => Ok(ReachabilityOutcome::ProviderLost),
        _ => Err(runtime_error(
            NetworkReason::MalformedPacket,
            "unknown reachability outcome",
        )),
    }
}

pub(crate) fn reachability_value(
    observation: NetworkReachabilityObservation,
) -> Result<Value, RuntimeError> {
    observation
        .validate(observation.observed_at_tick)
        .map_err(|reason| runtime_error(reason, "invalid reachability observation"))?;
    let mut bytes = Vec::with_capacity(47);
    bytes.extend_from_slice(b"CNRO");
    bytes.push(family_tag(observation.family));
    bytes.extend_from_slice(&observation.target);
    bytes.push(reachability_scope_tag(observation.scope));
    bytes.push(reachability_outcome_tag(observation.outcome));
    push_u64(&mut bytes, observation.latency_ticks.unwrap_or(u64::MAX));
    push_u64(&mut bytes, observation.observed_at_tick);
    push_u64(&mut bytes, observation.valid_until_tick);
    Ok(Value {
        value_type: REACHABILITY_OBSERVATION_TYPE,
        bytes,
    })
}

#[cfg(test)]
pub(crate) fn parse_reachability(
    value: &Value,
) -> Result<NetworkReachabilityObservation, RuntimeError> {
    if value.value_type != REACHABILITY_OBSERVATION_TYPE
        || !value.bytes.starts_with(b"CNRO")
        || value.bytes.len() != 47
    {
        return Err(runtime_error(
            NetworkReason::MalformedPacket,
            "value is not an exact reachability observation",
        ));
    }
    let latency_ticks = read_u64(&value.bytes, 23)?;
    let observation = NetworkReachabilityObservation {
        family: family_from_tag(value.bytes[4])?,
        target: value.bytes[5..21]
            .try_into()
            .expect("exact reachability width"),
        scope: reachability_scope_from_tag(value.bytes[21])?,
        outcome: reachability_outcome_from_tag(value.bytes[22])?,
        latency_ticks: (latency_ticks != u64::MAX).then_some(latency_ticks),
        observed_at_tick: read_u64(&value.bytes, 31)?,
        valid_until_tick: read_u64(&value.bytes, 39)?,
    };
    observation
        .validate(observation.observed_at_tick)
        .map_err(|reason| runtime_error(reason, "invalid reachability observation"))?;
    Ok(observation)
}

fn family_tag(family: AddressFamily) -> u8 {
    match family {
        AddressFamily::Ipv4 => 4,
        AddressFamily::Ipv6 => 6,
    }
}

fn family_from_tag(tag: u8) -> Result<AddressFamily, RuntimeError> {
    match tag {
        4 => Ok(AddressFamily::Ipv4),
        6 => Ok(AddressFamily::Ipv6),
        _ => Err(runtime_error(
            NetworkReason::MalformedPacket,
            "unknown network address family",
        )),
    }
}

fn transport_tag(protocol: TransportProtocol) -> u8 {
    match protocol {
        TransportProtocol::Tcp => 1,
        TransportProtocol::Udp => 2,
    }
}

fn transport_from_tag(tag: u8) -> Result<TransportProtocol, RuntimeError> {
    match tag {
        1 => Ok(TransportProtocol::Tcp),
        2 => Ok(TransportProtocol::Udp),
        _ => Err(runtime_error(
            NetworkReason::MalformedPacket,
            "unknown transport protocol",
        )),
    }
}

fn lifecycle_tag(lifecycle: SessionLifecycle) -> u8 {
    match lifecycle {
        SessionLifecycle::Accepted => 0,
        SessionLifecycle::Connected => 1,
        SessionLifecycle::Authenticated => 2,
        SessionLifecycle::HalfClosed => 3,
        SessionLifecycle::Draining => 4,
        SessionLifecycle::Closed => 5,
        SessionLifecycle::TimedOut => 6,
        SessionLifecycle::Reset => 7,
        SessionLifecycle::Cancelled => 8,
        SessionLifecycle::Failed => 9,
    }
}

fn lifecycle_from_tag(tag: u8) -> Result<SessionLifecycle, RuntimeError> {
    match tag {
        0 => Ok(SessionLifecycle::Accepted),
        1 => Ok(SessionLifecycle::Connected),
        2 => Ok(SessionLifecycle::Authenticated),
        3 => Ok(SessionLifecycle::HalfClosed),
        4 => Ok(SessionLifecycle::Draining),
        5 => Ok(SessionLifecycle::Closed),
        6 => Ok(SessionLifecycle::TimedOut),
        7 => Ok(SessionLifecycle::Reset),
        8 => Ok(SessionLifecycle::Cancelled),
        9 => Ok(SessionLifecycle::Failed),
        _ => Err(runtime_error(
            NetworkReason::MalformedPacket,
            "unknown session lifecycle",
        )),
    }
}

fn session_value(session: crate::NetworkSession) -> Value {
    let mut bytes = Vec::with_capacity(64);
    bytes.extend_from_slice(b"CNS0");
    push_u64(&mut bytes, session.identity);
    push_u32(&mut bytes, session.generation);
    bytes.push(family_tag(session.family));
    bytes.push(transport_tag(session.protocol));
    bytes.extend_from_slice(&session.local);
    push_u16(&mut bytes, session.local_port);
    bytes.extend_from_slice(&session.peer);
    push_u16(&mut bytes, session.peer_port);
    bytes.push(lifecycle_tag(session.lifecycle));
    bytes.push(u8::from(session.authenticated));
    push_u64(&mut bytes, session.expires_at_tick);
    Value {
        value_type: SESSION_TYPE,
        bytes,
    }
}

fn parse_session(value: &Value) -> Result<crate::NetworkSession, RuntimeError> {
    if value.value_type != SESSION_TYPE
        || !value.bytes.starts_with(b"CNS0")
        || value.bytes.len() != 64
    {
        return Err(runtime_error(
            NetworkReason::MalformedPacket,
            "value is not an exact network session",
        ));
    }
    Ok(crate::NetworkSession {
        identity: read_u64(&value.bytes, 4)?,
        generation: read_u32(&value.bytes, 12)?,
        family: family_from_tag(value.bytes[16])?,
        protocol: transport_from_tag(value.bytes[17])?,
        local: value.bytes[18..34].try_into().expect("exact session width"),
        local_port: read_u16(&value.bytes, 34)?,
        peer: value.bytes[36..52].try_into().expect("exact session width"),
        peer_port: read_u16(&value.bytes, 52)?,
        lifecycle: lifecycle_from_tag(value.bytes[54])?,
        authenticated: value.bytes[55] != 0,
        expires_at_tick: read_u64(&value.bytes, 56)?,
    })
}

#[derive(Default)]
struct LinkObserver {
    generation: u32,
    deadline_tick: Option<u64>,
}

impl Handler for LinkObserver {
    fn step(
        &mut self,
        _node: &Node,
        inputs: &[Value],
        context: HostedServiceStepContext,
        _io: &mut RunIo<'_>,
    ) -> Result<HostedServiceStep, RuntimeError> {
        if !inputs.is_empty() {
            return Err(runtime_error(
                NetworkReason::MalformedPacket,
                "link observer received hidden input",
            ));
        }
        if let Some(deadline_tick) = self.deadline_tick {
            if context.tick < deadline_tick {
                return Ok(HostedServiceStep::waiting(HostedServiceInterest::Timer {
                    subject: Id("conduit/net-link-observe"),
                    deadline_tick,
                }));
            }
        }
        self.generation = self
            .generation
            .checked_add(1)
            .ok_or_else(|| runtime_error(NetworkReason::Bounds, "link generation exhausted"))?;
        let next_tick = context
            .tick
            .checked_add(10)
            .ok_or_else(|| runtime_error(NetworkReason::Bounds, "link timer overflow"))?;
        let valid_until_tick = context
            .tick
            .checked_add(20)
            .ok_or_else(|| runtime_error(NetworkReason::Bounds, "link freshness overflow"))?;
        self.deadline_tick = Some(next_tick);
        let observation = LinkObservation {
            interface: 1,
            generation: self.generation,
            kind: LinkKind::Virtual,
            carrier: true,
            mtu: 1_500,
            address_ready: true,
            availability: NetworkAvailability::Active,
            observed_at_tick: context.tick,
            valid_until_tick,
        };
        Ok(HostedServiceStep::produced(vec![
            link_value(observation),
            control_value(NetworkControlEvent {
                identity: 1,
                generation: self.generation,
                kind: NetworkControlKind::Link,
                outcome: NetworkControlOutcome::Observed,
                tick: context.tick,
            }),
        ]))
    }
}

#[derive(Default)]
struct FrameSource {
    deadline_tick: Option<u64>,
}

impl Handler for FrameSource {
    fn step(
        &mut self,
        _node: &Node,
        inputs: &[Value],
        context: HostedServiceStepContext,
        _io: &mut RunIo<'_>,
    ) -> Result<HostedServiceStep, RuntimeError> {
        if !inputs.is_empty() {
            return Err(runtime_error(
                NetworkReason::MalformedPacket,
                "frame source received hidden input",
            ));
        }
        if let Some(deadline_tick) = self.deadline_tick
            && context.tick < deadline_tick
        {
            return Ok(HostedServiceStep::waiting(HostedServiceInterest::Timer {
                subject: Id("conduit/net-frame-source"),
                deadline_tick,
            }));
        }
        self.deadline_tick = Some(
            context
                .tick
                .checked_add(1_000)
                .ok_or_else(|| runtime_error(NetworkReason::Bounds, "frame timer overflow"))?,
        );
        Ok(HostedServiceStep::produced(vec![frame_value(
            &NetworkFrame {
                interface: 1,
                direction: NetworkDirection::Ingress,
                protocol: Some(0x0800),
                observed_at_tick: context.tick,
                payload: vec![0x5a; 64],
            },
        )?]))
    }
}

#[derive(Default)]
struct FrameSink {
    frames: u32,
    bytes: u32,
}

impl Handler for FrameSink {
    fn step(
        &mut self,
        _node: &Node,
        inputs: &[Value],
        context: HostedServiceStepContext,
        _io: &mut RunIo<'_>,
    ) -> Result<HostedServiceStep, RuntimeError> {
        let [input] = inputs else {
            return Err(runtime_error(
                NetworkReason::MalformedPacket,
                "frame sink requires one frame",
            ));
        };
        let frame = parse_frame(input)?;
        self.frames = self
            .frames
            .checked_add(1)
            .ok_or_else(|| runtime_error(NetworkReason::Bounds, "frame counter overflow"))?;
        self.bytes = self
            .bytes
            .checked_add(
                u32::try_from(frame.payload.len()).map_err(|_| {
                    runtime_error(NetworkReason::Bounds, "frame byte counter overflow")
                })?,
            )
            .ok_or_else(|| runtime_error(NetworkReason::Bounds, "frame byte counter overflow"))?;
        Ok(HostedServiceStep::produced(vec![state_value(
            RetainedNetworkState {
                table: 4,
                generation: self.frames,
                items: 1,
                bytes: self.bytes,
                observed_at_tick: context.tick,
                expires_at_tick: None,
                policy: RetainedStatePolicy::ReplaceLatest,
            },
        )]))
    }
}

#[derive(Default)]
struct DatagramSource {
    sequence: u64,
    deadline_tick: Option<u64>,
}

impl Handler for DatagramSource {
    fn step(
        &mut self,
        _node: &Node,
        inputs: &[Value],
        context: HostedServiceStepContext,
        _io: &mut RunIo<'_>,
    ) -> Result<HostedServiceStep, RuntimeError> {
        if !inputs.is_empty() {
            return Err(runtime_error(
                NetworkReason::MalformedPacket,
                "datagram source received hidden input",
            ));
        }
        if let Some(deadline_tick) = self.deadline_tick
            && context.tick < deadline_tick
        {
            return Ok(HostedServiceStep::waiting(HostedServiceInterest::Timer {
                subject: Id("conduit/net-datagram-source"),
                deadline_tick,
            }));
        }
        self.sequence = self
            .sequence
            .checked_add(1)
            .ok_or_else(|| runtime_error(NetworkReason::Bounds, "datagram sequence exhausted"))?;
        self.deadline_tick = Some(
            context
                .tick
                .checked_add(1_000)
                .ok_or_else(|| runtime_error(NetworkReason::Bounds, "datagram timer overflow"))?,
        );
        let mut source = [0; 16];
        source[..4].copy_from_slice(&[10, 0, 0, 2]);
        let mut destination = [0; 16];
        destination[..4].copy_from_slice(&[10, 0, 0, 3]);
        Ok(HostedServiceStep::produced(vec![datagram_value(
            &NetworkDatagram {
                session: None,
                family: AddressFamily::Ipv4,
                source,
                source_port: 30_000,
                destination,
                destination_port: 30_001,
                sequence: self.sequence,
                delivery: DatagramDelivery::Pending,
                payload: vec![u8::try_from(self.sequence & 0xff).expect("masked"); 64],
            },
        )?]))
    }
}

struct DatagramImpair;

impl Handler for DatagramImpair {
    fn step(
        &mut self,
        _node: &Node,
        inputs: &[Value],
        _context: HostedServiceStepContext,
        _io: &mut RunIo<'_>,
    ) -> Result<HostedServiceStep, RuntimeError> {
        let [input] = inputs else {
            return Err(runtime_error(
                NetworkReason::MalformedPacket,
                "datagram impairment requires one datagram",
            ));
        };
        let mut datagram = parse_datagram(input)?;
        datagram.delivery = match datagram.sequence % 4 {
            1 => DatagramDelivery::Delivered,
            2 => DatagramDelivery::Lost,
            3 => DatagramDelivery::Duplicated,
            _ => DatagramDelivery::Reordered,
        };
        Ok(HostedServiceStep::produced(vec![datagram_value(
            &datagram,
        )?]))
    }
}

#[derive(Default)]
struct DatagramSink {
    datagrams: u32,
    bytes: u32,
}

impl Handler for DatagramSink {
    fn step(
        &mut self,
        _node: &Node,
        inputs: &[Value],
        context: HostedServiceStepContext,
        _io: &mut RunIo<'_>,
    ) -> Result<HostedServiceStep, RuntimeError> {
        let [input] = inputs else {
            return Err(runtime_error(
                NetworkReason::MalformedPacket,
                "datagram sink requires one datagram",
            ));
        };
        let datagram = parse_datagram(input)?;
        self.datagrams = self
            .datagrams
            .checked_add(1)
            .ok_or_else(|| runtime_error(NetworkReason::Bounds, "datagram counter overflow"))?;
        self.bytes = self
            .bytes
            .checked_add(u32::try_from(datagram.payload.len()).map_err(|_| {
                runtime_error(NetworkReason::Bounds, "datagram byte counter overflow")
            })?)
            .ok_or_else(|| {
                runtime_error(NetworkReason::Bounds, "datagram byte counter overflow")
            })?;
        Ok(HostedServiceStep::produced(vec![state_value(
            RetainedNetworkState {
                table: 5,
                generation: self.datagrams,
                items: 1,
                bytes: self.bytes,
                observed_at_tick: context.tick,
                expires_at_tick: None,
                policy: RetainedStatePolicy::ReplaceLatest,
            },
        )]))
    }
}

#[derive(Default)]
struct StreamSource {
    offset: u64,
    deadline_tick: Option<u64>,
}

impl Handler for StreamSource {
    fn step(
        &mut self,
        _node: &Node,
        inputs: &[Value],
        context: HostedServiceStepContext,
        _io: &mut RunIo<'_>,
    ) -> Result<HostedServiceStep, RuntimeError> {
        if !inputs.is_empty() {
            return Err(runtime_error(
                NetworkReason::MalformedPacket,
                "stream source received hidden input",
            ));
        }
        if let Some(deadline_tick) = self.deadline_tick
            && context.tick < deadline_tick
        {
            return Ok(HostedServiceStep::waiting(HostedServiceInterest::Timer {
                subject: Id("conduit/net-stream-source"),
                deadline_tick,
            }));
        }
        let chunk = ByteStreamChunk {
            session: 1,
            offset: self.offset,
            eof: false,
            read_half_closed: false,
            write_half_closed: false,
            pressure: StreamPressure::Ready,
            bytes: vec![0x42; 64],
        };
        self.offset = self
            .offset
            .checked_add(64)
            .ok_or_else(|| runtime_error(NetworkReason::Bounds, "stream offset exhausted"))?;
        self.deadline_tick = Some(
            context
                .tick
                .checked_add(1_000)
                .ok_or_else(|| runtime_error(NetworkReason::Bounds, "stream timer overflow"))?,
        );
        Ok(HostedServiceStep::produced(vec![stream_value(&chunk)?]))
    }
}

#[derive(Default)]
struct StreamSink {
    chunks: u32,
    bytes: u32,
    next_offset: u64,
}

impl Handler for StreamSink {
    fn step(
        &mut self,
        _node: &Node,
        inputs: &[Value],
        context: HostedServiceStepContext,
        _io: &mut RunIo<'_>,
    ) -> Result<HostedServiceStep, RuntimeError> {
        let [input] = inputs else {
            return Err(runtime_error(
                NetworkReason::MalformedPacket,
                "stream sink requires one chunk",
            ));
        };
        let chunk = parse_stream(input)?;
        if chunk.offset != self.next_offset {
            return Err(runtime_error(
                NetworkReason::StaleGeneration,
                "stream offset is not contiguous",
            ));
        }
        self.next_offset = chunk
            .offset
            .checked_add(
                u64::try_from(chunk.bytes.len())
                    .map_err(|_| runtime_error(NetworkReason::Bounds, "stream offset overflow"))?,
            )
            .ok_or_else(|| runtime_error(NetworkReason::Bounds, "stream offset overflow"))?;
        self.chunks = self
            .chunks
            .checked_add(1)
            .ok_or_else(|| runtime_error(NetworkReason::Bounds, "stream chunk counter overflow"))?;
        self.bytes = self
            .bytes
            .checked_add(u32::try_from(chunk.bytes.len()).map_err(|_| {
                runtime_error(NetworkReason::Bounds, "stream byte counter overflow")
            })?)
            .ok_or_else(|| runtime_error(NetworkReason::Bounds, "stream byte counter overflow"))?;
        Ok(HostedServiceStep::produced(vec![state_value(
            RetainedNetworkState {
                table: 6,
                generation: self.chunks,
                items: 1,
                bytes: self.bytes,
                observed_at_tick: context.tick,
                expires_at_tick: None,
                policy: RetainedStatePolicy::ReplaceLatest,
            },
        )]))
    }
}

#[derive(Default)]
struct PacketSource {
    sequence: u64,
    deadline_tick: Option<u64>,
}

impl Handler for PacketSource {
    fn step(
        &mut self,
        _node: &Node,
        inputs: &[Value],
        context: HostedServiceStepContext,
        _io: &mut RunIo<'_>,
    ) -> Result<HostedServiceStep, RuntimeError> {
        if !inputs.is_empty() {
            return Err(runtime_error(
                NetworkReason::MalformedPacket,
                "packet source received hidden input",
            ));
        }
        if let Some(deadline_tick) = self.deadline_tick {
            if context.tick < deadline_tick {
                return Ok(HostedServiceStep::waiting(HostedServiceInterest::Timer {
                    subject: Id("conduit/net-packet-source"),
                    deadline_tick,
                }));
            }
        }
        self.sequence = self
            .sequence
            .checked_add(1)
            .ok_or_else(|| runtime_error(NetworkReason::Bounds, "packet sequence exhausted"))?;
        self.deadline_tick = Some(
            context
                .tick
                .checked_add(10)
                .ok_or_else(|| runtime_error(NetworkReason::Bounds, "packet timer overflow"))?,
        );
        let packet = NetworkPacket::ipv4(
            self.sequence,
            Ipv4Address([10, 0, 0, 2]),
            Ipv4Address([10, 1, 0, 2]),
            4,
            vec![u8::try_from(self.sequence & 0xff).expect("masked"); 64],
        )
        .map_err(|reason| runtime_error(reason, "packet source failed"))?;
        Ok(HostedServiceStep::produced(vec![packet_value(&packet)?]))
    }
}

struct PacketClassifier;

impl Handler for PacketClassifier {
    fn step(
        &mut self,
        _node: &Node,
        inputs: &[Value],
        _context: HostedServiceStepContext,
        _io: &mut RunIo<'_>,
    ) -> Result<HostedServiceStep, RuntimeError> {
        let [input] = inputs else {
            return Err(runtime_error(
                NetworkReason::MalformedPacket,
                "packet classifier requires one packet",
            ));
        };
        let mut packet = parse_packet(input)?;
        if packet.family != AddressFamily::Ipv4 || packet.destination[0] != 10 {
            packet.disposition = PacketDisposition::PolicyDenied;
        }
        Ok(HostedServiceStep::produced(vec![packet_value(&packet)?]))
    }
}

struct PacketRouter {
    routes: RouteTable,
}

impl Default for PacketRouter {
    fn default() -> Self {
        let mut routes = RouteTable::new();
        routes
            .install(RouteEntry::ipv4(
                Ipv4Address([10, 1, 0, 0]),
                16,
                2,
                1_500,
                true,
            ))
            .expect("constant route is valid");
        Self { routes }
    }
}

/// Independent finite userspace implementation of the same exact semantic
/// routing contract. It intentionally does not share `RouteTable` with the
/// reference implementation, so substitution exercises an implementation
/// boundary rather than a second manifest name over the same handler.
struct NativeUserspacePacketRouter;

impl Handler for NativeUserspacePacketRouter {
    fn step(
        &mut self,
        _node: &Node,
        inputs: &[Value],
        _context: HostedServiceStepContext,
        _io: &mut RunIo<'_>,
    ) -> Result<HostedServiceStep, RuntimeError> {
        let [input] = inputs else {
            return Err(runtime_error(
                NetworkReason::MalformedPacket,
                "native userspace router requires one packet",
            ));
        };
        let mut packet = parse_packet(input)?;
        if packet.disposition == PacketDisposition::Pending {
            if packet.hop_limit <= 1 {
                packet.disposition = PacketDisposition::HopExhausted;
            } else if packet.family != AddressFamily::Ipv4 || packet.destination[..2] != [10, 1] {
                packet.disposition = PacketDisposition::NoRoute;
            } else if packet.payload.len() > 1_500 {
                packet.disposition = PacketDisposition::MtuExceeded;
            } else {
                packet.hop_limit -= 1;
                packet.egress_interface = Some(2);
                packet.disposition = PacketDisposition::Forwarded;
            }
        }
        Ok(HostedServiceStep::produced(vec![packet_value(&packet)?]))
    }
}

impl Handler for PacketRouter {
    fn step(
        &mut self,
        _node: &Node,
        inputs: &[Value],
        _context: HostedServiceStepContext,
        _io: &mut RunIo<'_>,
    ) -> Result<HostedServiceStep, RuntimeError> {
        let [input] = inputs else {
            return Err(runtime_error(
                NetworkReason::MalformedPacket,
                "router requires one packet",
            ));
        };
        let packet = parse_packet(input)?;
        let packet = if packet.disposition == PacketDisposition::Pending {
            self.routes.forward(packet)
        } else {
            packet
        };
        Ok(HostedServiceStep::produced(vec![packet_value(&packet)?]))
    }
}

#[derive(Default)]
struct PacketSink {
    packets: u32,
    bytes: u32,
}

impl Handler for PacketSink {
    fn step(
        &mut self,
        _node: &Node,
        inputs: &[Value],
        context: HostedServiceStepContext,
        _io: &mut RunIo<'_>,
    ) -> Result<HostedServiceStep, RuntimeError> {
        let [input] = inputs else {
            return Err(runtime_error(
                NetworkReason::MalformedPacket,
                "packet sink requires one packet",
            ));
        };
        let packet = parse_packet(input)?;
        self.packets = self
            .packets
            .checked_add(1)
            .ok_or_else(|| runtime_error(NetworkReason::Bounds, "packet counter overflow"))?;
        self.bytes = self
            .bytes
            .checked_add(u32::try_from(packet.payload.len()).map_err(|_| {
                runtime_error(NetworkReason::Bounds, "packet byte counter overflow")
            })?)
            .ok_or_else(|| runtime_error(NetworkReason::Bounds, "packet byte counter overflow"))?;
        Ok(HostedServiceStep::produced(vec![state_value(
            RetainedNetworkState {
                table: 1,
                generation: self.packets,
                items: 1,
                bytes: self.bytes,
                observed_at_tick: context.tick,
                expires_at_tick: None,
                policy: RetainedStatePolicy::ReplaceLatest,
            },
        )]))
    }
}

#[derive(Default)]
struct NetworkMeter {
    packets: u32,
    bytes: u32,
}

impl Handler for NetworkMeter {
    fn step(
        &mut self,
        _node: &Node,
        inputs: &[Value],
        context: HostedServiceStepContext,
        _io: &mut RunIo<'_>,
    ) -> Result<HostedServiceStep, RuntimeError> {
        let [input] = inputs else {
            return Err(runtime_error(
                NetworkReason::MalformedPacket,
                "network meter requires one packet",
            ));
        };
        let packet = parse_packet(input)?;
        self.packets = self
            .packets
            .checked_add(1)
            .ok_or_else(|| runtime_error(NetworkReason::Bounds, "meter counter overflow"))?;
        self.bytes = self
            .bytes
            .checked_add(
                u32::try_from(packet.payload.len()).map_err(|_| {
                    runtime_error(NetworkReason::Bounds, "meter byte counter overflow")
                })?,
            )
            .ok_or_else(|| runtime_error(NetworkReason::Bounds, "meter byte counter overflow"))?;
        Ok(HostedServiceStep::produced(vec![state_value(
            RetainedNetworkState {
                table: 2,
                generation: self.packets,
                items: 1,
                bytes: self.bytes,
                observed_at_tick: context.tick,
                expires_at_tick: None,
                policy: RetainedStatePolicy::ReplaceLatest,
            },
        )]))
    }
}

#[derive(Default)]
struct SessionListener {
    sessions: SessionTable,
    deadline_tick: Option<u64>,
    stop: Option<StopPolicy>,
    cleanup_remaining: usize,
}

#[derive(Clone, Copy)]
struct SessionMarker {
    identity: u64,
    generation: u32,
    lifecycle: SessionLifecycle,
}

struct ServiceObserver {
    sessions: [Option<SessionMarker>; MAXIMUM_SESSIONS * 2],
    events: [Option<NetworkControlEvent>; MAXIMUM_SESSIONS * 2],
    last_state_generation: u32,
    correlated_events: u32,
}

impl Default for ServiceObserver {
    fn default() -> Self {
        Self {
            sessions: [None; MAXIMUM_SESSIONS * 2],
            events: [None; MAXIMUM_SESSIONS * 2],
            last_state_generation: 0,
            correlated_events: 0,
        }
    }
}

impl ServiceObserver {
    fn observe_session(&mut self, session: crate::NetworkSession) -> Result<(), RuntimeError> {
        let marker = SessionMarker {
            identity: session.identity,
            generation: session.generation,
            lifecycle: session.lifecycle,
        };
        if let Some(slot) = self.sessions.iter_mut().find(|slot| {
            slot.is_some_and(|known| {
                known.identity == marker.identity
                    && known.generation == marker.generation
                    && known.lifecycle == marker.lifecycle
            })
        }) {
            *slot = Some(marker);
            return Ok(());
        }
        if self
            .sessions
            .iter()
            .flatten()
            .any(|known| known.identity == marker.identity && known.generation != marker.generation)
        {
            return Err(runtime_error(
                NetworkReason::StaleGeneration,
                "session identity was observed with two generations",
            ));
        }
        let slot = self
            .sessions
            .iter_mut()
            .find(|slot| slot.is_none())
            .ok_or_else(|| {
                runtime_error(
                    NetworkReason::Bounds,
                    "service observer session correlation window is full",
                )
            })?;
        *slot = Some(marker);
        Ok(())
    }

    fn observe_event(&mut self, event: NetworkControlEvent) -> Result<(), RuntimeError> {
        if let Some(slot) = self.events.iter_mut().find(|slot| {
            slot.is_some_and(|known| {
                known.identity == event.identity
                    && known.generation == event.generation
                    && known.outcome == event.outcome
            })
        }) {
            *slot = Some(event);
            return Ok(());
        }
        if self
            .events
            .iter()
            .flatten()
            .any(|known| known.identity == event.identity && known.generation != event.generation)
        {
            return Err(runtime_error(
                NetworkReason::StaleGeneration,
                "control-event identity was observed with two generations",
            ));
        }
        let slot = self
            .events
            .iter_mut()
            .find(|slot| slot.is_none())
            .ok_or_else(|| {
                runtime_error(
                    NetworkReason::Bounds,
                    "service observer event correlation window is full",
                )
            })?;
        *slot = Some(event);
        Ok(())
    }

    fn correlate(&mut self) -> Result<(), RuntimeError> {
        fn expected_outcome(lifecycle: SessionLifecycle) -> NetworkControlOutcome {
            match lifecycle {
                SessionLifecycle::Accepted => NetworkControlOutcome::Admitted,
                SessionLifecycle::Connected
                | SessionLifecycle::Authenticated
                | SessionLifecycle::HalfClosed
                | SessionLifecycle::Draining
                | SessionLifecycle::Closed => NetworkControlOutcome::Applied,
                SessionLifecycle::TimedOut => NetworkControlOutcome::Expired,
                SessionLifecycle::Reset | SessionLifecycle::Failed => NetworkControlOutcome::Failed,
                SessionLifecycle::Cancelled => NetworkControlOutcome::Cancelled,
            }
        }

        loop {
            let Some((session_index, session)) =
                self.sessions
                    .iter()
                    .enumerate()
                    .find_map(|(index, session)| {
                        session.and_then(|session| {
                            self.events
                                .iter()
                                .flatten()
                                .any(|event| {
                                    event.identity == session.identity
                                        && event.generation == session.generation
                                        && event.outcome == expected_outcome(session.lifecycle)
                                })
                                .then_some((index, session))
                        })
                    })
            else {
                return Ok(());
            };
            let event_index = self
                .events
                .iter()
                .position(|event| {
                    event.is_some_and(|event| {
                        event.identity == session.identity
                            && event.generation == session.generation
                            && event.outcome == expected_outcome(session.lifecycle)
                    })
                })
                .expect("matching event was found");
            let event = self.events[event_index].expect("matching event is present");
            debug_assert_eq!(event.outcome, expected_outcome(session.lifecycle));
            self.sessions[session_index] = None;
            self.events[event_index] = None;
            self.correlated_events = self.correlated_events.checked_add(1).ok_or_else(|| {
                runtime_error(NetworkReason::Bounds, "correlation counter overflow")
            })?;
        }
    }
}

impl Handler for ServiceObserver {
    fn step(
        &mut self,
        _node: &Node,
        inputs: &[Value],
        context: HostedServiceStepContext,
        _io: &mut RunIo<'_>,
    ) -> Result<HostedServiceStep, RuntimeError> {
        if inputs.len() != 3 {
            return Err(runtime_error(
                NetworkReason::MalformedPacket,
                "service observer requires correlated session, event, and state values",
            ));
        }
        let session = inputs
            .iter()
            .find(|value| value.value_type == SESSION_TYPE)
            .ok_or_else(|| runtime_error(NetworkReason::MalformedPacket, "missing network session"))
            .and_then(parse_session)?;
        let event = inputs
            .iter()
            .find(|value| value.value_type == CONTROL_EVENT_TYPE)
            .ok_or_else(|| {
                runtime_error(
                    NetworkReason::MalformedPacket,
                    "missing network control event",
                )
            })
            .and_then(parse_control)?;
        let state = inputs
            .iter()
            .find(|value| value.value_type == RETAINED_NETWORK_STATE_TYPE)
            .ok_or_else(|| {
                runtime_error(
                    NetworkReason::MalformedPacket,
                    "missing retained network state",
                )
            })
            .and_then(parse_state)?;
        if event.kind != NetworkControlKind::Session {
            return Err(runtime_error(
                NetworkReason::StaleGeneration,
                "service observer received a non-session control event",
            ));
        }
        if event.identity == session.identity && event.generation != session.generation {
            return Err(runtime_error(
                NetworkReason::StaleGeneration,
                "session identity or generation does not match its control event",
            ));
        }
        if state.table != 3
            || state.generation == 0
            || state.generation < self.last_state_generation
            || state.observed_at_tick > context.tick
            || state.policy != RetainedStatePolicy::GenerationFenced
        {
            return Err(runtime_error(
                NetworkReason::StaleGeneration,
                "service state is not a valid generation-fenced projection",
            ));
        }
        self.last_state_generation = state.generation;
        self.observe_session(session)?;
        self.observe_event(event)?;
        self.correlate()?;
        Ok(HostedServiceStep::produced(Vec::new()))
    }
}

impl Handler for SessionListener {
    fn step(
        &mut self,
        _node: &Node,
        inputs: &[Value],
        context: HostedServiceStepContext,
        _io: &mut RunIo<'_>,
    ) -> Result<HostedServiceStep, RuntimeError> {
        if !inputs.is_empty() {
            return Err(runtime_error(
                NetworkReason::MalformedPacket,
                "listener received hidden input",
            ));
        }
        if let Some(deadline_tick) = self.deadline_tick {
            if context.tick < deadline_tick {
                return Ok(HostedServiceStep::waiting(HostedServiceInterest::Timer {
                    subject: Id("conduit/net-session-listen"),
                    deadline_tick,
                }));
            }
        }
        let (session, outcome) = if let Some(expired) = self
            .sessions
            .expire_one(context.tick)
            .map_err(|reason| runtime_error(reason, "bounded session expiry failed"))?
        {
            (expired, NetworkControlOutcome::Expired)
        } else {
            let peer_port = 40_000_u16
                .checked_add(u16::try_from(self.sessions.len()).map_err(|_| {
                    runtime_error(NetworkReason::Bounds, "session table length overflow")
                })?)
                .ok_or_else(|| runtime_error(NetworkReason::Bounds, "peer port overflow"))?;
            let mut local = [0; 16];
            local[..4].copy_from_slice(&[127, 0, 0, 1]);
            let mut peer = [0; 16];
            peer[..4].copy_from_slice(&[10, 0, 0, 2]);
            (
                self.sessions
                    .accept(SessionAdmission {
                        protocol: TransportProtocol::Tcp,
                        local,
                        local_port: 8080,
                        peer,
                        peer_port,
                        now_tick: context.tick,
                        timeout_ticks: 25,
                    })
                    .map_err(|reason| runtime_error(reason, "bounded listener admission failed"))?,
                NetworkControlOutcome::Admitted,
            )
        };
        self.deadline_tick = Some(
            context
                .tick
                .checked_add(10)
                .ok_or_else(|| runtime_error(NetworkReason::Bounds, "listener timer overflow"))?,
        );
        let items = u16::try_from(self.sessions.len())
            .map_err(|_| runtime_error(NetworkReason::Bounds, "session item count overflow"))?;
        let bytes = self
            .sessions
            .len()
            .checked_mul(core::mem::size_of::<crate::NetworkSession>())
            .and_then(|bytes| u32::try_from(bytes).ok())
            .ok_or_else(|| runtime_error(NetworkReason::Bounds, "session byte count overflow"))?;
        Ok(HostedServiceStep::produced(vec![
            session_value(session),
            control_value(NetworkControlEvent {
                identity: session.identity,
                generation: session.generation,
                kind: NetworkControlKind::Session,
                outcome,
                tick: context.tick,
            }),
            state_value(RetainedNetworkState {
                table: 3,
                generation: self.sessions.generation(),
                items,
                bytes,
                observed_at_tick: context.tick,
                expires_at_tick: self.sessions.next_expiry(),
                policy: RetainedStatePolicy::GenerationFenced,
            }),
        ]))
    }

    fn cancel(&mut self, _node: &Node, stop: StopPolicy) -> Result<(), RuntimeError> {
        self.stop = Some(stop);
        self.cleanup_remaining = match stop {
            StopPolicy::Drain => self.sessions.len(),
            StopPolicy::Abort => 0,
        };
        Ok(())
    }

    fn cleanup(
        &mut self,
        _node: &Node,
        context: HostedServiceStepContext,
    ) -> Result<HostedServiceCleanup, RuntimeError> {
        if self.stop != Some(StopPolicy::Drain) || self.cleanup_remaining == 0 {
            return Ok(HostedServiceCleanup::Complete);
        }
        self.cleanup_remaining -= 1;
        if self.cleanup_remaining == 0 {
            return Ok(HostedServiceCleanup::Complete);
        }
        let deadline_tick = context
            .tick
            .checked_add(1)
            .ok_or_else(|| runtime_error(NetworkReason::Bounds, "cleanup timer overflow"))?;
        Ok(HostedServiceCleanup::waiting(
            HostedServiceInterest::Timer {
                subject: Id("conduit/net-session-drain"),
                deadline_tick,
            },
        ))
    }
}

fn link_observer() -> Box<dyn Handler> {
    recorded_handler(LinkObserver::default())
}
fn frame_source() -> Box<dyn Handler> {
    recorded_handler(FrameSource::default())
}
fn frame_sink() -> Box<dyn Handler> {
    recorded_handler(FrameSink::default())
}
fn packet_source() -> Box<dyn Handler> {
    recorded_handler(PacketSource::default())
}
fn packet_classifier() -> Box<dyn Handler> {
    recorded_handler(PacketClassifier)
}
fn packet_router() -> Box<dyn Handler> {
    recorded_handler(PacketRouter::default())
}
fn native_userspace_packet_router() -> Box<dyn Handler> {
    recorded_handler(NativeUserspacePacketRouter)
}
fn packet_sink() -> Box<dyn Handler> {
    recorded_handler(PacketSink::default())
}
fn datagram_source() -> Box<dyn Handler> {
    recorded_handler(DatagramSource::default())
}
fn datagram_impair() -> Box<dyn Handler> {
    recorded_handler(DatagramImpair)
}
fn datagram_sink() -> Box<dyn Handler> {
    recorded_handler(DatagramSink::default())
}
fn stream_source() -> Box<dyn Handler> {
    recorded_handler(StreamSource::default())
}
fn stream_sink() -> Box<dyn Handler> {
    recorded_handler(StreamSink::default())
}
fn session_listener() -> Box<dyn Handler> {
    recorded_handler(SessionListener::default())
}
fn network_meter() -> Box<dyn Handler> {
    recorded_handler(NetworkMeter::default())
}
fn service_observer() -> Box<dyn Handler> {
    recorded_handler(ServiceObserver::default())
}

pub fn register_standing_network_contracts(registry: &mut Registry) {
    for contract in STANDING_NETWORK_CONTRACTS {
        registry.register_contract_only(contract);
    }
}

pub fn register_deterministic_standing_network_providers(
    registry: &mut Registry,
) -> Result<(), RegistryError> {
    register_standing_network_contracts(registry);
    static NO_AUTHORITIES: [SemanticHash; 0] = [];
    for (contract, implementation_id, artifact_id, entrypoint, factory, validate_config) in [
        (
            &LINK_OBSERVE_CONTRACT,
            "conduit.net/virtual-link-observer",
            "conduit.net/virtual-link-observer-artifact",
            "net-virtual-link-observer",
            link_observer as conduit_runtime::HandlerFactory,
            validate_link as conduit_runtime::ConfigValidator,
        ),
        (
            &FRAME_SOURCE_CONTRACT,
            "conduit.net/frame-source-reference",
            "conduit.net/frame-source-reference-artifact",
            "net-frame-source-reference",
            frame_source as conduit_runtime::HandlerFactory,
            validate_frame_source as conduit_runtime::ConfigValidator,
        ),
        (
            &FRAME_SINK_CONTRACT,
            "conduit.net/frame-sink-reference",
            "conduit.net/frame-sink-reference-artifact",
            "net-frame-sink-reference",
            frame_sink as conduit_runtime::HandlerFactory,
            validate_frame_sink as conduit_runtime::ConfigValidator,
        ),
        (
            &PACKET_SOURCE_CONTRACT,
            "conduit.net/packet-source-reference",
            "conduit.net/packet-source-reference-artifact",
            "net-packet-source-reference",
            packet_source as conduit_runtime::HandlerFactory,
            validate_packet_source as conduit_runtime::ConfigValidator,
        ),
        (
            &PACKET_CLASSIFY_CONTRACT,
            "conduit.net/packet-classifier-reference",
            "conduit.net/packet-classifier-reference-artifact",
            "net-packet-classifier-reference",
            packet_classifier as conduit_runtime::HandlerFactory,
            validate_classify as conduit_runtime::ConfigValidator,
        ),
        (
            &PACKET_ROUTE_CONTRACT,
            "conduit.net/packet-router-reference",
            "conduit.net/packet-router-reference-artifact",
            "net-packet-router-reference",
            packet_router as conduit_runtime::HandlerFactory,
            validate_route as conduit_runtime::ConfigValidator,
        ),
        (
            &PACKET_SINK_CONTRACT,
            "conduit.net/packet-sink-reference",
            "conduit.net/packet-sink-reference-artifact",
            "net-packet-sink-reference",
            packet_sink as conduit_runtime::HandlerFactory,
            validate_sink as conduit_runtime::ConfigValidator,
        ),
        (
            &DATAGRAM_SOURCE_CONTRACT,
            "conduit.net/datagram-source-reference",
            "conduit.net/datagram-source-reference-artifact",
            "net-datagram-source-reference",
            datagram_source as conduit_runtime::HandlerFactory,
            validate_datagram_source as conduit_runtime::ConfigValidator,
        ),
        (
            &DATAGRAM_IMPAIR_CONTRACT,
            "conduit.net/datagram-impair-reference",
            "conduit.net/datagram-impair-reference-artifact",
            "net-datagram-impair-reference",
            datagram_impair as conduit_runtime::HandlerFactory,
            validate_datagram_impair as conduit_runtime::ConfigValidator,
        ),
        (
            &DATAGRAM_SINK_CONTRACT,
            "conduit.net/datagram-sink-reference",
            "conduit.net/datagram-sink-reference-artifact",
            "net-datagram-sink-reference",
            datagram_sink as conduit_runtime::HandlerFactory,
            validate_datagram_sink as conduit_runtime::ConfigValidator,
        ),
        (
            &STREAM_SOURCE_CONTRACT,
            "conduit.net/stream-source-reference",
            "conduit.net/stream-source-reference-artifact",
            "net-stream-source-reference",
            stream_source as conduit_runtime::HandlerFactory,
            validate_stream_source as conduit_runtime::ConfigValidator,
        ),
        (
            &STREAM_SINK_CONTRACT,
            "conduit.net/stream-sink-reference",
            "conduit.net/stream-sink-reference-artifact",
            "net-stream-sink-reference",
            stream_sink as conduit_runtime::HandlerFactory,
            validate_stream_sink as conduit_runtime::ConfigValidator,
        ),
        (
            &SESSION_LISTEN_CONTRACT,
            "conduit.net/session-listener-reference",
            "conduit.net/session-listener-reference-artifact",
            "net-session-listener-reference",
            session_listener as conduit_runtime::HandlerFactory,
            validate_listener as conduit_runtime::ConfigValidator,
        ),
        (
            &NETWORK_METER_CONTRACT,
            "conduit.net/network-meter-reference",
            "conduit.net/network-meter-reference-artifact",
            "net-network-meter-reference",
            network_meter as conduit_runtime::HandlerFactory,
            validate_sink as conduit_runtime::ConfigValidator,
        ),
        (
            &SERVICE_OBSERVE_CONTRACT,
            "conduit.net/service-observer-reference",
            "conduit.net/service-observer-reference-artifact",
            "net-service-observer-reference",
            service_observer as conduit_runtime::HandlerFactory,
            validate_service_observe as conduit_runtime::ConfigValidator,
        ),
    ] {
        registry.register_compiled_in_host_service(CompiledInHostService {
            contract,
            implementation_id,
            artifact_id,
            entrypoint,
            source_bytes: include_bytes!("standing.rs"),
            required_authorities: &NO_AUTHORITIES,
            factory,
            validate_config,
        })?;
    }
    Ok(())
}

pub const NATIVE_USERSPACE_ROUTE_IMPLEMENTATION_ID: &str =
    "conduit.net/native-userspace-route-table";
pub const NATIVE_USERSPACE_ROUTE_ARTIFACT_ID: &str =
    "conduit.net/native-userspace-route-table-artifact";

/// Exact capability predicate for the source-attested native route adapter.
///
/// The implementation declares this predicate at installation. A caller-owned
/// host observation must independently report the matching current target and
/// linked-code digest before the generic resolver may select it.
#[must_use]
pub fn native_userspace_route_capability_requirement() -> InstalledCapabilityRequirement {
    const INTERFACE: &str = "conduit.host/network/native-userspace-route";
    let source_digest =
        SemanticHash::from_bytes(Sha256::digest(include_bytes!("standing.rs")).into());
    InstalledCapabilityRequirement {
        interface: PinnedDescriptor {
            id: Id(INTERFACE),
            schema_version: 0,
            semantic_hash: SemanticHash::from_bytes(Sha256::digest(INTERFACE).into()),
        },
        mode: "linked".to_owned(),
        subject: Some(format!(
            "{}/{}",
            std::env::consts::OS,
            std::env::consts::ARCH
        )),
        details: Some(source_digest),
        minimum_capacity: PlanResourceBudget::ZERO,
    }
}

/// Installs a second executable for the same host-neutral route contract via
/// the shared implementation-manifest and artifact path.
///
/// The artifact is the actual linked adapter source, not a synthetic fixture.
/// Installation performs no discovery or effect, and the implementation
/// remains unusable until the generic host snapshot satisfies
/// [`native_userspace_route_capability_requirement`]. The adapter only
/// validates and translates values; it does not mutate an ambient host route
/// table or claim packet-injection authority.
pub fn install_native_userspace_route_implementation(
    registry: &mut Registry,
) -> Result<(), RegistryError> {
    const SOURCE: &[u8] = include_bytes!("standing.rs");
    let digest = ArtifactDigest::from_bytes(Sha256::digest(SOURCE).into());
    let recipe_digest =
        ArtifactDigest::from_bytes(Sha256::digest(b"cargo build -p conduit-net").into());
    let profile = "conduit/network-native-in-process-profile";
    registry.register_installed_implementation(InstalledImplementationRegistration {
        contract: &PACKET_ROUTE_CONTRACT,
        implementation_id: NATIVE_USERSPACE_ROUTE_IMPLEMENTATION_ID.to_owned(),
        implementation_version: digest.to_string(),
        executor: ExecutorKind::NativeInProcess,
        entrypoint_name: "net-native-userspace-route-table".to_owned(),
        entrypoint_adapter: "conduit/host-service-step".to_owned(),
        entrypoint_abi: "conduit/rust-in-process".to_owned(),
        entrypoint_protocol_version: 0,
        execution_profile: PinnedDescriptor {
            id: Id(profile),
            schema_version: 0,
            semantic_hash: SemanticHash::from_bytes(Sha256::digest(profile).into()),
        },
        artifacts: vec![InstalledArtifactRegistration {
            id: NATIVE_USERSPACE_ROUTE_ARTIFACT_ID.to_owned(),
            digest,
            media_type: "application/vnd.conduit.compiled-in-provider".to_owned(),
            byte_size: u64::try_from(SOURCE.len()).expect("artifact size fits u64"),
            target: Some(std::env::consts::ARCH.to_owned()),
            abi: Some("conduit/rust-in-process".to_owned()),
            builder: "conduit/rustc-workspace-build".to_owned(),
            source_digest: digest,
            build_recipe_digest: recipe_digest,
            reproducible: true,
            license_expressions: vec!["MIT".to_owned(), "Apache-2.0".to_owned()],
            role: "implementation".to_owned(),
            required: true,
        }],
        required_capabilities: vec![native_userspace_route_capability_requirement()],
        required_authorities: Vec::new(),
        required_effects: Vec::new(),
        minimum_plan_version: 0,
        maximum_plan_version: u32::MAX,
        minimum_runtime_protocol: 1,
        maximum_runtime_protocol: 1,
        coexistence_memory_bytes: 0,
        managed_lifecycle: None,
        factory: native_userspace_packet_router,
        validate_config: validate_route,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_runtime::AvailabilityState;

    fn node(source: &str) -> Node {
        let mut panel = conduit_panel::parse(&format!("panel 0\nnetwork: {source}\n"))
            .expect("network source parses");
        panel.nodes.remove(0)
    }

    #[test]
    fn standing_ports_keep_taxonomy_lifecycle_sensitivity_and_state_distinct() {
        assert_eq!(PACKET_SOURCE_CONTRACT.outputs[0].value_type, PACKET_TYPE);
        assert_eq!(
            PACKET_SOURCE_CONTRACT.outputs[0].values,
            ValueCardinality::ZeroOrMore
        );
        assert_eq!(
            PACKET_SOURCE_CONTRACT.outputs[0].terminal,
            TerminalContract::OpenEnded
        );
        assert_eq!(
            PACKET_SOURCE_CONTRACT.outputs[0].sensitivity,
            Sensitivity::Restricted
        );
        assert_eq!(
            PACKET_SINK_CONTRACT.outputs[0].delivery,
            Delivery::LatestState
        );
        assert_eq!(
            PACKET_SINK_CONTRACT.outputs[0].temporal,
            TemporalContract::RetainedState
        );
        assert_eq!(
            LINK_OBSERVE_CONTRACT.outputs[0].temporal,
            TemporalContract::Committed
        );
        assert_ne!(LINK_OBSERVE_CONTRACT.outputs[0].value_type, PACKET_TYPE);
        assert_ne!(
            SESSION_LISTEN_CONTRACT.outputs[0].value_type,
            CONTROL_EVENT_TYPE
        );
        assert_eq!(
            SESSION_LISTEN_CONTRACT.outputs[1].sensitivity,
            Sensitivity::Restricted
        );
    }

    #[test]
    fn source_cannot_claim_network_authority_or_host_observation() {
        for contract in STANDING_NETWORK_CONTRACTS {
            let fields = contract
                .config
                .fields
                .iter()
                .map(|field| field.key.as_str())
                .collect::<Vec<_>>();
            for forbidden in [
                "resource",
                "grant",
                "authority",
                "provider",
                "interface_observation",
                "authenticated",
                "member",
                "internet",
            ] {
                assert!(
                    !fields.contains(&forbidden),
                    "{} exposes {forbidden}",
                    contract.id
                );
            }
        }
    }

    #[test]
    fn contracts_are_visible_without_forging_provider_availability() {
        let mut registry = Registry::default();
        register_standing_network_contracts(&mut registry);
        for contract in STANDING_NETWORK_CONTRACTS {
            assert_eq!(
                registry.node_availability(contract.id.as_str()).state,
                AvailabilityState::ContractOnly
            );
        }
        register_deterministic_standing_network_providers(&mut registry).unwrap();
        for contract in EXECUTABLE_STANDING_NETWORK_CONTRACTS {
            assert_eq!(
                registry.node_availability(contract.id.as_str()).state,
                AvailabilityState::ProviderAvailable
            );
        }
        for contract in NETWORK_EFFECT_CONTRACTS {
            assert_eq!(
                registry.node_availability(contract.id.as_str()).state,
                AvailabilityState::ContractOnly
            );
        }
    }

    #[test]
    fn one_semantic_router_has_two_generic_installed_implementations() {
        let mut registry = Registry::default();
        register_deterministic_standing_network_providers(&mut registry).unwrap();
        install_native_userspace_route_implementation(&mut registry).unwrap();
        let router_providers = registry
            .installed_providers()
            .into_iter()
            .filter(|provider| provider.contract.id == PACKET_ROUTE_CONTRACT.id)
            .collect::<Vec<_>>();
        assert_eq!(router_providers.len(), 2);
        assert_ne!(
            router_providers[0].manifest.id,
            router_providers[1].manifest.id
        );
        let native = router_providers
            .iter()
            .find(|provider| provider.manifest.id == Id(NATIVE_USERSPACE_ROUTE_IMPLEMENTATION_ID))
            .unwrap();
        assert!(
            native
                .manifest
                .implementation_version
                .starts_with("sha256:")
        );
        assert_eq!(native.manifest.executor, ExecutorKind::NativeInProcess);
        assert_eq!(native.artifacts.len(), 1);
        assert_eq!(native.artifact.id, Id(NATIVE_USERSPACE_ROUTE_ARTIFACT_ID));
        assert_eq!(native.artifact.target, Some(Id(std::env::consts::ARCH)));
        assert_eq!(native.required_capabilities.len(), 1);
        assert!(native.manifest.required_authorities.is_empty());
        assert!(native.manifest.required_effects.is_empty());
    }

    #[test]
    fn installed_router_adapter_preserves_reference_normalized_semantics() {
        let route_node = node(
            "net/packet/route { lifecycle = \"standing\" prefix = \"10.1.0.0\" prefix_length = 16 egress_interface = 2 mtu = 1500 forwarding = \"admitted\" maximum_routes = 16 maximum_packet_bytes = 1500 maximum_evidence_events = 64 }",
        );
        let mut reference = PacketRouter::default();
        let mut native = NativeUserspacePacketRouter;
        let mut input = std::io::empty();
        let mut output = std::io::sink();
        let mut error = std::io::sink();
        let mut display = std::io::sink();
        let mut io = RunIo {
            input: &mut input,
            output: &mut output,
            error: &mut error,
            display: &mut display,
        };
        for (destination, hop_limit) in [
            (Ipv4Address([10, 1, 0, 2]), 4),
            (Ipv4Address([10, 2, 0, 2]), 4),
            (Ipv4Address([10, 1, 0, 2]), 1),
        ] {
            let packet = NetworkPacket::ipv4(
                1,
                Ipv4Address([10, 0, 0, 2]),
                destination,
                hop_limit,
                vec![0; 64],
            )
            .unwrap();
            let input = packet_value(&packet).unwrap();
            let HostedServiceStep::Produced {
                outputs: reference_outputs,
            } = reference
                .step(
                    &route_node,
                    core::slice::from_ref(&input),
                    HostedServiceStepContext { tick: 0 },
                    &mut io,
                )
                .unwrap()
            else {
                panic!("reference router did not produce");
            };
            let HostedServiceStep::Produced {
                outputs: native_outputs,
            } = native
                .step(
                    &route_node,
                    core::slice::from_ref(&input),
                    HostedServiceStepContext { tick: 0 },
                    &mut io,
                )
                .unwrap()
            else {
                panic!("native userspace router did not produce");
            };
            assert_eq!(reference_outputs, native_outputs);
        }
    }

    #[test]
    fn exact_current_profiles_validate_and_ambient_claims_fail() {
        assert!(
            validate_route(&node(
                "net/packet/route { lifecycle = \"standing\" prefix = \"10.1.0.0\" prefix_length = 16 egress_interface = 2 mtu = 1500 forwarding = \"admitted\" maximum_routes = 16 maximum_packet_bytes = 1500 maximum_evidence_events = 64 }"
            ))
            .is_ok()
        );
        let forged = node(
            "net/packet/route { lifecycle = \"standing\" prefix = \"10.1.0.0\" prefix_length = 16 egress_interface = 2 mtu = 1500 forwarding = \"admitted\" maximum_routes = 16 maximum_packet_bytes = 1500 maximum_evidence_events = 64 internet = \"granted\" }",
        );
        assert_eq!(validate_route(&forged).unwrap_err().code, "CND-SRC-002");
    }

    #[test]
    fn packet_encoding_preserves_disposition_and_rejects_truncation() {
        let mut packet = NetworkPacket::ipv4(
            7,
            Ipv4Address([10, 0, 0, 2]),
            Ipv4Address([10, 1, 0, 2]),
            3,
            vec![1, 2, 3],
        )
        .unwrap();
        packet.egress_interface = Some(2);
        packet.disposition = PacketDisposition::Forwarded;
        let encoded = packet_value(&packet).unwrap();
        assert_eq!(parse_packet(&encoded).unwrap(), packet);
        let mut truncated = encoded;
        truncated.bytes.pop();
        assert_eq!(parse_packet(&truncated).unwrap_err().code, "CND-NET-004");
    }

    #[test]
    fn frame_datagram_and_stream_encodings_preserve_their_distinct_semantics() {
        let frame = NetworkFrame {
            interface: 1,
            direction: NetworkDirection::Ingress,
            protocol: Some(0x0800),
            observed_at_tick: 4,
            payload: vec![1, 2, 3],
        };
        assert_eq!(parse_frame(&frame_value(&frame).unwrap()).unwrap(), frame);

        let mut source = [0; 16];
        source[..4].copy_from_slice(&[10, 0, 0, 2]);
        let mut destination = [0; 16];
        destination[..4].copy_from_slice(&[10, 0, 0, 3]);
        let datagram = NetworkDatagram {
            session: Some(9),
            family: AddressFamily::Ipv4,
            source,
            source_port: 30_000,
            destination,
            destination_port: 30_001,
            sequence: 7,
            delivery: DatagramDelivery::Reordered,
            payload: vec![4, 5, 6],
        };
        assert_eq!(
            parse_datagram(&datagram_value(&datagram).unwrap()).unwrap(),
            datagram
        );

        let chunk = ByteStreamChunk {
            session: 9,
            offset: 12,
            eof: false,
            read_half_closed: true,
            write_half_closed: false,
            pressure: StreamPressure::Backpressured,
            bytes: vec![7, 8],
        };
        assert_eq!(parse_stream(&stream_value(&chunk).unwrap()).unwrap(), chunk);
        assert_ne!(FRAME_TYPE, DATAGRAM_TYPE);
        assert_ne!(DATAGRAM_TYPE, BYTE_STREAM_TYPE);
    }

    #[test]
    fn datagram_impairment_executes_every_declared_delivery_outcome() {
        let impairment_node = node(
            "net/datagram/impair { lifecycle = \"standing\" pattern = \"deliver,loss,duplicate,reorder\" maximum_datagram_bytes = 1472 maximum_evidence_events = 64 }",
        );
        let mut handler = DatagramImpair;
        let mut input_io = std::io::empty();
        let mut output_io = std::io::sink();
        let mut error_io = std::io::sink();
        let mut display_io = std::io::sink();
        let mut io = RunIo {
            input: &mut input_io,
            output: &mut output_io,
            error: &mut error_io,
            display: &mut display_io,
        };
        let mut source = [0; 16];
        source[..4].copy_from_slice(&[10, 0, 0, 2]);
        let mut destination = [0; 16];
        destination[..4].copy_from_slice(&[10, 0, 0, 3]);
        for (sequence, expected) in [
            (1, DatagramDelivery::Delivered),
            (2, DatagramDelivery::Lost),
            (3, DatagramDelivery::Duplicated),
            (4, DatagramDelivery::Reordered),
        ] {
            let input = datagram_value(&NetworkDatagram {
                session: None,
                family: AddressFamily::Ipv4,
                source,
                source_port: 30_000,
                destination,
                destination_port: 30_001,
                sequence,
                delivery: DatagramDelivery::Pending,
                payload: vec![1],
            })
            .unwrap();
            let HostedServiceStep::Produced { outputs } = handler
                .step(
                    &impairment_node,
                    core::slice::from_ref(&input),
                    HostedServiceStepContext { tick: sequence },
                    &mut io,
                )
                .unwrap()
            else {
                panic!("impairment did not produce");
            };
            assert_eq!(parse_datagram(&outputs[0]).unwrap().delivery, expected);
        }
    }

    #[test]
    fn session_event_and_state_encodings_round_trip_and_require_correlation() {
        let mut local = [0; 16];
        local[..4].copy_from_slice(&[127, 0, 0, 1]);
        let mut peer = [0; 16];
        peer[..4].copy_from_slice(&[10, 0, 0, 2]);
        let session = crate::NetworkSession {
            identity: 7,
            generation: 3,
            family: AddressFamily::Ipv4,
            protocol: TransportProtocol::Tcp,
            local,
            local_port: 8080,
            peer,
            peer_port: 40_000,
            lifecycle: SessionLifecycle::Accepted,
            authenticated: false,
            expires_at_tick: 25,
        };
        let event = NetworkControlEvent {
            identity: 7,
            generation: 3,
            kind: NetworkControlKind::Session,
            outcome: NetworkControlOutcome::Admitted,
            tick: 0,
        };
        let state = RetainedNetworkState {
            table: 3,
            generation: 2,
            items: 1,
            bytes: core::mem::size_of::<crate::NetworkSession>() as u32,
            observed_at_tick: 0,
            expires_at_tick: Some(25),
            policy: RetainedStatePolicy::GenerationFenced,
        };
        let encoded_session = session_value(session);
        let event_value = control_value(event);
        let encoded_state = state_value(state);
        assert_eq!(parse_session(&encoded_session).unwrap(), session);
        assert_eq!(parse_control(&event_value).unwrap(), event);
        assert_eq!(parse_state(&encoded_state).unwrap(), state);

        let mut observer = ServiceObserver::default();
        let observer_node = node(
            "net/observe/service { lifecycle = \"standing\" maximum_retained_items = 0 maximum_evidence_events = 64 }",
        );
        let mut input = std::io::empty();
        let mut output = std::io::sink();
        let mut error = std::io::sink();
        let mut display = std::io::sink();
        let mut io = RunIo {
            input: &mut input,
            output: &mut output,
            error: &mut error,
            display: &mut display,
        };
        assert_eq!(
            observer
                .step(
                    &observer_node,
                    &[encoded_session.clone(), event_value, encoded_state],
                    HostedServiceStepContext { tick: 0 },
                    &mut io,
                )
                .unwrap(),
            HostedServiceStep::produced(Vec::new())
        );
        let mismatched_event = control_value(NetworkControlEvent {
            generation: 4,
            ..event
        });
        assert_eq!(
            observer
                .step(
                    &observer_node,
                    &[encoded_session, mismatched_event, state_value(state)],
                    HostedServiceStepContext { tick: 0 },
                    &mut io,
                )
                .unwrap_err()
                .code,
            NetworkReason::StaleGeneration.code()
        );

        let timed_out = crate::NetworkSession {
            lifecycle: SessionLifecycle::TimedOut,
            ..session
        };
        let expired = NetworkControlEvent {
            outcome: NetworkControlOutcome::Expired,
            tick: 1_000,
            ..event
        };
        let later_state = RetainedNetworkState {
            generation: 3,
            items: 0,
            bytes: 0,
            observed_at_tick: 1_000,
            expires_at_tick: None,
            ..state
        };
        observer
            .step(
                &observer_node,
                &[
                    session_value(timed_out),
                    control_value(event),
                    state_value(later_state),
                ],
                HostedServiceStepContext { tick: 1_000 },
                &mut io,
            )
            .unwrap();
        observer
            .step(
                &observer_node,
                &[
                    session_value(session),
                    control_value(expired),
                    state_value(later_state),
                ],
                HostedServiceStepContext { tick: 1_000 },
                &mut io,
            )
            .unwrap();
        assert_eq!(observer.correlated_events, 3);
    }

    #[test]
    fn listener_reports_expiry_as_a_correlated_terminal_lifecycle() {
        let listener_node = node(
            "net/session/listen { lifecycle = \"standing\" transport = \"tcp-reference\" local_port = 8080 period_ticks = 10 session_timeout_ticks = 25 maximum_sessions = 8 maximum_retained_items = 8 maximum_evidence_events = 64 }",
        );
        let mut listener = SessionListener::default();
        let mut input = std::io::empty();
        let mut output = std::io::sink();
        let mut error = std::io::sink();
        let mut display = std::io::sink();
        let mut io = RunIo {
            input: &mut input,
            output: &mut output,
            error: &mut error,
            display: &mut display,
        };
        for tick in [0, 10, 20] {
            let HostedServiceStep::Produced { outputs } = listener
                .step(
                    &listener_node,
                    &[],
                    HostedServiceStepContext { tick },
                    &mut io,
                )
                .unwrap()
            else {
                panic!("listener did not admit at tick {tick}");
            };
            assert_eq!(
                parse_session(&outputs[0]).unwrap().lifecycle,
                SessionLifecycle::Accepted
            );
            assert_eq!(
                parse_control(&outputs[1]).unwrap().outcome,
                NetworkControlOutcome::Admitted
            );
        }
        let HostedServiceStep::Produced { outputs } = listener
            .step(
                &listener_node,
                &[],
                HostedServiceStepContext { tick: 30 },
                &mut io,
            )
            .unwrap()
        else {
            panic!("listener did not report expiry");
        };
        let session = parse_session(&outputs[0]).unwrap();
        let event = parse_control(&outputs[1]).unwrap();
        let state = parse_state(&outputs[2]).unwrap();
        assert_eq!(session.lifecycle, SessionLifecycle::TimedOut);
        assert_eq!(event.outcome, NetworkControlOutcome::Expired);
        assert_eq!(event.identity, session.identity);
        assert_eq!(event.generation, session.generation);
        assert_eq!(state.items, 2);
        assert_eq!(state.observed_at_tick, event.tick);
    }

    #[test]
    fn listener_drain_and_abort_have_distinct_bounded_cleanup() {
        let listener_node = node(
            "net/session/listen { lifecycle = \"standing\" transport = \"tcp-reference\" local_port = 8080 period_ticks = 10 session_timeout_ticks = 25 maximum_sessions = 8 maximum_retained_items = 8 maximum_evidence_events = 64 }",
        );
        let mut input = std::io::empty();
        let mut output = std::io::sink();
        let mut error = std::io::sink();
        let mut display = std::io::sink();
        let mut io = RunIo {
            input: &mut input,
            output: &mut output,
            error: &mut error,
            display: &mut display,
        };
        let mut draining = SessionListener::default();
        for tick in [0, 10, 20] {
            assert!(matches!(
                draining
                    .step(
                        &listener_node,
                        &[],
                        HostedServiceStepContext { tick },
                        &mut io,
                    )
                    .unwrap(),
                HostedServiceStep::Produced { .. }
            ));
        }
        draining.cancel(&listener_node, StopPolicy::Drain).unwrap();
        assert!(matches!(
            draining
                .cleanup(&listener_node, HostedServiceStepContext { tick: 20 })
                .unwrap(),
            HostedServiceCleanup::Waiting { .. }
        ));
        assert!(matches!(
            draining
                .cleanup(&listener_node, HostedServiceStepContext { tick: 21 })
                .unwrap(),
            HostedServiceCleanup::Waiting { .. }
        ));
        assert_eq!(
            draining
                .cleanup(&listener_node, HostedServiceStepContext { tick: 22 })
                .unwrap(),
            HostedServiceCleanup::Complete
        );

        let mut aborting = SessionListener::default();
        aborting
            .step(
                &listener_node,
                &[],
                HostedServiceStepContext { tick: 0 },
                &mut io,
            )
            .unwrap();
        aborting.cancel(&listener_node, StopPolicy::Abort).unwrap();
        assert_eq!(
            aborting
                .cleanup(&listener_node, HostedServiceStepContext { tick: 0 })
                .unwrap(),
            HostedServiceCleanup::Complete
        );
    }

    #[test]
    fn standing_sources_produce_then_wait_for_exact_timer() {
        let mut source = PacketSource::default();
        let source_node = node(
            "net/packet/source { lifecycle = \"standing\" source = \"10.0.0.2\" destination = \"10.1.0.2\" hop_limit = 4 payload_bytes = 64 period_ticks = 10 maximum_packets_per_step = 1 maximum_packet_bytes = 1500 maximum_evidence_events = 64 }",
        );
        let mut input = std::io::empty();
        let mut output = std::io::sink();
        let mut error = std::io::sink();
        let mut display = std::io::sink();
        let mut io = RunIo {
            input: &mut input,
            output: &mut output,
            error: &mut error,
            display: &mut display,
        };
        assert!(matches!(
            source
                .step(
                    &source_node,
                    &[],
                    HostedServiceStepContext { tick: 0 },
                    &mut io
                )
                .unwrap(),
            HostedServiceStep::Produced { .. }
        ));
        assert_eq!(
            source
                .step(
                    &source_node,
                    &[],
                    HostedServiceStepContext { tick: 0 },
                    &mut io
                )
                .unwrap(),
            HostedServiceStep::waiting(HostedServiceInterest::Timer {
                subject: Id("conduit/net-packet-source"),
                deadline_tick: 10,
            })
        );
    }
}
