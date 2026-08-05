use conduit_core::{
    mandatory_evidence_storage_requirement, seal_plan, CancellationPolicy, CapabilityId,
    ConnectionId, ConnectionProvider, ExpectedEvidence, ExpectedTerminal, FragmentId,
    HostAdvertisement, HostId, OperationId, PlacementId, Plan, PlanFragment, PlanId,
    PlannedConnection, PlannedOperation, StartupDependency, TerminalPolicy,
    DEFAULT_CONNECTION_BYTE_CAPACITY, DEFAULT_CONNECTION_ITEM_CAPACITY,
};
use conduit_form::{CheckedForm, CheckedOperation};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

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
    WrongKindContractRevision(String),
    IncompatiblePortContract(String),
    InvalidHostOperationRequirement(String),
    UnavailableConnectionProvider(String),
    QueueRequirementAboveHostLimit(String),
    CapabilityInstanceLimitExceeded(String),
    CyclicStartupDependencies(String),
    EvidenceBudgetOverflow(String),
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
            PlannerError::WrongKindContractRevision(value) => {
                write!(f, "wrong kind contract revision: {value}")
            }
            PlannerError::IncompatiblePortContract(value) => {
                write!(f, "incompatible port contract: {value}")
            }
            PlannerError::InvalidHostOperationRequirement(value) => {
                write!(f, "invalid host-operation requirement: {value}")
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
            PlannerError::CyclicStartupDependencies(value) => {
                write!(f, "cyclic startup dependencies: {value}")
            }
            PlannerError::EvidenceBudgetOverflow(value) => {
                write!(f, "mandatory evidence budget overflow: {value}")
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
            .find(|offer| {
                offer.kind_id == operation.kind_id
                    && offer.kind_contract_revision == operation.kind_contract_revision
                    && offer.inputs == operation.inputs
                    && offer.outputs == operation.outputs
            })
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
            form.checked_form_id.as_str(),
            operation.operation_id.as_str(),
            host.host_id.as_str(),
            capability.capability_id.as_str()
        )));
        placement_lookup.insert(operation.operation_id.clone(), placement_id.clone());
        planned_operations.push(PlannedOperation {
            placement_id,
            operation_id: operation.operation_id.clone(),
            kind_id: operation.kind_id.clone(),
            kind_contract_revision: operation.kind_contract_revision.clone(),
            execution_profile_id: capability.execution_profile_id.clone(),
            configuration: operation.configuration.clone(),
            host_id: host.host_id.clone(),
            boot_id: host.boot_id.clone(),
            offer_generation: host.offer_generation,
            capability_id: capability.capability_id.clone(),
            implementation_id: capability.implementation_id.clone(),
            artifact_id: capability.artifact_id.clone(),
            inputs: operation.inputs.clone(),
            outputs: operation.outputs.clone(),
            host_operations: capability.host_operations.clone(),
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
                form.checked_form_id.as_str(),
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

    let global_startup_order = startup_order(&planned_operations, &planned_connections)
        .ok_or_else(|| PlannerError::CyclicStartupDependencies(form.name.clone()))?;

    let fragments = realm
        .iter()
        .map(|host| -> Result<Option<PlanFragment>, PlannerError> {
            let placements = planned_operations
                .iter()
                .filter(|item| item.host_id == host.host_id)
                .cloned()
                .collect::<Vec<_>>();
            if placements.is_empty() {
                return Ok(None);
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
            let startup_order = global_startup_order
                .iter()
                .filter(|placement_id| {
                    placements
                        .iter()
                        .any(|placement| &placement.placement_id == *placement_id)
                })
                .cloned()
                .collect();
            let startup_dependencies = connections
                .iter()
                .map(|connection| StartupDependency {
                    prerequisite_placement_id: connection.sink_placement_id.clone(),
                    dependent_placement_id: connection.source_placement_id.clone(),
                })
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect();
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
                .collect::<Vec<_>>();
            let evidence_storage_budget =
                mandatory_evidence_storage_requirement(&expected_evidence).ok_or_else(|| {
                    PlannerError::EvidenceBudgetOverflow(host.host_id.as_str().to_string())
                })?;
            Ok(Some(PlanFragment {
                plan_id: PlanId::from(""),
                fragment_id: FragmentId::from(""),
                source_document_id: form.source_document_id.clone(),
                checked_form_id: form.checked_form_id.clone(),
                expanded_form_id: form.expanded_form_id.clone(),
                host_id: host.host_id.clone(),
                boot_id: host.boot_id.clone(),
                offer_generation: host.offer_generation,
                placements,
                connections,
                startup_dependencies,
                startup_order,
                cancellation_policy: CancellationPolicy::CancelAllAndRejectLateCompletion,
                terminal_policy: TerminalPolicy::RequireAllPlacementsAndConnections,
                expected_terminals,
                expected_evidence,
                evidence_storage_budget,
                plan_fragments: Vec::new(),
            }))
        })
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();

    Ok(seal_plan(form.identity(), fragments))
}

fn startup_order(
    placements: &[PlannedOperation],
    connections: &[PlannedConnection],
) -> Option<Vec<PlacementId>> {
    let mut remaining = placements
        .iter()
        .map(|placement| placement.placement_id.clone())
        .collect::<BTreeSet<_>>();
    let mut ordered = Vec::with_capacity(remaining.len());
    while !remaining.is_empty() {
        let next = remaining
            .iter()
            .find(|candidate| {
                connections.iter().all(|connection| {
                    &connection.source_placement_id != *candidate
                        || !remaining.contains(&connection.sink_placement_id)
                })
            })
            .cloned()?;
        remaining.remove(&next);
        ordered.push(next);
    }
    Some(ordered)
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
    if capability.kind_contract_revision != operation.kind_contract_revision {
        return Err(PlannerError::WrongKindContractRevision(format!(
            "operation '{}' requires '{}', capability '{}' offers '{}'",
            operation.operation_id.as_str(),
            operation.kind_contract_revision.as_str(),
            capability.capability_id.as_str(),
            capability.kind_contract_revision.as_str()
        )));
    }
    if capability.inputs != operation.inputs || capability.outputs != operation.outputs {
        return Err(PlannerError::IncompatiblePortContract(format!(
            "operation '{}' ports differ from capability '{}'",
            operation.operation_id.as_str(),
            capability.capability_id.as_str()
        )));
    }
    if capability.host_operations.iter().any(|requirement| {
        requirement.contract_id.as_str().is_empty()
            || requirement
                .target_kind
                .as_ref()
                .is_some_and(|target| target.as_str().is_empty())
            || requirement.maximum_in_flight == 0
    }) || capability
        .host_operations
        .windows(2)
        .any(|pair| pair[0] >= pair[1])
    {
        return Err(PlannerError::InvalidHostOperationRequirement(format!(
            "capability '{}' requirements must have non-empty identities, unique canonical ordering, and nonzero in-flight bounds",
            capability.capability_id.as_str()
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
            | ConnectionProvider::FixtureFrame
            | ConnectionProvider::FixtureDatagram => source.host_id != sink.host_id,
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
    if source.host_id != sink.host_id && providers.contains(&ConnectionProvider::FixtureFrame) {
        return Ok(ConnectionProvider::FixtureFrame);
    }
    if source.host_id != sink.host_id && providers.contains(&ConnectionProvider::FixtureDatagram) {
        return Ok(ConnectionProvider::FixtureDatagram);
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
    use super::{default_placements, parse_placements, plan, startup_order, PlannerError};
    use conduit_core::{
        kind_id, mandatory_evidence_storage_requirement, verify_plan, ArtifactId,
        CancellationPolicy, CapabilityLimits, CapabilityOffer, ConnectionProvider, ExpandedFormId,
        HostAdvertisement, HostId, HostProfileId, ImplementationId, OfferGeneration,
        SourceDocumentId, StartupDependency, TerminalPolicy, PROTOCOL_VERSION,
    };
    use conduit_form::parse;
    use conduit_signal::{
        pulse_contract_revision, pulse_execution_profile, pulse_host_operation_requirements,
        pulse_outputs, show_contract_revision, show_execution_profile,
        show_host_operation_requirements, show_inputs, signal_profile_catalog, PULSE_KIND,
        SHOW_KIND,
    };

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
                    kind_contract_revision: pulse_contract_revision(),
                    execution_profile_id: pulse_execution_profile(),
                    implementation_id: ImplementationId::from("std/pulse-v1"),
                    artifact_id: ArtifactId::from("test/pulse-artifact-v1"),
                    inputs: vec![],
                    outputs: pulse_outputs(),
                    host_operations: pulse_host_operation_requirements(),
                    limits: CapabilityLimits {
                        max_active_instances: 4,
                        max_queue_items: 4,
                        max_queue_bytes: 64,
                    },
                },
                CapabilityOffer {
                    capability_id: conduit_core::CapabilityId::from("stdout-show-1"),
                    kind_id: kind_id(SHOW_KIND),
                    kind_contract_revision: show_contract_revision(),
                    execution_profile_id: show_execution_profile(),
                    implementation_id: ImplementationId::from("std/stdout-show-signal-v1"),
                    artifact_id: ArtifactId::from("test/show-artifact-v1"),
                    inputs: show_inputs(),
                    outputs: vec![],
                    host_operations: show_host_operation_requirements(),
                    limits: CapabilityLimits {
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
    fn planning_binds_exact_contract_profile_and_every_port() {
        let form = form();
        let host = host();
        let placements = default_placements(&form, std::slice::from_ref(&host))
            .expect("placements must resolve");
        let plan = plan(
            &form,
            std::slice::from_ref(&host),
            &placements,
            &[ConnectionProvider::Local],
        )
        .expect("exact plan resolves");
        assert_eq!(plan.source_document_id, form.source_document_id);
        assert_eq!(plan.checked_form_id, form.checked_form_id);
        assert_eq!(plan.expanded_form_id, form.expanded_form_id);
        assert!(plan.fragments.iter().all(|fragment| {
            fragment.source_document_id == form.source_document_id
                && fragment.checked_form_id == form.checked_form_id
                && fragment.expanded_form_id == form.expanded_form_id
        }));
        for placement in &plan.fragments[0].placements {
            let operation = form
                .operations
                .iter()
                .find(|operation| operation.operation_id == placement.operation_id)
                .expect("checked operation exists");
            let capability = host
                .capabilities
                .iter()
                .find(|capability| capability.capability_id == placement.capability_id)
                .expect("capability exists");
            assert_eq!(
                placement.kind_contract_revision,
                operation.kind_contract_revision
            );
            assert_eq!(
                placement.kind_contract_revision,
                capability.kind_contract_revision
            );
            assert_eq!(
                placement.execution_profile_id,
                capability.execution_profile_id
            );
            assert_eq!(placement.inputs, operation.inputs);
            assert_eq!(placement.outputs, operation.outputs);
            assert_eq!(placement.host_operations, capability.host_operations);
        }
        assert!(plan.fragments[0]
            .placements
            .iter()
            .all(|placement| !placement.host_operations.is_empty()));
        let fragment = &plan.fragments[0];
        assert_eq!(
            fragment.startup_dependencies,
            vec![StartupDependency {
                prerequisite_placement_id: fragment.connections[0].sink_placement_id.clone(),
                dependent_placement_id: fragment.connections[0].source_placement_id.clone(),
            }]
        );
        assert_eq!(
            fragment.startup_order,
            vec![
                fragment.connections[0].sink_placement_id.clone(),
                fragment.connections[0].source_placement_id.clone(),
            ]
        );
        assert_eq!(
            fragment.cancellation_policy,
            CancellationPolicy::CancelAllAndRejectLateCompletion
        );
        assert_eq!(
            fragment.terminal_policy,
            TerminalPolicy::RequireAllPlacementsAndConnections
        );
        assert_eq!(
            fragment.evidence_storage_budget,
            mandatory_evidence_storage_requirement(&fragment.expected_evidence)
                .expect("focused evidence fits public budget types")
        );
    }

    #[test]
    fn planning_rejects_cyclic_startup_dependencies() {
        let form = form();
        let host = host();
        let placements = default_placements(&form, std::slice::from_ref(&host))
            .expect("placements must resolve");
        let plan = plan(
            &form,
            std::slice::from_ref(&host),
            &placements,
            &[ConnectionProvider::Local],
        )
        .expect("acyclic plan resolves");
        let fragment = &plan.fragments[0];
        let mut connections = fragment.connections.clone();
        let mut reverse = connections[0].clone();
        core::mem::swap(
            &mut reverse.source_placement_id,
            &mut reverse.sink_placement_id,
        );
        connections.push(reverse);
        assert_eq!(startup_order(&fragment.placements, &connections), None);
    }

    #[test]
    fn planning_rejects_invalid_host_operation_requirements() {
        let form = form();
        let mut host = host();
        host.capabilities[0].host_operations[0].maximum_in_flight = 0;
        let placements = default_placements(&form, std::slice::from_ref(&host))
            .expect("placements still resolve");
        assert!(matches!(
            plan(
                &form,
                std::slice::from_ref(&host),
                &placements,
                &[ConnectionProvider::Local],
            ),
            Err(PlannerError::InvalidHostOperationRequirement(_))
        ));
    }

    #[test]
    fn planning_verification_rejects_each_top_level_form_identity_mutation() {
        let form = form();
        let host = host();
        let placements = default_placements(&form, std::slice::from_ref(&host))
            .expect("placements must resolve");
        let original = plan(
            &form,
            std::slice::from_ref(&host),
            &placements,
            &[ConnectionProvider::Local],
        )
        .expect("exact plan resolves");

        let mut source_changed = form.clone();
        source_changed.source_document_id = SourceDocumentId::from("changed-source");
        let source_plan = plan(
            &source_changed,
            std::slice::from_ref(&host),
            &placements,
            &[ConnectionProvider::Local],
        )
        .expect("source-identity plan resolves");
        assert_ne!(original.plan_id, source_plan.plan_id);

        let mut checked_changed = form.clone();
        checked_changed.checked_form_id = conduit_core::CheckedFormId::from("changed-checked");
        let checked_plan = plan(
            &checked_changed,
            std::slice::from_ref(&host),
            &placements,
            &[ConnectionProvider::Local],
        )
        .expect("checked-identity plan resolves");
        assert_ne!(original.plan_id, checked_plan.plan_id);

        let mut expanded_changed = form.clone();
        expanded_changed.expanded_form_id = ExpandedFormId::from("changed-expanded");
        let expanded_plan = plan(
            &expanded_changed,
            std::slice::from_ref(&host),
            &placements,
            &[ConnectionProvider::Local],
        )
        .expect("expanded-identity plan resolves");
        assert_ne!(original.plan_id, expanded_plan.plan_id);

        let mut mutated = original.clone();
        mutated.source_document_id = SourceDocumentId::from("mutated-source");
        assert!(!verify_plan(&mutated));

        let mut mutated = original.clone();
        mutated.checked_form_id = conduit_core::CheckedFormId::from("mutated-checked");
        assert!(!verify_plan(&mutated));

        let mut mutated = original;
        mutated.expanded_form_id = ExpandedFormId::from("mutated-expanded");
        assert!(!verify_plan(&mutated));
    }

    #[test]
    fn planning_rejects_contract_revision_and_nonfirst_port_mismatch() {
        let form = form();
        let original_host = host();
        let placements = default_placements(&form, std::slice::from_ref(&original_host))
            .expect("placements must resolve");

        let mut mismatched_revision = original_host.clone();
        mismatched_revision.capabilities[0].kind_contract_revision =
            conduit_core::KindContractRevision::from("mutated/flow-pulse@1");
        assert!(matches!(
            plan(
                &form,
                std::slice::from_ref(&mismatched_revision),
                &placements,
                &[ConnectionProvider::Local]
            ),
            Err(PlannerError::WrongKindContractRevision(_))
        ));

        let mut mismatched_ports = original_host;
        mismatched_ports.capabilities[0]
            .outputs
            .push(conduit_core::PortDescriptor {
                port_id: conduit_core::PortId::from("unexpected"),
                value_kind: kind_id("value/unexpected"),
                direction: conduit_core::PortDirection::Output,
            });
        assert!(matches!(
            plan(
                &form,
                std::slice::from_ref(&mismatched_ports),
                &placements,
                &[ConnectionProvider::Local]
            ),
            Err(PlannerError::IncompatiblePortContract(_))
        ));
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
