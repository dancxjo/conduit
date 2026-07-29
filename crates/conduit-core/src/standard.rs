//! Portable contracts shared by standard node libraries.
//!
//! This module deliberately describes behavior rather than providing a
//! registry. Hosted libraries may implement these contracts, but discovery is
//! not authority and no contract here can mint a grant or resource handle.

use crate::Id;

/// A behavior family supplied by a standard node library.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StandardNodeKind {
    Literal,
    Transform,
    Filter,
    Fold,
    Window,
    Debounce,
    Throttle,
    Delay,
    Retry,
    Probe,
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum StandardContractError {
    InvalidIdentifier,
    Unbounded,
    IncompatibleLimits,
    UnsafeReferenceDefault,
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
    );
    if needs_timer && limits.timers == 0 {
        return Err(StandardContractError::IncompatibleLimits);
    }
    let needs_state = matches!(
        contract.kind,
        StandardNodeKind::Fold | StandardNodeKind::Window | StandardNodeKind::Debounce
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
    if contract.maximum_request_bytes == 0
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

fn validate_id(id: Id<'_>) -> Result<(), StandardContractError> {
    Id::new(id.as_str())
        .map(|_| ())
        .map_err(|_| StandardContractError::InvalidIdentifier)
}
