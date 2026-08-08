use conduit_core::{
    AuthorityGrant, CapabilityId, ConnectionProvider, HostId, LinkBinding, LinkBindingId,
    OperationId, ProtectedResourceGrant,
};
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

#[derive(Debug, Clone, Copy)]
pub struct PlanningOptions<'a> {
    pub connection_providers: &'a BTreeMap<(OperationId, OperationId), ConnectionProvider>,
    /// Exact observed binding identities to seal, in deterministic preference order.
    pub route_candidates: &'a BTreeMap<(OperationId, OperationId), Vec<LinkBindingId>>,
    pub connection_item_capacity: u16,
    pub connection_byte_capacity: u32,
    pub authority_grants: &'a [AuthorityGrant],
    pub protected_resource_grants: &'a [ProtectedResourceGrant],
    pub link_bindings: &'a [LinkBinding],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlannerError {
    InvalidFormIdentity(String),
    PlannerCapabilityNotAdvertised(String),
    PlannerCapabilityAmbiguous(String),
    PlannerLimitExceeded(String),
    UnknownOperation(String),
    MissingPlacement(String),
    DuplicatePlacement(String),
    UnknownHost(String),
    UnknownCapability(String),
    WrongSemanticKind(String),
    WrongKindContractRevision(String),
    IncompatiblePortContract(String),
    IncompatibleCheckedFace(String),
    InvalidHardRealizationRequirement(String),
    HardRealizationRequirementUnsatisfied(String),
    InvalidRealizationPolicy(String),
    InvalidResourceObservation(String),
    CurrentResourceObservationUnavailable(String),
    InvalidHostOperationRequirement(String),
    InvalidResourceContract(String),
    UnavailableResource(String),
    ResourceCapacityExceeded(String),
    InvalidProtectedResourceGrant(String),
    ProtectedResourceGrantMissing(String),
    ProtectedResourceGrantAmbiguous(String),
    InvalidAuthorityContract(String),
    AuthorityGrantMissing(String),
    AuthorityGrantAmbiguous(String),
    InvalidLinkBinding(String),
    LinkBindingMissing(String),
    LinkBindingUnavailable(String),
    LinkBindingAmbiguous(String),
    UnavailableConnectionProvider(String),
    QueueRequirementAboveHostLimit(String),
    CapabilityInstanceLimitExceeded(String),
    CyclicStartupDependencies(String),
    EvidenceBudgetOverflow(String),
    InvalidPlacementSyntax(String),
    InvalidSharedPool(String),
}

impl std::fmt::Display for PlannerError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidFormIdentity(value) => write!(f, "invalid form identity: {value}"),
            Self::PlannerCapabilityNotAdvertised(value) => {
                write!(f, "planner capability not advertised: {value}")
            }
            Self::PlannerCapabilityAmbiguous(value) => {
                write!(f, "planner capability ambiguous: {value}")
            }
            Self::PlannerLimitExceeded(value) => write!(f, "planner limit exceeded: {value}"),
            Self::UnknownOperation(value) => write!(f, "unknown operation '{value}'"),
            Self::MissingPlacement(value) => write!(f, "missing placement for '{value}'"),
            Self::DuplicatePlacement(value) => write!(f, "duplicate placement for '{value}'"),
            Self::UnknownHost(value) => write!(f, "unknown host '{value}'"),
            Self::UnknownCapability(value) => write!(f, "unknown capability '{value}'"),
            Self::WrongSemanticKind(value) => write!(f, "wrong semantic kind: {value}"),
            Self::WrongKindContractRevision(value) => {
                write!(f, "wrong kind contract revision: {value}")
            }
            Self::IncompatiblePortContract(value) => {
                write!(f, "incompatible port contract: {value}")
            }
            Self::IncompatibleCheckedFace(value) => {
                write!(f, "incompatible checked face: {value}")
            }
            Self::InvalidHardRealizationRequirement(value) => {
                write!(f, "invalid hard realization requirement: {value}")
            }
            Self::HardRealizationRequirementUnsatisfied(value) => {
                write!(f, "hard realization requirement unsatisfied: {value}")
            }
            Self::InvalidRealizationPolicy(value) => {
                write!(f, "invalid realization policy: {value}")
            }
            Self::InvalidResourceObservation(value) => {
                write!(f, "invalid resource observation: {value}")
            }
            Self::CurrentResourceObservationUnavailable(value) => {
                write!(f, "current resource observation unavailable: {value}")
            }
            Self::InvalidHostOperationRequirement(value) => {
                write!(f, "invalid host-operation requirement: {value}")
            }
            Self::InvalidResourceContract(value) => {
                write!(f, "invalid resource contract: {value}")
            }
            Self::UnavailableResource(value) => write!(f, "unavailable resource: {value}"),
            Self::ResourceCapacityExceeded(value) => {
                write!(f, "resource capacity exceeded: {value}")
            }
            Self::InvalidProtectedResourceGrant(value) => {
                write!(f, "invalid protected-resource grant: {value}")
            }
            Self::ProtectedResourceGrantMissing(value) => {
                write!(f, "protected-resource grant missing: {value}")
            }
            Self::ProtectedResourceGrantAmbiguous(value) => {
                write!(f, "protected-resource grant ambiguous: {value}")
            }
            Self::InvalidAuthorityContract(value) => {
                write!(f, "invalid authority contract: {value}")
            }
            Self::AuthorityGrantMissing(value) => write!(f, "authority grant missing: {value}"),
            Self::AuthorityGrantAmbiguous(value) => {
                write!(f, "authority grant ambiguous: {value}")
            }
            Self::InvalidLinkBinding(value) => write!(f, "invalid link binding: {value}"),
            Self::LinkBindingMissing(value) => write!(f, "link binding missing: {value}"),
            Self::LinkBindingUnavailable(value) => write!(f, "link binding unavailable: {value}"),
            Self::LinkBindingAmbiguous(value) => write!(f, "link binding ambiguous: {value}"),
            Self::UnavailableConnectionProvider(value) => {
                write!(f, "unavailable connection provider: {value}")
            }
            Self::QueueRequirementAboveHostLimit(value) => {
                write!(f, "queue requirement above host limit: {value}")
            }
            Self::CapabilityInstanceLimitExceeded(value) => {
                write!(f, "capability instance limit exceeded: {value}")
            }
            Self::CyclicStartupDependencies(value) => {
                write!(f, "cyclic startup dependencies: {value}")
            }
            Self::EvidenceBudgetOverflow(value) => {
                write!(f, "mandatory evidence budget overflow: {value}")
            }
            Self::InvalidPlacementSyntax(value) => write!(f, "invalid placement syntax: {value}"),
            Self::InvalidSharedPool(value) => write!(f, "invalid shared pool: {value}"),
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
