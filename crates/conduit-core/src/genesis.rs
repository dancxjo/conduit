//! Safe realm genesis, quarantine, recovery, and distribution defaults.
//!
//! This module defines portable facts only. It does not establish a realm,
//! enroll a member, install a provider, persist evidence, or interpret a
//! domain's effect taxonomy. Hosts perform those operations and present the
//! exact bounded observations validated here.

use core::convert::Infallible;

use crate::canonical::semantic_hash_with_hash_set;
use crate::{
    AdministrativeProof, AdministrativeSubject, ArtifactDigest, CanonicalDescriptor,
    CanonicalError, CanonicalValue, ContainmentContext, FieldDisposition, Id, MapField,
    PinnedDescriptor, SemanticHash, validate_administrative_proof,
};

pub const GENESIS_PROFILE_SCHEMA_VERSION: u32 = 1;
pub const DISTRIBUTION_PROFILE_SCHEMA_VERSION: u32 = 1;
pub const GENESIS_CONTROL_SCHEMA_VERSION: u32 = 1;
pub const MAX_BOOTSTRAP_CHANNELS: usize = 4;
pub const MAX_PUBLIC_OPERATIONS: usize = 16;
pub const MAX_GENESIS_MEMBERS: usize = 32;
pub const MAX_MEMBER_ROLES: usize = 16;
pub const MAX_MEMBER_GRANTS: usize = 32;
pub const MAX_MEMBER_DELEGATIONS: usize = 16;
pub const MAX_DISTRIBUTION_PROVIDERS: usize = 32;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RealmGenesisClass {
    Private,
    SharedPrivate,
    DeliberatelyPublic,
    SimulationOnly,
    ConstrainedOffline,
}

impl RealmGenesisClass {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Private => "private",
            Self::SharedPrivate => "shared-private",
            Self::DeliberatelyPublic => "deliberately-public",
            Self::SimulationOnly => "simulation-only",
            Self::ConstrainedOffline => "constrained-offline",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum SafePlanDisposition {
    Disabled,
    SimulationOnly,
}

impl SafePlanDisposition {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::SimulationOnly => "simulation-only",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BootstrapChannel {
    PhysicalPresence,
    Usb,
    Ble,
    TemporaryLocal,
}

impl BootstrapChannel {
    const fn as_str(self) -> &'static str {
        match self {
            Self::PhysicalPresence => "physical-presence",
            Self::Usb => "usb",
            Self::Ble => "ble",
            Self::TemporaryLocal => "temporary-local",
        }
    }
}

/// The event that caused a host to consider bootstrap. Only
/// `LocalCeremony` can enter the bootstrap validator.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BootstrapOrigin {
    LocalCeremony,
    NetworkAttachment,
    BrowserNavigation,
    PwaInstall,
    BrowserPermission,
    TransportHandshake,
    CapabilityReport,
    Callsign,
}

/// One domain-owned operation deliberately exposed by a public genesis
/// profile. The generic traits prohibit using this list for administration,
/// deployment, protected subscriptions, or actuation.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PublicGenesisOperation<'a> {
    pub operation: PinnedDescriptor<'a>,
    pub maximum_uses: u32,
    pub administrative: bool,
    pub deployment: bool,
    pub protected_subscription: bool,
    pub actuating: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RealmGenesisProfile<'a> {
    pub schema_version: u32,
    pub identity: SemanticHash,
    pub descriptor: PinnedDescriptor<'a>,
    pub class: RealmGenesisClass,
    pub safe_plan: PinnedDescriptor<'a>,
    pub safe_plan_disposition: SafePlanDisposition,
    /// Optional exact local-only bootstrap realm. Absence means that the
    /// unconfigured state has no realm.
    pub local_bootstrap_realm: Option<Id<'a>>,
    /// At most this one identity may exist during local bootstrap.
    pub bootstrap_identity: Option<Id<'a>>,
    pub bootstrap_authority: PinnedDescriptor<'a>,
    /// Selected append-only control evidence recorder. This is not the
    /// bootstrap authorizer and cannot grant membership or effects.
    pub control_recorder: PinnedDescriptor<'a>,
    pub recovery_effect_class: PinnedDescriptor<'a>,
    pub recovery_operation: PinnedDescriptor<'a>,
    pub bootstrap_channels: &'a [BootstrapChannel],
    pub time_basis: Id<'a>,
    pub bootstrap_ttl_ticks: u64,
    pub maximum_bootstrap_attempts: u16,
    pub maximum_evidence_events: u32,
    pub public_operations: &'a [PublicGenesisOperation<'a>],
}

impl RealmGenesisProfile<'_> {
    pub const fn identity_fact_count(&self) -> usize {
        self.bootstrap_channels.len() + self.public_operations.len()
    }

    pub fn computed_semantic_hash(
        &self,
        scratch: &mut [SemanticHash],
    ) -> Result<SemanticHash, GenesisIdentityError> {
        let needed = self.identity_fact_count();
        if scratch.len() < needed {
            return Err(GenesisIdentityError::ScratchTooSmall);
        }
        let mut at = 0;
        for channel in self.bootstrap_channels {
            scratch[at] = descriptor_hash(
                "conduit/bootstrap-channel",
                &[semantic(
                    "channel",
                    CanonicalValue::Identifier(Id(channel.as_str())),
                )],
            )
            .map_err(GenesisIdentityError::Canonical)?;
            at += 1;
        }
        for operation in self.public_operations {
            scratch[at] = public_operation_hash(*operation)?;
            at += 1;
        }
        let descriptor = pin_hash(self.descriptor)?;
        let safe_plan = pin_hash(self.safe_plan)?;
        let bootstrap_authority = pin_hash(self.bootstrap_authority)?;
        let control_recorder = pin_hash(self.control_recorder)?;
        let recovery_effect_class = pin_hash(self.recovery_effect_class)?;
        let recovery_operation = pin_hash(self.recovery_operation)?;
        let fields = [
            semantic("descriptor", CanonicalValue::Bytes(descriptor.as_bytes())),
            semantic("class", CanonicalValue::Identifier(Id(self.class.as_str()))),
            semantic("safe_plan", CanonicalValue::Bytes(safe_plan.as_bytes())),
            semantic(
                "safe_plan_disposition",
                CanonicalValue::Identifier(Id(self.safe_plan_disposition.as_str())),
            ),
            semantic(
                "local_bootstrap_realm",
                self.local_bootstrap_realm
                    .map_or(CanonicalValue::Null, CanonicalValue::Identifier),
            ),
            semantic(
                "bootstrap_identity",
                self.bootstrap_identity
                    .map_or(CanonicalValue::Null, CanonicalValue::Identifier),
            ),
            semantic(
                "bootstrap_authority",
                CanonicalValue::Bytes(bootstrap_authority.as_bytes()),
            ),
            semantic(
                "control_recorder",
                CanonicalValue::Bytes(control_recorder.as_bytes()),
            ),
            semantic(
                "recovery_effect_class",
                CanonicalValue::Bytes(recovery_effect_class.as_bytes()),
            ),
            semantic(
                "recovery_operation",
                CanonicalValue::Bytes(recovery_operation.as_bytes()),
            ),
            semantic("time_basis", CanonicalValue::Identifier(self.time_basis)),
            semantic(
                "bootstrap_ttl_ticks",
                CanonicalValue::Integer(i128::from(self.bootstrap_ttl_ticks)),
            ),
            semantic(
                "maximum_bootstrap_attempts",
                CanonicalValue::Integer(i128::from(self.maximum_bootstrap_attempts)),
            ),
            semantic(
                "maximum_evidence_events",
                CanonicalValue::Integer(i128::from(self.maximum_evidence_events)),
            ),
        ];
        semantic_hash_with_hash_set(
            Id("conduit/realm-genesis-profile"),
            self.schema_version,
            &fields,
            Id("facts"),
            &scratch[..needed],
        )
        .map_err(GenesisIdentityError::Canonical)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GenesisPhase {
    Unconfigured,
    LocalBootstrap,
    Established,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemberDisposition {
    Quarantined,
    Authorized,
}

/// Current security projection for one member. Membership and passport
/// identity remain separate from every authority-bearing collection.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct MemberSecurityState<'a> {
    pub entity: Id<'a>,
    pub passport: SemanticHash,
    pub disposition: MemberDisposition,
    pub roles: &'a [PinnedDescriptor<'a>],
    pub grants: &'a [SemanticHash],
    pub delegations: &'a [SemanticHash],
    pub federations: u16,
    pub installed_providers: u16,
    pub protected_subscriptions: u16,
    pub remote_plan_activations: u16,
    pub administrative_effects: u16,
    pub actuating_effects: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GenesisStateObservation<'a> {
    pub profile_identity: SemanticHash,
    pub phase: GenesisPhase,
    pub realm: Option<Id<'a>>,
    pub active_plan: PinnedDescriptor<'a>,
    pub active_plan_disposition: SafePlanDisposition,
    pub remote_discovery_enabled: bool,
    pub public_listener_enabled: bool,
    pub unrestricted_network_enabled: bool,
    pub members: &'a [MemberSecurityState<'a>],
    pub federations: u16,
    pub authority_grants: u16,
    pub dangerous_providers_enabled: u16,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GenesisControlKind {
    BootstrapRequested,
    BootstrapDenied,
    EnrolledQuarantined,
    RoleBound,
    GrantIssued,
    ProviderEnabled,
    FactoryReset,
    RecoveryRequested,
    RecoveryDenied,
    Restored,
}

impl GenesisControlKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::BootstrapRequested => "bootstrap-requested",
            Self::BootstrapDenied => "bootstrap-denied",
            Self::EnrolledQuarantined => "enrolled-quarantined",
            Self::RoleBound => "role-bound",
            Self::GrantIssued => "grant-issued",
            Self::ProviderEnabled => "provider-enabled",
            Self::FactoryReset => "factory-reset",
            Self::RecoveryRequested => "recovery-requested",
            Self::RecoveryDenied => "recovery-denied",
            Self::Restored => "restored",
        }
    }
}

/// Immutable, secret-free control evidence. A host stores the receipt and
/// predecessor records; this contract carries only their identities.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct GenesisControlRecord<'a> {
    pub schema_version: u32,
    pub identity: SemanticHash,
    pub sequence: u64,
    pub predecessor: Option<SemanticHash>,
    pub profile_identity: SemanticHash,
    pub kind: GenesisControlKind,
    pub subject: Id<'a>,
    pub authority: PinnedDescriptor<'a>,
    pub time_basis: Id<'a>,
    pub observed_at_tick: u64,
    pub receipt: SemanticHash,
}

impl GenesisControlRecord<'_> {
    pub fn computed_semantic_hash(&self) -> Result<SemanticHash, CanonicalError<Infallible>> {
        let authority = pin_hash(self.authority).map_err(identity_canonical)?;
        descriptor_hash(
            "conduit/genesis-control-record",
            &[
                semantic(
                    "sequence",
                    CanonicalValue::Integer(i128::from(self.sequence)),
                ),
                semantic(
                    "predecessor",
                    self.predecessor
                        .as_ref()
                        .map_or(CanonicalValue::Null, |value| {
                            CanonicalValue::Bytes(value.as_bytes())
                        }),
                ),
                semantic(
                    "profile_identity",
                    CanonicalValue::Bytes(self.profile_identity.as_bytes()),
                ),
                semantic("kind", CanonicalValue::Identifier(Id(self.kind.as_str()))),
                semantic("subject", CanonicalValue::Identifier(self.subject)),
                semantic("authority", CanonicalValue::Bytes(authority.as_bytes())),
                semantic("time_basis", CanonicalValue::Identifier(self.time_basis)),
                semantic(
                    "observed_at_tick",
                    CanonicalValue::Integer(i128::from(self.observed_at_tick)),
                ),
                semantic("receipt", CanonicalValue::Bytes(self.receipt.as_bytes())),
            ],
        )
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BootstrapAttempt<'a> {
    pub id: Id<'a>,
    pub profile_identity: SemanticHash,
    pub candidate_entity: Id<'a>,
    pub candidate_key: Id<'a>,
    pub authorization: PinnedDescriptor<'a>,
    pub origin: BootstrapOrigin,
    pub channel: Option<BootstrapChannel>,
    pub time_basis: Id<'a>,
    pub issued_at_tick: u64,
    pub expires_at_tick: u64,
    pub ordinal: u16,
    pub local_confirmation: bool,
    pub replayed: bool,
    pub remote_session: bool,
    pub receipt: SemanticHash,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostDistributionKind {
    Hosted,
    Browser,
    Constrained,
}

impl HostDistributionKind {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Hosted => "hosted",
            Self::Browser => "browser",
            Self::Constrained => "constrained",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderAvailability {
    Absent,
    Disabled,
    Enabled,
    Unsupported,
}

impl ProviderAvailability {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Absent => "absent",
            Self::Disabled => "disabled",
            Self::Enabled => "enabled",
            Self::Unsupported => "unsupported",
        }
    }
}

/// Generic provider traits. Domains retain ownership of exact provider and
/// effect-class descriptors.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct ProviderRiskTraits {
    pub enrollment_issuer: bool,
    pub unrestricted_native_execution: bool,
    pub remote_artifact_installation: bool,
    pub firmware_mutation: bool,
    pub unrestricted_network: bool,
    pub realm_root_administration: bool,
    pub remote_plan_activation: bool,
    pub actuating_effects: bool,
}

impl ProviderRiskTraits {
    #[must_use]
    pub const fn any(self) -> bool {
        self.enrollment_issuer
            || self.unrestricted_native_execution
            || self.remote_artifact_installation
            || self.firmware_mutation
            || self.unrestricted_network
            || self.realm_root_administration
            || self.remote_plan_activation
            || self.actuating_effects
    }

    const fn contains(self, required: Self) -> bool {
        (!required.enrollment_issuer || self.enrollment_issuer)
            && (!required.unrestricted_native_execution || self.unrestricted_native_execution)
            && (!required.remote_artifact_installation || self.remote_artifact_installation)
            && (!required.firmware_mutation || self.firmware_mutation)
            && (!required.unrestricted_network || self.unrestricted_network)
            && (!required.realm_root_administration || self.realm_root_administration)
            && (!required.remote_plan_activation || self.remote_plan_activation)
            && (!required.actuating_effects || self.actuating_effects)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DistributionProvider<'a> {
    pub provider: PinnedDescriptor<'a>,
    pub artifact: Option<ArtifactDigest>,
    pub availability: ProviderAvailability,
    pub traits: ProviderRiskTraits,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ReferenceDistributionProfile<'a> {
    pub schema_version: u32,
    pub identity: SemanticHash,
    pub descriptor: PinnedDescriptor<'a>,
    pub kind: HostDistributionKind,
    pub genesis_profile: SemanticHash,
    pub control_recorder: PinnedDescriptor<'a>,
    pub provider_enablement_effect_class: PinnedDescriptor<'a>,
    pub provider_enablement_operation: PinnedDescriptor<'a>,
    pub providers: &'a [DistributionProvider<'a>],
    pub maximum_provider_enablement_ticks: u64,
    pub maximum_provider_install_attempts: u16,
    pub maximum_evidence_events: u32,
}

impl ReferenceDistributionProfile<'_> {
    pub const fn identity_fact_count(&self) -> usize {
        self.providers.len()
    }

    pub fn computed_semantic_hash(
        &self,
        scratch: &mut [SemanticHash],
    ) -> Result<SemanticHash, GenesisIdentityError> {
        if scratch.len() < self.providers.len() {
            return Err(GenesisIdentityError::ScratchTooSmall);
        }
        for (slot, provider) in scratch.iter_mut().zip(self.providers) {
            *slot = distribution_provider_hash(*provider)?;
        }
        let descriptor = pin_hash(self.descriptor)?;
        let control_recorder = pin_hash(self.control_recorder)?;
        let provider_enablement_effect_class = pin_hash(self.provider_enablement_effect_class)?;
        let provider_enablement_operation = pin_hash(self.provider_enablement_operation)?;
        let fields = [
            semantic("descriptor", CanonicalValue::Bytes(descriptor.as_bytes())),
            semantic("kind", CanonicalValue::Identifier(Id(self.kind.as_str()))),
            semantic(
                "genesis_profile",
                CanonicalValue::Bytes(self.genesis_profile.as_bytes()),
            ),
            semantic(
                "control_recorder",
                CanonicalValue::Bytes(control_recorder.as_bytes()),
            ),
            semantic(
                "provider_enablement_effect_class",
                CanonicalValue::Bytes(provider_enablement_effect_class.as_bytes()),
            ),
            semantic(
                "provider_enablement_operation",
                CanonicalValue::Bytes(provider_enablement_operation.as_bytes()),
            ),
            semantic(
                "maximum_provider_enablement_ticks",
                CanonicalValue::Integer(i128::from(self.maximum_provider_enablement_ticks)),
            ),
            semantic(
                "maximum_provider_install_attempts",
                CanonicalValue::Integer(i128::from(self.maximum_provider_install_attempts)),
            ),
            semantic(
                "maximum_evidence_events",
                CanonicalValue::Integer(i128::from(self.maximum_evidence_events)),
            ),
        ];
        semantic_hash_with_hash_set(
            Id("conduit/reference-distribution-profile"),
            self.schema_version,
            &fields,
            Id("providers"),
            &scratch[..self.providers.len()],
        )
        .map_err(GenesisIdentityError::Canonical)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProviderRequirement<'a> {
    pub provider: PinnedDescriptor<'a>,
    pub traits: ProviderRiskTraits,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderSelection {
    Available,
    Absent,
    Disabled,
    Unsupported,
    TraitMismatch,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProviderDecision<'a> {
    pub provider: PinnedDescriptor<'a>,
    pub selection: ProviderSelection,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProviderEnablement<'a> {
    pub distribution_identity: SemanticHash,
    pub provider: PinnedDescriptor<'a>,
    pub artifact: ArtifactDigest,
    pub ordinal: u16,
    pub time_basis: Id<'a>,
    pub enabled_at_tick: u64,
    pub expires_at_tick: u64,
    /// Installing a provider never grants any of its effects.
    pub effect_grants: &'a [SemanticHash],
    pub evidence: GenesisControlRecord<'a>,
    pub approval: AdministrativeProof<'a>,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct AuthoritySurface {
    pub members: u32,
    pub grants: u32,
    pub delegations: u32,
    pub federations: u32,
    pub executable_providers: u32,
    pub root_authorities: u16,
    pub remote_plan_activations: u32,
    pub protected_subscriptions: u32,
    pub actuating_bindings: u32,
    pub remote_discovery: bool,
    pub public_listener: bool,
    pub unrestricted_network: bool,
    pub ambient_root: bool,
    pub trust_on_first_use: bool,
}

impl AuthoritySurface {
    #[must_use]
    pub const fn no_wider_than(self, ceiling: Self) -> bool {
        self.members <= ceiling.members
            && self.grants <= ceiling.grants
            && self.delegations <= ceiling.delegations
            && self.federations <= ceiling.federations
            && self.executable_providers <= ceiling.executable_providers
            && self.root_authorities <= ceiling.root_authorities
            && self.remote_plan_activations <= ceiling.remote_plan_activations
            && self.protected_subscriptions <= ceiling.protected_subscriptions
            && self.actuating_bindings <= ceiling.actuating_bindings
            && (!self.remote_discovery || ceiling.remote_discovery)
            && (!self.public_listener || ceiling.public_listener)
            && (!self.unrestricted_network || ceiling.unrestricted_network)
            && (!self.ambient_root || ceiling.ambient_root)
            && (!self.trust_on_first_use || ceiling.trust_on_first_use)
    }

    #[must_use]
    pub const fn isolated(self) -> bool {
        self.members == 0
            && self.grants == 0
            && self.delegations == 0
            && self.federations == 0
            && self.executable_providers == 0
            && self.root_authorities == 0
            && self.remote_plan_activations == 0
            && self.protected_subscriptions == 0
            && self.actuating_bindings == 0
            && !self.remote_discovery
            && !self.public_listener
            && !self.unrestricted_network
            && !self.ambient_root
            && !self.trust_on_first_use
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RecoveryKind {
    FactoryReset,
    LostRoot,
    FailedRestore,
    Restore,
    Rollback,
    Emergency,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct RecoveryTransition<'a> {
    pub profile_identity: SemanticHash,
    pub kind: RecoveryKind,
    pub prior: AuthoritySurface,
    pub candidate: AuthoritySurface,
    /// Exact independently reviewed recovery snapshot ceiling.
    pub recovery_ceiling: Option<AuthoritySurface>,
    pub snapshot: Option<SemanticHash>,
    pub evidence: GenesisControlRecord<'a>,
    pub approval: Option<AdministrativeProof<'a>>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GenesisReason {
    UnsupportedVersion,
    InvalidDescriptor,
    IdentityMismatch,
    UnsafeInitialState,
    ImplicitOrRemoteBootstrap,
    BootstrapExpiredOrReplayed,
    EvidenceInvalidOrExhausted,
    QuarantineViolated,
    PublicOperationDenied,
    ProviderUnavailable,
    DangerousProviderEnabledByDefault,
    ProviderEnablementInvalid,
    RecoveryWidened,
    StorageBoundExceeded,
}

impl GenesisReason {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::UnsupportedVersion => "CND-GEN-001",
            Self::InvalidDescriptor => "CND-GEN-002",
            Self::IdentityMismatch => "CND-GEN-003",
            Self::UnsafeInitialState => "CND-GEN-004",
            Self::ImplicitOrRemoteBootstrap => "CND-GEN-005",
            Self::BootstrapExpiredOrReplayed => "CND-GEN-006",
            Self::EvidenceInvalidOrExhausted => "CND-GEN-007",
            Self::QuarantineViolated => "CND-GEN-008",
            Self::PublicOperationDenied => "CND-GEN-009",
            Self::ProviderUnavailable => "CND-GEN-010",
            Self::DangerousProviderEnabledByDefault => "CND-GEN-011",
            Self::ProviderEnablementInvalid => "CND-GEN-012",
            Self::RecoveryWidened => "CND-GEN-013",
            Self::StorageBoundExceeded => "CND-GEN-014",
        }
    }

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::UnsupportedVersion => "unsupported-genesis-version",
            Self::InvalidDescriptor => "invalid-genesis-descriptor",
            Self::IdentityMismatch => "genesis-identity-mismatch",
            Self::UnsafeInitialState => "unsafe-initial-state",
            Self::ImplicitOrRemoteBootstrap => "implicit-or-remote-bootstrap",
            Self::BootstrapExpiredOrReplayed => "bootstrap-expired-replayed-or-retry-exhausted",
            Self::EvidenceInvalidOrExhausted => "genesis-evidence-invalid-or-exhausted",
            Self::QuarantineViolated => "member-quarantine-violated",
            Self::PublicOperationDenied => "public-genesis-operation-denied",
            Self::ProviderUnavailable => "required-provider-absent-disabled-or-unsupported",
            Self::DangerousProviderEnabledByDefault => {
                "dangerous-provider-enabled-in-reference-distribution"
            }
            Self::ProviderEnablementInvalid => "provider-enablement-invalid-or-unapproved",
            Self::RecoveryWidened => "recovery-authority-widened",
            Self::StorageBoundExceeded => "genesis-storage-bound-exceeded",
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum GenesisIdentityError {
    ScratchTooSmall,
    Canonical(CanonicalError<Infallible>),
}

/// Validate one immutable genesis profile independently of current host state.
pub fn validate_genesis_profile(
    profile: RealmGenesisProfile<'_>,
    scratch: &mut [SemanticHash],
) -> Result<(), GenesisReason> {
    if profile.schema_version != GENESIS_PROFILE_SCHEMA_VERSION {
        return Err(GenesisReason::UnsupportedVersion);
    }
    if !valid_pin(profile.descriptor)
        || !valid_pin(profile.safe_plan)
        || !valid_pin(profile.bootstrap_authority)
        || !valid_pin(profile.control_recorder)
        || !valid_pin(profile.recovery_effect_class)
        || !valid_pin(profile.recovery_operation)
        || !valid_id(profile.time_basis)
        || profile.bootstrap_channels.is_empty()
        || profile.bootstrap_channels.len() > MAX_BOOTSTRAP_CHANNELS
        || profile.bootstrap_ttl_ticks == 0
        || profile.maximum_bootstrap_attempts == 0
        || profile.maximum_evidence_events == 0
        || profile.public_operations.len() > MAX_PUBLIC_OPERATIONS
        || profile
            .local_bootstrap_realm
            .is_some_and(|value| !valid_id(value))
        || profile
            .bootstrap_identity
            .is_some_and(|value| !valid_id(value))
        || profile.local_bootstrap_realm.is_some() != profile.bootstrap_identity.is_some()
        || has_duplicate_channels(profile.bootstrap_channels)
        || profile.public_operations.iter().any(|operation| {
            !valid_pin(operation.operation)
                || operation.maximum_uses == 0
                || operation.administrative
                || operation.deployment
                || operation.protected_subscription
                || operation.actuating
        })
        || (profile.class != RealmGenesisClass::DeliberatelyPublic
            && !profile.public_operations.is_empty())
        || (profile.class == RealmGenesisClass::SimulationOnly
            && profile.safe_plan_disposition != SafePlanDisposition::SimulationOnly)
        || (profile.class != RealmGenesisClass::SimulationOnly
            && profile.safe_plan_disposition != SafePlanDisposition::Disabled)
    {
        return Err(GenesisReason::InvalidDescriptor);
    }
    let computed = profile
        .computed_semantic_hash(scratch)
        .map_err(identity_reason)?;
    if computed != profile.identity {
        return Err(GenesisReason::IdentityMismatch);
    }
    Ok(())
}

/// Prove that a fresh installation or local bootstrap state exposes no
/// administrative, remote, deployment, federation, or actuating authority.
pub fn validate_safe_initial_state(
    profile: RealmGenesisProfile<'_>,
    state: GenesisStateObservation<'_>,
    scratch: &mut [SemanticHash],
) -> Result<(), GenesisReason> {
    validate_genesis_profile(profile, scratch)?;
    if state.profile_identity != profile.identity
        || state.active_plan != profile.safe_plan
        || state.active_plan_disposition != profile.safe_plan_disposition
        || state.phase == GenesisPhase::Established
        || state.remote_discovery_enabled
        || state.public_listener_enabled
        || state.unrestricted_network_enabled
        || state.federations != 0
        || state.authority_grants != 0
        || state.dangerous_providers_enabled != 0
        || state.members.len() > MAX_GENESIS_MEMBERS
    {
        return Err(GenesisReason::UnsafeInitialState);
    }
    match state.phase {
        GenesisPhase::Unconfigured => {
            if state.realm.is_some() || !state.members.is_empty() {
                return Err(GenesisReason::UnsafeInitialState);
            }
        }
        GenesisPhase::LocalBootstrap => {
            if state.realm != profile.local_bootstrap_realm
                || state.members.len() > usize::from(profile.bootstrap_identity.is_some())
            {
                return Err(GenesisReason::UnsafeInitialState);
            }
            if let Some(member) = state.members.first() {
                if Some(member.entity) != profile.bootstrap_identity {
                    return Err(GenesisReason::UnsafeInitialState);
                }
                validate_quarantined_member(*member)?;
            }
        }
        GenesisPhase::Established => return Err(GenesisReason::UnsafeInitialState),
    }
    Ok(())
}

pub fn validate_control_record(
    profile: RealmGenesisProfile<'_>,
    record: GenesisControlRecord<'_>,
) -> Result<(), GenesisReason> {
    let mut profile_scratch =
        [SemanticHash::from_bytes([0; 32]); MAX_BOOTSTRAP_CHANNELS + MAX_PUBLIC_OPERATIONS];
    validate_genesis_profile(profile, &mut profile_scratch)?;
    if record.schema_version != GENESIS_CONTROL_SCHEMA_VERSION
        || record.sequence == 0
        || (record.sequence == 1) != record.predecessor.is_none()
        || !valid_id(record.subject)
        || !valid_pin(record.authority)
        || !valid_id(record.time_basis)
        || record.profile_identity != profile.identity
        || record.authority != profile.control_recorder
        || record.time_basis != profile.time_basis
        || record.sequence > u64::from(profile.maximum_evidence_events)
    {
        return Err(GenesisReason::EvidenceInvalidOrExhausted);
    }
    let computed = record
        .computed_semantic_hash()
        .map_err(|_| GenesisReason::EvidenceInvalidOrExhausted)?;
    if computed != record.identity
        || record.identity == record.receipt
        || record.predecessor == Some(record.identity)
    {
        return Err(GenesisReason::EvidenceInvalidOrExhausted);
    }
    Ok(())
}

/// A successful result is only permission to issue a quarantined passport.
/// Role binding, grant issuance, federation, provider enablement, and effect
/// authorization remain separate operations.
pub fn validate_bootstrap_attempt(
    profile: RealmGenesisProfile<'_>,
    attempt: BootstrapAttempt<'_>,
    evidence: GenesisControlRecord<'_>,
    now_tick: u64,
) -> Result<MemberDisposition, GenesisReason> {
    let mut profile_scratch =
        [SemanticHash::from_bytes([0; 32]); MAX_BOOTSTRAP_CHANNELS + MAX_PUBLIC_OPERATIONS];
    validate_genesis_profile(profile, &mut profile_scratch)?;
    if !valid_id(attempt.id)
        || !valid_id(attempt.candidate_entity)
        || !valid_id(attempt.candidate_key)
        || attempt.authorization != profile.bootstrap_authority
        || attempt.profile_identity != profile.identity
        || attempt.origin != BootstrapOrigin::LocalCeremony
        || attempt.remote_session
        || !attempt.local_confirmation
        || attempt
            .channel
            .is_none_or(|channel| !profile.bootstrap_channels.contains(&channel))
    {
        return Err(GenesisReason::ImplicitOrRemoteBootstrap);
    }
    if attempt.replayed
        || attempt.ordinal == 0
        || attempt.ordinal > profile.maximum_bootstrap_attempts
        || attempt.time_basis != profile.time_basis
        || attempt.issued_at_tick >= attempt.expires_at_tick
        || attempt.expires_at_tick - attempt.issued_at_tick > profile.bootstrap_ttl_ticks
        || now_tick < attempt.issued_at_tick
        || now_tick >= attempt.expires_at_tick
    {
        return Err(GenesisReason::BootstrapExpiredOrReplayed);
    }
    validate_control_record(profile, evidence)?;
    if evidence.kind != GenesisControlKind::BootstrapRequested
        || evidence.subject != attempt.candidate_entity
        || evidence.observed_at_tick != attempt.issued_at_tick
        || evidence.receipt != attempt.receipt
    {
        return Err(GenesisReason::EvidenceInvalidOrExhausted);
    }
    Ok(MemberDisposition::Quarantined)
}

pub fn validate_quarantined_member(member: MemberSecurityState<'_>) -> Result<(), GenesisReason> {
    if !valid_id(member.entity)
        || member.disposition != MemberDisposition::Quarantined
        || member.roles.len() > MAX_MEMBER_ROLES
        || member.grants.len() > MAX_MEMBER_GRANTS
        || member.delegations.len() > MAX_MEMBER_DELEGATIONS
        || !member.roles.is_empty()
        || !member.grants.is_empty()
        || !member.delegations.is_empty()
        || member.federations != 0
        || member.installed_providers != 0
        || member.protected_subscriptions != 0
        || member.remote_plan_activations != 0
        || member.administrative_effects != 0
        || member.actuating_effects != 0
    {
        return Err(GenesisReason::QuarantineViolated);
    }
    Ok(())
}

pub fn authorize_public_operation(
    profile: RealmGenesisProfile<'_>,
    operation: PinnedDescriptor<'_>,
    requested_uses: u32,
) -> Result<(), GenesisReason> {
    let mut profile_scratch =
        [SemanticHash::from_bytes([0; 32]); MAX_BOOTSTRAP_CHANNELS + MAX_PUBLIC_OPERATIONS];
    validate_genesis_profile(profile, &mut profile_scratch)?;
    if profile.class != RealmGenesisClass::DeliberatelyPublic
        || requested_uses == 0
        || !profile.public_operations.iter().any(|allowed| {
            allowed.operation == operation
                && requested_uses <= allowed.maximum_uses
                && !allowed.administrative
                && !allowed.deployment
                && !allowed.protected_subscription
                && !allowed.actuating
        })
    {
        return Err(GenesisReason::PublicOperationDenied);
    }
    Ok(())
}

pub fn validate_reference_distribution(
    profile: ReferenceDistributionProfile<'_>,
    scratch: &mut [SemanticHash],
) -> Result<(), GenesisReason> {
    if profile.schema_version != DISTRIBUTION_PROFILE_SCHEMA_VERSION {
        return Err(GenesisReason::UnsupportedVersion);
    }
    if !valid_pin(profile.descriptor)
        || !valid_pin(profile.control_recorder)
        || !valid_pin(profile.provider_enablement_effect_class)
        || !valid_pin(profile.provider_enablement_operation)
        || profile.providers.len() > MAX_DISTRIBUTION_PROVIDERS
        || profile.maximum_provider_enablement_ticks == 0
        || profile.maximum_provider_install_attempts == 0
        || profile.maximum_evidence_events == 0
        || profile
            .providers
            .iter()
            .any(|provider| !valid_pin(provider.provider))
    {
        return Err(GenesisReason::InvalidDescriptor);
    }
    if has_duplicate_providers(profile.providers) {
        return Err(GenesisReason::InvalidDescriptor);
    }
    if profile.providers.iter().any(|provider| {
        provider.availability == ProviderAvailability::Enabled && provider.traits.any()
    }) {
        return Err(GenesisReason::DangerousProviderEnabledByDefault);
    }
    let computed = profile
        .computed_semantic_hash(scratch)
        .map_err(identity_reason)?;
    if computed != profile.identity {
        return Err(GenesisReason::IdentityMismatch);
    }
    Ok(())
}

pub fn assess_provider_requirement<'a>(
    distribution: ReferenceDistributionProfile<'a>,
    requirement: ProviderRequirement<'a>,
) -> Result<ProviderDecision<'a>, GenesisReason> {
    let mut distribution_scratch = [SemanticHash::from_bytes([0; 32]); MAX_DISTRIBUTION_PROVIDERS];
    validate_reference_distribution(distribution, &mut distribution_scratch)?;
    if !valid_pin(requirement.provider) {
        return Err(GenesisReason::InvalidDescriptor);
    }
    let Some(provider) = distribution
        .providers
        .iter()
        .find(|provider| provider.provider == requirement.provider)
    else {
        return Ok(ProviderDecision {
            provider: requirement.provider,
            selection: ProviderSelection::Absent,
        });
    };
    let selection = match provider.availability {
        ProviderAvailability::Absent => ProviderSelection::Absent,
        ProviderAvailability::Disabled => ProviderSelection::Disabled,
        ProviderAvailability::Unsupported => ProviderSelection::Unsupported,
        ProviderAvailability::Enabled if !provider.traits.contains(requirement.traits) => {
            ProviderSelection::TraitMismatch
        }
        ProviderAvailability::Enabled => ProviderSelection::Available,
    };
    Ok(ProviderDecision {
        provider: requirement.provider,
        selection,
    })
}

pub fn require_provider<'a>(
    distribution: ReferenceDistributionProfile<'a>,
    requirement: ProviderRequirement<'a>,
) -> Result<ProviderDecision<'a>, GenesisReason> {
    let decision = assess_provider_requirement(distribution, requirement)?;
    if decision.selection != ProviderSelection::Available {
        return Err(GenesisReason::ProviderUnavailable);
    }
    Ok(decision)
}

pub fn validate_provider_enablement(
    distribution: ReferenceDistributionProfile<'_>,
    enablement: ProviderEnablement<'_>,
    context: ContainmentContext<'_>,
) -> Result<(), GenesisReason> {
    let mut distribution_scratch = [SemanticHash::from_bytes([0; 32]); MAX_DISTRIBUTION_PROVIDERS];
    validate_reference_distribution(distribution, &mut distribution_scratch)?;
    let declaration = distribution
        .providers
        .iter()
        .find(|provider| provider.provider == enablement.provider)
        .ok_or(GenesisReason::ProviderEnablementInvalid)?;
    if enablement.distribution_identity != distribution.identity
        || !declaration.traits.any()
        || declaration.availability == ProviderAvailability::Enabled
        || declaration
            .artifact
            .is_some_and(|artifact| artifact != enablement.artifact)
        || enablement.ordinal == 0
        || enablement.ordinal > distribution.maximum_provider_install_attempts
        || enablement.time_basis != context.time_basis
        || enablement.enabled_at_tick > context.now_tick
        || context.now_tick >= enablement.expires_at_tick
        || enablement.expires_at_tick - enablement.enabled_at_tick
            > distribution.maximum_provider_enablement_ticks
        || !enablement.effect_grants.is_empty()
        || enablement.approval.proposal.subject.artifact != Some(enablement.artifact)
        || enablement.approval.proposal.subject.budget != Some(enablement.provider)
        || enablement.approval.proposal.subject != context.subject
        || enablement.approval.proposal.effect_class
            != distribution.provider_enablement_effect_class
        || enablement.approval.policy.effect_class != distribution.provider_enablement_effect_class
        || enablement.approval.proposal.operation != distribution.provider_enablement_operation
        || enablement.evidence.kind != GenesisControlKind::ProviderEnabled
        || enablement.evidence.subject != enablement.provider.id
        || enablement.evidence.profile_identity != distribution.genesis_profile
        || enablement.evidence.authority != distribution.control_recorder
        || enablement.evidence.schema_version != GENESIS_CONTROL_SCHEMA_VERSION
        || enablement.evidence.sequence == 0
        || (enablement.evidence.sequence == 1) != enablement.evidence.predecessor.is_none()
        || enablement.evidence.time_basis != enablement.time_basis
        || enablement.evidence.observed_at_tick != enablement.enabled_at_tick
        || enablement.evidence.sequence > u64::from(distribution.maximum_evidence_events)
    {
        return Err(GenesisReason::ProviderEnablementInvalid);
    }
    let evidence_identity = enablement
        .evidence
        .computed_semantic_hash()
        .map_err(|_| GenesisReason::ProviderEnablementInvalid)?;
    if evidence_identity != enablement.evidence.identity
        || evidence_identity == enablement.evidence.receipt
        || enablement.evidence.predecessor == Some(enablement.evidence.identity)
    {
        return Err(GenesisReason::ProviderEnablementInvalid);
    }
    validate_administrative_proof(enablement.approval, context)
        .map_err(|_| GenesisReason::ProviderEnablementInvalid)
}

/// Failure recovery never widens. Restore, rollback, and emergency recovery
/// may select only an exact snapshot ceiling and require an independent
/// administrative proof; they still cannot create an ambient root or TOFU.
pub fn validate_recovery_transition(
    profile: RealmGenesisProfile<'_>,
    transition: RecoveryTransition<'_>,
    context: ContainmentContext<'_>,
) -> Result<(), GenesisReason> {
    let mut profile_scratch =
        [SemanticHash::from_bytes([0; 32]); MAX_BOOTSTRAP_CHANNELS + MAX_PUBLIC_OPERATIONS];
    validate_genesis_profile(profile, &mut profile_scratch)?;
    if transition.profile_identity != profile.identity
        || transition.candidate.ambient_root
        || transition.candidate.trust_on_first_use
        || transition.evidence.profile_identity != profile.identity
    {
        return Err(GenesisReason::RecoveryWidened);
    }
    validate_control_record(profile, transition.evidence)?;
    match transition.kind {
        RecoveryKind::FactoryReset | RecoveryKind::LostRoot | RecoveryKind::FailedRestore => {
            if !transition.candidate.isolated()
                || !transition.candidate.no_wider_than(transition.prior)
                || transition.approval.is_some()
                || transition.snapshot.is_some()
                || (transition.kind == RecoveryKind::FactoryReset
                    && transition.evidence.kind != GenesisControlKind::FactoryReset)
                || (transition.kind != RecoveryKind::FactoryReset
                    && transition.evidence.kind != GenesisControlKind::RecoveryDenied)
            {
                return Err(GenesisReason::RecoveryWidened);
            }
        }
        RecoveryKind::Restore | RecoveryKind::Rollback | RecoveryKind::Emergency => {
            let ceiling = transition
                .recovery_ceiling
                .ok_or(GenesisReason::RecoveryWidened)?;
            let proof = transition.approval.ok_or(GenesisReason::RecoveryWidened)?;
            let snapshot = transition.snapshot.ok_or(GenesisReason::RecoveryWidened)?;
            if !transition.candidate.no_wider_than(ceiling)
                || ceiling.ambient_root
                || ceiling.trust_on_first_use
                || (transition.kind == RecoveryKind::Emergency
                    && transition.candidate.root_authorities > 1)
                || transition.evidence.kind != GenesisControlKind::Restored
                || proof.proposal.effect_class != profile.recovery_effect_class
                || proof.policy.effect_class != profile.recovery_effect_class
                || proof.proposal.operation != profile.recovery_operation
                || proof.proposal.subject.plan != snapshot
                || proof.proposal.subject.budget != Some(profile.descriptor)
            {
                return Err(GenesisReason::RecoveryWidened);
            }
            validate_administrative_proof(proof, context)
                .map_err(|_| GenesisReason::RecoveryWidened)?;
        }
    }
    Ok(())
}

fn public_operation_hash(
    operation: PublicGenesisOperation<'_>,
) -> Result<SemanticHash, GenesisIdentityError> {
    let pin = pin_hash(operation.operation)?;
    descriptor_hash(
        "conduit/public-genesis-operation",
        &[
            semantic("operation", CanonicalValue::Bytes(pin.as_bytes())),
            semantic(
                "maximum_uses",
                CanonicalValue::Integer(i128::from(operation.maximum_uses)),
            ),
            semantic(
                "administrative",
                CanonicalValue::Boolean(operation.administrative),
            ),
            semantic("deployment", CanonicalValue::Boolean(operation.deployment)),
            semantic(
                "protected_subscription",
                CanonicalValue::Boolean(operation.protected_subscription),
            ),
            semantic("actuating", CanonicalValue::Boolean(operation.actuating)),
        ],
    )
    .map_err(GenesisIdentityError::Canonical)
}

fn distribution_provider_hash(
    provider: DistributionProvider<'_>,
) -> Result<SemanticHash, GenesisIdentityError> {
    let pin = pin_hash(provider.provider)?;
    let artifact = provider.artifact;
    let artifact = artifact.as_ref().map_or(CanonicalValue::Null, |value| {
        CanonicalValue::Bytes(value.as_bytes())
    });
    descriptor_hash(
        "conduit/distribution-provider",
        &[
            semantic("provider", CanonicalValue::Bytes(pin.as_bytes())),
            semantic("artifact", artifact),
            semantic(
                "availability",
                CanonicalValue::Identifier(Id(provider.availability.as_str())),
            ),
            semantic(
                "enrollment_issuer",
                CanonicalValue::Boolean(provider.traits.enrollment_issuer),
            ),
            semantic(
                "unrestricted_native_execution",
                CanonicalValue::Boolean(provider.traits.unrestricted_native_execution),
            ),
            semantic(
                "remote_artifact_installation",
                CanonicalValue::Boolean(provider.traits.remote_artifact_installation),
            ),
            semantic(
                "firmware_mutation",
                CanonicalValue::Boolean(provider.traits.firmware_mutation),
            ),
            semantic(
                "unrestricted_network",
                CanonicalValue::Boolean(provider.traits.unrestricted_network),
            ),
            semantic(
                "realm_root_administration",
                CanonicalValue::Boolean(provider.traits.realm_root_administration),
            ),
            semantic(
                "remote_plan_activation",
                CanonicalValue::Boolean(provider.traits.remote_plan_activation),
            ),
            semantic(
                "actuating_effects",
                CanonicalValue::Boolean(provider.traits.actuating_effects),
            ),
        ],
    )
    .map_err(GenesisIdentityError::Canonical)
}

fn pin_hash(pin: PinnedDescriptor<'_>) -> Result<SemanticHash, GenesisIdentityError> {
    descriptor_hash(
        "conduit/pin",
        &[
            semantic("id", CanonicalValue::Identifier(pin.id)),
            semantic(
                "schema_version",
                CanonicalValue::Integer(i128::from(pin.schema_version)),
            ),
            semantic(
                "semantic_hash",
                CanonicalValue::Bytes(pin.semantic_hash.as_bytes()),
            ),
        ],
    )
    .map_err(GenesisIdentityError::Canonical)
}

fn descriptor_hash(
    kind: &str,
    fields: &[MapField<'_>],
) -> Result<SemanticHash, CanonicalError<Infallible>> {
    CanonicalDescriptor {
        kind: Id(kind),
        schema_version: 1,
        body: CanonicalValue::Map(fields),
    }
    .semantic_hash()
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

fn has_duplicate_channels(channels: &[BootstrapChannel]) -> bool {
    channels
        .iter()
        .enumerate()
        .any(|(index, value)| channels[..index].contains(value))
}

fn has_duplicate_providers(providers: &[DistributionProvider<'_>]) -> bool {
    providers.iter().enumerate().any(|(index, provider)| {
        providers[..index]
            .iter()
            .any(|prior| prior.provider == provider.provider)
    })
}

fn identity_reason(error: GenesisIdentityError) -> GenesisReason {
    match error {
        GenesisIdentityError::ScratchTooSmall => GenesisReason::StorageBoundExceeded,
        GenesisIdentityError::Canonical(_) => GenesisReason::InvalidDescriptor,
    }
}

fn identity_canonical(error: GenesisIdentityError) -> CanonicalError<Infallible> {
    match error {
        GenesisIdentityError::Canonical(error) => error,
        GenesisIdentityError::ScratchTooSmall => CanonicalError::NestingTooDeep,
    }
}

/// Exact subject helper for hosted callers that need to bind a provider
/// enablement approval without reconstructing or weakening its subject.
#[must_use]
pub const fn provider_enablement_subject(
    proof: AdministrativeProof<'_>,
) -> AdministrativeSubject<'_> {
    proof.proposal.subject
}
