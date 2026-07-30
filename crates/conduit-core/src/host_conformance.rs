//! Bounded cross-host provider and extension conformance.
//!
//! Static inventory, current observation, behavioral conformance, and exact
//! binding are deliberately different values. Descriptor discovery consumes
//! these values but performs no installation, initialization, authority, or
//! host mutation.

use core::convert::Infallible;

use crate::canonical::semantic_hash_with_hash_set;
use crate::{
    CanonicalDescriptor, CanonicalError, CanonicalValue, CompatibilityOutcome, FieldDisposition,
    Id, MapField, PinnedDescriptor, SatisfactionProof, SemanticHash, validate_satisfaction_proof,
};

pub const HOST_CONFORMANCE_PROFILE_SCHEMA_VERSION: u32 = 1;
pub const PROVIDER_CONFORMANCE_RESULT_SCHEMA_VERSION: u32 = 1;
pub const MAXIMUM_HOST_MANDATORY_FACTS: usize = 16;
pub const MAXIMUM_HOST_OPTIONAL_PROVIDERS: usize = 32;
pub const MAXIMUM_HOST_EXTENSIONS: usize = 32;
pub const MAXIMUM_PROVIDER_FACETS: usize = 32;

/// Materially different host classes. This classification selects no provider
/// and carries no implementation behavior.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostClass {
    LinuxHosted,
    BrowserWasm,
    ConstrainedFirmware,
    DeterministicTest,
    DescribeOnly,
}

impl HostClass {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::LinuxHosted => "linux-hosted",
            Self::BrowserWasm => "browser-wasm",
            Self::ConstrainedFirmware => "constrained-firmware",
            Self::DeterministicTest => "deterministic-test",
            Self::DescribeOnly => "describe-only",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostExecutionMode {
    Executable,
    DescribeOnly,
}

impl HostExecutionMode {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Executable => "executable",
            Self::DescribeOnly => "describe-only",
        }
    }
}

/// Static relationship between a profile and a known optional provider.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderInventoryState {
    /// The contract is known but cannot be provided by this host profile.
    Unsupported,
    /// Code is linked/installed, but availability still needs an observation.
    Linked,
}

impl ProviderInventoryState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Unsupported => "known-unsupported",
            Self::Linked => "linked",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProviderInventory<'a> {
    pub contract: PinnedDescriptor<'a>,
    pub provider_bundle: PinnedDescriptor<'a>,
    pub state: ProviderInventoryState,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostExtensionKind {
    Type,
    Node,
    Implementation,
    Adapter,
}

impl HostExtensionKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Type => "type",
            Self::Node => "node",
            Self::Implementation => "implementation",
            Self::Adapter => "adapter",
        }
    }
}

/// A published extension descriptor. Publication means only that the
/// descriptor is understood; it does not make an implementation selectable.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HostExtension<'a> {
    pub kind: HostExtensionKind,
    pub descriptor: PinnedDescriptor<'a>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HostConformanceProfile<'a> {
    pub schema_version: u32,
    pub identity: SemanticHash,
    pub id: Id<'a>,
    pub class: HostClass,
    pub execution_mode: HostExecutionMode,
    /// Profile identity pins mandatory host facts separately from providers.
    pub mandatory_facts: &'a [PinnedDescriptor<'a>],
    /// An empty optional-provider set is valid.
    pub optional_providers: &'a [ProviderInventory<'a>],
    /// Domain-open types, nodes, implementations, and adapters.
    pub extensions: &'a [HostExtension<'a>],
}

impl HostConformanceProfile<'_> {
    #[must_use]
    pub const fn identity_fact_count(&self) -> usize {
        self.mandatory_facts.len() + self.optional_providers.len() + self.extensions.len()
    }

    pub fn computed_semantic_hash(
        &self,
        scratch: &mut [SemanticHash],
    ) -> Result<SemanticHash, HostConformanceIdentityError> {
        let needed = self.identity_fact_count();
        if scratch.len() < needed {
            return Err(HostConformanceIdentityError::ScratchTooSmall);
        }
        let mut cursor = 0;
        for fact in self.mandatory_facts {
            scratch[cursor] = hash_pin("conduit/host-mandatory-fact", *fact)?;
            cursor += 1;
        }
        for provider in self.optional_providers {
            scratch[cursor] = hash_provider_inventory(*provider)?;
            cursor += 1;
        }
        for extension in self.extensions {
            scratch[cursor] = hash_extension(*extension)?;
            cursor += 1;
        }
        let fields = [
            semantic("id", CanonicalValue::Identifier(self.id)),
            semantic("class", CanonicalValue::Identifier(Id(self.class.as_str()))),
            semantic(
                "execution_mode",
                CanonicalValue::Identifier(Id(self.execution_mode.as_str())),
            ),
        ];
        semantic_hash_with_hash_set(
            Id("conduit/host-conformance-profile"),
            self.schema_version,
            &fields,
            Id("facts"),
            &scratch[..needed],
        )
        .map_err(HostConformanceIdentityError::Canonical)
    }
}

/// Runtime/provider boundary used for behavioral conformance. Language is an
/// observation of the adapter boundary, never semantic compatibility.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderBoundary {
    Native,
    SupervisedProcess,
    WasmBrowser,
    FirmwareFfi,
}

impl ProviderBoundary {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Native => "native",
            Self::SupervisedProcess => "supervised-process",
            Self::WasmBrowser => "wasm-browser",
            Self::FirmwareFfi => "firmware-ffi",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderObservationState {
    Uninitialized,
    Available,
    Lost,
}

impl ProviderObservationState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Uninitialized => "linked-uninitialized",
            Self::Available => "available",
            Self::Lost => "lost",
        }
    }
}

/// Current state of one linked provider. It is not part of profile identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProviderObservation<'a> {
    pub id: Id<'a>,
    pub identity: SemanticHash,
    pub profile: PinnedDescriptor<'a>,
    pub provider_bundle: PinnedDescriptor<'a>,
    pub host_report: PinnedDescriptor<'a>,
    pub state: ProviderObservationState,
    pub time_basis: Id<'a>,
    pub observed_at_tick: u64,
    pub valid_until_tick: u64,
}

impl ProviderObservation<'_> {
    pub fn computed_semantic_hash(&self) -> Result<SemanticHash, HostConformanceIdentityError> {
        let fields = [
            semantic("id", CanonicalValue::Identifier(self.id)),
            semantic("profile_id", CanonicalValue::Identifier(self.profile.id)),
            semantic(
                "profile_version",
                CanonicalValue::Integer(i128::from(self.profile.schema_version)),
            ),
            semantic(
                "profile_hash",
                CanonicalValue::Bytes(self.profile.semantic_hash.as_bytes()),
            ),
            semantic(
                "provider_bundle_id",
                CanonicalValue::Identifier(self.provider_bundle.id),
            ),
            semantic(
                "provider_bundle_version",
                CanonicalValue::Integer(i128::from(self.provider_bundle.schema_version)),
            ),
            semantic(
                "provider_bundle_hash",
                CanonicalValue::Bytes(self.provider_bundle.semantic_hash.as_bytes()),
            ),
            semantic(
                "host_report_id",
                CanonicalValue::Identifier(self.host_report.id),
            ),
            semantic(
                "host_report_version",
                CanonicalValue::Integer(i128::from(self.host_report.schema_version)),
            ),
            semantic(
                "host_report_hash",
                CanonicalValue::Bytes(self.host_report.semantic_hash.as_bytes()),
            ),
            semantic("state", CanonicalValue::Identifier(Id(self.state.as_str()))),
            semantic("time_basis", CanonicalValue::Identifier(self.time_basis)),
            semantic(
                "observed_at_tick",
                CanonicalValue::Integer(i128::from(self.observed_at_tick)),
            ),
            semantic(
                "valid_until_tick",
                CanonicalValue::Integer(i128::from(self.valid_until_tick)),
            ),
        ];
        CanonicalDescriptor {
            kind: Id("conduit/provider-observation"),
            schema_version: 1,
            body: CanonicalValue::Map(&fields),
        }
        .semantic_hash()
        .map_err(HostConformanceIdentityError::Canonical)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProviderBounds {
    pub maximum_in_flight: u16,
    pub maximum_foreign_queue: u16,
    pub maximum_memory_bytes: u64,
    pub maximum_cancellation_ticks: u64,
    pub maximum_evidence_events: u32,
}

impl ProviderBounds {
    const fn valid(self) -> bool {
        self.maximum_in_flight > 0
            && self.maximum_memory_bytes > 0
            && self.maximum_cancellation_ticks > 0
            && self.maximum_evidence_events > 0
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderConformanceOutcome {
    Passed,
    Failed,
    Unsupported,
}

impl ProviderConformanceOutcome {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Passed => "passed",
            Self::Failed => "failed",
            Self::Unsupported => "unsupported",
        }
    }
}

/// Reproducible behavioral proof for one exact implementation, artifact,
/// adapter boundary, profile, and fixture suite.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProviderConformanceResult<'a> {
    pub schema_version: u32,
    pub identity: SemanticHash,
    pub required_contract: PinnedDescriptor<'a>,
    pub implementation: PinnedDescriptor<'a>,
    pub artifact: PinnedDescriptor<'a>,
    pub adapter: PinnedDescriptor<'a>,
    pub profile: PinnedDescriptor<'a>,
    pub fixture_suite: PinnedDescriptor<'a>,
    pub offered_facets: &'a [PinnedDescriptor<'a>],
    pub satisfaction_proof: SemanticHash,
    pub boundary: ProviderBoundary,
    pub outcome: ProviderConformanceOutcome,
    pub bounds: ProviderBounds,
    pub time_basis: Id<'a>,
    pub observed_at_tick: u64,
    pub valid_until_tick: u64,
}

impl ProviderConformanceResult<'_> {
    #[must_use]
    pub const fn identity_fact_count(&self) -> usize {
        self.offered_facets.len()
    }

    pub fn computed_semantic_hash(
        &self,
        scratch: &mut [SemanticHash],
    ) -> Result<SemanticHash, HostConformanceIdentityError> {
        if scratch.len() < self.offered_facets.len() {
            return Err(HostConformanceIdentityError::ScratchTooSmall);
        }
        for (slot, facet) in scratch.iter_mut().zip(self.offered_facets) {
            *slot = hash_pin("conduit/provider-offered-facet", *facet)?;
        }
        let fields = [
            semantic(
                "required_contract_id",
                CanonicalValue::Identifier(self.required_contract.id),
            ),
            semantic(
                "required_contract_version",
                CanonicalValue::Integer(i128::from(self.required_contract.schema_version)),
            ),
            semantic(
                "required_contract_hash",
                CanonicalValue::Bytes(self.required_contract.semantic_hash.as_bytes()),
            ),
            semantic(
                "implementation_id",
                CanonicalValue::Identifier(self.implementation.id),
            ),
            semantic(
                "implementation_version",
                CanonicalValue::Integer(i128::from(self.implementation.schema_version)),
            ),
            semantic(
                "implementation_hash",
                CanonicalValue::Bytes(self.implementation.semantic_hash.as_bytes()),
            ),
            semantic("artifact_id", CanonicalValue::Identifier(self.artifact.id)),
            semantic(
                "artifact_version",
                CanonicalValue::Integer(i128::from(self.artifact.schema_version)),
            ),
            semantic(
                "artifact_hash",
                CanonicalValue::Bytes(self.artifact.semantic_hash.as_bytes()),
            ),
            semantic("adapter_id", CanonicalValue::Identifier(self.adapter.id)),
            semantic(
                "adapter_version",
                CanonicalValue::Integer(i128::from(self.adapter.schema_version)),
            ),
            semantic(
                "adapter_hash",
                CanonicalValue::Bytes(self.adapter.semantic_hash.as_bytes()),
            ),
            semantic("profile_id", CanonicalValue::Identifier(self.profile.id)),
            semantic(
                "profile_version",
                CanonicalValue::Integer(i128::from(self.profile.schema_version)),
            ),
            semantic(
                "profile_hash",
                CanonicalValue::Bytes(self.profile.semantic_hash.as_bytes()),
            ),
            semantic(
                "fixture_suite_id",
                CanonicalValue::Identifier(self.fixture_suite.id),
            ),
            semantic(
                "fixture_suite_version",
                CanonicalValue::Integer(i128::from(self.fixture_suite.schema_version)),
            ),
            semantic(
                "fixture_suite_hash",
                CanonicalValue::Bytes(self.fixture_suite.semantic_hash.as_bytes()),
            ),
            semantic(
                "satisfaction_proof",
                CanonicalValue::Bytes(self.satisfaction_proof.as_bytes()),
            ),
            semantic(
                "boundary",
                CanonicalValue::Identifier(Id(self.boundary.as_str())),
            ),
            semantic(
                "outcome",
                CanonicalValue::Identifier(Id(self.outcome.as_str())),
            ),
            semantic(
                "maximum_in_flight",
                CanonicalValue::Integer(i128::from(self.bounds.maximum_in_flight)),
            ),
            semantic(
                "maximum_foreign_queue",
                CanonicalValue::Integer(i128::from(self.bounds.maximum_foreign_queue)),
            ),
            semantic(
                "maximum_memory_bytes",
                CanonicalValue::Integer(i128::from(self.bounds.maximum_memory_bytes)),
            ),
            semantic(
                "maximum_cancellation_ticks",
                CanonicalValue::Integer(i128::from(self.bounds.maximum_cancellation_ticks)),
            ),
            semantic(
                "maximum_evidence_events",
                CanonicalValue::Integer(i128::from(self.bounds.maximum_evidence_events)),
            ),
            semantic("time_basis", CanonicalValue::Identifier(self.time_basis)),
            semantic(
                "observed_at_tick",
                CanonicalValue::Integer(i128::from(self.observed_at_tick)),
            ),
            semantic(
                "valid_until_tick",
                CanonicalValue::Integer(i128::from(self.valid_until_tick)),
            ),
        ];
        semantic_hash_with_hash_set(
            Id("conduit/provider-conformance-result"),
            self.schema_version,
            &fields,
            Id("offered_facets"),
            &scratch[..self.offered_facets.len()],
        )
        .map_err(HostConformanceIdentityError::Canonical)
    }
}

/// Exact requested chain. The actual satisfaction proof is supplied so a
/// label or port-shape-only implementation cannot impersonate the contract.
pub struct ProviderBindingRequest<'a, 'proof> {
    pub required_contract: PinnedDescriptor<'a>,
    pub required_type: PinnedDescriptor<'a>,
    pub offered_type: PinnedDescriptor<'a>,
    pub explicit_adapter: Option<PinnedDescriptor<'a>>,
    pub provider_bundle: PinnedDescriptor<'a>,
    pub implementation: PinnedDescriptor<'a>,
    pub artifact: PinnedDescriptor<'a>,
    pub satisfaction: &'proof SatisfactionProof<'a>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExactProviderBinding<'a> {
    pub profile: PinnedDescriptor<'a>,
    pub required_contract: PinnedDescriptor<'a>,
    pub provider_bundle: PinnedDescriptor<'a>,
    pub implementation: PinnedDescriptor<'a>,
    pub artifact: PinnedDescriptor<'a>,
    pub adapter: PinnedDescriptor<'a>,
    pub host_report: PinnedDescriptor<'a>,
    pub observation: SemanticHash,
    pub satisfaction_proof: SemanticHash,
    pub conformance_result: SemanticHash,
    pub bounds: ProviderBounds,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostConformanceReason {
    UnsupportedVersion,
    InvalidProfile,
    DescribeOnly,
    ProviderAbsent,
    ProviderUnsupported,
    ProviderUninitialized,
    ProviderLost,
    ObservationStale,
    ObservationMismatch,
    ConformanceFailed,
    ConformanceStale,
    ConformanceMismatch,
    SatisfactionInvalid,
    TypeIncompatible,
    AdapterAbsent,
    AdapterMismatch,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostConformanceIdentityError {
    ScratchTooSmall,
    Canonical(CanonicalError<Infallible>),
}

impl HostConformanceReason {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::UnsupportedVersion => "CND-HCF-001",
            Self::InvalidProfile => "CND-HCF-002",
            Self::DescribeOnly => "CND-HCF-003",
            Self::ProviderAbsent => "CND-HCF-004",
            Self::ProviderUnsupported => "CND-HCF-005",
            Self::ProviderUninitialized => "CND-HCF-006",
            Self::ProviderLost => "CND-HCF-007",
            Self::ObservationStale => "CND-HCF-008",
            Self::ObservationMismatch => "CND-HCF-009",
            Self::ConformanceFailed => "CND-HCF-010",
            Self::ConformanceStale => "CND-HCF-011",
            Self::ConformanceMismatch => "CND-HCF-012",
            Self::SatisfactionInvalid => "CND-HCF-013",
            Self::TypeIncompatible => "CND-HCF-014",
            Self::AdapterAbsent => "CND-HCF-015",
            Self::AdapterMismatch => "CND-HCF-016",
        }
    }
}

pub fn validate_host_conformance_profile(
    profile: HostConformanceProfile<'_>,
) -> Result<(), HostConformanceReason> {
    if profile.schema_version != HOST_CONFORMANCE_PROFILE_SCHEMA_VERSION {
        return Err(HostConformanceReason::UnsupportedVersion);
    }
    if profile.id.as_str().is_empty()
        || profile.mandatory_facts.is_empty()
        || profile.mandatory_facts.len() > MAXIMUM_HOST_MANDATORY_FACTS
        || profile.optional_providers.len() > MAXIMUM_HOST_OPTIONAL_PROVIDERS
        || profile.extensions.len() > MAXIMUM_HOST_EXTENSIONS
        || !pins_valid(profile.mandatory_facts)
        || profile
            .optional_providers
            .iter()
            .any(|provider| !pin_valid(provider.contract) || !pin_valid(provider.provider_bundle))
        || profile
            .extensions
            .iter()
            .any(|extension| !pin_valid(extension.descriptor))
        || duplicates(profile.optional_providers, |provider| {
            provider.provider_bundle.semantic_hash
        })
        || duplicates(profile.extensions, |extension| {
            extension.descriptor.semantic_hash
        })
    {
        return Err(HostConformanceReason::InvalidProfile);
    }
    if (profile.class == HostClass::DescribeOnly)
        != (profile.execution_mode == HostExecutionMode::DescribeOnly)
    {
        return Err(HostConformanceReason::InvalidProfile);
    }
    let mut scratch = [SemanticHash::from_bytes([0; 32]);
        MAXIMUM_HOST_MANDATORY_FACTS + MAXIMUM_HOST_OPTIONAL_PROVIDERS + MAXIMUM_HOST_EXTENSIONS];
    if profile
        .computed_semantic_hash(&mut scratch)
        .map_err(|_| HostConformanceReason::InvalidProfile)?
        != profile.identity
    {
        return Err(HostConformanceReason::InvalidProfile);
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn bind_provider<'a>(
    profile_pin: PinnedDescriptor<'a>,
    profile: HostConformanceProfile<'a>,
    observation: ProviderObservation<'a>,
    conformance: ProviderConformanceResult<'a>,
    request: ProviderBindingRequest<'a, '_>,
    time_basis: Id<'_>,
    current_tick: u64,
    satisfaction_scratch: &mut [SemanticHash],
) -> Result<ExactProviderBinding<'a>, HostConformanceReason> {
    validate_host_conformance_profile(profile)?;
    if profile.execution_mode == HostExecutionMode::DescribeOnly {
        return Err(HostConformanceReason::DescribeOnly);
    }
    if profile_pin.semantic_hash != profile.identity {
        return Err(HostConformanceReason::InvalidProfile);
    }
    let inventory = profile
        .optional_providers
        .iter()
        .find(|provider| {
            provider.contract == request.required_contract
                && provider.provider_bundle == request.provider_bundle
        })
        .ok_or(HostConformanceReason::ProviderAbsent)?;
    if inventory.state == ProviderInventoryState::Unsupported {
        return Err(HostConformanceReason::ProviderUnsupported);
    }
    if observation.profile != profile_pin
        || observation.provider_bundle != request.provider_bundle
        || observation.host_report.semantic_hash == SemanticHash::from_bytes([0; 32])
        || observation.time_basis != time_basis
        || observation.observed_at_tick > observation.valid_until_tick
        || observation
            .computed_semantic_hash()
            .map_err(|_| HostConformanceReason::ObservationMismatch)?
            != observation.identity
    {
        return Err(HostConformanceReason::ObservationMismatch);
    }
    match observation.state {
        ProviderObservationState::Uninitialized => {
            return Err(HostConformanceReason::ProviderUninitialized);
        }
        ProviderObservationState::Lost => return Err(HostConformanceReason::ProviderLost),
        ProviderObservationState::Available => {}
    }
    if current_tick < observation.observed_at_tick || current_tick >= observation.valid_until_tick {
        return Err(HostConformanceReason::ObservationStale);
    }
    validate_satisfaction_proof(request.satisfaction, satisfaction_scratch)
        .map_err(|_| HostConformanceReason::SatisfactionInvalid)?;
    if request.satisfaction.outcome != CompatibilityOutcome::Compatible
        || request.satisfaction.identity != conformance.satisfaction_proof
        || request.satisfaction.required.semantic_hash != request.required_contract.semantic_hash
    {
        return Err(HostConformanceReason::SatisfactionInvalid);
    }
    if conformance.schema_version != PROVIDER_CONFORMANCE_RESULT_SCHEMA_VERSION
        || conformance.required_contract != request.required_contract
        || conformance.implementation != request.implementation
        || conformance.artifact != request.artifact
        || conformance.profile != profile_pin
        || conformance.time_basis != time_basis
        || !conformance.bounds.valid()
        || conformance.offered_facets.is_empty()
        || conformance.offered_facets.len() > MAXIMUM_PROVIDER_FACETS
        || !pins_valid(conformance.offered_facets)
    {
        return Err(HostConformanceReason::ConformanceMismatch);
    }
    let mut conformance_scratch = [SemanticHash::from_bytes([0; 32]); MAXIMUM_PROVIDER_FACETS];
    if conformance
        .computed_semantic_hash(&mut conformance_scratch)
        .map_err(|_| HostConformanceReason::ConformanceMismatch)?
        != conformance.identity
    {
        return Err(HostConformanceReason::ConformanceMismatch);
    }
    match conformance.outcome {
        ProviderConformanceOutcome::Passed => {}
        ProviderConformanceOutcome::Failed | ProviderConformanceOutcome::Unsupported => {
            return Err(HostConformanceReason::ConformanceFailed);
        }
    }
    if current_tick < conformance.observed_at_tick
        || current_tick >= conformance.valid_until_tick
        || conformance.observed_at_tick > conformance.valid_until_tick
    {
        return Err(HostConformanceReason::ConformanceStale);
    }
    let adapter = if request.required_type == request.offered_type {
        if request.explicit_adapter.is_some() {
            return Err(HostConformanceReason::AdapterMismatch);
        }
        conformance.adapter
    } else {
        let adapter = request
            .explicit_adapter
            .ok_or(HostConformanceReason::AdapterAbsent)?;
        if adapter != conformance.adapter
            || !profile.extensions.iter().any(|extension| {
                extension.kind == HostExtensionKind::Adapter && extension.descriptor == adapter
            })
        {
            return Err(HostConformanceReason::AdapterMismatch);
        }
        adapter
    };
    Ok(ExactProviderBinding {
        profile: profile_pin,
        required_contract: request.required_contract,
        provider_bundle: request.provider_bundle,
        implementation: request.implementation,
        artifact: request.artifact,
        adapter,
        host_report: observation.host_report,
        observation: observation.identity,
        satisfaction_proof: request.satisfaction.identity,
        conformance_result: conformance.identity,
        bounds: conformance.bounds,
    })
}

fn pin_valid(pin: PinnedDescriptor<'_>) -> bool {
    !pin.id.as_str().is_empty()
        && pin.id.as_str().contains('/')
        && pin.schema_version > 0
        && pin.semantic_hash != SemanticHash::from_bytes([0; 32])
}

fn pins_valid(pins: &[PinnedDescriptor<'_>]) -> bool {
    pins.iter().copied().all(pin_valid)
}

fn duplicates<T, F>(values: &[T], identity: F) -> bool
where
    F: Fn(&T) -> SemanticHash,
{
    values.iter().enumerate().any(|(index, value)| {
        values[index + 1..]
            .iter()
            .any(|other| identity(value) == identity(other))
    })
}

fn hash_provider_inventory(
    provider: ProviderInventory<'_>,
) -> Result<SemanticHash, HostConformanceIdentityError> {
    let fields = [
        semantic(
            "contract_id",
            CanonicalValue::Identifier(provider.contract.id),
        ),
        semantic(
            "contract_version",
            CanonicalValue::Integer(i128::from(provider.contract.schema_version)),
        ),
        semantic(
            "contract_hash",
            CanonicalValue::Bytes(provider.contract.semantic_hash.as_bytes()),
        ),
        semantic(
            "provider_bundle_id",
            CanonicalValue::Identifier(provider.provider_bundle.id),
        ),
        semantic(
            "provider_bundle_version",
            CanonicalValue::Integer(i128::from(provider.provider_bundle.schema_version)),
        ),
        semantic(
            "provider_bundle_hash",
            CanonicalValue::Bytes(provider.provider_bundle.semantic_hash.as_bytes()),
        ),
        semantic(
            "state",
            CanonicalValue::Identifier(Id(provider.state.as_str())),
        ),
    ];
    hash_fields("conduit/host-provider-inventory", &fields)
}

fn hash_extension(
    extension: HostExtension<'_>,
) -> Result<SemanticHash, HostConformanceIdentityError> {
    let fields = [
        semantic(
            "kind",
            CanonicalValue::Identifier(Id(extension.kind.as_str())),
        ),
        semantic(
            "descriptor_id",
            CanonicalValue::Identifier(extension.descriptor.id),
        ),
        semantic(
            "descriptor_version",
            CanonicalValue::Integer(i128::from(extension.descriptor.schema_version)),
        ),
        semantic(
            "descriptor_hash",
            CanonicalValue::Bytes(extension.descriptor.semantic_hash.as_bytes()),
        ),
    ];
    hash_fields("conduit/host-extension", &fields)
}

fn hash_pin(
    kind: &str,
    pin: PinnedDescriptor<'_>,
) -> Result<SemanticHash, HostConformanceIdentityError> {
    let fields = [
        semantic("id", CanonicalValue::Identifier(pin.id)),
        semantic(
            "schema_version",
            CanonicalValue::Integer(i128::from(pin.schema_version)),
        ),
        semantic(
            "semantic_hash",
            CanonicalValue::Bytes(pin.semantic_hash.as_bytes()),
        ),
    ];
    hash_fields(kind, &fields)
}

fn hash_fields(
    kind: &str,
    fields: &[MapField<'_>],
) -> Result<SemanticHash, HostConformanceIdentityError> {
    CanonicalDescriptor {
        kind: Id(kind),
        schema_version: 1,
        body: CanonicalValue::Map(fields),
    }
    .semantic_hash()
    .map_err(HostConformanceIdentityError::Canonical)
}

fn semantic<'a>(name: &'a str, value: CanonicalValue<'a>) -> MapField<'a> {
    MapField {
        name: Id(name),
        value,
        disposition: FieldDisposition::Semantic,
    }
}
