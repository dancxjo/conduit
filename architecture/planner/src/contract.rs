use crate::prelude::*;
use alloc::collections::BTreeMap;
use conduit_core::{
    AuthorityGrant, BaseImplementationId, CapabilityId, GearId, HostId, LineId, LineOffer, PortId,
    ProtectedResourceGrant,
};

pub type ConnectionEndpoints = (GearId, PortId, GearId, PortId);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConnectionQueueLimits {
    pub item_capacity: u16,
    pub byte_capacity: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacementChoice {
    pub host_id: HostId,
    pub capability_id: CapabilityId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlacementChoices {
    pub by_gear: BTreeMap<GearId, PlacementChoice>,
}

#[derive(Debug, Clone, Copy)]
pub struct PlanningOptions<'a> {
    pub connection_bases: &'a BTreeMap<(GearId, GearId), BaseImplementationId>,
    /// Exact offered Line identities to seal, in deterministic preference order.
    pub line_candidates: &'a BTreeMap<(GearId, GearId), Vec<LineId>>,
    pub connection_item_capacity: u16,
    pub connection_byte_capacity: u32,
    pub authority_grants: &'a [AuthorityGrant],
    pub protected_resource_grants: &'a [ProtectedResourceGrant],
    pub line_offers: &'a [LineOffer],
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlannerError {
    InvalidFormIdentity(String),
    PlannerCapabilityNotAdvertised(String),
    PlannerCapabilityAmbiguous(String),
    PlannerLimitExceeded(String),
    UnknownGear(String),
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
    InvalidPlanningObservation(String),
    CurrentResourceObservationUnavailable(String),
    InvalidHostOperationRequirement(String),
    InvalidResourceContract(String),
    ResourceContentRefused(conduit_core::ResourceContentRefusal),
    UnavailableResource(String),
    ResourceCapacityExceeded(String),
    ResourceAllowanceUnsatisfied(String),
    InvalidProtectedResourceGrant(String),
    ProtectedResourceGrantMissing(String),
    ProtectedResourceGrantAmbiguous(String),
    InvalidAuthorityContract(String),
    AuthorityGrantMissing(String),
    AuthorityGrantAmbiguous(String),
    InvalidLineOffer(String),
    LineOfferMissing(String),
    LineOfferUnavailable(String),
    LineOfferAmbiguous(String),
    UnavailableBaseImplementationId(String),
    InvalidConnectionBudget(String),
    QueueRequirementAboveHostLimit(String),
    CapabilityInstanceLimitExceeded(String),
    CyclicStartupDependencies(String),
    SignBudgetOverflow(String),
    InvalidPlacementSyntax(String),
    InvalidSharedPool(String),
}

impl core::fmt::Display for PlannerError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::InvalidFormIdentity(value) => write!(f, "invalid form identity: {value}"),
            Self::PlannerCapabilityNotAdvertised(value) => {
                write!(f, "planner capability not advertised: {value}")
            }
            Self::PlannerCapabilityAmbiguous(value) => {
                write!(f, "planner capability ambiguous: {value}")
            }
            Self::PlannerLimitExceeded(value) => write!(f, "planner limit exceeded: {value}"),
            Self::UnknownGear(value) => write!(f, "unknown gear '{value}'"),
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
            Self::InvalidPlanningObservation(value) => {
                write!(f, "invalid planning observation: {value}")
            }
            Self::CurrentResourceObservationUnavailable(value) => {
                write!(f, "current resource observation unavailable: {value}")
            }
            Self::InvalidHostOperationRequirement(value) => {
                write!(f, "invalid host-operation requirement: {value}")
            }
            Self::ResourceContentRefused(refusal) => {
                write!(f, "resource content refused: {refusal:?}")
            }
            Self::InvalidResourceContract(value) => {
                write!(f, "invalid resource contract: {value}")
            }
            Self::UnavailableResource(value) => write!(f, "unavailable resource: {value}"),
            Self::ResourceCapacityExceeded(value) => {
                write!(f, "resource capacity exceeded: {value}")
            }
            Self::ResourceAllowanceUnsatisfied(value) => {
                write!(f, "Body resource envelope unsatisfied: {value}")
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
            Self::InvalidLineOffer(value) => write!(f, "invalid Line offer: {value}"),
            Self::LineOfferMissing(value) => write!(f, "Line offer missing: {value}"),
            Self::LineOfferUnavailable(value) => write!(f, "Line offer unavailable: {value}"),
            Self::LineOfferAmbiguous(value) => write!(f, "Line offer ambiguous: {value}"),
            Self::UnavailableBaseImplementationId(value) => {
                write!(f, "unavailable connection base: {value}")
            }
            Self::InvalidConnectionBudget(value) => {
                write!(f, "invalid connection budget: {value}")
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
            Self::SignBudgetOverflow(value) => {
                write!(f, "mandatory sign budget overflow: {value}")
            }
            Self::InvalidPlacementSyntax(value) => write!(f, "invalid placement syntax: {value}"),
            Self::InvalidSharedPool(value) => write!(f, "invalid shared pool: {value}"),
        }
    }
}

impl core::error::Error for PlannerError {}

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
    let mut by_gear = BTreeMap::new();
    while index < lines.len() {
        let header = lines[index];
        let operation_name = header
            .strip_suffix(':')
            .ok_or_else(|| PlannerError::InvalidPlacementSyntax(header.to_string()))?;
        if by_gear.contains_key(&GearId::from(operation_name)) {
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
        by_gear.insert(
            GearId::from(operation_name),
            PlacementChoice {
                host_id: HostId::from(host_id),
                capability_id: CapabilityId::from(capability_id),
            },
        );
        index += 3;
    }

    Ok(PlacementChoices { by_gear })
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
