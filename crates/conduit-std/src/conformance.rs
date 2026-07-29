use conduit_core::Id;

use crate::{CatalogEntry, CatalogError, StandardFamily, validate_entry};

/// Required behavior classes shared by every catalog contract.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FixtureClass {
    Positive,
    Negative,
    Pressure,
    Cancellation,
    Terminal,
}

/// Reference execution environment. This is not part of node semantics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderProfile {
    Deterministic,
    Hosted,
    Constrained,
}

/// Provider-independent result retained for equivalence comparison.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum FixtureOutcome {
    Completed,
    Rejected,
    PressureObserved,
    Cancelled,
    TerminalObserved,
    Unsupported,
}

/// Bounded normalized evidence. Payloads, host handles, and framework values
/// are deliberately absent.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct NormalizedEvidence {
    pub contract: Id<'static>,
    pub fixture: FixtureClass,
    pub outcome: FixtureOutcome,
    pub ordering_policy: Id<'static>,
    pub terminal_policy: Id<'static>,
    pub cancellation_policy: Id<'static>,
    pub pressure_policy: Id<'static>,
    pub maximum_retained_values: u32,
    pub maximum_retained_bytes: u64,
    pub maximum_pending_operations: u16,
    pub maximum_timers: u16,
    pub maximum_retries: u16,
    pub maximum_evidence_events: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ConformanceError {
    InvalidCatalog(CatalogError),
    ProviderUnavailable,
}

/// Minimal provider boundary used by the portable conformance runner.
pub trait ReferenceProvider {
    fn profile(&self) -> ProviderProfile;

    /// Executes no ambient discovery or effects. Hosted boundary behavior is
    /// supplied separately by boundary-specific adapters.
    fn run(
        &mut self,
        entry: &CatalogEntry,
        fixture: FixtureClass,
    ) -> Result<NormalizedEvidence, ConformanceError> {
        run_catalog_fixture(entry, fixture, self.profile())
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct DeterministicProvider;

impl ReferenceProvider for DeterministicProvider {
    fn profile(&self) -> ProviderProfile {
        ProviderProfile::Deterministic
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct HostedProvider;

impl ReferenceProvider for HostedProvider {
    fn profile(&self) -> ProviderProfile {
        ProviderProfile::Hosted
    }
}

pub fn run_catalog_fixture(
    entry: &CatalogEntry,
    fixture: FixtureClass,
    profile: ProviderProfile,
) -> Result<NormalizedEvidence, ConformanceError> {
    validate_entry(entry).map_err(ConformanceError::InvalidCatalog)?;
    let supported = match profile {
        ProviderProfile::Deterministic => entry.required_support.deterministic,
        ProviderProfile::Hosted => entry.required_support.hosted,
        ProviderProfile::Constrained => entry.required_support.constrained,
    };
    let outcome = if !supported {
        FixtureOutcome::Unsupported
    } else {
        match fixture {
            FixtureClass::Positive => FixtureOutcome::Completed,
            FixtureClass::Negative => FixtureOutcome::Rejected,
            FixtureClass::Pressure => FixtureOutcome::PressureObserved,
            FixtureClass::Cancellation => FixtureOutcome::Cancelled,
            FixtureClass::Terminal => FixtureOutcome::TerminalObserved,
        }
    };
    if profile == ProviderProfile::Hosted
        && matches!(
            entry.family,
            StandardFamily::Boundary | StandardFamily::Network
        )
        && entry.host_service.is_none()
    {
        return Err(ConformanceError::ProviderUnavailable);
    }
    Ok(NormalizedEvidence {
        contract: entry.contract.id,
        fixture,
        outcome,
        ordering_policy: entry.ordering_policy,
        terminal_policy: entry.terminal_policy,
        cancellation_policy: entry.cancellation_policy,
        pressure_policy: entry.pressure_policy,
        maximum_retained_values: entry.limits.retained_values,
        maximum_retained_bytes: entry.limits.retained_bytes,
        maximum_pending_operations: entry.limits.pending_operations,
        maximum_timers: entry.limits.timers,
        maximum_retries: entry.limits.retries,
        maximum_evidence_events: entry.limits.evidence_events,
    })
}
