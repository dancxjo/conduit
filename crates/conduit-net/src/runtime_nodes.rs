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

use crate::{
    AP_ADDRESS, ClientIdentity, DHCP_LEASE_TICKS, DhcpLeaseTable, DhcpMessage, DhcpOutcome,
    DnsSdTable, ICMP_PACKETS_PER_WINDOW, ICMP_WINDOW_TICKS, IcmpRateLimiter, Ipv4Address,
    MAXIMUM_CLIENTS, MAXIMUM_EVIDENCE_EVENTS, MAXIMUM_NAME_BYTES, MAXIMUM_PACKET_BYTES,
    NetworkReason, ServiceName,
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
const U64: TypeContractRef<'static> = TypeContractRef {
    contract_id: Id("std/u64"),
    schema_version: 0,
    semantic_hash: SemanticHash::from_bytes([
        0xf9, 0xba, 0xd3, 0xea, 0x53, 0xd3, 0xca, 0x01, 0xa0, 0xa4, 0xd6, 0x9f, 0x86, 0xc8, 0x25,
        0x65, 0x17, 0x07, 0x16, 0x45, 0xea, 0x7d, 0x68, 0xef, 0x63, 0x6b, 0x6d, 0x94, 0x87, 0x70,
        0xf0, 0xec,
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

const fn field(
    key: &'static str,
    value_type: TypeContractRef<'static>,
    sensitivity: Sensitivity,
) -> ConfigFieldContract<'static> {
    ConfigFieldContract {
        key: Id(key),
        value_type,
        requirement: ConfigRequirement::Required,
        sensitivity,
        mutability: ConfigMutability::PreStart,
        identity: ConfigIdentity::Plan,
    }
}

const COMMON_FIELDS: [ConfigFieldContract<'static>; 9] = [
    field("resource", REFERENCE, Sensitivity::Restricted),
    field("grant", REFERENCE, Sensitivity::Restricted),
    field("interface", TEXT, Sensitivity::Public),
    field("maximum_request_bytes", U64, Sensitivity::Public),
    field("maximum_response_bytes", U64, Sensitivity::Public),
    field("maximum_pending", U64, Sensitivity::Public),
    field("maximum_evidence_events", U64, Sensitivity::Public),
    field("deadline_ticks", U64, Sensitivity::Public),
    field("cancellation", TEXT, Sensitivity::Public),
];
const AP_FIELDS: [ConfigFieldContract<'static>; 15] = [
    COMMON_FIELDS[0],
    COMMON_FIELDS[1],
    COMMON_FIELDS[2],
    COMMON_FIELDS[3],
    COMMON_FIELDS[4],
    COMMON_FIELDS[5],
    COMMON_FIELDS[6],
    COMMON_FIELDS[7],
    COMMON_FIELDS[8],
    field("ssid_prefix", TEXT, Sensitivity::Public),
    field("address", TEXT, Sensitivity::Public),
    field("maximum_clients", U64, Sensitivity::Public),
    field("routing", TEXT, Sensitivity::Public),
    field("bridging", TEXT, Sensitivity::Public),
    field("nat", TEXT, Sensitivity::Public),
];
const DHCP_FIELDS: [ConfigFieldContract<'static>; 14] = [
    COMMON_FIELDS[0],
    COMMON_FIELDS[1],
    COMMON_FIELDS[2],
    COMMON_FIELDS[3],
    COMMON_FIELDS[4],
    COMMON_FIELDS[5],
    COMMON_FIELDS[6],
    COMMON_FIELDS[7],
    COMMON_FIELDS[8],
    field("server_address", TEXT, Sensitivity::Public),
    field("pool_first", TEXT, Sensitivity::Public),
    field("pool_last", TEXT, Sensitivity::Public),
    field("maximum_leases", U64, Sensitivity::Public),
    field("lease_ticks", U64, Sensitivity::Public),
];
const REACHABILITY_FIELDS: [ConfigFieldContract<'static>; 13] = [
    COMMON_FIELDS[0],
    COMMON_FIELDS[1],
    COMMON_FIELDS[2],
    COMMON_FIELDS[3],
    COMMON_FIELDS[4],
    COMMON_FIELDS[5],
    COMMON_FIELDS[6],
    COMMON_FIELDS[7],
    COMMON_FIELDS[8],
    field("address", TEXT, Sensitivity::Public),
    field("maximum_packet_bytes", U64, Sensitivity::Public),
    field("maximum_packets_per_window", U64, Sensitivity::Public),
    field("window_ticks", U64, Sensitivity::Public),
];
const DNS_SD_FIELDS: [ConfigFieldContract<'static>; 14] = [
    COMMON_FIELDS[0],
    COMMON_FIELDS[1],
    COMMON_FIELDS[2],
    COMMON_FIELDS[3],
    COMMON_FIELDS[4],
    COMMON_FIELDS[5],
    COMMON_FIELDS[6],
    COMMON_FIELDS[7],
    COMMON_FIELDS[8],
    field("name", TEXT, Sensitivity::Public),
    field("address", TEXT, Sensitivity::Public),
    field("ttl_ticks", U64, Sensitivity::Public),
    field("maximum_records", U64, Sensitivity::Public),
    field("maximum_name_bytes", U64, Sensitivity::Public),
];

const fn port(id: &'static str, direction: Direction) -> PortContract<'static> {
    PortContract {
        id: Id(id),
        direction,
        value_type: TEXT,
        presence: Presence::Required,
        connections: match direction {
            Direction::Input => ConnectionCardinality::ExactlyOne,
            Direction::Output => ConnectionCardinality::OneOrMore,
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

const AP_INPUTS: [PortContract<'static>; 1] = [port("configuration", Direction::Input)];
const AP_OUTPUTS: [PortContract<'static>; 1] = [port("state", Direction::Output)];
const DHCP_INPUTS: [PortContract<'static>; 1] = [port("request", Direction::Input)];
const DHCP_OUTPUTS: [PortContract<'static>; 1] = [port("lease", Direction::Output)];
const REACHABILITY_INPUTS: [PortContract<'static>; 1] = [port("target", Direction::Input)];
const REACHABILITY_OUTPUTS: [PortContract<'static>; 1] = [port("observation", Direction::Output)];
const DNS_SD_INPUTS: [PortContract<'static>; 1] = [port("request", Direction::Input)];
const DNS_SD_OUTPUTS: [PortContract<'static>; 1] = [port("record", Direction::Output)];

pub const WIFI_AP_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("net/wifi/access-point"),
    config: ConfigContract { fields: &AP_FIELDS },
    inputs: &AP_INPUTS,
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
pub const REACHABILITY_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("net/reachability"),
    config: ConfigContract {
        fields: &REACHABILITY_FIELDS,
    },
    inputs: &REACHABILITY_INPUTS,
    outputs: &REACHABILITY_OUTPUTS,
};
pub const DNS_SD_CONTRACT: NodeContract<'static> = NodeContract {
    id: Id("net/dns-sd"),
    config: ConfigContract {
        fields: &DNS_SD_FIELDS,
    },
    inputs: &DNS_SD_INPUTS,
    outputs: &DNS_SD_OUTPUTS,
};

pub const NETWORK_CONTRACTS: [&NodeContract<'static>; 4] = [
    &WIFI_AP_CONTRACT,
    &DHCP_SERVER_CONTRACT,
    &REACHABILITY_CONTRACT,
    &DNS_SD_CONTRACT,
];

const RESOURCE: &str = "conduit.resource/network-fixture";
const GRANT: &str = "conduit.grant/network-fixture";

fn error(reason: NetworkReason, detail: &'static str) -> RuntimeError {
    RuntimeError::new(reason.code(), detail)
}

fn resolution(reason: NetworkReason, detail: &'static str) -> ResolutionError {
    ResolutionError::new(reason.code(), detail)
}

fn integer(node: &Node, key: &str) -> Option<u64> {
    match node.config_value(key) {
        Some(SourceValue::Integer(value)) => u64::try_from(*value).ok(),
        _ => None,
    }
}

fn reference(node: &Node, key: &str, expected: &str) -> bool {
    matches!(
        node.config_value(key),
        Some(SourceValue::Reference(value) | SourceValue::SecretReference(value)) if value == expected
    )
}

fn validate_common(node: &Node, expected_fields: usize) -> Result<(), ResolutionError> {
    if node.config.len() != expected_fields
        || !reference(node, "resource", RESOURCE)
        || !reference(node, "grant", GRANT)
        || node.config("interface") != Some("fixture/ap0")
        || integer(node, "maximum_request_bytes") != Some(MAXIMUM_PACKET_BYTES as u64)
        || integer(node, "maximum_response_bytes") != Some(256)
        || integer(node, "maximum_pending") != Some(1)
        || integer(node, "maximum_evidence_events") != Some(MAXIMUM_EVIDENCE_EVENTS as u64)
        || integer(node, "deadline_ticks") != Some(1_000)
        || node.config("cancellation") != Some("cancel-before-commit")
    {
        return Err(resolution(
            NetworkReason::MalformedPacket,
            "network node does not match the exact bounded fixture profile",
        ));
    }
    Ok(())
}

fn validate_ap(node: &Node) -> Result<(), ResolutionError> {
    validate_common(node, AP_FIELDS.len())?;
    if node.config("ssid_prefix") != Some("pete-")
        || node.config("address") != Some("192.168.4.1")
        || integer(node, "maximum_clients") != Some(MAXIMUM_CLIENTS as u64)
        || node.config("routing") != Some("forbidden")
        || node.config("bridging") != Some("forbidden")
        || node.config("nat") != Some("forbidden")
    {
        return Err(resolution(
            NetworkReason::RoutingForbidden,
            "access point must retain the isolated no-route/no-bridge/no-NAT profile",
        ));
    }
    Ok(())
}

fn validate_dhcp(node: &Node) -> Result<(), ResolutionError> {
    validate_common(node, DHCP_FIELDS.len())?;
    if node.config("server_address") != Some("192.168.4.1")
        || node.config("pool_first") != Some("192.168.4.2")
        || node.config("pool_last") != Some("192.168.4.9")
        || integer(node, "maximum_leases") != Some(MAXIMUM_CLIENTS as u64)
        || integer(node, "lease_ticks") != Some(DHCP_LEASE_TICKS)
    {
        return Err(resolution(
            NetworkReason::PoolExhausted,
            "DHCP server must retain the exact finite eight-client lease profile",
        ));
    }
    Ok(())
}

fn validate_reachability(node: &Node) -> Result<(), ResolutionError> {
    validate_common(node, REACHABILITY_FIELDS.len())?;
    if node.config("address") != Some("192.168.4.1")
        || integer(node, "maximum_packet_bytes") != Some(MAXIMUM_PACKET_BYTES as u64)
        || integer(node, "maximum_packets_per_window") != Some(u64::from(ICMP_PACKETS_PER_WINDOW))
        || integer(node, "window_ticks") != Some(ICMP_WINDOW_TICKS)
    {
        return Err(resolution(
            NetworkReason::RateLimited,
            "ICMP reachability requires exact packet and rate bounds",
        ));
    }
    Ok(())
}

fn validate_dns_sd(node: &Node) -> Result<(), ResolutionError> {
    validate_common(node, DNS_SD_FIELDS.len())?;
    if node.config("name") != Some("pete.local")
        || node.config("address") != Some("192.168.4.1")
        || integer(node, "ttl_ticks") != Some(120_000)
        || integer(node, "maximum_records") != Some(MAXIMUM_CLIENTS as u64)
        || integer(node, "maximum_name_bytes") != Some(MAXIMUM_NAME_BYTES as u64)
    {
        return Err(resolution(
            NetworkReason::NameConflict,
            "DNS-SD requires the exact bounded local-name profile",
        ));
    }
    Ok(())
}

fn single_text<'a>(
    inputs: &'a [Value],
    contract: &NodeContract<'_>,
) -> Result<&'a str, RuntimeError> {
    let input = inputs
        .iter()
        .find(|value| value.value_type == contract.inputs[0].value_type)
        .ok_or_else(|| error(NetworkReason::MalformedPacket, "network request is missing"))?;
    core::str::from_utf8(&input.bytes).map_err(|_| {
        error(
            NetworkReason::MalformedPacket,
            "network request is not UTF-8",
        )
    })
}

fn output(contract: &NodeContract<'static>, text: String) -> Vec<Value> {
    vec![Value {
        value_type: contract.outputs[0].value_type,
        bytes: text.into_bytes(),
    }]
}

struct FixtureAp;

impl Handler for FixtureAp {
    fn run(
        &mut self,
        _node: &Node,
        inputs: &[Value],
        _io: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        if single_text(inputs, &WIFI_AP_CONTRACT)? != "start-isolated" {
            return Err(error(
                NetworkReason::MalformedPacket,
                "unsupported AP request",
            ));
        }
        Ok(output(
            &WIFI_AP_CONTRACT,
            "wifi-ap:pete-fixture:192.168.4.1:clients=8:no-route:no-bridge:no-nat".to_owned(),
        ))
    }
}

struct FixtureDhcp {
    leases: DhcpLeaseTable,
}

impl Handler for FixtureDhcp {
    fn run(
        &mut self,
        _node: &Node,
        inputs: &[Value],
        _io: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        let request = single_text(inputs, &DHCP_SERVER_CONTRACT)?;
        let (operation, client) = request.split_once(':').ok_or_else(|| {
            error(
                NetworkReason::MalformedPacket,
                "malformed DHCP fixture request",
            )
        })?;
        let client = ClientIdentity::new(client.as_bytes())
            .map_err(|reason| error(reason, "invalid DHCP client identity"))?;
        let message = match operation {
            "discover" => DhcpMessage::Discover,
            "renew" => DhcpMessage::Renew,
            "release" => DhcpMessage::Release,
            _ => {
                return Err(error(
                    NetworkReason::MalformedPacket,
                    "unknown DHCP operation",
                ));
            }
        };
        let outcome = self
            .leases
            .handle(message, client, request.len(), 10)
            .map_err(|reason| error(reason, "bounded DHCP operation failed"))?;
        let text = match outcome {
            DhcpOutcome::Offered(lease) => format!(
                "offered:{}.{}.{}.{}:generation={}:expires={}",
                lease.address.0[0],
                lease.address.0[1],
                lease.address.0[2],
                lease.address.0[3],
                lease.generation,
                lease.expires_at_tick
            ),
            DhcpOutcome::Acknowledged(lease) => format!(
                "acknowledged:{}.{}.{}.{}:generation={}:expires={}",
                lease.address.0[0],
                lease.address.0[1],
                lease.address.0[2],
                lease.address.0[3],
                lease.generation,
                lease.expires_at_tick
            ),
            DhcpOutcome::Released => "released".to_owned(),
        };
        Ok(output(&DHCP_SERVER_CONTRACT, text))
    }
}

struct FixtureReachability {
    limiter: IcmpRateLimiter,
}

impl Handler for FixtureReachability {
    fn run(
        &mut self,
        _node: &Node,
        inputs: &[Value],
        _io: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        let target = single_text(inputs, &REACHABILITY_CONTRACT)?;
        if target != "192.168.4.1" {
            return Err(error(
                NetworkReason::MalformedPacket,
                "target is outside binding",
            ));
        }
        self.limiter
            .admit(target.len(), 10)
            .map_err(|reason| error(reason, "ICMP request rejected"))?;
        Ok(output(
            &REACHABILITY_CONTRACT,
            "icmp:192.168.4.1:reachable:rate=4/1000".to_owned(),
        ))
    }
}

struct FixtureDnsSd {
    records: DnsSdTable,
}

impl Handler for FixtureDnsSd {
    fn run(
        &mut self,
        _node: &Node,
        inputs: &[Value],
        _io: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        let request = single_text(inputs, &DNS_SD_CONTRACT)?;
        if request != "publish:pete.local" {
            return Err(error(
                NetworkReason::MalformedPacket,
                "unsupported DNS-SD request",
            ));
        }
        let name = ServiceName::new("pete.local")
            .map_err(|reason| error(reason, "invalid DNS-SD name"))?;
        let record = self
            .records
            .publish(name, Ipv4Address(AP_ADDRESS.0), 120_000, 10)
            .map_err(|reason| error(reason, "DNS-SD publication failed"))?;
        Ok(output(
            &DNS_SD_CONTRACT,
            format!(
                "dns-sd:{}:192.168.4.1:generation={}:expires={}",
                record.name.as_str(),
                record.generation,
                record.expires_at_tick
            ),
        ))
    }
}

pub fn register_network_contracts(registry: &mut Registry) {
    for contract in NETWORK_CONTRACTS {
        registry.register_contract_only(contract);
    }
}

/// Installs only a deterministic, no-radio fixture provider. A physical
/// adapter must instead bind an exact current observation through the enforced
/// effect backend; this function is never evidence of Pico W availability.
pub fn register_deterministic_network_fixture_providers(
    registry: &mut Registry,
) -> Result<(), RegistryError> {
    register_network_contracts(registry);
    static NO_AUTHORITIES: [SemanticHash; 0] = [];
    for provider in [
        CompiledInHostService {
            contract: &WIFI_AP_CONTRACT,
            implementation_id: "conduit.net/fixture-wifi-ap",
            artifact_id: "conduit.net/fixture-wifi-ap-artifact",
            entrypoint: "network-fixture-wifi-ap",
            source_bytes: include_bytes!("runtime_nodes.rs"),
            required_authorities: &NO_AUTHORITIES,
            factory: || Box::new(FixtureAp),
            validate_config: validate_ap,
        },
        CompiledInHostService {
            contract: &DHCP_SERVER_CONTRACT,
            implementation_id: "conduit.net/fixture-dhcp-server",
            artifact_id: "conduit.net/fixture-dhcp-server-artifact",
            entrypoint: "network-fixture-dhcp-server",
            source_bytes: include_bytes!("runtime_nodes.rs"),
            required_authorities: &NO_AUTHORITIES,
            factory: || {
                Box::new(FixtureDhcp {
                    leases: DhcpLeaseTable::new(),
                })
            },
            validate_config: validate_dhcp,
        },
        CompiledInHostService {
            contract: &REACHABILITY_CONTRACT,
            implementation_id: "conduit.net/fixture-icmp",
            artifact_id: "conduit.net/fixture-icmp-artifact",
            entrypoint: "network-fixture-icmp",
            source_bytes: include_bytes!("runtime_nodes.rs"),
            required_authorities: &NO_AUTHORITIES,
            factory: || {
                Box::new(FixtureReachability {
                    limiter: IcmpRateLimiter::new(),
                })
            },
            validate_config: validate_reachability,
        },
        CompiledInHostService {
            contract: &DNS_SD_CONTRACT,
            implementation_id: "conduit.net/fixture-dns-sd",
            artifact_id: "conduit.net/fixture-dns-sd-artifact",
            entrypoint: "network-fixture-dns-sd",
            source_bytes: include_bytes!("runtime_nodes.rs"),
            required_authorities: &NO_AUTHORITIES,
            factory: || {
                Box::new(FixtureDnsSd {
                    records: DnsSdTable::new(),
                })
            },
            validate_config: validate_dns_sd,
        },
    ] {
        registry.register_compiled_in_host_service(provider)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_runtime::AvailabilityState;

    #[test]
    fn contract_inventory_has_no_provider_until_fixture_is_explicitly_installed() {
        let mut registry = Registry::default();
        register_network_contracts(&mut registry);
        for contract in NETWORK_CONTRACTS {
            assert_eq!(
                registry.node_availability(contract.id.as_str()).state,
                AvailabilityState::ContractOnly
            );
        }
        register_deterministic_network_fixture_providers(&mut registry).unwrap();
        for contract in NETWORK_CONTRACTS {
            assert_eq!(
                registry.node_availability(contract.id.as_str()).state,
                AvailabilityState::ProviderAvailable
            );
        }
    }

    #[test]
    fn source_cannot_claim_initialized_fresh_or_authorized_provider_state() {
        for contract in NETWORK_CONTRACTS {
            let fields = contract
                .config
                .fields
                .iter()
                .map(|field| field.key.as_str())
                .collect::<Vec<_>>();
            for forbidden in [
                "initialized",
                "fresh",
                "observation",
                "provider_available",
                "destination_allowed",
                "possession",
                "motor_authority",
                "safety_authority",
            ] {
                assert!(
                    !fields.contains(&forbidden),
                    "{} exposes {forbidden}",
                    contract.id
                );
            }
        }
    }
}
