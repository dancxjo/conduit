//! Portable contracts shared by standard node libraries.
//!
//! This module deliberately describes behavior rather than providing a
//! registry. Hosted libraries may implement these contracts, but discovery is
//! not authority and no contract here can mint a grant or resource handle.

use crate::{Id, SupervisionContract};

/// A behavior family supplied by a standard node library.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StandardNodeKind {
    Identity,
    Literal,
    Sequence,
    Empty,
    Never,
    Collect,
    First,
    Last,
    Count,
    Discard,
    Tee,
    Merge,
    Zip,
    Mux,
    Demux,
    Select,
    Gate,
    Switch,
    Take,
    Skip,
    Fallback,
    Map,
    Filter,
    FilterMap,
    Validate,
    Adapter,
    Encode,
    Decode,
    Frame,
    Deframe,
    Transform,
    Fold,
    Window,
    Debounce,
    Throttle,
    Delay,
    Ticker,
    Deadline,
    Timeout,
    Sample,
    RateLimit,
    Batch,
    Retry,
    Supervisor,
    TerminalProjection,
    OperatorAction,
    FaultSource,
    Probe,
    Log,
    Meter,
    Trace,
    Assert,
    ControlGate,
    Record,
    Replay,
    SequenceSource,
    InjectedClock,
    InjectedEntropy,
    FileRead,
    FileWrite,
    DirectoryList,
    BlobStore,
    KeyValueStore,
    ProcessSpawn,
    GpioPin,
    SerialPort,
    I2cBus,
    SpiBus,
}

/// Finite resources that are part of a standard node's semantic contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StandardNodeLimits {
    pub retained_values: u32,
    pub retained_bytes: u64,
    pub pending_operations: u16,
    pub timers: u16,
    pub work_per_step: u32,
    pub evidence_events: u32,
}

/// Plan-visible contract for one standard behavior.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StandardNodeContract<'a> {
    pub id: Id<'a>,
    pub kind: StandardNodeKind,
    pub limits: StandardNodeLimits,
    /// Exact terminal policy descriptor; wake order is never a policy.
    pub terminal_policy: Id<'a>,
    /// Exact cancellation policy descriptor.
    pub cancellation_policy: Id<'a>,
}

/// Retry spacing measured in the plan's explicit time basis.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BackoffSchedule {
    Fixed {
        ticks: u64,
    },
    Exponential {
        initial_ticks: u64,
        maximum_ticks: u64,
    },
}

/// Bounded retry of one operation against one already selected provider.
///
/// `provider_binding`, `resource_binding`, and `grant` are immutable across
/// attempts. A supervisor that wants different bindings needs a new resolved
/// plan; retry is not provider fallback or authority amplification.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RetryContract<'a> {
    pub maximum_attempts: u16,
    pub deadline_ticks: u64,
    pub backoff: BackoffSchedule,
    pub provider_binding: Id<'a>,
    pub resource_binding: Id<'a>,
    pub grant: Id<'a>,
    pub cancellation_scope: Id<'a>,
    pub evidence_events: u32,
}

/// Distribution risk of a narrow hosted interface.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostServiceRisk {
    ReferenceSafe,
    Dangerous,
}

/// One narrow host-service operation shape.
///
/// A filesystem reader and process spawner, for example, are separate
/// contracts. The caller supplies exact bindings; the provider cannot search
/// ambient authority or substitute another provider.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HostServiceContract<'a> {
    pub interface: Id<'a>,
    pub interface_version: u32,
    pub operation: Id<'a>,
    pub provider_binding: Id<'a>,
    pub resource_binding: Id<'a>,
    pub grant: Id<'a>,
    pub cancellation_scope: Id<'a>,
    pub maximum_request_bytes: u64,
    pub maximum_response_bytes: u64,
    pub maximum_pending: u16,
    pub evidence_events: u32,
    pub risk: HostServiceRisk,
    /// Dangerous services must remain absent from reference/default registries.
    pub enabled_in_reference_registry: bool,
}

/// Availability reported by one narrow host-service provider.
///
/// Unsupported is an explicit negative capability, not an authority denial.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostServiceAvailability {
    Supported,
    Unsupported,
}

/// Finite, time-bounded limits reported by one selected provider.
///
/// This report authorizes nothing. In particular, matching `provider_binding`
/// does not prove that `grant` is usable for the requested resource.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HostServiceCapability<'a> {
    pub interface: Id<'a>,
    pub interface_version: u32,
    pub operation: Id<'a>,
    pub provider_binding: Id<'a>,
    pub observed_at_tick: u64,
    pub valid_until_tick: u64,
    pub maximum_request_bytes: u64,
    pub maximum_response_bytes: u64,
    pub maximum_pending: u16,
    pub evidence_events: u32,
    pub availability: HostServiceAvailability,
}

/// Result of validating the exact plan-supplied grant outside capability
/// discovery.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostServiceAuthorization<'a> {
    Authorized { grant: Id<'a> },
    Denied,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StandardContractError {
    InvalidIdentifier,
    Unbounded,
    IncompatibleLimits,
    UnsafeReferenceDefault,
    IncompatibleSupervisionContract,
    InvalidCapability,
    StaleCapability,
    UnsupportedHostService,
    InsufficientCapability,
    AuthorityDenied,
}

/// Validate that an ordinary standard supervisor node has enough exact
/// resources to consume the portable typed supervision contract.
pub fn validate_standard_supervisor(
    standard: StandardNodeContract<'_>,
    supervision: SupervisionContract<'_>,
) -> Result<(), StandardContractError> {
    validate_standard_node_contract(standard)?;
    supervision
        .validate()
        .map_err(|_| StandardContractError::IncompatibleSupervisionContract)?;
    let required_bytes = u64::from(supervision.limits.observation_bytes)
        .checked_add(u64::from(supervision.limits.decision_bytes))
        .and_then(|value| value.checked_add(u64::from(supervision.limits.scratch_bytes)))
        .ok_or(StandardContractError::IncompatibleLimits)?;
    if standard.kind != StandardNodeKind::Supervisor
        || standard.limits.retained_values < u32::from(supervision.limits.maximum_in_flight)
        || standard.limits.retained_bytes < required_bytes
        || standard.limits.pending_operations < supervision.limits.maximum_in_flight
        || standard.limits.timers < 3
        || standard.limits.evidence_events < u32::from(supervision.limits.maximum_evidence_events)
    {
        return Err(StandardContractError::IncompatibleSupervisionContract);
    }
    Ok(())
}

/// Validate finite storage, work, cancellation, terminal, and evidence facts.
pub fn validate_standard_node_contract(
    contract: StandardNodeContract<'_>,
) -> Result<(), StandardContractError> {
    validate_id(contract.id)?;
    validate_id(contract.terminal_policy)?;
    validate_id(contract.cancellation_policy)?;
    let limits = contract.limits;
    if limits.work_per_step == 0 || limits.evidence_events == 0 {
        return Err(StandardContractError::Unbounded);
    }
    let needs_timer = matches!(
        contract.kind,
        StandardNodeKind::Window
            | StandardNodeKind::Debounce
            | StandardNodeKind::Throttle
            | StandardNodeKind::Delay
            | StandardNodeKind::Retry
            | StandardNodeKind::Supervisor
            | StandardNodeKind::Ticker
            | StandardNodeKind::Deadline
            | StandardNodeKind::Timeout
            | StandardNodeKind::Sample
            | StandardNodeKind::RateLimit
            | StandardNodeKind::InjectedClock
            | StandardNodeKind::ControlGate
    );
    if needs_timer && limits.timers == 0 {
        return Err(StandardContractError::IncompatibleLimits);
    }
    let needs_state = matches!(
        contract.kind,
        StandardNodeKind::Fold
            | StandardNodeKind::Window
            | StandardNodeKind::Debounce
            | StandardNodeKind::Throttle
            | StandardNodeKind::Supervisor
            | StandardNodeKind::Sequence
            | StandardNodeKind::Collect
            | StandardNodeKind::Zip
            | StandardNodeKind::Select
            | StandardNodeKind::Batch
            | StandardNodeKind::RateLimit
            | StandardNodeKind::Record
            | StandardNodeKind::Replay
            | StandardNodeKind::Probe
            | StandardNodeKind::Log
            | StandardNodeKind::Meter
            | StandardNodeKind::Trace
            | StandardNodeKind::SequenceSource
            | StandardNodeKind::InjectedEntropy
            | StandardNodeKind::FileRead
            | StandardNodeKind::FileWrite
            | StandardNodeKind::BlobStore
            | StandardNodeKind::KeyValueStore
            | StandardNodeKind::ProcessSpawn
            | StandardNodeKind::SerialPort
    );
    if needs_state && (limits.retained_values == 0 || limits.retained_bytes == 0) {
        return Err(StandardContractError::IncompatibleLimits);
    }
    Ok(())
}

pub fn validate_retry_contract(contract: RetryContract<'_>) -> Result<(), StandardContractError> {
    validate_id(contract.provider_binding)?;
    validate_id(contract.resource_binding)?;
    validate_id(contract.grant)?;
    validate_id(contract.cancellation_scope)?;
    let valid_backoff = match contract.backoff {
        BackoffSchedule::Fixed { ticks } => ticks > 0,
        BackoffSchedule::Exponential {
            initial_ticks,
            maximum_ticks,
        } => initial_ticks > 0 && maximum_ticks >= initial_ticks,
    };
    if contract.maximum_attempts == 0
        || contract.deadline_ticks == 0
        || contract.evidence_events < u32::from(contract.maximum_attempts)
        || !valid_backoff
    {
        return Err(StandardContractError::Unbounded);
    }
    Ok(())
}

pub fn validate_host_service_contract(
    contract: HostServiceContract<'_>,
) -> Result<(), StandardContractError> {
    validate_id(contract.interface)?;
    validate_id(contract.operation)?;
    validate_id(contract.provider_binding)?;
    validate_id(contract.resource_binding)?;
    validate_id(contract.grant)?;
    validate_id(contract.cancellation_scope)?;
    if contract.interface_version == 0
        || contract.maximum_request_bytes == 0
        || contract.maximum_response_bytes == 0
        || contract.maximum_pending == 0
        || contract.evidence_events == 0
    {
        return Err(StandardContractError::Unbounded);
    }
    if contract.risk == HostServiceRisk::Dangerous && contract.enabled_in_reference_registry {
        return Err(StandardContractError::UnsafeReferenceDefault);
    }
    Ok(())
}

/// Resolve one host-service request against one fresh provider report and the
/// independently validated exact grant.
///
/// This decision performs no discovery, permission prompt, provisioning, or
/// host mutation. The provider, operation, and grant must already be exact
/// plan inputs.
pub fn resolve_host_service_contract(
    contract: HostServiceContract<'_>,
    capability: HostServiceCapability<'_>,
    current_tick: u64,
    authorization: HostServiceAuthorization<'_>,
) -> Result<(), StandardContractError> {
    validate_host_service_contract(contract)?;
    validate_id(capability.interface).map_err(|_| StandardContractError::InvalidCapability)?;
    validate_id(capability.operation).map_err(|_| StandardContractError::InvalidCapability)?;
    validate_id(capability.provider_binding)
        .map_err(|_| StandardContractError::InvalidCapability)?;
    if capability.interface_version == 0
        || capability.observed_at_tick >= capability.valid_until_tick
        || capability.maximum_request_bytes == 0
        || capability.maximum_response_bytes == 0
        || capability.maximum_pending == 0
        || capability.evidence_events == 0
    {
        return Err(StandardContractError::InvalidCapability);
    }
    if current_tick < capability.observed_at_tick || current_tick >= capability.valid_until_tick {
        return Err(StandardContractError::StaleCapability);
    }
    if capability.interface != contract.interface
        || capability.interface_version != contract.interface_version
        || capability.operation != contract.operation
        || capability.provider_binding != contract.provider_binding
        || capability.availability == HostServiceAvailability::Unsupported
    {
        return Err(StandardContractError::UnsupportedHostService);
    }
    if capability.maximum_request_bytes < contract.maximum_request_bytes
        || capability.maximum_response_bytes < contract.maximum_response_bytes
        || capability.maximum_pending < contract.maximum_pending
        || capability.evidence_events < contract.evidence_events
    {
        return Err(StandardContractError::InsufficientCapability);
    }
    match authorization {
        HostServiceAuthorization::Authorized { grant } if grant == contract.grant => Ok(()),
        HostServiceAuthorization::Authorized { .. } | HostServiceAuthorization::Denied => {
            Err(StandardContractError::AuthorityDenied)
        }
    }
}

fn validate_id(id: Id<'_>) -> Result<(), StandardContractError> {
    Id::new(id.as_str())
        .map(|_| ())
        .map_err(|_| StandardContractError::InvalidIdentifier)
}
