//! Fresh, canonical host observations consumed by hosted resolvers.

use core::convert::Infallible;

use crate::canonical::semantic_hash_with_hash_set;
use crate::{
    CanonicalDescriptor, CanonicalError, CanonicalValue, ExecutorKind, FieldDisposition, Id,
    MapField, PassportStatusObservation, PinnedDescriptor, PlanResourceBudget, RealmReason,
    ResourceRef, SemanticHash, validate_passport_status,
};

pub const CAPABILITY_REPORT_SCHEMA_VERSION: u32 = 2;

/// Exact realm membership/status evidence attached to one host observation.
///
/// This proves only the report's current membership identity. It grants no
/// effects and says nothing about implementation, artifact, or transport
/// authority.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReportMembership<'a> {
    pub realm: Id<'a>,
    pub entity: Id<'a>,
    pub passport: SemanticHash,
    pub status: PassportStatusObservation<'a>,
}

/// One currently available semantic host/backend capability.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReportCapability<'a> {
    pub interface: PinnedDescriptor<'a>,
    pub mode: Id<'a>,
    pub subject: Id<'a>,
    /// Hash of capability-specific facets such as protocol/security modes.
    pub details: SemanticHash,
    pub capacity: PlanResourceBudget,
}

/// One currently available concrete resource pool.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReportResource<'a> {
    pub resource: ResourceRef<'a>,
    /// Domain-owned descriptor for limits and current constraints.
    pub descriptor: PinnedDescriptor<'a>,
    pub capacity: PlanResourceBudget,
    pub exclusive: bool,
}

/// One observed topology edge or endpoint relationship.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReportTopology<'a> {
    pub id: Id<'a>,
    pub contract: PinnedDescriptor<'a>,
    pub from: Id<'a>,
    pub to: Id<'a>,
    pub maximum_transfer_unit: u32,
    pub maximum_sessions: u32,
    pub reachable: bool,
    /// Hash of topology-specific facts such as address family or trust edge.
    pub details: SemanticHash,
}

/// A fresh observation. It describes current facts and authorizes/provisions
/// nothing.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct CapabilityReport<'a> {
    pub schema_version: u32,
    pub identity: SemanticHash,
    pub id: Id<'a>,
    pub host: Id<'a>,
    pub reporter: PinnedDescriptor<'a>,
    pub trust: PinnedDescriptor<'a>,
    pub membership: Option<ReportMembership<'a>>,
    pub time_basis: Id<'a>,
    pub observed_at_tick: u64,
    pub valid_until_tick: u64,
    pub available: PlanResourceBudget,
    pub capabilities: &'a [ReportCapability<'a>],
    pub resources: &'a [ReportResource<'a>],
    pub topology: &'a [ReportTopology<'a>],
    pub supported_executors: &'a [ExecutorKind],
    pub supported_targets: &'a [Id<'a>],
    pub supported_abis: &'a [Id<'a>],
    pub minimum_plan_version: u32,
    pub maximum_plan_version: u32,
    pub current_constraints: &'a [SemanticHash],
}

impl CapabilityReport<'_> {
    #[must_use]
    pub const fn identity_fact_count(&self) -> usize {
        self.capabilities.len()
            + self.resources.len()
            + self.topology.len()
            + self.supported_executors.len()
            + self.supported_targets.len()
            + self.supported_abis.len()
            + self.current_constraints.len()
    }

    pub fn computed_semantic_hash(
        &self,
        scratch: &mut [SemanticHash],
    ) -> Result<SemanticHash, HostReportIdentityError> {
        let needed = self.identity_fact_count();
        if scratch.len() < needed {
            return Err(HostReportIdentityError::ScratchTooSmall);
        }
        let mut cursor = 0;
        for capability in self.capabilities {
            scratch[cursor] = hash_capability(capability)?;
            cursor += 1;
        }
        for resource in self.resources {
            scratch[cursor] = hash_resource(resource)?;
            cursor += 1;
        }
        for topology in self.topology {
            scratch[cursor] = hash_topology(topology)?;
            cursor += 1;
        }
        for executor in self.supported_executors {
            scratch[cursor] = hash_tag("executor", Id(executor.as_str()))?;
            cursor += 1;
        }
        for target in self.supported_targets {
            scratch[cursor] = hash_tag("target", *target)?;
            cursor += 1;
        }
        for abi in self.supported_abis {
            scratch[cursor] = hash_tag("abi", *abi)?;
            cursor += 1;
        }
        for constraint in self.current_constraints {
            scratch[cursor] = hash_constraint(*constraint)?;
            cursor += 1;
        }
        let reporter = self.reporter;
        let trust = self.trust;
        let membership = self.membership.map(hash_membership).transpose()?;
        let membership_value = membership
            .as_ref()
            .map_or(CanonicalValue::Null, |identity| {
                CanonicalValue::Bytes(identity.as_bytes())
            });
        let fields = [
            semantic("id", CanonicalValue::Identifier(self.id)),
            semantic("host", CanonicalValue::Identifier(self.host)),
            semantic("reporter_id", CanonicalValue::Identifier(reporter.id)),
            semantic(
                "reporter_version",
                CanonicalValue::Integer(i128::from(reporter.schema_version)),
            ),
            semantic(
                "reporter_hash",
                CanonicalValue::Bytes(reporter.semantic_hash.as_bytes()),
            ),
            semantic("trust_id", CanonicalValue::Identifier(trust.id)),
            semantic(
                "trust_version",
                CanonicalValue::Integer(i128::from(trust.schema_version)),
            ),
            semantic(
                "trust_hash",
                CanonicalValue::Bytes(trust.semantic_hash.as_bytes()),
            ),
            semantic("membership", membership_value),
            semantic("time_basis", CanonicalValue::Identifier(self.time_basis)),
            semantic(
                "observed_at_tick",
                CanonicalValue::Integer(i128::from(self.observed_at_tick)),
            ),
            semantic(
                "valid_until_tick",
                CanonicalValue::Integer(i128::from(self.valid_until_tick)),
            ),
            semantic(
                "available_memory_bytes",
                CanonicalValue::Integer(i128::from(self.available.memory_bytes)),
            ),
            semantic(
                "available_storage_bytes",
                CanonicalValue::Integer(i128::from(self.available.storage_bytes)),
            ),
            semantic(
                "available_cpu_units",
                CanonicalValue::Integer(i128::from(self.available.cpu_units)),
            ),
            semantic(
                "available_timers",
                CanonicalValue::Integer(i128::from(self.available.timers)),
            ),
            semantic(
                "available_transports",
                CanonicalValue::Integer(i128::from(self.available.transports)),
            ),
            semantic(
                "available_checkpoints",
                CanonicalValue::Integer(i128::from(self.available.checkpoints)),
            ),
            semantic(
                "available_evidence_bytes",
                CanonicalValue::Integer(i128::from(self.available.evidence_bytes)),
            ),
            semantic(
                "minimum_plan_version",
                CanonicalValue::Integer(i128::from(self.minimum_plan_version)),
            ),
            semantic(
                "maximum_plan_version",
                CanonicalValue::Integer(i128::from(self.maximum_plan_version)),
            ),
        ];
        semantic_hash_with_hash_set(
            Id("conduit/capability-report"),
            self.schema_version,
            &fields,
            Id("facts"),
            &scratch[..needed],
        )
        .map_err(HostReportIdentityError::Canonical)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostReportIdentityError {
    ScratchTooSmall,
    Canonical(CanonicalError<Infallible>),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostReportReason {
    UnsupportedSchema,
    InvalidDescriptor,
    IdentityMismatch,
    TimeBasisMismatch,
    NotYetObserved,
    Stale,
    UnsupportedPlanVersion,
    MembershipInvalid,
}

impl HostReportReason {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::UnsupportedSchema => "CND-HST-001",
            Self::Stale => "CND-HST-002",
            Self::InvalidDescriptor => "CND-HST-003",
            Self::IdentityMismatch => "CND-HST-004",
            Self::TimeBasisMismatch => "CND-HST-005",
            Self::NotYetObserved => "CND-HST-006",
            Self::UnsupportedPlanVersion => "CND-HST-007",
            Self::MembershipInvalid => "CND-HST-008",
        }
    }
}

/// Validates report structure, identity, freshness, and plan-version support
/// without querying or mutating the host.
pub fn validate_capability_report(
    report: &CapabilityReport<'_>,
    time_basis: Id<'_>,
    current_tick: u64,
    plan_version: u32,
    scratch: &mut [SemanticHash],
) -> Result<(), HostReportReason> {
    if report.schema_version != CAPABILITY_REPORT_SCHEMA_VERSION {
        return Err(HostReportReason::UnsupportedSchema);
    }
    if !valid_id(report.id)
        || !valid_id(report.host)
        || !valid_pin(report.reporter)
        || !valid_pin(report.trust)
        || !valid_id(report.time_basis)
        || report.observed_at_tick > report.valid_until_tick
        || report.minimum_plan_version == 0
        || report.minimum_plan_version > report.maximum_plan_version
        || report.capabilities.iter().any(|capability| {
            !valid_capability(capability) || !budget_fits(capability.capacity, report.available)
        })
        || report.resources.iter().any(|resource| {
            !valid_resource(resource) || !budget_fits(resource.capacity, report.available)
        })
        || report
            .topology
            .iter()
            .any(|topology| !valid_topology(topology))
        || report
            .supported_targets
            .iter()
            .any(|target| !valid_id(*target))
        || report.supported_abis.iter().any(|abi| !valid_id(*abi))
    {
        return Err(HostReportReason::InvalidDescriptor);
    }
    if report.time_basis != time_basis {
        return Err(HostReportReason::TimeBasisMismatch);
    }
    if let Some(membership) = report.membership {
        validate_passport_status(
            membership.status,
            membership.passport,
            membership.realm,
            membership.entity,
            time_basis,
            current_tick,
        )
        .map_err(|_error: RealmReason| HostReportReason::MembershipInvalid)?;
    }
    if current_tick < report.observed_at_tick {
        return Err(HostReportReason::NotYetObserved);
    }
    if current_tick > report.valid_until_tick {
        return Err(HostReportReason::Stale);
    }
    if plan_version < report.minimum_plan_version || plan_version > report.maximum_plan_version {
        return Err(HostReportReason::UnsupportedPlanVersion);
    }
    let identity = report
        .computed_semantic_hash(scratch)
        .map_err(|_| HostReportReason::InvalidDescriptor)?;
    if identity != report.identity {
        return Err(HostReportReason::IdentityMismatch);
    }
    Ok(())
}

fn hash_membership(value: ReportMembership<'_>) -> Result<SemanticHash, HostReportIdentityError> {
    let reporter = value.status.reporter;
    let fields = [
        semantic("realm", CanonicalValue::Identifier(value.realm)),
        semantic("entity", CanonicalValue::Identifier(value.entity)),
        semantic("passport", CanonicalValue::Bytes(value.passport.as_bytes())),
        semantic(
            "status_passport",
            CanonicalValue::Bytes(value.status.passport.as_bytes()),
        ),
        semantic(
            "status_realm",
            CanonicalValue::Identifier(value.status.realm),
        ),
        semantic(
            "status_entity",
            CanonicalValue::Identifier(value.status.entity),
        ),
        semantic(
            "status_reporter_id",
            CanonicalValue::Identifier(reporter.id),
        ),
        semantic(
            "status_reporter_version",
            CanonicalValue::Integer(i128::from(reporter.schema_version)),
        ),
        semantic(
            "status_reporter_hash",
            CanonicalValue::Bytes(reporter.semantic_hash.as_bytes()),
        ),
        semantic(
            "status_time_basis",
            CanonicalValue::Identifier(value.status.time_basis),
        ),
        semantic(
            "status_observed_at_tick",
            CanonicalValue::Integer(i128::from(value.status.observed_at_tick)),
        ),
        semantic(
            "status_valid_until_tick",
            CanonicalValue::Integer(i128::from(value.status.valid_until_tick)),
        ),
        semantic(
            "status",
            CanonicalValue::Identifier(Id(value.status.status.as_str())),
        ),
    ];
    CanonicalDescriptor {
        kind: Id("conduit/host-report-membership"),
        schema_version: 1,
        body: CanonicalValue::Map(&fields),
    }
    .semantic_hash()
    .map_err(HostReportIdentityError::Canonical)
}

fn hash_capability(value: &ReportCapability<'_>) -> Result<SemanticHash, HostReportIdentityError> {
    let fields = [
        semantic(
            "interface_id",
            CanonicalValue::Identifier(value.interface.id),
        ),
        semantic(
            "interface_version",
            CanonicalValue::Integer(i128::from(value.interface.schema_version)),
        ),
        semantic(
            "interface_hash",
            CanonicalValue::Bytes(value.interface.semantic_hash.as_bytes()),
        ),
        semantic("mode", CanonicalValue::Identifier(value.mode)),
        semantic("subject", CanonicalValue::Identifier(value.subject)),
        semantic("details", CanonicalValue::Bytes(value.details.as_bytes())),
        semantic(
            "memory_bytes",
            CanonicalValue::Integer(i128::from(value.capacity.memory_bytes)),
        ),
        semantic(
            "storage_bytes",
            CanonicalValue::Integer(i128::from(value.capacity.storage_bytes)),
        ),
        semantic(
            "cpu_units",
            CanonicalValue::Integer(i128::from(value.capacity.cpu_units)),
        ),
        semantic(
            "timers",
            CanonicalValue::Integer(i128::from(value.capacity.timers)),
        ),
        semantic(
            "transports",
            CanonicalValue::Integer(i128::from(value.capacity.transports)),
        ),
        semantic(
            "checkpoints",
            CanonicalValue::Integer(i128::from(value.capacity.checkpoints)),
        ),
        semantic(
            "evidence_bytes",
            CanonicalValue::Integer(i128::from(value.capacity.evidence_bytes)),
        ),
    ];
    hash("conduit/report-capability", &fields)
}

fn hash_resource(value: &ReportResource<'_>) -> Result<SemanticHash, HostReportIdentityError> {
    let fields = [
        semantic(
            "resource_kind",
            CanonicalValue::Identifier(value.resource.kind),
        ),
        semantic("resource_id", CanonicalValue::Identifier(value.resource.id)),
        semantic(
            "descriptor_id",
            CanonicalValue::Identifier(value.descriptor.id),
        ),
        semantic(
            "descriptor_version",
            CanonicalValue::Integer(i128::from(value.descriptor.schema_version)),
        ),
        semantic(
            "descriptor_hash",
            CanonicalValue::Bytes(value.descriptor.semantic_hash.as_bytes()),
        ),
        semantic(
            "memory_bytes",
            CanonicalValue::Integer(i128::from(value.capacity.memory_bytes)),
        ),
        semantic(
            "storage_bytes",
            CanonicalValue::Integer(i128::from(value.capacity.storage_bytes)),
        ),
        semantic(
            "cpu_units",
            CanonicalValue::Integer(i128::from(value.capacity.cpu_units)),
        ),
        semantic(
            "timers",
            CanonicalValue::Integer(i128::from(value.capacity.timers)),
        ),
        semantic(
            "transports",
            CanonicalValue::Integer(i128::from(value.capacity.transports)),
        ),
        semantic(
            "checkpoints",
            CanonicalValue::Integer(i128::from(value.capacity.checkpoints)),
        ),
        semantic(
            "evidence_bytes",
            CanonicalValue::Integer(i128::from(value.capacity.evidence_bytes)),
        ),
        semantic("exclusive", CanonicalValue::Boolean(value.exclusive)),
    ];
    hash("conduit/report-resource", &fields)
}

fn hash_topology(value: &ReportTopology<'_>) -> Result<SemanticHash, HostReportIdentityError> {
    let fields = [
        semantic("id", CanonicalValue::Identifier(value.id)),
        semantic("contract_id", CanonicalValue::Identifier(value.contract.id)),
        semantic(
            "contract_version",
            CanonicalValue::Integer(i128::from(value.contract.schema_version)),
        ),
        semantic(
            "contract_hash",
            CanonicalValue::Bytes(value.contract.semantic_hash.as_bytes()),
        ),
        semantic("from", CanonicalValue::Identifier(value.from)),
        semantic("to", CanonicalValue::Identifier(value.to)),
        semantic(
            "maximum_transfer_unit",
            CanonicalValue::Integer(i128::from(value.maximum_transfer_unit)),
        ),
        semantic(
            "maximum_sessions",
            CanonicalValue::Integer(i128::from(value.maximum_sessions)),
        ),
        semantic("reachable", CanonicalValue::Boolean(value.reachable)),
        semantic("details", CanonicalValue::Bytes(value.details.as_bytes())),
    ];
    hash("conduit/report-topology", &fields)
}

fn hash_tag(tag: &str, value: Id<'_>) -> Result<SemanticHash, HostReportIdentityError> {
    let fields = [
        semantic("tag", CanonicalValue::Identifier(Id(tag))),
        semantic("value", CanonicalValue::Identifier(value)),
    ];
    hash("conduit/report-support", &fields)
}

fn hash_constraint(value: SemanticHash) -> Result<SemanticHash, HostReportIdentityError> {
    let fields = [semantic(
        "constraint",
        CanonicalValue::Bytes(value.as_bytes()),
    )];
    hash("conduit/report-constraint", &fields)
}

fn hash(kind: &str, fields: &[MapField<'_>]) -> Result<SemanticHash, HostReportIdentityError> {
    CanonicalDescriptor {
        kind: Id(kind),
        schema_version: 1,
        body: CanonicalValue::Map(fields),
    }
    .semantic_hash()
    .map_err(HostReportIdentityError::Canonical)
}

fn semantic<'a>(name: &'a str, value: CanonicalValue<'a>) -> MapField<'a> {
    MapField {
        name: Id(name),
        value,
        disposition: FieldDisposition::Semantic,
    }
}

fn valid_id(value: Id<'_>) -> bool {
    Id::new(value.as_str()).is_ok()
}

fn valid_pin(value: PinnedDescriptor<'_>) -> bool {
    valid_id(value.id) && value.schema_version > 0
}

fn valid_capability(value: &ReportCapability<'_>) -> bool {
    valid_pin(value.interface) && valid_id(value.mode) && valid_id(value.subject)
}

fn valid_resource(value: &ReportResource<'_>) -> bool {
    valid_id(value.resource.kind) && valid_id(value.resource.id) && valid_pin(value.descriptor)
}

fn valid_topology(value: &ReportTopology<'_>) -> bool {
    valid_id(value.id)
        && valid_pin(value.contract)
        && valid_id(value.from)
        && valid_id(value.to)
        && value.maximum_transfer_unit > 0
}

fn budget_fits(value: PlanResourceBudget, ceiling: PlanResourceBudget) -> bool {
    value.memory_bytes <= ceiling.memory_bytes
        && value.storage_bytes <= ceiling.storage_bytes
        && value.cpu_units <= ceiling.cpu_units
        && value.timers <= ceiling.timers
        && value.transports <= ceiling.transports
        && value.checkpoints <= ceiling.checkpoints
        && value.evidence_bytes <= ceiling.evidence_bytes
}
