//! Independent inhibit-plane and hazardous-host contracts.
//!
//! This module describes portable, domain-neutral facts. It does not claim
//! that a process, signature, role, graph node, or conformance run is a
//! physical interlock. Hosts must enforce the pinned safe state and operating
//! envelope at the effect boundary outside ordinary plan execution.

use core::convert::Infallible;

use crate::canonical::semantic_hash_with_hash_set;
use crate::{
    AdministrativeProof, AdministrativeSubject, CanonicalDescriptor, CanonicalError,
    CanonicalValue, ContainmentContext, FieldDisposition, Id, MapField, PinnedDescriptor,
    SemanticHash, validate_administrative_proof,
};

pub const HAZARDOUS_HOST_PROFILE_SCHEMA_VERSION: u32 = 1;
pub const INHIBIT_OBSERVATION_SCHEMA_VERSION: u32 = 1;
pub const MAX_OPERATING_ENVELOPE_LIMITS: usize = 16;
pub const MAX_HAZARDOUS_HOST_BINDINGS: usize = 32;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OperatingEnvelopeLimit<'a> {
    /// Domain-owned quantity, such as a motion axis or energy class.
    pub dimension: PinnedDescriptor<'a>,
    pub minimum: i64,
    pub maximum: i64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HazardousHostProfile<'a> {
    pub schema_version: u32,
    pub identity: SemanticHash,
    pub descriptor: PinnedDescriptor<'a>,
    pub safe_state: PinnedDescriptor<'a>,
    pub inhibit_boundary: PinnedDescriptor<'a>,
    pub watchdog: PinnedDescriptor<'a>,
    pub effect_boundary: PinnedDescriptor<'a>,
    pub command_effect_class: PinnedDescriptor<'a>,
    pub clear_effect_class: PinnedDescriptor<'a>,
    pub clear_operation: PinnedDescriptor<'a>,
    pub clear_ceremony: PinnedDescriptor<'a>,
    pub time_basis: Id<'a>,
    pub maximum_command_horizon_ticks: u64,
    pub maximum_observation_age_ticks: u64,
    pub maximum_evidence_records: u32,
    pub require_physical_presence_to_clear: bool,
    pub require_isolated_implementation: bool,
    pub envelope: &'a [OperatingEnvelopeLimit<'a>],
}

impl HazardousHostProfile<'_> {
    pub fn computed_semantic_hash(
        &self,
        scratch: &mut [SemanticHash],
    ) -> Result<SemanticHash, InhibitIdentityError> {
        if scratch.len() < self.envelope.len() {
            return Err(InhibitIdentityError::ScratchTooSmall);
        }
        for (slot, limit) in scratch.iter_mut().zip(self.envelope) {
            *slot = envelope_hash(*limit).map_err(InhibitIdentityError::Canonical)?;
        }
        let descriptor = pin_hash(self.descriptor)?;
        let safe_state = pin_hash(self.safe_state)?;
        let inhibit_boundary = pin_hash(self.inhibit_boundary)?;
        let watchdog = pin_hash(self.watchdog)?;
        let effect_boundary = pin_hash(self.effect_boundary)?;
        let command_effect_class = pin_hash(self.command_effect_class)?;
        let clear_effect_class = pin_hash(self.clear_effect_class)?;
        let clear_operation = pin_hash(self.clear_operation)?;
        let clear_ceremony = pin_hash(self.clear_ceremony)?;
        let fields = [
            semantic("descriptor", CanonicalValue::Bytes(descriptor.as_bytes())),
            semantic("safe_state", CanonicalValue::Bytes(safe_state.as_bytes())),
            semantic(
                "inhibit_boundary",
                CanonicalValue::Bytes(inhibit_boundary.as_bytes()),
            ),
            semantic("watchdog", CanonicalValue::Bytes(watchdog.as_bytes())),
            semantic(
                "effect_boundary",
                CanonicalValue::Bytes(effect_boundary.as_bytes()),
            ),
            semantic(
                "command_effect_class",
                CanonicalValue::Bytes(command_effect_class.as_bytes()),
            ),
            semantic(
                "clear_effect_class",
                CanonicalValue::Bytes(clear_effect_class.as_bytes()),
            ),
            semantic(
                "clear_operation",
                CanonicalValue::Bytes(clear_operation.as_bytes()),
            ),
            semantic(
                "clear_ceremony",
                CanonicalValue::Bytes(clear_ceremony.as_bytes()),
            ),
            semantic("time_basis", CanonicalValue::Identifier(self.time_basis)),
            semantic(
                "maximum_command_horizon_ticks",
                CanonicalValue::Integer(i128::from(self.maximum_command_horizon_ticks)),
            ),
            semantic(
                "maximum_observation_age_ticks",
                CanonicalValue::Integer(i128::from(self.maximum_observation_age_ticks)),
            ),
            semantic(
                "maximum_evidence_records",
                CanonicalValue::Integer(i128::from(self.maximum_evidence_records)),
            ),
            semantic(
                "require_physical_presence_to_clear",
                CanonicalValue::Boolean(self.require_physical_presence_to_clear),
            ),
            semantic(
                "require_isolated_implementation",
                CanonicalValue::Boolean(self.require_isolated_implementation),
            ),
        ];
        semantic_hash_with_hash_set(
            Id("conduit/hazardous-host-profile"),
            self.schema_version,
            &fields,
            Id("envelope"),
            &scratch[..self.envelope.len()],
        )
        .map_err(InhibitIdentityError::Canonical)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ImplementationConfinement {
    EffectBoundaryEnforced,
    UnconfinedNative,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InhibitLatchState {
    SafeDisarmed,
    Inhibited,
}

/// A host observation is separate from the profile and from the plan. Its
/// identity captures what the host actually observed at the local boundary.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InhibitObservation<'a> {
    pub schema_version: u32,
    pub identity: SemanticHash,
    pub profile_identity: SemanticHash,
    pub host: Id<'a>,
    pub safe_state: PinnedDescriptor<'a>,
    pub inhibit_boundary: PinnedDescriptor<'a>,
    pub watchdog: PinnedDescriptor<'a>,
    pub effect_boundary: PinnedDescriptor<'a>,
    pub time_basis: Id<'a>,
    pub observed_at_tick: u64,
    pub valid_until_tick: u64,
    pub latch_generation: u64,
    pub latch_state: InhibitLatchState,
    pub independent_from_plan: bool,
    pub local_safe_path: bool,
    pub survives_executor_loss: bool,
    pub survives_partition: bool,
    pub graph_cannot_replace: bool,
    pub confinement: ImplementationConfinement,
}

impl InhibitObservation<'_> {
    pub fn computed_semantic_hash(&self) -> Result<SemanticHash, CanonicalError<Infallible>> {
        let safe_state = pin_hash(self.safe_state)?;
        let inhibit_boundary = pin_hash(self.inhibit_boundary)?;
        let watchdog = pin_hash(self.watchdog)?;
        let effect_boundary = pin_hash(self.effect_boundary)?;
        let fields = [
            semantic(
                "profile_identity",
                CanonicalValue::Bytes(self.profile_identity.as_bytes()),
            ),
            semantic("host", CanonicalValue::Identifier(self.host)),
            semantic("safe_state", CanonicalValue::Bytes(safe_state.as_bytes())),
            semantic(
                "inhibit_boundary",
                CanonicalValue::Bytes(inhibit_boundary.as_bytes()),
            ),
            semantic("watchdog", CanonicalValue::Bytes(watchdog.as_bytes())),
            semantic(
                "effect_boundary",
                CanonicalValue::Bytes(effect_boundary.as_bytes()),
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
            semantic(
                "latch_generation",
                CanonicalValue::Integer(i128::from(self.latch_generation)),
            ),
            semantic(
                "latch_state",
                CanonicalValue::Identifier(Id(match self.latch_state {
                    InhibitLatchState::SafeDisarmed => "safe-disarmed",
                    InhibitLatchState::Inhibited => "inhibited",
                })),
            ),
            semantic(
                "independent_from_plan",
                CanonicalValue::Boolean(self.independent_from_plan),
            ),
            semantic(
                "local_safe_path",
                CanonicalValue::Boolean(self.local_safe_path),
            ),
            semantic(
                "survives_executor_loss",
                CanonicalValue::Boolean(self.survives_executor_loss),
            ),
            semantic(
                "survives_partition",
                CanonicalValue::Boolean(self.survives_partition),
            ),
            semantic(
                "graph_cannot_replace",
                CanonicalValue::Boolean(self.graph_cannot_replace),
            ),
            semantic(
                "confinement",
                CanonicalValue::Identifier(Id(match self.confinement {
                    ImplementationConfinement::EffectBoundaryEnforced => "effect-boundary-enforced",
                    ImplementationConfinement::UnconfinedNative => "unconfined-native",
                })),
            ),
        ];
        descriptor_hash("conduit/inhibit-observation", self.schema_version, &fields)
    }
}

/// Exact profile and observation retained in a runnable plan. The observation
/// remains independently refreshable and does not become a plan-owned service.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HazardousHostBinding<'a> {
    pub host: Id<'a>,
    pub profile: HazardousHostProfile<'a>,
    pub observation: InhibitObservation<'a>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HazardControlPhase {
    SafeDisarmed,
    Armed,
    Inhibited,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HazardControlState {
    pub phase: HazardControlPhase,
    pub profile_identity: SemanticHash,
    pub safe_state_identity: SemanticHash,
    pub plan: SemanticHash,
    pub epoch: u64,
    pub command_authority: SemanticHash,
    pub next_sequence: u64,
    pub active_until_tick: u64,
    pub latch_generation: u64,
    pub latch_identity: SemanticHash,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HazardArmRequest<'a> {
    pub plan: SemanticHash,
    pub epoch: u64,
    pub command_authority: SemanticHash,
    pub time_basis: Id<'a>,
    pub now_tick: u64,
}

impl HazardControlState {
    #[must_use]
    pub const fn safe_disarmed(latch_generation: u64, latch_identity: SemanticHash) -> Self {
        Self {
            phase: HazardControlPhase::SafeDisarmed,
            profile_identity: SemanticHash::from_bytes([0; 32]),
            safe_state_identity: SemanticHash::from_bytes([0; 32]),
            plan: SemanticHash::from_bytes([0; 32]),
            epoch: 0,
            command_authority: SemanticHash::from_bytes([0; 32]),
            next_sequence: 0,
            active_until_tick: 0,
            latch_generation,
            latch_identity,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HazardousCommand<'a> {
    pub plan: SemanticHash,
    pub epoch: u64,
    pub authority: SemanticHash,
    pub sequence: u64,
    pub time_basis: Id<'a>,
    pub issued_at_tick: u64,
    pub expires_at_tick: u64,
    pub values: &'a [EnvelopeValue<'a>],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct EnvelopeValue<'a> {
    pub dimension: PinnedDescriptor<'a>,
    pub value: i64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InhibitCause {
    StopRequest,
    LeaseExpired,
    CommandLost,
    HostLost,
    SensorStale,
    Watchdog,
    Partition,
    AuthorityRevoked,
    PlanTransition,
    ImplementationFailed,
    EvidenceFailed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostLifecycleChange {
    PlanReplacement,
    Rollback,
    Reboot,
    FirmwareUpdate,
    Reconnect,
    RealmRecovery,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct InhibitClearRequest<'a> {
    pub profile_identity: SemanticHash,
    pub host: Id<'a>,
    pub latch_identity: SemanticHash,
    pub latch_generation: u64,
    pub subject: AdministrativeSubject<'a>,
    /// Exact current host receipt for the selected physical ceremony. This is
    /// evidence supplied by the host, not proof that core observed hardware.
    pub physical_presence_receipt: Option<SemanticHash>,
    pub proof: AdministrativeProof<'a>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HazardEvidenceKind {
    Armed,
    LeaseAccepted,
    CommandAccepted,
    CommandRejected(InhibitReason),
    EnvelopeLimited,
    Inhibited(InhibitCause),
    SafeStateEntered,
    ClearAttempted,
    ClearApproved,
    RecoveredSafeDisarmed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct HazardEvidenceRecord<'a> {
    pub identity: SemanticHash,
    pub sequence: u64,
    pub predecessor: Option<SemanticHash>,
    pub profile_identity: SemanticHash,
    pub host: Id<'a>,
    pub plan: SemanticHash,
    pub epoch: u64,
    pub kind: HazardEvidenceKind,
    pub time_basis: Id<'a>,
    pub observed_at_tick: u64,
    pub receipt: SemanticHash,
}

impl HazardEvidenceRecord<'_> {
    pub fn computed_semantic_hash(&self) -> Result<SemanticHash, CanonicalError<Infallible>> {
        let (kind, detail) = match self.kind {
            HazardEvidenceKind::Armed => ("armed", None),
            HazardEvidenceKind::LeaseAccepted => ("lease-accepted", None),
            HazardEvidenceKind::CommandAccepted => ("command-accepted", None),
            HazardEvidenceKind::CommandRejected(reason) => {
                ("command-rejected", Some(reason.code()))
            }
            HazardEvidenceKind::EnvelopeLimited => ("envelope-limited", None),
            HazardEvidenceKind::Inhibited(cause) => ("inhibited", Some(cause.as_str())),
            HazardEvidenceKind::SafeStateEntered => ("safe-state-entered", None),
            HazardEvidenceKind::ClearAttempted => ("clear-attempted", None),
            HazardEvidenceKind::ClearApproved => ("clear-approved", None),
            HazardEvidenceKind::RecoveredSafeDisarmed => ("recovered-safe-disarmed", None),
        };
        let fields = [
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
            semantic("host", CanonicalValue::Identifier(self.host)),
            semantic("plan", CanonicalValue::Bytes(self.plan.as_bytes())),
            semantic("epoch", CanonicalValue::Integer(i128::from(self.epoch))),
            semantic("kind", CanonicalValue::Identifier(Id(kind))),
            semantic(
                "detail",
                detail.map_or(CanonicalValue::Null, |value| {
                    CanonicalValue::Identifier(Id(value))
                }),
            ),
            semantic("time_basis", CanonicalValue::Identifier(self.time_basis)),
            semantic(
                "observed_at_tick",
                CanonicalValue::Integer(i128::from(self.observed_at_tick)),
            ),
            semantic("receipt", CanonicalValue::Bytes(self.receipt.as_bytes())),
        ];
        descriptor_hash("conduit/hazard-evidence-record", 1, &fields)
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum InhibitReason {
    UnsupportedVersion,
    InvalidDescriptor,
    IdentityMismatch,
    ObservationAbsentOrStale,
    IndependentBoundaryMissing,
    ImplementationNotConfined,
    NotSafeToArm,
    CommandLeaseInvalid,
    CommandBindingMismatch,
    CommandSequenceInvalid,
    EnvelopeExceeded,
    Inhibited,
    ClearCeremonyInvalid,
    TransitionCannotClear,
    EvidenceInvalidOrExhausted,
}

impl InhibitReason {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::UnsupportedVersion => "CND-INH-001",
            Self::InvalidDescriptor => "CND-INH-002",
            Self::IdentityMismatch => "CND-INH-003",
            Self::ObservationAbsentOrStale => "CND-INH-004",
            Self::IndependentBoundaryMissing => "CND-INH-005",
            Self::ImplementationNotConfined => "CND-INH-006",
            Self::NotSafeToArm => "CND-INH-007",
            Self::CommandLeaseInvalid => "CND-INH-008",
            Self::CommandBindingMismatch => "CND-INH-009",
            Self::CommandSequenceInvalid => "CND-INH-010",
            Self::EnvelopeExceeded => "CND-INH-011",
            Self::Inhibited => "CND-INH-012",
            Self::ClearCeremonyInvalid => "CND-INH-013",
            Self::TransitionCannotClear => "CND-INH-014",
            Self::EvidenceInvalidOrExhausted => "CND-INH-015",
        }
    }
}

impl InhibitCause {
    const fn as_str(self) -> &'static str {
        match self {
            Self::StopRequest => "stop-request",
            Self::LeaseExpired => "lease-expired",
            Self::CommandLost => "command-lost",
            Self::HostLost => "host-lost",
            Self::SensorStale => "sensor-stale",
            Self::Watchdog => "watchdog",
            Self::Partition => "partition",
            Self::AuthorityRevoked => "authority-revoked",
            Self::PlanTransition => "plan-transition",
            Self::ImplementationFailed => "implementation-failed",
            Self::EvidenceFailed => "evidence-failed",
        }
    }
}

#[derive(Debug, Eq, PartialEq)]
pub enum InhibitIdentityError {
    ScratchTooSmall,
    Canonical(CanonicalError<Infallible>),
}

impl From<CanonicalError<Infallible>> for InhibitIdentityError {
    fn from(error: CanonicalError<Infallible>) -> Self {
        Self::Canonical(error)
    }
}

pub fn validate_hazardous_host_profile(
    profile: HazardousHostProfile<'_>,
    scratch: &mut [SemanticHash],
) -> Result<(), InhibitReason> {
    if profile.schema_version != HAZARDOUS_HOST_PROFILE_SCHEMA_VERSION {
        return Err(InhibitReason::UnsupportedVersion);
    }
    if !valid_pin(profile.descriptor)
        || !valid_pin(profile.safe_state)
        || !valid_pin(profile.inhibit_boundary)
        || !valid_pin(profile.watchdog)
        || !valid_pin(profile.effect_boundary)
        || !valid_pin(profile.command_effect_class)
        || !valid_pin(profile.clear_effect_class)
        || !valid_pin(profile.clear_operation)
        || !valid_pin(profile.clear_ceremony)
        || !valid_id(profile.time_basis)
        || profile.maximum_command_horizon_ticks == 0
        || profile.maximum_observation_age_ticks == 0
        || profile.maximum_evidence_records == 0
        || profile.envelope.is_empty()
        || profile.envelope.len() > MAX_OPERATING_ENVELOPE_LIMITS
        || profile.envelope.iter().enumerate().any(|(index, limit)| {
            !valid_pin(limit.dimension)
                || limit.minimum > limit.maximum
                || profile.envelope[..index]
                    .iter()
                    .any(|prior| prior.dimension == limit.dimension)
        })
    {
        return Err(InhibitReason::InvalidDescriptor);
    }
    let computed = profile
        .computed_semantic_hash(scratch)
        .map_err(|_| InhibitReason::InvalidDescriptor)?;
    if computed != profile.identity {
        return Err(InhibitReason::IdentityMismatch);
    }
    Ok(())
}

/// Validate current host evidence for resolution or again at run start.
pub fn validate_hazardous_host_binding(
    binding: HazardousHostBinding<'_>,
    now_basis: Id<'_>,
    now_tick: u64,
    scratch: &mut [SemanticHash],
) -> Result<(), InhibitReason> {
    validate_hazardous_host_profile(binding.profile, scratch)?;
    let observation = binding.observation;
    if observation.schema_version != INHIBIT_OBSERVATION_SCHEMA_VERSION
        || !valid_id(binding.host)
        || !valid_id(observation.host)
        || !valid_id(observation.time_basis)
        || observation.valid_until_tick <= observation.observed_at_tick
        || observation.latch_generation == 0
    {
        return Err(InhibitReason::InvalidDescriptor);
    }
    if observation.identity
        != observation
            .computed_semantic_hash()
            .map_err(|_| InhibitReason::InvalidDescriptor)?
        || observation.profile_identity != binding.profile.identity
        || observation.host != binding.host
        || observation.safe_state != binding.profile.safe_state
        || observation.inhibit_boundary != binding.profile.inhibit_boundary
        || observation.watchdog != binding.profile.watchdog
        || observation.effect_boundary != binding.profile.effect_boundary
    {
        return Err(InhibitReason::IdentityMismatch);
    }
    if observation.time_basis != binding.profile.time_basis
        || observation.time_basis != now_basis
        || now_tick < observation.observed_at_tick
        || now_tick >= observation.valid_until_tick
        || now_tick - observation.observed_at_tick > binding.profile.maximum_observation_age_ticks
    {
        return Err(InhibitReason::ObservationAbsentOrStale);
    }
    if !observation.independent_from_plan
        || !observation.local_safe_path
        || !observation.survives_executor_loss
        || !observation.survives_partition
        || !observation.graph_cannot_replace
    {
        return Err(InhibitReason::IndependentBoundaryMissing);
    }
    if binding.profile.require_isolated_implementation
        && observation.confinement != ImplementationConfinement::EffectBoundaryEnforced
    {
        return Err(InhibitReason::ImplementationNotConfined);
    }
    Ok(())
}

pub fn validate_required_hazardous_host_binding(
    binding: Option<HazardousHostBinding<'_>>,
    now_basis: Id<'_>,
    now_tick: u64,
    scratch: &mut [SemanticHash],
) -> Result<(), InhibitReason> {
    let binding = binding.ok_or(InhibitReason::ObservationAbsentOrStale)?;
    validate_hazardous_host_binding(binding, now_basis, now_tick, scratch)
}

pub fn arm_hazardous_host(
    binding: HazardousHostBinding<'_>,
    state: HazardControlState,
    request: HazardArmRequest<'_>,
    scratch: &mut [SemanticHash],
) -> Result<HazardControlState, InhibitReason> {
    validate_hazardous_host_binding(binding, request.time_basis, request.now_tick, scratch)?;
    if state.phase != HazardControlPhase::SafeDisarmed
        || binding.observation.latch_state != InhibitLatchState::SafeDisarmed
        || state.latch_generation != binding.observation.latch_generation
        || request.epoch == 0
        || request.plan == zero_hash()
        || request.command_authority == zero_hash()
    {
        return Err(InhibitReason::NotSafeToArm);
    }
    Ok(HazardControlState {
        phase: HazardControlPhase::Armed,
        profile_identity: binding.profile.identity,
        safe_state_identity: binding.profile.safe_state.semantic_hash,
        plan: request.plan,
        epoch: request.epoch,
        command_authority: request.command_authority,
        next_sequence: 1,
        active_until_tick: request.now_tick,
        ..state
    })
}

pub fn accept_hazardous_command(
    profile: HazardousHostProfile<'_>,
    state: HazardControlState,
    command: HazardousCommand<'_>,
    now_tick: u64,
) -> Result<HazardControlState, InhibitReason> {
    if state.phase == HazardControlPhase::Inhibited {
        return Err(InhibitReason::Inhibited);
    }
    if state.phase != HazardControlPhase::Armed {
        return Err(InhibitReason::NotSafeToArm);
    }
    if command.plan != state.plan
        || command.epoch != state.epoch
        || command.authority != state.command_authority
        || command.time_basis != profile.time_basis
    {
        return Err(InhibitReason::CommandBindingMismatch);
    }
    if command.sequence != state.next_sequence {
        return Err(InhibitReason::CommandSequenceInvalid);
    }
    if command.issued_at_tick > now_tick
        || now_tick >= command.expires_at_tick
        || command.expires_at_tick <= command.issued_at_tick
        || command.expires_at_tick - command.issued_at_tick > profile.maximum_command_horizon_ticks
    {
        return Err(InhibitReason::CommandLeaseInvalid);
    }
    if command.values.len() != profile.envelope.len() {
        return Err(InhibitReason::EnvelopeExceeded);
    }
    for (index, value) in command.values.iter().enumerate() {
        let Some(limit) = profile
            .envelope
            .iter()
            .find(|limit| limit.dimension == value.dimension)
        else {
            return Err(InhibitReason::EnvelopeExceeded);
        };
        if value.value < limit.minimum
            || value.value > limit.maximum
            || command.values[..index]
                .iter()
                .any(|prior| prior.dimension == value.dimension)
        {
            return Err(InhibitReason::EnvelopeExceeded);
        }
    }
    Ok(HazardControlState {
        next_sequence: state.next_sequence.saturating_add(1),
        active_until_tick: command.expires_at_tick,
        ..state
    })
}

/// Local stop/failure is intentionally narrower than re-enable: it requires no
/// administrative proof and can only remove capability.
#[must_use]
pub const fn inhibit_hazardous_host(
    state: HazardControlState,
    latch_identity: SemanticHash,
    _cause: InhibitCause,
) -> HazardControlState {
    if matches!(state.phase, HazardControlPhase::Inhibited) {
        return state;
    }
    HazardControlState {
        phase: HazardControlPhase::Inhibited,
        profile_identity: state.profile_identity,
        safe_state_identity: state.safe_state_identity,
        plan: SemanticHash::from_bytes([0; 32]),
        epoch: 0,
        command_authority: SemanticHash::from_bytes([0; 32]),
        next_sequence: 0,
        active_until_tick: 0,
        latch_generation: state.latch_generation.saturating_add(1),
        latch_identity,
    }
}

pub fn enforce_command_expiry(
    state: HazardControlState,
    now_tick: u64,
    latch_identity: SemanticHash,
) -> Result<HazardControlState, InhibitReason> {
    if state.phase != HazardControlPhase::Armed || now_tick < state.active_until_tick {
        return Err(InhibitReason::CommandLeaseInvalid);
    }
    Ok(inhibit_hazardous_host(
        state,
        latch_identity,
        InhibitCause::LeaseExpired,
    ))
}

/// Every lifecycle change retains an inhibit latch and drops any old command.
/// A non-inhibited host returns safe and disarmed; it never automatically arms.
#[must_use]
pub const fn recover_after_host_change(
    state: HazardControlState,
    _change: HostLifecycleChange,
) -> HazardControlState {
    if matches!(state.phase, HazardControlPhase::Inhibited) {
        state
    } else {
        HazardControlState {
            phase: HazardControlPhase::SafeDisarmed,
            plan: SemanticHash::from_bytes([0; 32]),
            epoch: 0,
            command_authority: SemanticHash::from_bytes([0; 32]),
            next_sequence: 0,
            active_until_tick: 0,
            ..state
        }
    }
}

pub fn clear_inhibit(
    profile: HazardousHostProfile<'_>,
    state: HazardControlState,
    request: InhibitClearRequest<'_>,
    now_tick: u64,
) -> Result<HazardControlState, InhibitReason> {
    if state.phase != HazardControlPhase::Inhibited
        || state.profile_identity != profile.identity
        || state.safe_state_identity != profile.safe_state.semantic_hash
        || request.profile_identity != profile.identity
        || request.subject.entity != request.host
        || request.latch_identity != state.latch_identity
        || request.latch_generation != state.latch_generation
        || !valid_id(request.host)
        || request.proof.proposal.effect_class != profile.clear_effect_class
        || request.proof.proposal.operation != profile.clear_operation
        || request.proof.proposal.ceremony != Some(profile.clear_ceremony)
        || request.proof.proposal.protected_handle != Some(profile.inhibit_boundary)
        || (profile.require_physical_presence_to_clear
            && request
                .physical_presence_receipt
                .is_none_or(|receipt| receipt == zero_hash()))
    {
        return Err(InhibitReason::ClearCeremonyInvalid);
    }
    validate_administrative_proof(
        request.proof,
        ContainmentContext {
            subject: request.subject,
            time_basis: profile.time_basis,
            now_tick,
        },
    )
    .map_err(|_| InhibitReason::ClearCeremonyInvalid)?;
    Ok(HazardControlState {
        phase: HazardControlPhase::SafeDisarmed,
        plan: SemanticHash::from_bytes([0; 32]),
        epoch: 0,
        command_authority: SemanticHash::from_bytes([0; 32]),
        next_sequence: 0,
        active_until_tick: 0,
        ..state
    })
}

pub fn validate_hazard_evidence(
    profile: HazardousHostProfile<'_>,
    record: HazardEvidenceRecord<'_>,
) -> Result<(), InhibitReason> {
    if record.sequence == 0
        || record.sequence > u64::from(profile.maximum_evidence_records)
        || (record.sequence == 1) != record.predecessor.is_none()
        || record.profile_identity != profile.identity
        || !valid_id(record.host)
        || record.time_basis != profile.time_basis
        || record.receipt == zero_hash()
        || record.predecessor == Some(record.receipt)
    {
        return Err(InhibitReason::EvidenceInvalidOrExhausted);
    }
    if record.identity
        != record
            .computed_semantic_hash()
            .map_err(|_| InhibitReason::EvidenceInvalidOrExhausted)?
        || record.identity == record.receipt
        || record.predecessor == Some(record.identity)
    {
        return Err(InhibitReason::EvidenceInvalidOrExhausted);
    }
    Ok(())
}

fn envelope_hash(
    limit: OperatingEnvelopeLimit<'_>,
) -> Result<SemanticHash, CanonicalError<Infallible>> {
    let dimension = pin_hash(limit.dimension)?;
    let fields = [
        semantic("dimension", CanonicalValue::Bytes(dimension.as_bytes())),
        semantic(
            "minimum",
            CanonicalValue::Integer(i128::from(limit.minimum)),
        ),
        semantic(
            "maximum",
            CanonicalValue::Integer(i128::from(limit.maximum)),
        ),
    ];
    descriptor_hash("conduit/operating-envelope-limit", 1, &fields)
}

fn pin_hash(descriptor: PinnedDescriptor<'_>) -> Result<SemanticHash, CanonicalError<Infallible>> {
    let fields = [
        semantic("id", CanonicalValue::Identifier(descriptor.id)),
        semantic(
            "schema_version",
            CanonicalValue::Integer(i128::from(descriptor.schema_version)),
        ),
        semantic(
            "semantic_hash",
            CanonicalValue::Bytes(descriptor.semantic_hash.as_bytes()),
        ),
    ];
    descriptor_hash("conduit/pinned-descriptor", 1, &fields)
}

fn descriptor_hash(
    name: &'static str,
    version: u32,
    fields: &[MapField<'_>],
) -> Result<SemanticHash, CanonicalError<Infallible>> {
    CanonicalDescriptor {
        kind: Id(name),
        schema_version: version,
        body: CanonicalValue::Map(fields),
    }
    .semantic_hash()
}

const fn semantic<'a>(name: &'a str, value: CanonicalValue<'a>) -> MapField<'a> {
    MapField {
        name: Id(name),
        disposition: FieldDisposition::Semantic,
        value,
    }
}

fn valid_id(value: Id<'_>) -> bool {
    Id::new(value.as_str()).is_ok()
}

fn valid_pin(value: PinnedDescriptor<'_>) -> bool {
    valid_id(value.id) && value.schema_version > 0
}

const fn zero_hash() -> SemanticHash {
    SemanticHash::from_bytes([0; 32])
}
