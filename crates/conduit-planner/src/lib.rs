use conduit_core::{
    kind_id, seal_plan, CapabilityId, ConnectionId, ConnectionProvider, ExpectedEvidence,
    ExpectedTerminal, FragmentId, HostAdvertisement, HostId, OperationId, PlacementId, Plan,
    PlanFragment, PlanId, PlannedConnection, PlannedOperation, DEFAULT_CONNECTION_BYTE_CAPACITY,
    DEFAULT_CONNECTION_ITEM_CAPACITY,
};
use conduit_form::{CheckedConnection, CheckedForm, CheckedOperation};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacementChoice {
    pub host_id: HostId,
    pub capability_id: CapabilityId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacementChoices {
    pub by_operation: BTreeMap<OperationId, PlacementChoice>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlannerError {
    UnknownOperation(String),
    MissingPlacement(String),
    DuplicatePlacement(String),
    UnknownHost(String),
    UnknownCapability(String),
    WrongSemanticKind(String),
    IncompatibleValueKind(String),
    UnavailableConnectionProvider(String),
    QueueRequirementAboveHostLimit(String),
    CapabilityInstanceLimitExceeded(String),
    InvalidPlacementSyntax(String),
}

impl std::fmt::Display for PlannerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlannerError::UnknownOperation(value) => write!(f, "unknown operation '{value}'"),
            PlannerError::MissingPlacement(value) => write!(f, "missing placement for '{value}'"),
            PlannerError::DuplicatePlacement(value) => {
                write!(f, "duplicate placement for '{value}'")
            }
            PlannerError::UnknownHost(value) => write!(f, "unknown host '{value}'"),
            PlannerError::UnknownCapability(value) => write!(f, "unknown capability '{value}'"),
            PlannerError::WrongSemanticKind(value) => write!(f, "wrong semantic kind: {value}"),
            PlannerError::IncompatibleValueKind(value) => {
                write!(f, "incompatible value kind: {value}")
            }
            PlannerError::UnavailableConnectionProvider(value) => {
                write!(f, "unavailable connection provider: {value}")
            }
            PlannerError::QueueRequirementAboveHostLimit(value) => {
                write!(f, "queue requirement above host limit: {value}")
            }
            PlannerError::CapabilityInstanceLimitExceeded(value) => {
                write!(f, "capability instance limit exceeded: {value}")
            }
            PlannerError::InvalidPlacementSyntax(value) => {
                write!(f, "invalid placement syntax: {value}")
            }
        }
    }
}

impl std::error::Error for PlannerError {}

pub fn parse_placements(source: &str) -> Result<PlacementChoices, PlannerError> {
    let lines: Vec<&str> = source
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect();

    if lines.first().copied().unwrap_or("") != "placements 0" {
        return Err(PlannerError::InvalidPlacementSyntax(
            "expected first non-comment line to be 'placements 0'".to_string(),
        ));
    }

    let mut index = 1usize;
    let mut by_operation = BTreeMap::new();
    while index < lines.len() {
        let header = lines[index];
        let operation_name = header
            .strip_suffix(':')
            .ok_or_else(|| PlannerError::InvalidPlacementSyntax(header.to_string()))?;
        if by_operation.contains_key(&OperationId::from(operation_name)) {
            return Err(PlannerError::DuplicatePlacement(operation_name.to_string()));
        }
        let host_line = lines
            .get(index + 1)
            .ok_or_else(|| PlannerError::InvalidPlacementSyntax(operation_name.to_string()))?;
        let capability_line = lines
            .get(index + 2)
            .ok_or_else(|| PlannerError::InvalidPlacementSyntax(operation_name.to_string()))?;
        let host_id = parse_assignment(host_line, "host")?;
        let capability_id = parse_assignment(capability_line, "capability")?;
        by_operation.insert(
            OperationId::from(operation_name),
            PlacementChoice {
                host_id: HostId::from(host_id),
                capability_id: CapabilityId::from(capability_id),
            },
        );
        index += 3;
    }

    Ok(PlacementChoices { by_operation })
}

fn parse_assignment<'a>(line: &'a str, key: &str) -> Result<&'a str, PlannerError> {
    let (lhs, rhs) = line
        .split_once('=')
        .ok_or_else(|| PlannerError::InvalidPlacementSyntax(line.to_string()))?;
    if lhs.trim() != key {
        return Err(PlannerError::InvalidPlacementSyntax(line.to_string()));
    }
    let value = rhs.trim().trim_matches('"');
    if value.is_empty() {
        return Err(PlannerError::InvalidPlacementSyntax(line.to_string()));
    }
    Ok(value)
}

pub fn default_placements(
    form: &CheckedForm,
    realm: &[HostAdvertisement],
) -> Result<PlacementChoices, PlannerError> {
    let host = realm
        .first()
        .ok_or_else(|| PlannerError::UnknownHost("realm is empty".to_string()))?;
    let mut by_operation = BTreeMap::new();
    for operation in &form.operations {
        let offer = host
            .capabilities
            .iter()
            .find(|offer| offer.kind_id == operation.kind_id)
            .ok_or_else(|| {
                PlannerError::UnknownCapability(operation.kind_id.as_str().to_string())
            })?;
        by_operation.insert(
            operation.operation_id.clone(),
            PlacementChoice {
                host_id: host.host_id.clone(),
                capability_id: offer.capability_id.clone(),
            },
        );
    }
    Ok(PlacementChoices { by_operation })
}

pub fn plan(
    form: &CheckedForm,
    realm: &[HostAdvertisement],
    placements: &PlacementChoices,
    providers: &[ConnectionProvider],
) -> Result<Plan, PlannerError> {
    plan_with_connection_limits(
        form,
        realm,
        placements,
        providers,
        DEFAULT_CONNECTION_ITEM_CAPACITY,
        DEFAULT_CONNECTION_BYTE_CAPACITY,
    )
}

pub fn plan_with_connection_limits(
    form: &CheckedForm,
    realm: &[HostAdvertisement],
    placements: &PlacementChoices,
    providers: &[ConnectionProvider],
    connection_item_capacity: u16,
    connection_byte_capacity: u32,
) -> Result<Plan, PlannerError> {
    plan_with_connection_limits_and_provider_overrides(
        form,
        realm,
        placements,
        providers,
        &BTreeMap::new(),
        connection_item_capacity,
        connection_byte_capacity,
    )
}

pub fn plan_with_connection_limits_and_provider_overrides(
    form: &CheckedForm,
    realm: &[HostAdvertisement],
    placements: &PlacementChoices,
    providers: &[ConnectionProvider],
    connection_providers: &BTreeMap<(OperationId, OperationId), ConnectionProvider>,
    connection_item_capacity: u16,
    connection_byte_capacity: u32,
) -> Result<Plan, PlannerError> {
    let realm_index = realm
        .iter()
        .map(|host| (host.host_id.clone(), host))
        .collect::<BTreeMap<_, _>>();

    let mut placement_count = BTreeMap::<(HostId, CapabilityId), u16>::new();
    let mut planned_operations = Vec::<PlannedOperation>::new();
    let mut placement_lookup = BTreeMap::<OperationId, PlacementId>::new();

    for operation in &form.operations {
        let choice = placements
            .by_operation
            .get(&operation.operation_id)
            .ok_or_else(|| {
                PlannerError::MissingPlacement(operation.operation_id.as_str().to_string())
            })?;
        let host = realm_index
            .get(&choice.host_id)
            .ok_or_else(|| PlannerError::UnknownHost(choice.host_id.as_str().to_string()))?;
        let capability = host
            .capabilities
            .iter()
            .find(|offer| offer.capability_id == choice.capability_id)
            .ok_or_else(|| {
                PlannerError::UnknownCapability(choice.capability_id.as_str().to_string())
            })?;
        validate_operation_capability(operation, capability)?;

        let count = placement_count
            .entry((host.host_id.clone(), capability.capability_id.clone()))
            .or_insert(0);
        *count += 1;
        if *count > capability.limits.max_active_instances {
            return Err(PlannerError::CapabilityInstanceLimitExceeded(format!(
                "capability '{}' exceeds max {}",
                capability.capability_id.as_str(),
                capability.limits.max_active_instances
            )));
        }

        let placement_id = PlacementId::from(hash_string(&format!(
            "placement:{}:{}:{}:{}",
            form.form_id.as_str(),
            operation.operation_id.as_str(),
            host.host_id.as_str(),
            capability.capability_id.as_str()
        )));
        placement_lookup.insert(operation.operation_id.clone(), placement_id.clone());
        planned_operations.push(PlannedOperation {
            placement_id,
            operation_id: operation.operation_id.clone(),
            kind_id: operation.kind_id.clone(),
            configuration: operation.configuration.clone(),
            host_id: host.host_id.clone(),
            boot_id: host.boot_id.clone(),
            offer_generation: host.offer_generation,
            capability_id: capability.capability_id.clone(),
            implementation_id: capability.implementation_id.clone(),
            artifact_id: capability.artifact_id.clone(),
            inputs: operation.inputs.clone(),
            outputs: operation.outputs.clone(),
        });
    }

    for operation in placements.by_operation.keys() {
        if !form
            .operations
            .iter()
            .any(|item| &item.operation_id == operation)
        {
            return Err(PlannerError::UnknownOperation(
                operation.as_str().to_string(),
            ));
        }
    }

    let mut planned_connections = Vec::<PlannedConnection>::new();
    for connection in &form.connections {
        let source_placement = placement_lookup
            .get(&connection.source_operation_id)
            .ok_or_else(|| {
                PlannerError::UnknownOperation(connection.source_operation_id.as_str().to_string())
            })?;
        let sink_placement = placement_lookup
            .get(&connection.sink_operation_id)
            .ok_or_else(|| {
                PlannerError::UnknownOperation(connection.sink_operation_id.as_str().to_string())
            })?;
        let source_plan = planned_operations
            .iter()
            .find(|item| &item.placement_id == source_placement)
            .expect("source placement must exist");
        let sink_plan = planned_operations
            .iter()
            .find(|item| &item.placement_id == sink_placement)
            .expect("sink placement must exist");
        let provider = select_provider(
            source_plan,
            sink_plan,
            providers,
            connection_providers
                .get(&(
                    connection.source_operation_id.clone(),
                    connection.sink_operation_id.clone(),
                ))
                .copied(),
        )?;
        let source_capability =
            find_capability(realm, &source_plan.host_id, &source_plan.capability_id)?;
        let sink_capability = find_capability(realm, &sink_plan.host_id, &sink_plan.capability_id)?;
        if connection_item_capacity > source_capability.limits.max_queue_items
            || connection_item_capacity > sink_capability.limits.max_queue_items
        {
            return Err(PlannerError::QueueRequirementAboveHostLimit(format!(
                "connection from '{}' to '{}' requires item capacity {}",
                source_plan.operation_id.as_str(),
                sink_plan.operation_id.as_str(),
                connection_item_capacity
            )));
        }
        if connection_byte_capacity > source_capability.limits.max_queue_bytes
            || connection_byte_capacity > sink_capability.limits.max_queue_bytes
        {
            return Err(PlannerError::QueueRequirementAboveHostLimit(format!(
                "connection from '{}' to '{}' requires byte capacity {}",
                source_plan.operation_id.as_str(),
                sink_plan.operation_id.as_str(),
                connection_byte_capacity
            )));
        }
        planned_connections.push(PlannedConnection {
            connection_id: ConnectionId::from(hash_string(&format!(
                "connection:{}:{}:{}:{}:{}:{}",
                form.form_id.as_str(),
                connection.source_operation_id.as_str(),
                connection.source_port_id.as_str(),
                connection.sink_operation_id.as_str(),
                connection.sink_port_id.as_str(),
                connection.value_kind.as_str()
            ))),
            source_placement_id: source_plan.placement_id.clone(),
            source_port_id: connection.source_port_id.clone(),
            sink_placement_id: sink_plan.placement_id.clone(),
            sink_port_id: connection.sink_port_id.clone(),
            value_kind: connection.value_kind.clone(),
            provider,
            item_capacity: connection_item_capacity,
            byte_capacity: connection_byte_capacity,
        });
    }

    let fragments = realm
        .iter()
        .filter_map(|host| {
            let placements = planned_operations
                .iter()
                .filter(|item| item.host_id == host.host_id)
                .cloned()
                .collect::<Vec<_>>();
            if placements.is_empty() {
                return None;
            }
            let connections = planned_connections
                .iter()
                .filter(|connection| {
                    placements
                        .iter()
                        .any(|item| item.placement_id == connection.source_placement_id)
                        || placements
                            .iter()
                            .any(|item| item.placement_id == connection.sink_placement_id)
                })
                .cloned()
                .collect::<Vec<_>>();
            let startup_order = startup_order(&placements, &form.connections);
            let expected_terminals = placements
                .iter()
                .map(|placement| {
                    ExpectedTerminal::PlacementCompleted(placement.placement_id.clone())
                })
                .chain(connections.iter().map(|connection| {
                    ExpectedTerminal::ConnectionCompleted(connection.connection_id.clone())
                }))
                .chain(core::iter::once(ExpectedTerminal::PlanCompleted))
                .collect();
            let expected_evidence = core::iter::once(ExpectedEvidence::PlanFragmentReceived)
                .chain(placements.iter().map(|placement| {
                    ExpectedEvidence::PlacementPrepared(placement.placement_id.clone())
                }))
                .chain(placements.iter().map(|placement| {
                    ExpectedEvidence::PlacementTerminal(placement.placement_id.clone())
                }))
                .chain(connections.iter().map(|connection| {
                    ExpectedEvidence::ConnectionTerminal(connection.connection_id.clone())
                }))
                .chain(core::iter::once(ExpectedEvidence::PlanTerminal))
                .collect();
            Some(PlanFragment {
                plan_id: PlanId::from(""),
                fragment_id: FragmentId::from(""),
                form_id: form.form_id.clone(),
                host_id: host.host_id.clone(),
                boot_id: host.boot_id.clone(),
                offer_generation: host.offer_generation,
                placements,
                connections,
                startup_order,
                expected_terminals,
                expected_evidence,
                plan_fragments: Vec::new(),
            })
        })
        .collect::<Vec<_>>();

    Ok(seal_plan(form.form_id.clone(), fragments))
}

fn startup_order(
    placements: &[PlannedOperation],
    connections: &[CheckedConnection],
) -> Vec<PlacementId> {
    let mut ordered = placements.to_vec();
    ordered.sort_by_key(|placement| {
        if connections
            .iter()
            .any(|connection| connection.sink_operation_id == placement.operation_id)
        {
            0u8
        } else {
            1u8
        }
    });
    ordered
        .into_iter()
        .map(|placement| placement.placement_id)
        .collect()
}

fn validate_operation_capability(
    operation: &CheckedOperation,
    capability: &conduit_core::CapabilityOffer,
) -> Result<(), PlannerError> {
    if capability.kind_id != operation.kind_id {
        return Err(PlannerError::WrongSemanticKind(format!(
            "operation '{}' requires '{}', capability '{}' offers '{}'",
            operation.operation_id.as_str(),
            operation.kind_id.as_str(),
            capability.capability_id.as_str(),
            capability.kind_id.as_str()
        )));
    }

    let value_kind = operation
        .outputs
        .first()
        .or_else(|| operation.inputs.first())
        .map(|port| port.value_kind.clone())
        .unwrap_or_else(|| kind_id(""));
    if capability.limits.value_kind != value_kind {
        return Err(PlannerError::IncompatibleValueKind(format!(
            "operation '{}' expects '{}', capability '{}' supports '{}'",
            operation.operation_id.as_str(),
            value_kind.as_str(),
            capability.capability_id.as_str(),
            capability.limits.value_kind.as_str()
        )));
    }
    Ok(())
}

fn find_capability<'a>(
    realm: &'a [HostAdvertisement],
    host_id: &HostId,
    capability_id: &CapabilityId,
) -> Result<&'a conduit_core::CapabilityOffer, PlannerError> {
    realm
        .iter()
        .find(|host| &host.host_id == host_id)
        .and_then(|host| {
            host.capabilities
                .iter()
                .find(|item| &item.capability_id == capability_id)
        })
        .ok_or_else(|| PlannerError::UnknownCapability(capability_id.as_str().to_string()))
}

fn select_provider(
    source: &PlannedOperation,
    sink: &PlannedOperation,
    providers: &[ConnectionProvider],
    requested: Option<ConnectionProvider>,
) -> Result<ConnectionProvider, PlannerError> {
    if let Some(provider) = requested {
        let supported = match provider {
            ConnectionProvider::Local => source.host_id == sink.host_id,
            ConnectionProvider::InMemory
            | ConnectionProvider::WebSocket
            | ConnectionProvider::Udp => source.host_id != sink.host_id,
        };
        if supported && providers.contains(&provider) {
            return Ok(provider);
        }
        return Err(PlannerError::UnavailableConnectionProvider(format!(
            "provider {:?} unavailable for '{}' -> '{}'",
            provider,
            source.operation_id.as_str(),
            sink.operation_id.as_str()
        )));
    }
    if source.host_id == sink.host_id && providers.contains(&ConnectionProvider::Local) {
        return Ok(ConnectionProvider::Local);
    }
    if source.host_id != sink.host_id && providers.contains(&ConnectionProvider::InMemory) {
        return Ok(ConnectionProvider::InMemory);
    }
    if source.host_id != sink.host_id && providers.contains(&ConnectionProvider::WebSocket) {
        return Ok(ConnectionProvider::WebSocket);
    }
    if source.host_id != sink.host_id && providers.contains(&ConnectionProvider::Udp) {
        return Ok(ConnectionProvider::Udp);
    }
    Err(PlannerError::UnavailableConnectionProvider(format!(
        "no provider for '{}' -> '{}'",
        source.operation_id.as_str(),
        sink.operation_id.as_str()
    )))
}

fn hash_string(text: &str) -> String {
    let digest = Sha256::digest(text.as_bytes());
    let mut encoded = String::with_capacity(digest.len() * 2);
    for byte in digest {
        encoded.push(hex(byte >> 4));
        encoded.push(hex(byte & 0x0f));
    }
    encoded
}

fn hex(nibble: u8) -> char {
    match nibble {
        0..=9 => (b'0' + nibble) as char,
        10..=15 => (b'a' + (nibble - 10)) as char,
        _ => unreachable!("nibble out of range"),
    }
}

#[cfg(test)]
mod tests {
    use super::{default_placements, parse_placements, plan, PlannerError};
    use conduit_core::{
        kind_id, ArtifactId, CapabilityLimits, CapabilityOffer, ConnectionProvider,
        HostAdvertisement, HostId, HostProfileId, ImplementationId, OfferGeneration,
        PROTOCOL_VERSION,
    };
    use conduit_form::parse;
    use conduit_signal::{signal_profile_catalog, PULSE_KIND, SHOW_KIND, SIGNAL_VALUE_KIND};

    fn form() -> conduit_form::CheckedForm {
        parse(
            "form 0\n\nsignal-demo {\n    pulse: flow/pulse\n    show: presentation/show\n\n    pulse.count = 2\n    pulse.period-ms = 0\n    pulse.initial = false\n\n    pulse > show\n}\n",
            &signal_profile_catalog(),
        )
        .expect("form must parse")
    }

    fn host() -> HostAdvertisement {
        HostAdvertisement {
            protocol_version: PROTOCOL_VERSION,
            host_id: HostId::from("std-host-1"),
            boot_id: conduit_core::BootId::from("boot-1"),
            offer_generation: OfferGeneration(1),
            profile: HostProfileId::from("rust-std"),
            capabilities: vec![
                CapabilityOffer {
                    capability_id: conduit_core::CapabilityId::from("pulse-1"),
                    kind_id: kind_id(PULSE_KIND),
                    implementation_id: ImplementationId::from("std/pulse-v1"),
                    artifact_id: ArtifactId::from("test/pulse-artifact-v1"),
                    limits: CapabilityLimits {
                        value_kind: kind_id(SIGNAL_VALUE_KIND),
                        max_active_instances: 4,
                        max_queue_items: 4,
                        max_queue_bytes: 64,
                    },
                },
                CapabilityOffer {
                    capability_id: conduit_core::CapabilityId::from("stdout-show-1"),
                    kind_id: kind_id(SHOW_KIND),
                    implementation_id: ImplementationId::from("std/stdout-show-signal-v1"),
                    artifact_id: ArtifactId::from("test/show-artifact-v1"),
                    limits: CapabilityLimits {
                        value_kind: kind_id(SIGNAL_VALUE_KIND),
                        max_active_instances: 4,
                        max_queue_items: 4,
                        max_queue_bytes: 64,
                    },
                },
            ],
        }
    }

    #[test]
    fn parses_block_placement_file() {
        let placements = parse_placements(
            "placements 0\npulse:\n    host = \"std-host-1\"\n    capability = \"pulse-1\"\nshow:\n    host = \"std-host-1\"\n    capability = \"stdout-show-1\"\n",
        )
        .expect("placements should parse");
        assert_eq!(placements.by_operation.len(), 2);
    }

    #[test]
    fn default_placement_uses_realm() {
        let placements = default_placements(&form(), &[host()]).expect("placements must work");
        assert_eq!(placements.by_operation.len(), 2);
    }

    #[test]
    fn planning_rejects_unknown_host() {
        let form = form();
        let placements = parse_placements(
            "placements 0\npulse:\n    host = \"missing\"\n    capability = \"pulse-1\"\nshow:\n    host = \"missing\"\n    capability = \"stdout-show-1\"\n",
        )
        .expect("placements should parse");
        let error = plan(&form, &[host()], &placements, &[ConnectionProvider::Local])
            .expect_err("planning should fail");
        assert!(matches!(error, PlannerError::UnknownHost(_)));
    }
}
