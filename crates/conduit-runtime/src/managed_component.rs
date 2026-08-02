//! Optional managed-component lifecycle facet for exact hosted implementations.
//!
//! This module is deliberately an observation and authorized-request protocol
//! around the existing hosted provider adapter. It does not execute provider
//! work, acquire authority, discover implementations, or own a second state
//! machine for run, work, host, or plan-transition lifecycle.

use std::collections::{BTreeSet, VecDeque};
use std::fmt;

use serde::{Deserialize, Serialize};

use conduit_core::{
    CanonicalDescriptor, CanonicalValue, FieldDisposition, Id, MapField, SemanticHash,
};

pub const MANAGED_COMPONENT_SCHEMA_VERSION: u32 = 0;
pub const MANAGED_COMPONENT_INTERFACE_ID: &str = "conduit.lifecycle/managed-component";

/// Canonical semantic identity shared by every adapter that speaks the same
/// managed-component request/observation/evidence protocol. Provider profile
/// descriptors remain separate and may differ by provable facets or boundary.
#[must_use]
pub fn managed_component_interface_hash() -> SemanticHash {
    let fields = [
        semantic(
            "observation",
            CanonicalValue::Text("exact-component-state-readiness-cleanup-freshness"),
        ),
        semantic(
            "request",
            CanonicalValue::Text("authorized-epoch-generation-sequence-deadline-fencing"),
        ),
        semantic(
            "evidence",
            CanonicalValue::Text("bounded-request-progress-commit-failure-retirement"),
        ),
    ];
    CanonicalDescriptor {
        kind: Id(MANAGED_COMPONENT_INTERFACE_ID),
        schema_version: MANAGED_COMPONENT_SCHEMA_VERSION,
        body: CanonicalValue::Map(&fields),
    }
    .semantic_hash()
    .expect("managed-component interface uses static canonical facts")
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ManagedAdapterBoundary {
    Native,
    Wasm,
    SupervisedProcess,
    FfiFirmware,
    Remote,
    Deterministic,
}

impl ManagedAdapterBoundary {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Native => "native",
            Self::Wasm => "wasm",
            Self::SupervisedProcess => "supervised-process",
            Self::FfiFirmware => "ffi-firmware",
            Self::Remote => "remote",
            Self::Deterministic => "deterministic",
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ManagedLifecycleAction {
    Prepare,
    Activate,
    Quiesce,
    Deactivate,
    Clean,
    Stop,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ManagedLifecycleState {
    Configured,
    Prepared,
    Active,
    Quiescing,
    Inactive,
    Cleaning,
    Stopped,
    Failed,
    Unsupported,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ManagedRuntimeReadiness {
    Ready,
    Waiting,
    NotApplicable,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ManagedCleanupState {
    NotRequired,
    Required,
    InProgress,
    Complete,
    Failed,
    TimedOut,
    Unprovable,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ManagedLifecycleReason {
    Configured,
    RequestAccepted,
    Progress,
    Prepared,
    Activated,
    AdmissionClosed,
    Quiesced,
    CleanupStarted,
    CleanupComplete,
    RequestCancelled,
    UnsupportedFacet,
    WrongState,
    DuplicateRequest,
    StaleRequest,
    StalePlanEpoch,
    WrongGeneration,
    UnavailableImplementation,
    StaleHostFact,
    DeniedGrant,
    RevokedGrant,
    ResourceConflict,
    ExpiredLease,
    PreparationFailed,
    DrainDeadline,
    Cancelled,
    ProviderLost,
    HostLost,
    CleanupFailed,
    CleanupTimeout,
    InhibitAsserted,
    PlanReplaced,
    RetiredGenerationWake,
    ActivationFailed,
}

impl ManagedLifecycleReason {
    #[must_use]
    pub const fn code(self) -> &'static str {
        match self {
            Self::Configured => "CND-MCL-000",
            Self::RequestAccepted => "CND-MCL-001",
            Self::Progress => "CND-MCL-002",
            Self::Prepared => "CND-MCL-003",
            Self::Activated => "CND-MCL-004",
            Self::AdmissionClosed => "CND-MCL-005",
            Self::Quiesced => "CND-MCL-006",
            Self::CleanupStarted => "CND-MCL-007",
            Self::CleanupComplete => "CND-MCL-008",
            Self::RequestCancelled => "CND-MCL-009",
            Self::UnsupportedFacet => "CND-MCL-010",
            Self::WrongState => "CND-MCL-011",
            Self::DuplicateRequest => "CND-MCL-012",
            Self::StaleRequest => "CND-MCL-013",
            Self::StalePlanEpoch => "CND-MCL-014",
            Self::WrongGeneration => "CND-MCL-015",
            Self::UnavailableImplementation => "CND-MCL-016",
            Self::StaleHostFact => "CND-MCL-017",
            Self::DeniedGrant => "CND-MCL-018",
            Self::RevokedGrant => "CND-MCL-019",
            Self::ResourceConflict => "CND-MCL-020",
            Self::ExpiredLease => "CND-MCL-021",
            Self::PreparationFailed => "CND-MCL-022",
            Self::DrainDeadline => "CND-MCL-023",
            Self::Cancelled => "CND-MCL-024",
            Self::ProviderLost => "CND-MCL-025",
            Self::HostLost => "CND-MCL-026",
            Self::CleanupFailed => "CND-MCL-027",
            Self::CleanupTimeout => "CND-MCL-028",
            Self::InhibitAsserted => "CND-MCL-029",
            Self::PlanReplaced => "CND-MCL-030",
            Self::RetiredGenerationWake => "CND-MCL-031",
            Self::ActivationFailed => "CND-MCL-035",
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedLifecycleFacets {
    pub prepare: bool,
    pub activate: bool,
    pub quiesce: bool,
    pub retained_prepared_state: bool,
    pub cleanup: bool,
    pub bounded_cancellation: bool,
    pub progress: bool,
}

impl ManagedLifecycleFacets {
    #[must_use]
    pub const fn full() -> Self {
        Self {
            prepare: true,
            activate: true,
            quiesce: true,
            retained_prepared_state: true,
            cleanup: true,
            bounded_cancellation: true,
            progress: true,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedComponentDescriptor {
    pub schema_version: u32,
    pub identity: String,
    pub id: String,
    pub boundary: ManagedAdapterBoundary,
    pub facets: ManagedLifecycleFacets,
    pub maximum_retained_events: u32,
    pub maximum_progress_events: u32,
    pub maximum_request_ticks: u64,
    pub sensitivity: String,
}

impl ManagedComponentDescriptor {
    pub fn new(
        id: impl Into<String>,
        boundary: ManagedAdapterBoundary,
        facets: ManagedLifecycleFacets,
        maximum_retained_events: u32,
        maximum_progress_events: u32,
        maximum_request_ticks: u64,
        sensitivity: impl Into<String>,
    ) -> Result<Self, ManagedLifecycleError> {
        let mut descriptor = Self {
            schema_version: MANAGED_COMPONENT_SCHEMA_VERSION,
            identity: String::new(),
            id: id.into(),
            boundary,
            facets,
            maximum_retained_events,
            maximum_progress_events,
            maximum_request_ticks,
            sensitivity: sensitivity.into(),
        };
        descriptor.identity = descriptor.computed_identity()?;
        descriptor.validate()?;
        Ok(descriptor)
    }

    #[must_use]
    pub fn full_standing_service(boundary: ManagedAdapterBoundary) -> Self {
        Self::new(
            "conduit.lifecycle/managed-standing-service",
            boundary,
            ManagedLifecycleFacets::full(),
            64,
            32,
            1_000,
            "public-operational",
        )
        .expect("built-in managed standing-service descriptor is valid")
    }

    #[must_use]
    pub fn leased_provider(boundary: ManagedAdapterBoundary) -> Self {
        Self::new(
            "conduit.lifecycle/managed-leased-provider",
            boundary,
            ManagedLifecycleFacets::full(),
            64,
            32,
            1_000,
            "restricted-resource-identities",
        )
        .expect("built-in managed leased-provider descriptor is valid")
    }

    pub fn validate(&self) -> Result<(), ManagedLifecycleError> {
        if self.schema_version != MANAGED_COMPONENT_SCHEMA_VERSION
            || self.id.is_empty()
            || self.sensitivity.is_empty()
            || self.maximum_retained_events == 0
            || self.maximum_progress_events > self.maximum_retained_events
            || self.maximum_request_ticks == 0
            || (!self.facets.prepare
                && !self.facets.activate
                && !self.facets.quiesce
                && !self.facets.cleanup)
            || (self.facets.activate && !self.facets.prepare)
            || (self.facets.retained_prepared_state && !self.facets.quiesce)
            || (self.facets.bounded_cancellation && (!self.facets.quiesce || !self.facets.cleanup))
        {
            return Err(ManagedLifecycleError::new(
                "CND-MCL-032",
                ManagedLifecycleReason::UnsupportedFacet,
                "managed lifecycle descriptor is malformed or proves no facet",
            ));
        }
        let computed = self.computed_identity()?;
        if self.identity != computed {
            return Err(ManagedLifecycleError::new(
                "CND-MCL-033",
                ManagedLifecycleReason::StaleRequest,
                "managed lifecycle descriptor identity does not match its facts",
            ));
        }
        Ok(())
    }

    pub fn computed_identity(&self) -> Result<String, ManagedLifecycleError> {
        Ok(self.semantic_hash()?.to_string())
    }

    pub fn semantic_hash(&self) -> Result<SemanticHash, ManagedLifecycleError> {
        let id = Id::new(&self.id).map_err(|_| {
            ManagedLifecycleError::new(
                "CND-MCL-032",
                ManagedLifecycleReason::UnsupportedFacet,
                "managed lifecycle descriptor id is invalid",
            )
        })?;
        let fields = [
            semantic("id", CanonicalValue::Identifier(id)),
            semantic(
                "boundary",
                CanonicalValue::Identifier(Id(self.boundary.as_str())),
            ),
            semantic("prepare", CanonicalValue::Boolean(self.facets.prepare)),
            semantic("activate", CanonicalValue::Boolean(self.facets.activate)),
            semantic("quiesce", CanonicalValue::Boolean(self.facets.quiesce)),
            semantic(
                "retained_prepared_state",
                CanonicalValue::Boolean(self.facets.retained_prepared_state),
            ),
            semantic("cleanup", CanonicalValue::Boolean(self.facets.cleanup)),
            semantic(
                "bounded_cancellation",
                CanonicalValue::Boolean(self.facets.bounded_cancellation),
            ),
            semantic("progress", CanonicalValue::Boolean(self.facets.progress)),
            semantic(
                "maximum_retained_events",
                CanonicalValue::Integer(i128::from(self.maximum_retained_events)),
            ),
            semantic(
                "maximum_progress_events",
                CanonicalValue::Integer(i128::from(self.maximum_progress_events)),
            ),
            semantic(
                "maximum_request_ticks",
                CanonicalValue::Integer(i128::from(self.maximum_request_ticks)),
            ),
            semantic("sensitivity", CanonicalValue::Text(&self.sensitivity)),
        ];
        CanonicalDescriptor {
            kind: Id("conduit/managed-component-descriptor"),
            schema_version: self.schema_version,
            body: CanonicalValue::Map(&fields),
        }
        .semantic_hash()
        .map_err(|_| {
            ManagedLifecycleError::new(
                "CND-MCL-032",
                ManagedLifecycleReason::UnsupportedFacet,
                "managed lifecycle descriptor cannot be canonically encoded",
            )
        })
    }

    #[must_use]
    pub fn supports(&self, action: ManagedLifecycleAction) -> bool {
        match action {
            ManagedLifecycleAction::Prepare => self.facets.prepare,
            ManagedLifecycleAction::Activate => self.facets.activate,
            ManagedLifecycleAction::Quiesce => self.facets.quiesce,
            ManagedLifecycleAction::Deactivate => {
                self.facets.quiesce && self.facets.retained_prepared_state
            }
            ManagedLifecycleAction::Clean => self.facets.cleanup,
            ManagedLifecycleAction::Stop => {
                self.facets.quiesce && self.facets.cleanup && self.facets.bounded_cancellation
            }
        }
    }
}

const fn semantic<'a>(name: &'a str, value: CanonicalValue<'a>) -> MapField<'a> {
    MapField {
        name: Id(name),
        value,
        disposition: FieldDisposition::Semantic,
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedArtifactIdentity {
    pub id: String,
    pub digest: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedComponentIdentity {
    pub component: String,
    pub semantic_contract: String,
    pub implementation_id: String,
    pub implementation_version: String,
    pub implementation_identity: String,
    pub artifacts: Vec<ManagedArtifactIdentity>,
    pub host_id: String,
    pub host_boot_id: String,
    pub host_observation_id: String,
    pub run_id: String,
    pub plan_identity: String,
    pub plan_epoch: u64,
    pub activation_generation: u64,
    pub resources: Vec<String>,
    pub grants: Vec<String>,
    pub leases: Vec<String>,
}

impl ManagedComponentIdentity {
    pub fn validate(&self) -> Result<(), ManagedLifecycleError> {
        let required = [
            &self.component,
            &self.semantic_contract,
            &self.implementation_id,
            &self.implementation_version,
            &self.implementation_identity,
            &self.host_id,
            &self.host_boot_id,
            &self.host_observation_id,
            &self.run_id,
            &self.plan_identity,
        ];
        if required.iter().any(|value| value.is_empty())
            || self.activation_generation == 0
            || self.artifacts.is_empty()
            || self
                .artifacts
                .iter()
                .any(|artifact| artifact.id.is_empty() || artifact.digest.is_empty())
            || has_duplicates(self.artifacts.iter().map(|artifact| artifact.id.as_str()))
            || has_duplicates(self.resources.iter().map(String::as_str))
            || has_duplicates(self.grants.iter().map(String::as_str))
            || has_duplicates(self.leases.iter().map(String::as_str))
        {
            return Err(ManagedLifecycleError::new(
                "CND-MCL-034",
                ManagedLifecycleReason::WrongGeneration,
                "managed component identity is incomplete or ambiguous",
            ));
        }
        Ok(())
    }
}

fn has_duplicates<'a>(values: impl Iterator<Item = &'a str>) -> bool {
    let mut seen = BTreeSet::new();
    values.into_iter().any(|value| !seen.insert(value))
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedLifecycleRequest {
    pub schema_version: u32,
    pub request_id: String,
    pub component: String,
    pub action: ManagedLifecycleAction,
    pub expected_plan_epoch: u64,
    pub expected_activation_generation: u64,
    pub expected_observation_sequence: u64,
    pub issued_at_tick: u64,
    pub deadline_tick: u64,
    pub causation: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedLifecycleAuthority {
    pub requester: String,
    /// Exact identity of the external authority admitting this request. It
    /// may be a grant, an admitted run controller, or another host policy
    /// authority; the component never manufactures it.
    pub authority_id: String,
    pub provider: ManagedProviderAvailability,
    pub grant: ManagedGrantState,
    pub resources: ManagedResourceState,
    pub leases: ManagedLeaseState,
    pub not_before_tick: u64,
    pub expires_at_tick: u64,
    pub actions: Vec<ManagedLifecycleAction>,
    pub inhibit_asserted: bool,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ManagedProviderAvailability {
    Available,
    Unavailable,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ManagedGrantState {
    Active,
    Denied,
    Revoked,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ManagedResourceState {
    Available,
    Conflict,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ManagedLeaseState {
    Current,
    Expired,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedLifecycleProgress {
    pub completed_units: u64,
    pub total_units: Option<u64>,
    pub detail: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case", tag = "kind")]
pub enum ManagedProviderEvent {
    Prepared {
        resource_evidence: Vec<String>,
    },
    Activated,
    AdmissionClosed {
        in_flight: u32,
    },
    Quiesced {
        drained: u32,
        cancelled: u32,
    },
    CleanupStarted,
    CleanupComplete {
        released_resources: Vec<String>,
    },
    Progress {
        progress: ManagedLifecycleProgress,
    },
    Failed {
        reason: ManagedLifecycleReason,
        cleanup: ManagedCleanupState,
    },
    Unsupported,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ManagedEvidenceKind {
    Observation,
    Request,
    Commit,
    Progress,
    Failure,
    Retirement,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedLifecycleEvidence {
    pub sequence: u64,
    pub tick: u64,
    pub kind: ManagedEvidenceKind,
    pub state: ManagedLifecycleState,
    pub readiness: ManagedRuntimeReadiness,
    pub cleanup: ManagedCleanupState,
    pub reason: ManagedLifecycleReason,
    pub reason_code: String,
    pub request_id: Option<String>,
    pub causation: String,
    pub progress: Option<ManagedLifecycleProgress>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ManagedComponentObservation {
    pub schema_version: u32,
    pub descriptor_id: String,
    pub descriptor_identity: String,
    pub identity: ManagedComponentIdentity,
    pub state: ManagedLifecycleState,
    pub readiness: ManagedRuntimeReadiness,
    pub cleanup: ManagedCleanupState,
    pub reason: ManagedLifecycleReason,
    pub reason_code: String,
    pub sequence: u64,
    pub observed_at_tick: u64,
    pub valid_until_tick: u64,
    pub pending_request_id: Option<String>,
    pub pending_action: Option<ManagedLifecycleAction>,
    pub progress: Option<ManagedLifecycleProgress>,
    pub retired: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct PendingRequest {
    request: ManagedLifecycleRequest,
    action_origin: ManagedLifecycleState,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManagedRequestReceipt {
    pub request_id: String,
    pub accepted: bool,
    pub duplicate: bool,
    pub observation_sequence: u64,
}

#[derive(Clone, Debug)]
pub struct ManagedComponentMachine {
    descriptor: ManagedComponentDescriptor,
    observation: ManagedComponentObservation,
    pending: Option<PendingRequest>,
    last_completed_request: Option<String>,
    progress_events: u32,
    evidence: VecDeque<ManagedLifecycleEvidence>,
    earliest_sequence: u64,
}

impl ManagedComponentMachine {
    pub fn new(
        descriptor: ManagedComponentDescriptor,
        identity: ManagedComponentIdentity,
        observed_at_tick: u64,
        valid_until_tick: u64,
    ) -> Result<Self, ManagedLifecycleError> {
        descriptor.validate()?;
        identity.validate()?;
        if valid_until_tick <= observed_at_tick {
            return Err(ManagedLifecycleError::new(
                ManagedLifecycleReason::StaleHostFact.code(),
                ManagedLifecycleReason::StaleHostFact,
                "managed component observation validity is empty",
            ));
        }
        let observation = ManagedComponentObservation {
            schema_version: MANAGED_COMPONENT_SCHEMA_VERSION,
            descriptor_id: descriptor.id.clone(),
            descriptor_identity: descriptor.identity.clone(),
            identity,
            state: ManagedLifecycleState::Configured,
            readiness: ManagedRuntimeReadiness::NotApplicable,
            cleanup: ManagedCleanupState::NotRequired,
            reason: ManagedLifecycleReason::Configured,
            reason_code: ManagedLifecycleReason::Configured.code().to_owned(),
            sequence: 0,
            observed_at_tick,
            valid_until_tick,
            pending_request_id: None,
            pending_action: None,
            progress: None,
            retired: false,
        };
        let mut machine = Self {
            descriptor,
            observation,
            pending: None,
            last_completed_request: None,
            progress_events: 0,
            evidence: VecDeque::new(),
            earliest_sequence: 0,
        };
        machine.record(
            observed_at_tick,
            ManagedEvidenceKind::Observation,
            ManagedLifecycleReason::Configured,
            None,
            "exact-component-configured",
            None,
        );
        Ok(machine)
    }

    #[must_use]
    pub fn descriptor(&self) -> &ManagedComponentDescriptor {
        &self.descriptor
    }

    #[must_use]
    pub fn observation(&self) -> &ManagedComponentObservation {
        &self.observation
    }

    #[must_use]
    pub fn earliest_evidence_sequence(&self) -> u64 {
        self.earliest_sequence
    }

    pub fn evidence(&self) -> impl Iterator<Item = &ManagedLifecycleEvidence> {
        self.evidence.iter()
    }

    pub fn request(
        &mut self,
        request: ManagedLifecycleRequest,
        authority: &ManagedLifecycleAuthority,
        now_tick: u64,
    ) -> Result<ManagedRequestReceipt, ManagedLifecycleError> {
        if self
            .pending
            .as_ref()
            .is_some_and(|pending| pending.request.request_id == request.request_id)
            || self.last_completed_request.as_deref() == Some(&request.request_id)
        {
            return Ok(ManagedRequestReceipt {
                request_id: request.request_id,
                accepted: true,
                duplicate: true,
                observation_sequence: self.observation.sequence,
            });
        }
        self.validate_request(&request, authority, now_tick)?;
        if self.pending.is_some() {
            return Err(ManagedLifecycleError::new(
                ManagedLifecycleReason::StaleRequest.code(),
                ManagedLifecycleReason::StaleRequest,
                "a different lifecycle request is already pending",
            ));
        }
        let action_origin = self.observation.state;
        self.pending = Some(PendingRequest {
            request: request.clone(),
            action_origin,
        });
        self.observation.pending_request_id = Some(request.request_id.clone());
        self.observation.pending_action = Some(request.action);
        self.record(
            now_tick,
            ManagedEvidenceKind::Request,
            ManagedLifecycleReason::RequestAccepted,
            Some(request.request_id.clone()),
            request.causation.clone(),
            None,
        );
        Ok(ManagedRequestReceipt {
            request_id: request.request_id,
            accepted: true,
            duplicate: false,
            observation_sequence: self.observation.sequence,
        })
    }

    pub fn apply_provider_event(
        &mut self,
        request_id: &str,
        event: ManagedProviderEvent,
        tick: u64,
    ) -> Result<(), ManagedLifecycleError> {
        if self.observation.retired {
            return Err(ManagedLifecycleError::new(
                ManagedLifecycleReason::RetiredGenerationWake.code(),
                ManagedLifecycleReason::RetiredGenerationWake,
                "retired component generation rejected a late provider callback",
            ));
        }
        if tick > self.observation.valid_until_tick {
            return Err(ManagedLifecycleError::new(
                ManagedLifecycleReason::StaleHostFact.code(),
                ManagedLifecycleReason::StaleHostFact,
                "provider event relies on a stale host observation",
            ));
        }
        let pending = self.pending.as_ref().ok_or_else(|| {
            ManagedLifecycleError::new(
                ManagedLifecycleReason::StaleRequest.code(),
                ManagedLifecycleReason::StaleRequest,
                "provider event has no pending authorized request",
            )
        })?;
        if pending.request.request_id != request_id {
            return Err(ManagedLifecycleError::new(
                ManagedLifecycleReason::StaleRequest.code(),
                ManagedLifecycleReason::StaleRequest,
                "provider event names a different lifecycle request",
            ));
        }
        let action = pending.request.action;
        let origin = pending.action_origin;
        let causation = pending.request.causation.clone();
        self.refresh_freshness(tick);
        match event {
            ManagedProviderEvent::Prepared { resource_evidence } => {
                let expected_resources = self
                    .observation
                    .identity
                    .resources
                    .iter()
                    .map(String::as_str)
                    .collect::<BTreeSet<_>>();
                let observed_resources = resource_evidence
                    .iter()
                    .map(String::as_str)
                    .collect::<BTreeSet<_>>();
                if action != ManagedLifecycleAction::Prepare
                    || origin != ManagedLifecycleState::Configured
                    || resource_evidence.iter().any(String::is_empty)
                    || expected_resources != observed_resources
                {
                    return Err(self.wrong_state("prepare commit does not match the request"));
                }
                self.observation.state = ManagedLifecycleState::Prepared;
                self.observation.readiness = ManagedRuntimeReadiness::NotApplicable;
                self.observation.cleanup = if resource_evidence.is_empty() {
                    ManagedCleanupState::NotRequired
                } else {
                    ManagedCleanupState::Required
                };
                self.finish_request(
                    tick,
                    ManagedEvidenceKind::Commit,
                    ManagedLifecycleReason::Prepared,
                    request_id,
                    causation,
                );
            }
            ManagedProviderEvent::Activated => {
                if action != ManagedLifecycleAction::Activate
                    || !matches!(
                        origin,
                        ManagedLifecycleState::Prepared | ManagedLifecycleState::Inactive
                    )
                {
                    return Err(self.wrong_state("activation commit does not match the request"));
                }
                self.observation.state = ManagedLifecycleState::Active;
                self.observation.readiness = ManagedRuntimeReadiness::Waiting;
                self.finish_request(
                    tick,
                    ManagedEvidenceKind::Commit,
                    ManagedLifecycleReason::Activated,
                    request_id,
                    causation,
                );
            }
            ManagedProviderEvent::AdmissionClosed { .. } => {
                if !matches!(
                    action,
                    ManagedLifecycleAction::Quiesce
                        | ManagedLifecycleAction::Deactivate
                        | ManagedLifecycleAction::Stop
                ) || origin != ManagedLifecycleState::Active
                    || self.observation.state != ManagedLifecycleState::Active
                {
                    return Err(self.wrong_state("admission closure does not match the request"));
                }
                self.observation.state = ManagedLifecycleState::Quiescing;
                self.observation.readiness = ManagedRuntimeReadiness::Waiting;
                self.record(
                    tick,
                    ManagedEvidenceKind::Commit,
                    ManagedLifecycleReason::AdmissionClosed,
                    Some(request_id.to_owned()),
                    causation,
                    None,
                );
            }
            ManagedProviderEvent::Quiesced { .. } => {
                if self.observation.state != ManagedLifecycleState::Quiescing
                    || !matches!(
                        action,
                        ManagedLifecycleAction::Quiesce
                            | ManagedLifecycleAction::Deactivate
                            | ManagedLifecycleAction::Stop
                    )
                {
                    return Err(self.wrong_state("quiescence commit does not match the request"));
                }
                self.observation.state = ManagedLifecycleState::Inactive;
                self.observation.readiness = ManagedRuntimeReadiness::NotApplicable;
                if action == ManagedLifecycleAction::Stop {
                    self.record(
                        tick,
                        ManagedEvidenceKind::Commit,
                        ManagedLifecycleReason::Quiesced,
                        Some(request_id.to_owned()),
                        causation,
                        None,
                    );
                } else {
                    self.finish_request(
                        tick,
                        ManagedEvidenceKind::Commit,
                        ManagedLifecycleReason::Quiesced,
                        request_id,
                        causation,
                    );
                }
            }
            ManagedProviderEvent::CleanupStarted => {
                if !matches!(
                    action,
                    ManagedLifecycleAction::Clean | ManagedLifecycleAction::Stop
                ) || !matches!(
                    self.observation.state,
                    ManagedLifecycleState::Prepared
                        | ManagedLifecycleState::Inactive
                        | ManagedLifecycleState::Failed
                ) {
                    return Err(self.wrong_state("cleanup start does not match the request"));
                }
                self.observation.state = ManagedLifecycleState::Cleaning;
                self.observation.readiness = ManagedRuntimeReadiness::NotApplicable;
                self.observation.cleanup = ManagedCleanupState::InProgress;
                self.record(
                    tick,
                    ManagedEvidenceKind::Commit,
                    ManagedLifecycleReason::CleanupStarted,
                    Some(request_id.to_owned()),
                    causation,
                    None,
                );
            }
            ManagedProviderEvent::CleanupComplete { released_resources } => {
                let expected_resources = self
                    .observation
                    .identity
                    .resources
                    .iter()
                    .map(String::as_str)
                    .collect::<BTreeSet<_>>();
                let released = released_resources
                    .iter()
                    .map(String::as_str)
                    .collect::<BTreeSet<_>>();
                if self.observation.state != ManagedLifecycleState::Cleaning
                    || !matches!(
                        action,
                        ManagedLifecycleAction::Clean | ManagedLifecycleAction::Stop
                    )
                    || released_resources.iter().any(String::is_empty)
                    || expected_resources != released
                {
                    return Err(self.wrong_state("cleanup completion does not match the request"));
                }
                self.observation.state = ManagedLifecycleState::Stopped;
                self.observation.readiness = ManagedRuntimeReadiness::NotApplicable;
                self.observation.cleanup = ManagedCleanupState::Complete;
                self.finish_request(
                    tick,
                    ManagedEvidenceKind::Commit,
                    ManagedLifecycleReason::CleanupComplete,
                    request_id,
                    causation,
                );
            }
            ManagedProviderEvent::Progress { progress } => {
                if !self.descriptor.facets.progress
                    || self.progress_events >= self.descriptor.maximum_progress_events
                    || progress.detail.is_empty()
                    || progress
                        .total_units
                        .is_some_and(|total| progress.completed_units > total)
                {
                    return Err(ManagedLifecycleError::new(
                        ManagedLifecycleReason::Progress.code(),
                        ManagedLifecycleReason::Progress,
                        "lifecycle progress is unsupported, invalid, or exhausted",
                    ));
                }
                self.progress_events += 1;
                self.observation.progress = Some(progress.clone());
                self.record(
                    tick,
                    ManagedEvidenceKind::Progress,
                    ManagedLifecycleReason::Progress,
                    Some(request_id.to_owned()),
                    causation,
                    Some(progress),
                );
            }
            ManagedProviderEvent::Failed { reason, cleanup } => {
                self.observation.state = ManagedLifecycleState::Failed;
                self.observation.readiness = ManagedRuntimeReadiness::NotApplicable;
                self.observation.cleanup = cleanup;
                self.finish_request(
                    tick,
                    ManagedEvidenceKind::Failure,
                    reason,
                    request_id,
                    causation,
                );
            }
            ManagedProviderEvent::Unsupported => {
                self.observation.state = ManagedLifecycleState::Unsupported;
                self.observation.readiness = ManagedRuntimeReadiness::NotApplicable;
                self.finish_request(
                    tick,
                    ManagedEvidenceKind::Failure,
                    ManagedLifecycleReason::UnsupportedFacet,
                    request_id,
                    causation,
                );
            }
        }
        Ok(())
    }

    pub fn cancel_request(
        &mut self,
        request_id: &str,
        tick: u64,
    ) -> Result<(), ManagedLifecycleError> {
        let pending = self.pending.take().ok_or_else(|| {
            ManagedLifecycleError::new(
                ManagedLifecycleReason::StaleRequest.code(),
                ManagedLifecycleReason::StaleRequest,
                "no lifecycle request is pending",
            )
        })?;
        if pending.request.request_id != request_id {
            self.pending = Some(pending);
            return Err(ManagedLifecycleError::new(
                ManagedLifecycleReason::StaleRequest.code(),
                ManagedLifecycleReason::StaleRequest,
                "cannot cancel a different lifecycle request",
            ));
        }
        self.observation.pending_request_id = None;
        self.observation.pending_action = None;
        self.observation.progress = None;
        if self.observation.state != pending.action_origin {
            self.observation.state = ManagedLifecycleState::Failed;
            self.observation.readiness = ManagedRuntimeReadiness::NotApplicable;
            if self.observation.cleanup != ManagedCleanupState::Complete {
                self.observation.cleanup = ManagedCleanupState::Required;
            }
        }
        self.record(
            tick,
            ManagedEvidenceKind::Request,
            ManagedLifecycleReason::RequestCancelled,
            Some(request_id.to_owned()),
            pending.request.causation,
            None,
        );
        Ok(())
    }

    pub fn set_readiness(
        &mut self,
        readiness: ManagedRuntimeReadiness,
        tick: u64,
        causation: impl Into<String>,
    ) -> Result<(), ManagedLifecycleError> {
        if self.observation.state != ManagedLifecycleState::Active
            || readiness == ManagedRuntimeReadiness::NotApplicable
        {
            return Err(self.wrong_state("readiness is observable only for an active component"));
        }
        self.observation.readiness = readiness;
        self.refresh_freshness(tick);
        self.record(
            tick,
            ManagedEvidenceKind::Observation,
            ManagedLifecycleReason::Progress,
            None,
            causation,
            None,
        );
        Ok(())
    }

    pub fn check_deadline(&mut self, tick: u64) -> Result<(), ManagedLifecycleError> {
        let Some(pending) = &self.pending else {
            return Ok(());
        };
        if tick <= pending.request.deadline_tick {
            return Ok(());
        }
        let request_id = pending.request.request_id.clone();
        let causation = pending.request.causation.clone();
        let reason = if self.observation.state == ManagedLifecycleState::Cleaning {
            self.observation.cleanup = ManagedCleanupState::TimedOut;
            ManagedLifecycleReason::CleanupTimeout
        } else if self.observation.state == ManagedLifecycleState::Quiescing {
            self.observation.cleanup = ManagedCleanupState::Required;
            ManagedLifecycleReason::DrainDeadline
        } else {
            ManagedLifecycleReason::Cancelled
        };
        self.observation.state = ManagedLifecycleState::Failed;
        self.observation.readiness = ManagedRuntimeReadiness::NotApplicable;
        self.finish_request(
            tick,
            ManagedEvidenceKind::Failure,
            reason,
            &request_id,
            causation,
        );
        Err(ManagedLifecycleError::new(
            reason.code(),
            reason,
            "managed lifecycle request exceeded its admitted deadline",
        ))
    }

    pub fn report_loss(&mut self, host_lost: bool, tick: u64, causation: impl Into<String>) {
        let reason = if host_lost {
            ManagedLifecycleReason::HostLost
        } else {
            ManagedLifecycleReason::ProviderLost
        };
        self.pending = None;
        self.observation.pending_request_id = None;
        self.observation.pending_action = None;
        self.observation.state = ManagedLifecycleState::Failed;
        self.observation.readiness = ManagedRuntimeReadiness::NotApplicable;
        self.observation.cleanup = if host_lost {
            ManagedCleanupState::Unprovable
        } else {
            ManagedCleanupState::Required
        };
        self.record(
            tick,
            ManagedEvidenceKind::Failure,
            reason,
            None,
            causation,
            None,
        );
    }

    pub fn retire_for_plan_replacement(&mut self, tick: u64, causation: impl Into<String>) {
        self.pending = None;
        self.observation.pending_request_id = None;
        self.observation.pending_action = None;
        self.observation.retired = true;
        self.record(
            tick,
            ManagedEvidenceKind::Retirement,
            ManagedLifecycleReason::PlanReplaced,
            None,
            causation,
            None,
        );
    }

    fn validate_request(
        &self,
        request: &ManagedLifecycleRequest,
        authority: &ManagedLifecycleAuthority,
        now_tick: u64,
    ) -> Result<(), ManagedLifecycleError> {
        if request.schema_version != MANAGED_COMPONENT_SCHEMA_VERSION
            || request.request_id.is_empty()
            || request.causation.is_empty()
            || request.component != self.observation.identity.component
        {
            return Err(ManagedLifecycleError::new(
                ManagedLifecycleReason::StaleRequest.code(),
                ManagedLifecycleReason::StaleRequest,
                "lifecycle request identity or schema is invalid",
            ));
        }
        if self.observation.retired {
            return Err(ManagedLifecycleError::new(
                ManagedLifecycleReason::PlanReplaced.code(),
                ManagedLifecycleReason::PlanReplaced,
                "retired component rejects lifecycle requests",
            ));
        }
        if request.expected_plan_epoch != self.observation.identity.plan_epoch {
            return Err(ManagedLifecycleError::new(
                ManagedLifecycleReason::StalePlanEpoch.code(),
                ManagedLifecycleReason::StalePlanEpoch,
                "lifecycle request names a stale plan epoch",
            ));
        }
        if request.expected_activation_generation != self.observation.identity.activation_generation
        {
            return Err(ManagedLifecycleError::new(
                ManagedLifecycleReason::WrongGeneration.code(),
                ManagedLifecycleReason::WrongGeneration,
                "lifecycle request names a different activation generation",
            ));
        }
        if request.expected_observation_sequence != self.observation.sequence {
            return Err(ManagedLifecycleError::new(
                ManagedLifecycleReason::StaleRequest.code(),
                ManagedLifecycleReason::StaleRequest,
                "lifecycle request lost optimistic concurrency",
            ));
        }
        if now_tick > self.observation.valid_until_tick {
            return Err(ManagedLifecycleError::new(
                ManagedLifecycleReason::StaleHostFact.code(),
                ManagedLifecycleReason::StaleHostFact,
                "lifecycle request relies on stale host facts",
            ));
        }
        if request.issued_at_tick > now_tick
            || request.deadline_tick <= now_tick
            || request.deadline_tick - request.issued_at_tick
                > self.descriptor.maximum_request_ticks
        {
            return Err(ManagedLifecycleError::new(
                ManagedLifecycleReason::StaleRequest.code(),
                ManagedLifecycleReason::StaleRequest,
                "lifecycle request has an invalid or excessive deadline",
            ));
        }
        if authority.requester.is_empty() || authority.authority_id.is_empty() {
            return Err(ManagedLifecycleError::new(
                ManagedLifecycleReason::DeniedGrant.code(),
                ManagedLifecycleReason::DeniedGrant,
                "lifecycle request lacks a current authority grant",
            ));
        }
        if authority.provider == ManagedProviderAvailability::Unavailable {
            return Err(ManagedLifecycleError::new(
                ManagedLifecycleReason::UnavailableImplementation.code(),
                ManagedLifecycleReason::UnavailableImplementation,
                "managed implementation is unavailable",
            ));
        }
        if authority.grant == ManagedGrantState::Revoked {
            return Err(ManagedLifecycleError::new(
                ManagedLifecycleReason::RevokedGrant.code(),
                ManagedLifecycleReason::RevokedGrant,
                "lifecycle authority grant was revoked",
            ));
        }
        if authority.grant == ManagedGrantState::Denied
            || now_tick < authority.not_before_tick
            || now_tick >= authority.expires_at_tick
        {
            return Err(ManagedLifecycleError::new(
                ManagedLifecycleReason::DeniedGrant.code(),
                ManagedLifecycleReason::DeniedGrant,
                "lifecycle request lacks a current authority grant",
            ));
        }
        if authority.resources == ManagedResourceState::Conflict {
            return Err(ManagedLifecycleError::new(
                ManagedLifecycleReason::ResourceConflict.code(),
                ManagedLifecycleReason::ResourceConflict,
                "managed component resources conflict with another holder",
            ));
        }
        if authority.leases == ManagedLeaseState::Expired {
            return Err(ManagedLifecycleError::new(
                ManagedLifecycleReason::ExpiredLease.code(),
                ManagedLifecycleReason::ExpiredLease,
                "managed component resource lease expired",
            ));
        }
        if !authority.actions.contains(&request.action) {
            return Err(ManagedLifecycleError::new(
                ManagedLifecycleReason::DeniedGrant.code(),
                ManagedLifecycleReason::DeniedGrant,
                "lifecycle authority does not admit the requested action",
            ));
        }
        if authority.inhibit_asserted
            && matches!(
                request.action,
                ManagedLifecycleAction::Prepare | ManagedLifecycleAction::Activate
            )
        {
            return Err(ManagedLifecycleError::new(
                ManagedLifecycleReason::InhibitAsserted.code(),
                ManagedLifecycleReason::InhibitAsserted,
                "independent inhibit blocks preparation or activation",
            ));
        }
        if !self.descriptor.supports(request.action) {
            return Err(ManagedLifecycleError::new(
                ManagedLifecycleReason::UnsupportedFacet.code(),
                ManagedLifecycleReason::UnsupportedFacet,
                "implementation cannot prove the requested lifecycle facet",
            ));
        }
        if !legal_action(self.observation.state, request.action) {
            return Err(self.wrong_state("lifecycle action is illegal from the observed state"));
        }
        Ok(())
    }

    fn finish_request(
        &mut self,
        tick: u64,
        kind: ManagedEvidenceKind,
        reason: ManagedLifecycleReason,
        request_id: &str,
        causation: impl Into<String>,
    ) {
        self.last_completed_request = Some(request_id.to_owned());
        self.pending = None;
        self.observation.pending_request_id = None;
        self.observation.pending_action = None;
        self.observation.progress = None;
        self.record(
            tick,
            kind,
            reason,
            Some(request_id.to_owned()),
            causation,
            None,
        );
    }

    fn refresh_freshness(&mut self, tick: u64) {
        self.observation.valid_until_tick =
            tick.saturating_add(self.descriptor.maximum_request_ticks);
    }

    fn record(
        &mut self,
        tick: u64,
        kind: ManagedEvidenceKind,
        reason: ManagedLifecycleReason,
        request_id: Option<String>,
        causation: impl Into<String>,
        progress: Option<ManagedLifecycleProgress>,
    ) {
        self.observation.sequence = self.observation.sequence.saturating_add(1);
        self.observation.observed_at_tick = tick;
        self.observation.reason = reason;
        self.observation.reason_code = reason.code().to_owned();
        let event = ManagedLifecycleEvidence {
            sequence: self.observation.sequence,
            tick,
            kind,
            state: self.observation.state,
            readiness: self.observation.readiness,
            cleanup: self.observation.cleanup,
            reason,
            reason_code: reason.code().to_owned(),
            request_id,
            causation: causation.into(),
            progress,
        };
        self.evidence.push_back(event);
        let maximum = self.descriptor.maximum_retained_events as usize;
        while self.evidence.len() > maximum {
            if let Some(removed) = self.evidence.pop_front() {
                self.earliest_sequence = removed.sequence.saturating_add(1);
            }
        }
    }

    fn wrong_state(&self, message: impl Into<String>) -> ManagedLifecycleError {
        ManagedLifecycleError::new(
            ManagedLifecycleReason::WrongState.code(),
            ManagedLifecycleReason::WrongState,
            message,
        )
    }
}

fn legal_action(state: ManagedLifecycleState, action: ManagedLifecycleAction) -> bool {
    match action {
        ManagedLifecycleAction::Prepare => state == ManagedLifecycleState::Configured,
        ManagedLifecycleAction::Activate => matches!(
            state,
            ManagedLifecycleState::Prepared | ManagedLifecycleState::Inactive
        ),
        ManagedLifecycleAction::Quiesce | ManagedLifecycleAction::Deactivate => {
            state == ManagedLifecycleState::Active
        }
        ManagedLifecycleAction::Clean => matches!(
            state,
            ManagedLifecycleState::Prepared
                | ManagedLifecycleState::Inactive
                | ManagedLifecycleState::Failed
        ),
        ManagedLifecycleAction::Stop => matches!(
            state,
            ManagedLifecycleState::Active
                | ManagedLifecycleState::Prepared
                | ManagedLifecycleState::Inactive
                | ManagedLifecycleState::Failed
        ),
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ManagedLifecycleError {
    pub code: &'static str,
    pub reason: ManagedLifecycleReason,
    pub message: String,
}

impl ManagedLifecycleError {
    fn new(code: &'static str, reason: ManagedLifecycleReason, message: impl Into<String>) -> Self {
        Self {
            code,
            reason,
            message: message.into(),
        }
    }
}

impl fmt::Display for ManagedLifecycleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}: {}", self.code, self.message)
    }
}

impl std::error::Error for ManagedLifecycleError {}
