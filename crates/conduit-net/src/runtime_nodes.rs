//! Standing isolated-network service contracts and deterministic providers.
//!
//! These providers are deterministic no-radio reference implementations. They preserve the same
//! typed graph that a Linux or embedded provider can satisfy, but they neither
//! open an interface nor claim current host authority or reachability.

use conduit_core::{
    ConfigContract, ConfigFieldContract, ConfigIdentity, ConfigMutability, ConfigRequirement,
    ConnectionCardinality, Delivery, Direction, Id, LossAcceptance, NodeContract, PortContract,
    PortFlowConstraints, Presence, SemanticHash, Sensitivity, TemporalContract, TerminalContract,
    TypeContractRef, ValueCardinality,
};
use conduit_panel::{Node, SourceValue};
use conduit_runtime::{
    CompiledInHostService, Handler, HostedServiceInterest, HostedServiceStep,
    HostedServiceStepContext, Registry, RegistryError, ResolutionError, RunIo, RuntimeError, Value,
};

use crate::standing::{
    address_state_value, control_value, dhcp_lease_value, link_value, parse_address_state,
    parse_dhcp_lease, parse_service_registration, reachability_value, recorded_handler,
    service_registration_value, state_value,
};
use crate::{
    ADDRESS_STATE_TYPE, AddressFamily, AddressReadiness, CONTROL_EVENT_TYPE, DHCP_LEASE_TICKS,
    DHCP_LEASE_TYPE, DhcpLeaseTable, DhcpMessage, DhcpOutcome, ICMP_PACKETS_PER_WINDOW,
    ICMP_WINDOW_TICKS, IcmpRateLimiter, LINK_OBSERVATION_TYPE, LeasePhase, LinkKind,
    LinkObservation, MAXIMUM_CLIENTS, MAXIMUM_EVIDENCE_EVENTS, MAXIMUM_NAME_BYTES,
    MAXIMUM_PACKET_BYTES, NetworkAddressState, NetworkAvailability, NetworkControlEvent,
    NetworkControlKind, NetworkControlOutcome, NetworkDhcpLease, NetworkReachabilityObservation,
    NetworkReason, NetworkServiceRegistration, REACHABILITY_OBSERVATION_TYPE,
    RETAINED_NETWORK_STATE_TYPE, ReachabilityOutcome, ReachabilityScope, RetainedNetworkState,
    RetainedStatePolicy, SERVICE_REGISTRATION_TYPE, TransportProtocol,
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

const AP_FIELDS: [ConfigFieldContract<'static>; 9] = [
    field("lifecycle", TEXT_TYPE),
    field("topology", TEXT_TYPE),
    field("interface", U64_TYPE),
    field("address", TEXT_TYPE),
    field("prefix_length", U64_TYPE),
    field("maximum_clients", U64_TYPE),
    field("period_ticks", U64_TYPE),
    field("freshness_ticks", U64_TYPE),
    field("maximum_evidence_events", U64_TYPE),
];
const DHCP_FIELDS: [ConfigFieldContract<'static>; 6] = [
    field("lifecycle", TEXT_TYPE),
    field("lease_ticks", U64_TYPE),
    field("maximum_leases", U64_TYPE),
    field("maximum_pending", U64_TYPE),
    field("maximum_evidence_events", U64_TYPE),
    field("cancellation", TEXT_TYPE),
];
const DNS_SD_FIELDS: [ConfigFieldContract<'static>; 7] = [
    field("lifecycle", TEXT_TYPE),
    field("name", TEXT_TYPE),
    field("port", U64_TYPE),
    field("ttl_ticks", U64_TYPE),
    field("maximum_records", U64_TYPE),
    field("maximum_name_bytes", U64_TYPE),
    field("maximum_evidence_events", U64_TYPE),
];
const REACHABILITY_FIELDS: [ConfigFieldContract<'static>; 6] = [
    field("lifecycle", TEXT_TYPE),
    field("scope", TEXT_TYPE),
    field("maximum_packet_bytes", U64_TYPE),
    field("maximum_packets_per_window", U64_TYPE),
    field("window_ticks", U64_TYPE),
    field("maximum_evidence_events", U64_TYPE),
];

const fn input_port(
    id: &'static str,
    value_type: TypeContractRef<'static>,
) -> PortContract<'static> {
    PortContract {
        id: Id(id),
        direction: Direction::Input,
        value_type,
        presence: Presence::Required,
        connections: ConnectionCardinality::ExactlyOne,
        values: ValueCardinality::ZeroOrMore,
        delivery: Delivery::Stream,
        temporal: TemporalContract::Committed,
        terminal: TerminalContract::Either,
        sensitivity: Sensitivity::Restricted,
        flow: PortFlowConstraints {
            loss: LossAcceptance::LosslessOnly,
        },
    }
}

const fn output_port(
    id: &'static str,
    value_type: TypeContractRef<'static>,
    required: bool,
    sensitivity: Sensitivity,
) -> PortContract<'static> {
    PortContract {
        id: Id(id),
        direction: Direction::Output,
        value_type,
        presence: if required {
            Presence::Required
        } else {
            Presence::Optional
        },
        connections: if required {
            ConnectionCardinality::ExactlyOne
        } else {
            ConnectionCardinality::ZeroOrMore
        },
        values: ValueCardinality::ZeroOrMore,
        delivery: Delivery::Stream,
        temporal: TemporalContract::Committed,
        terminal: TerminalContract::OpenEnded,
        sensitivity,
        flow: PortFlowConstraints {
            loss: LossAcceptance::LosslessOnly,
        },
    }
}

const fn state_output(id: &'static str) -> PortContract<'static> {
    PortContract {
        id: Id(id),
        direction: Direction::Output,
        value_type: RETAINED_NETWORK_STATE_TYPE,
        presence: Presence::Optional,
        connections: ConnectionCardinality::ZeroOrMore,
        values: ValueCardinality::ZeroOrMore,
        delivery: Delivery::LatestState,
        temporal: TemporalContract::RetainedState,
        terminal: TerminalContract::OpenEnded,
        sensitivity: Sensitivity::Restricted,
        flow: PortFlowConstraints {
            loss: LossAcceptance::TypeContractDefined,
        },
    }
}

const AP_OUTPUTS: [PortContract<'static>; 3] = [
    output_port("link", LINK_OBSERVATION_TYPE, false, Sensitivity::Public),
    output_port("address", ADDRESS_STATE_TYPE, true, Sensitivity::Restricted),
    output_port("event", CONTROL_EVENT_TYPE, false, Sensitivity::Restricted),
];
const DHCP_INPUTS: [PortContract<'static>; 1] = [input_port("address", ADDRESS_STATE_TYPE)];
const DHCP_OUTPUTS: [PortContract<'static>; 2] = [
    output_port("lease", DHCP_LEASE_TYPE, true, Sensitivity::Restricted),
    state_output("state"),
];
const DNS_SD_INPUTS: [PortContract<'static>; 1] = [input_port("lease", DHCP_LEASE_TYPE)];
const DNS_SD_OUTPUTS: [PortContract<'static>; 2] = [
    output_port(
        "registration",
        SERVICE_REGISTRATION_TYPE,
        true,
        Sensitivity::Restricted,
    ),
    state_output("state"),
];
const REACHABILITY_INPUTS: [PortContract<'static>; 1] =
    [input_port("registration", SERVICE_REGISTRATION_TYPE)];
const REACHABILITY_OUTPUTS: [PortContract<'static>; 1] = [output_port(
    "observation",
    REACHABILITY_OBSERVATION_TYPE,
    false,
    Sensitivity::Public,
)];

pub const WIFI_AP_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("net/wifi/access-point"),
    config: ConfigContract { fields: &AP_FIELDS },
    inputs: &[],
    outputs: &AP_OUTPUTS,
};
pub const DHCP_SERVER_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("net/dhcp/server"),
    config: ConfigContract {
        fields: &DHCP_FIELDS,
    },
    inputs: &DHCP_INPUTS,
    outputs: &DHCP_OUTPUTS,
};
pub const DNS_SD_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("net/dns-sd"),
    config: ConfigContract {
        fields: &DNS_SD_FIELDS,
    },
    inputs: &DNS_SD_INPUTS,
    outputs: &DNS_SD_OUTPUTS,
};
pub const REACHABILITY_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("net/reachability"),
    config: ConfigContract {
        fields: &REACHABILITY_FIELDS,
    },
    inputs: &REACHABILITY_INPUTS,
    outputs: &REACHABILITY_OUTPUTS,
};

pub const NETWORK_CONTRACTS: [&NodeContract<'static>; 4] = [
    &WIFI_AP_CONTRACT,
    &DHCP_SERVER_CONTRACT,
    &DNS_SD_CONTRACT,
    &REACHABILITY_CONTRACT,
];

fn integer(node: &Node, key: &str) -> Option<u64> {
    match node.config_value(key) {
        Some(SourceValue::Integer(value)) => u64::try_from(*value).ok(),
        _ => None,
    }
}

fn validate_base(node: &Node, expected_fields: usize) -> Result<(), ResolutionError> {
    for forbidden in [
        "resource",
        "grant",
        "authority",
        "provider",
        "initialized",
        "fresh",
        "authenticated",
        "internet",
    ] {
        if node.config_value(forbidden).is_some() {
            return Err(ResolutionError::new(
                "CND-SRC-002",
                "network source cannot manufacture provider facts, authority, identity, or reachability",
            ));
        }
    }
    if node.config.len() != expected_fields || node.config("lifecycle") != Some("standing") {
        return Err(ResolutionError::new(
            NetworkReason::Bounds.code(),
            "network service requires its exact current standing profile",
        ));
    }
    Ok(())
}

fn validate_ap(node: &Node) -> Result<(), ResolutionError> {
    validate_base(node, AP_FIELDS.len())?;
    if node.config("topology") != Some("isolated-local-only")
        || integer(node, "interface") != Some(1)
        || node.config("address") != Some("192.168.4.1")
        || integer(node, "prefix_length") != Some(24)
        || integer(node, "maximum_clients") != Some(MAXIMUM_CLIENTS as u64)
        || integer(node, "period_ticks") != Some(1_000)
        || integer(node, "freshness_ticks") != Some(2_000)
        || integer(node, "maximum_evidence_events") != Some(MAXIMUM_EVIDENCE_EVENTS as u64)
    {
        return Err(ResolutionError::new(
            NetworkReason::RoutingForbidden.code(),
            "access point requires the exact isolated local-only standing profile",
        ));
    }
    Ok(())
}

fn validate_dhcp(node: &Node) -> Result<(), ResolutionError> {
    validate_base(node, DHCP_FIELDS.len())?;
    if integer(node, "lease_ticks") != Some(DHCP_LEASE_TICKS)
        || integer(node, "maximum_leases") != Some(MAXIMUM_CLIENTS as u64)
        || integer(node, "maximum_pending") != Some(1)
        || integer(node, "maximum_evidence_events") != Some(MAXIMUM_EVIDENCE_EVENTS as u64)
        || node.config("cancellation") != Some("cancel-before-commit")
    {
        return Err(ResolutionError::new(
            NetworkReason::PoolExhausted.code(),
            "DHCP server requires the finite eight-lease standing profile",
        ));
    }
    Ok(())
}

fn validate_dns_sd(node: &Node) -> Result<(), ResolutionError> {
    validate_base(node, DNS_SD_FIELDS.len())?;
    if node.config("name") != Some("pete.local")
        || integer(node, "port") != Some(8080)
        || integer(node, "ttl_ticks") != Some(120_000)
        || integer(node, "maximum_records") != Some(MAXIMUM_CLIENTS as u64)
        || integer(node, "maximum_name_bytes") != Some(MAXIMUM_NAME_BYTES as u64)
        || integer(node, "maximum_evidence_events") != Some(MAXIMUM_EVIDENCE_EVENTS as u64)
    {
        return Err(ResolutionError::new(
            NetworkReason::NameConflict.code(),
            "DNS-SD requires the exact finite local registration profile",
        ));
    }
    Ok(())
}

fn validate_reachability(node: &Node) -> Result<(), ResolutionError> {
    validate_base(node, REACHABILITY_FIELDS.len())?;
    if node.config("scope") != Some("local-network")
        || integer(node, "maximum_packet_bytes") != Some(MAXIMUM_PACKET_BYTES as u64)
        || integer(node, "maximum_packets_per_window") != Some(u64::from(ICMP_PACKETS_PER_WINDOW))
        || integer(node, "window_ticks") != Some(ICMP_WINDOW_TICKS)
        || integer(node, "maximum_evidence_events") != Some(MAXIMUM_EVIDENCE_EVENTS as u64)
    {
        return Err(ResolutionError::new(
            NetworkReason::RateLimited.code(),
            "reachability requires the exact local-only bounded probe profile",
        ));
    }
    Ok(())
}

fn runtime_error(reason: NetworkReason, detail: &'static str) -> RuntimeError {
    RuntimeError::new(reason.code(), detail)
}

#[derive(Default)]
struct DeterministicAp {
    generation: u32,
    deadline_tick: Option<u64>,
}

impl Handler for DeterministicAp {
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
                "deterministic access-point provider received hidden input",
            ));
        }
        if let Some(deadline_tick) = self.deadline_tick
            && context.tick < deadline_tick
        {
            return Ok(HostedServiceStep::waiting(HostedServiceInterest::Timer {
                subject: Id("conduit/net-isolated-ap"),
                deadline_tick,
            }));
        }
        self.generation = self
            .generation
            .checked_add(1)
            .ok_or_else(|| runtime_error(NetworkReason::Bounds, "AP generation exhausted"))?;
        let next_tick = context
            .tick
            .checked_add(1_000)
            .ok_or_else(|| runtime_error(NetworkReason::Bounds, "AP timer overflow"))?;
        let valid_until_tick = context
            .tick
            .checked_add(2_000)
            .ok_or_else(|| runtime_error(NetworkReason::Bounds, "AP freshness overflow"))?;
        self.deadline_tick = Some(next_tick);
        let mut address = [0; 16];
        address[..4].copy_from_slice(&[192, 168, 4, 1]);
        Ok(HostedServiceStep::produced(vec![
            link_value(LinkObservation {
                interface: 1,
                generation: self.generation,
                kind: LinkKind::WifiAccessPoint,
                carrier: true,
                mtu: 1_500,
                address_ready: true,
                availability: NetworkAvailability::Active,
                observed_at_tick: context.tick,
                valid_until_tick,
            }),
            address_state_value(NetworkAddressState {
                interface: 1,
                generation: self.generation,
                family: AddressFamily::Ipv4,
                address,
                prefix_length: 24,
                readiness: AddressReadiness::Ready,
                valid_until_tick: Some(valid_until_tick),
            })?,
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

struct DeterministicDhcp {
    leases: DhcpLeaseTable,
    client_cursor: u64,
}

impl Handler for DeterministicDhcp {
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
                "DHCP server requires one address-readiness value",
            ));
        };
        let address_state = parse_address_state(input)?;
        if address_state.readiness != AddressReadiness::Ready {
            return Ok(HostedServiceStep::waiting(
                HostedServiceInterest::HostOperation {
                    subject: Id("conduit/net-address-ready"),
                },
            ));
        }
        self.client_cursor = if self.client_cursor == MAXIMUM_CLIENTS as u64 {
            1
        } else {
            self.client_cursor.checked_add(1).ok_or_else(|| {
                runtime_error(NetworkReason::Bounds, "DHCP client cursor exhausted")
            })?
        };
        let client = crate::ClientIdentity::new(&self.client_cursor.to_be_bytes())
            .map_err(|reason| runtime_error(reason, "invalid deterministic client identity"))?;
        let DhcpOutcome::Offered(lease) = self
            .leases
            .handle(DhcpMessage::Discover, client, 8, context.tick)
            .map_err(|reason| runtime_error(reason, "bounded DHCP exchange failed"))?
        else {
            return Err(runtime_error(
                NetworkReason::MalformedPacket,
                "deterministic discover did not produce an offer",
            ));
        };
        let mut address = [0; 16];
        address[..4].copy_from_slice(&lease.address.0);
        let mut server = [0; 16];
        server[..4].copy_from_slice(&[192, 168, 4, 1]);
        Ok(HostedServiceStep::produced(vec![
            dhcp_lease_value(NetworkDhcpLease {
                client: self.client_cursor,
                family: AddressFamily::Ipv4,
                address,
                generation: lease.generation,
                phase: LeasePhase::Offered,
                expires_at_tick: Some(lease.expires_at_tick),
                server,
            })?,
            state_value(RetainedNetworkState {
                table: 7,
                generation: lease.generation,
                items: u16::try_from(self.leases.len()).expect("maximum clients fits u16"),
                bytes: u32::try_from(
                    self.leases
                        .len()
                        .checked_mul(core::mem::size_of::<crate::DhcpLease>())
                        .expect("bounded DHCP table bytes"),
                )
                .expect("bounded DHCP table bytes fit u32"),
                observed_at_tick: context.tick,
                expires_at_tick: Some(lease.expires_at_tick),
                policy: RetainedStatePolicy::GenerationFenced,
            }),
        ]))
    }
}

#[derive(Default)]
struct DeterministicDnsSd;

impl Handler for DeterministicDnsSd {
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
                "DNS-SD service requires one lease value",
            ));
        };
        let lease = parse_dhcp_lease(input)?;
        if !matches!(
            lease.phase,
            LeasePhase::Offered | LeasePhase::Bound | LeasePhase::Renewed
        ) {
            return Err(runtime_error(
                NetworkReason::RegistrationStale,
                "service registration requires a current lease",
            ));
        }
        let mut name = [0; MAXIMUM_NAME_BYTES];
        name[.."pete.local".len()].copy_from_slice(b"pete.local");
        let expires_at_tick = context
            .tick
            .checked_add(120_000)
            .ok_or_else(|| runtime_error(NetworkReason::Bounds, "DNS-SD expiry overflow"))?
            .min(lease.expires_at_tick.unwrap_or(u64::MAX));
        let registration = NetworkServiceRegistration {
            name,
            name_bytes: u8::try_from("pete.local".len()).expect("constant name length"),
            family: lease.family,
            address: lease.address,
            port: 8080,
            protocol: TransportProtocol::Tcp,
            generation: lease.generation,
            expires_at_tick,
        };
        Ok(HostedServiceStep::produced(vec![
            service_registration_value(registration)?,
            state_value(RetainedNetworkState {
                table: 8,
                generation: registration.generation,
                items: 1,
                bytes: u32::try_from(core::mem::size_of::<NetworkServiceRegistration>())
                    .expect("registration size fits u32"),
                observed_at_tick: context.tick,
                expires_at_tick: Some(expires_at_tick),
                policy: RetainedStatePolicy::Expiring,
            }),
        ]))
    }
}

struct DeterministicReachability {
    limiter: IcmpRateLimiter,
}

impl Handler for DeterministicReachability {
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
                "reachability requires one service registration",
            ));
        };
        let registration = parse_service_registration(input)?;
        let outcome = match self.limiter.admit(64, context.tick) {
            Ok(()) => ReachabilityOutcome::Reachable,
            Err(NetworkReason::RateLimited) => ReachabilityOutcome::RateLimited,
            Err(reason) => return Err(runtime_error(reason, "bounded reachability probe failed")),
        };
        let valid_until_tick = context
            .tick
            .checked_add(100)
            .ok_or_else(|| runtime_error(NetworkReason::Bounds, "probe freshness overflow"))?;
        Ok(HostedServiceStep::produced(vec![reachability_value(
            NetworkReachabilityObservation {
                family: registration.family,
                target: registration.address,
                scope: ReachabilityScope::LocalNetwork,
                outcome,
                latency_ticks: (outcome == ReachabilityOutcome::Reachable).then_some(1),
                observed_at_tick: context.tick,
                valid_until_tick,
            },
        )?]))
    }
}

pub fn register_network_contracts(registry: &mut Registry) {
    for contract in NETWORK_CONTRACTS {
        registry.register_contract_only(contract);
    }
    crate::register_standing_network_contracts(registry);
}

/// Installs only deterministic no-radio providers. A physical provider must
/// bind its own exact current observation, resource, grant, and use-time lease.
pub fn register_deterministic_network_providers(
    registry: &mut Registry,
) -> Result<(), RegistryError> {
    register_network_contracts(registry);
    static NO_AUTHORITIES: [SemanticHash; 0] = [];
    for provider in [
        CompiledInHostService {
            contract: &WIFI_AP_CONTRACT,
            implementation_id: "conduit.net/reference-wifi-ap",
            artifact_id: "conduit.net/reference-wifi-ap-artifact",
            entrypoint: "network-reference-wifi-ap",
            source_bytes: include_bytes!("runtime_nodes.rs"),
            required_authorities: &NO_AUTHORITIES,
            factory: || recorded_handler(DeterministicAp::default()),
            validate_config: validate_ap,
        },
        CompiledInHostService {
            contract: &DHCP_SERVER_CONTRACT,
            implementation_id: "conduit.net/reference-dhcp-server",
            artifact_id: "conduit.net/reference-dhcp-server-artifact",
            entrypoint: "network-reference-dhcp-server",
            source_bytes: include_bytes!("runtime_nodes.rs"),
            required_authorities: &NO_AUTHORITIES,
            factory: || {
                recorded_handler(DeterministicDhcp {
                    leases: DhcpLeaseTable::new(),
                    client_cursor: 0,
                })
            },
            validate_config: validate_dhcp,
        },
        CompiledInHostService {
            contract: &DNS_SD_CONTRACT,
            implementation_id: "conduit.net/reference-dns-sd",
            artifact_id: "conduit.net/reference-dns-sd-artifact",
            entrypoint: "network-reference-dns-sd",
            source_bytes: include_bytes!("runtime_nodes.rs"),
            required_authorities: &NO_AUTHORITIES,
            factory: || recorded_handler(DeterministicDnsSd),
            validate_config: validate_dns_sd,
        },
        CompiledInHostService {
            contract: &REACHABILITY_CONTRACT,
            implementation_id: "conduit.net/reference-reachability",
            artifact_id: "conduit.net/reference-reachability-artifact",
            entrypoint: "network-reference-reachability",
            source_bytes: include_bytes!("runtime_nodes.rs"),
            required_authorities: &NO_AUTHORITIES,
            factory: || {
                recorded_handler(DeterministicReachability {
                    limiter: IcmpRateLimiter::new(),
                })
            },
            validate_config: validate_reachability,
        },
    ] {
        registry.register_compiled_in_host_service(provider)?;
    }
    crate::register_deterministic_standing_network_providers(registry)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_runtime::AvailabilityState;

    #[test]
    fn contract_inventory_has_no_provider_until_reference_is_explicitly_installed() {
        let mut registry = Registry::default();
        register_network_contracts(&mut registry);
        for contract in NETWORK_CONTRACTS {
            assert_eq!(
                registry.node_availability(contract.id.as_str()).state,
                AvailabilityState::ContractOnly
            );
        }
        register_deterministic_network_providers(&mut registry).unwrap();
        for contract in NETWORK_CONTRACTS {
            assert_eq!(
                registry.node_availability(contract.id.as_str()).state,
                AvailabilityState::ProviderAvailable
            );
        }
    }

    #[test]
    fn current_network_services_are_standing_typed_and_do_not_author_host_facts() {
        assert_eq!(
            WIFI_AP_CONTRACT.outputs[0].value_type,
            LINK_OBSERVATION_TYPE
        );
        assert_eq!(WIFI_AP_CONTRACT.outputs[1].value_type, ADDRESS_STATE_TYPE);
        assert_eq!(
            DHCP_SERVER_CONTRACT.inputs[0].value_type,
            ADDRESS_STATE_TYPE
        );
        assert_eq!(DHCP_SERVER_CONTRACT.outputs[0].value_type, DHCP_LEASE_TYPE);
        assert_eq!(DNS_SD_CONTRACT.inputs[0].value_type, DHCP_LEASE_TYPE);
        assert_eq!(
            DNS_SD_CONTRACT.outputs[0].value_type,
            SERVICE_REGISTRATION_TYPE
        );
        assert_eq!(
            REACHABILITY_CONTRACT.outputs[0].value_type,
            REACHABILITY_OBSERVATION_TYPE
        );
        for contract in NETWORK_CONTRACTS {
            assert!(contract.inputs.iter().all(|port| {
                port.values == ValueCardinality::ZeroOrMore
                    && port.terminal == TerminalContract::Either
            }));
            assert!(contract.outputs.iter().all(|port| {
                port.values == ValueCardinality::ZeroOrMore
                    && port.terminal == TerminalContract::OpenEnded
            }));
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
                "initialized",
                "fresh",
                "authenticated",
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
    fn typed_isolated_chain_round_trips_without_conflating_readiness() {
        let mut address = [0; 16];
        address[..4].copy_from_slice(&[192, 168, 4, 1]);
        let address_state = NetworkAddressState {
            interface: 1,
            generation: 1,
            family: AddressFamily::Ipv4,
            address,
            prefix_length: 24,
            readiness: AddressReadiness::Ready,
            valid_until_tick: Some(2_000),
        };
        assert_eq!(
            parse_address_state(&address_state_value(address_state).unwrap()).unwrap(),
            address_state
        );

        let mut lease_address = [0; 16];
        lease_address[..4].copy_from_slice(&[192, 168, 4, 2]);
        let lease = NetworkDhcpLease {
            client: 1,
            family: AddressFamily::Ipv4,
            address: lease_address,
            generation: 1,
            phase: LeasePhase::Bound,
            expires_at_tick: Some(DHCP_LEASE_TICKS),
            server: address,
        };
        assert_eq!(
            parse_dhcp_lease(&dhcp_lease_value(lease).unwrap()).unwrap(),
            lease
        );

        let mut name = [0; MAXIMUM_NAME_BYTES];
        name[..10].copy_from_slice(b"pete.local");
        let registration = NetworkServiceRegistration {
            name,
            name_bytes: 10,
            family: AddressFamily::Ipv4,
            address: lease_address,
            port: 8080,
            protocol: TransportProtocol::Tcp,
            generation: 1,
            expires_at_tick: 120_000,
        };
        assert_eq!(
            parse_service_registration(&service_registration_value(registration).unwrap()).unwrap(),
            registration
        );

        let observation = NetworkReachabilityObservation {
            family: AddressFamily::Ipv4,
            target: lease_address,
            scope: ReachabilityScope::LocalNetwork,
            outcome: ReachabilityOutcome::Reachable,
            latency_ticks: Some(1),
            observed_at_tick: 1,
            valid_until_tick: 101,
        };
        assert_eq!(
            crate::standing::parse_reachability(&reachability_value(observation).unwrap()).unwrap(),
            observation
        );
    }

    #[test]
    fn standing_dhcp_reuses_eight_explicit_clients_without_exhausting_the_service() {
        let mut panel = conduit_panel::parse(
            "panel 0\ndhcp: net/dhcp/server { lifecycle = \"standing\" lease_ticks = 3600000 maximum_leases = 8 maximum_pending = 1 maximum_evidence_events = 64 cancellation = \"cancel-before-commit\" }\n",
        )
        .unwrap();
        let node = panel.nodes.remove(0);
        let mut address = [0; 16];
        address[..4].copy_from_slice(&[192, 168, 4, 1]);
        let input = address_state_value(NetworkAddressState {
            interface: 1,
            generation: 1,
            family: AddressFamily::Ipv4,
            address,
            prefix_length: 24,
            readiness: AddressReadiness::Ready,
            valid_until_tick: Some(20_000),
        })
        .unwrap();
        let mut handler = DeterministicDhcp {
            leases: DhcpLeaseTable::new(),
            client_cursor: 0,
        };
        let mut stdin = std::io::empty();
        let mut stdout = std::io::sink();
        let mut stderr = std::io::sink();
        let mut display = std::io::sink();
        let mut io = RunIo {
            input: &mut stdin,
            output: &mut stdout,
            error: &mut stderr,
            display: &mut display,
        };
        for cycle in 0..16_u64 {
            let HostedServiceStep::Produced { outputs } = handler
                .step(
                    &node,
                    core::slice::from_ref(&input),
                    HostedServiceStepContext {
                        tick: cycle * 1_000,
                    },
                    &mut io,
                )
                .unwrap()
            else {
                panic!("standing DHCP did not produce");
            };
            let lease = parse_dhcp_lease(&outputs[0]).unwrap();
            let state = crate::standing::parse_state(&outputs[1]).unwrap();
            assert_eq!(lease.client, cycle % MAXIMUM_CLIENTS as u64 + 1);
            assert_eq!(state.items, u16::try_from((cycle + 1).min(8)).unwrap());
        }
        assert_eq!(handler.leases.len(), MAXIMUM_CLIENTS);
    }
}
