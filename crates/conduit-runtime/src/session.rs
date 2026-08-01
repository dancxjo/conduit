//! Persistent ownership and cooperative pumping of exact scheduler runs.
//!
//! This module does not introduce another executor. It owns one already
//! admitted `DeterministicExecutor` and exposes bounded turns to the host.

use std::cell::RefCell;
use std::rc::Rc;

use conduit_core::{
    ArtifactDigest, CanonicalDescriptor, CanonicalValue, EvidenceCursorStatus, FieldDisposition,
    Id, MapField, SemanticHash, StopPolicy, TerminalClass,
};

use crate::{
    DeterministicExecutor, SchedulerError, SchedulerEventBatch, SchedulerHighWater, SchedulerNode,
    SchedulerStatus, ValueStorageUsage,
};

/// Immutable identities pinned when an authorized exact run starts.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactRunIdentity {
    pub plan_identity: SemanticHash,
    pub source_semantic_hash: SemanticHash,
    pub plan_epoch: u64,
    pub run_id: String,
}

/// Externally visible state of a persistent exact run.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExactRunState {
    /// There is ready work; a later bounded pump can continue it.
    Active,
    /// The run is alive but needs an authorized timer, host operation, input,
    /// output, or cancellation wake. Waiting is not terminal.
    Waiting,
    /// Drain cancellation was requested and retained work is being settled.
    Quiescing,
    /// Abort cancellation was requested and bounded provider cleanup is still
    /// settling. This is not terminal until the cleanup disposition is known.
    Aborting,
    /// The exact run reached one terminal class.
    Terminal(TerminalClass),
}

/// Bounded facts returned by one cooperative scheduling turn.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ExactRunPump {
    pub state: ExactRunState,
    pub decisions: u64,
    pub tick: u64,
    /// One-past-the-end monotonic scheduler-event cursor. This cursor never
    /// rewinds when an external recorder acknowledges retained observations.
    pub event_cursor: u64,
    pub high_water: SchedulerHighWater,
    /// Fixed hosted-value arena residency after this pump. Portable runs have
    /// no hosted arena and report `None`.
    pub value_storage: Option<ValueStorageUsage>,
}

/// One bounded exact-evidence projection. The sink owns this batch; the
/// scheduler retains the underlying observations until commit succeeds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactEvidenceBatch {
    pub status: EvidenceCursorStatus,
    /// Exclusive cursor for the next batch or acknowledgement.
    pub next_cursor: u64,
    pub records: Vec<crate::ExactEvidenceRecord>,
}

/// Exact plan-selected evidence-provider identity retained by the runtime.
/// Availability, storage identity, authority, and commitment are distinct
/// facts; this binding never makes a provider current merely by naming it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactEvidenceProviderBinding {
    pub implementation_id: String,
    pub implementation_identity: SemanticHash,
    pub artifact_id: String,
    pub artifact_digest: ArtifactDigest,
    pub host_observation_id: String,
    pub store_resource_kind: String,
    pub store_resource_id: String,
    pub store_generation: u64,
    pub grant_hash: SemanticHash,
    pub time_basis: String,
}

/// Fresh host observation used for one exact evidence-provider operation.
/// It is supplied at use time and is not part of exact-plan identity.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactEvidenceUseAuthority {
    pub grant_hash: SemanticHash,
    pub grant_active: bool,
    pub run_id: String,
    pub plan_epoch: u64,
    pub host_observation_id: String,
    pub store_resource_kind: String,
    pub store_resource_id: String,
    pub store_generation: u64,
    pub lease_id: String,
    pub lease_epoch: u64,
    pub lease_available: bool,
    pub time_basis: String,
    pub validated_at_tick: u64,
    pub valid_until_tick: u64,
}

/// Complete immutable request sent only to the exact bound evidence provider.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactEvidenceCommitRequest {
    pub plan_identity: SemanticHash,
    pub plan_epoch: u64,
    pub run_id: String,
    pub provider: ExactEvidenceProviderBinding,
    pub authority: ExactEvidenceUseAuthority,
    pub start_cursor: u64,
    pub end_cursor: u64,
    pub batch_digest: SemanticHash,
}

/// Provider acknowledgement verified by the runtime before reclamation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ExactEvidenceCommitReceipt {
    pub plan_identity: SemanticHash,
    pub plan_epoch: u64,
    pub run_id: String,
    pub provider_implementation_id: String,
    pub provider_implementation_identity: SemanticHash,
    pub provider_artifact_id: String,
    pub provider_artifact_digest: ArtifactDigest,
    pub host_observation_id: String,
    pub store_resource_kind: String,
    pub store_resource_id: String,
    pub store_generation: u64,
    pub grant_hash: SemanticHash,
    pub lease_id: String,
    pub lease_epoch: u64,
    pub start_cursor: u64,
    pub end_cursor: u64,
    pub batch_digest: SemanticHash,
    pub provider_commit_identity: SemanticHash,
}

impl ExactEvidenceCommitReceipt {
    /// Creates the one valid receipt shape for a successfully published
    /// request. Providers must call this only after their atomic commit or
    /// successful reconciliation of the same idempotent request.
    #[must_use]
    pub fn acknowledged(request: &ExactEvidenceCommitRequest) -> Self {
        let mut receipt = Self {
            plan_identity: request.plan_identity,
            plan_epoch: request.plan_epoch,
            run_id: request.run_id.clone(),
            provider_implementation_id: request.provider.implementation_id.clone(),
            provider_implementation_identity: request.provider.implementation_identity,
            provider_artifact_id: request.provider.artifact_id.clone(),
            provider_artifact_digest: request.provider.artifact_digest,
            host_observation_id: request.provider.host_observation_id.clone(),
            store_resource_kind: request.provider.store_resource_kind.clone(),
            store_resource_id: request.provider.store_resource_id.clone(),
            store_generation: request.provider.store_generation,
            grant_hash: request.authority.grant_hash,
            lease_id: request.authority.lease_id.clone(),
            lease_epoch: request.authority.lease_epoch,
            start_cursor: request.start_cursor,
            end_cursor: request.end_cursor,
            batch_digest: request.batch_digest,
            provider_commit_identity: SemanticHash::from_bytes([0; 32]),
        };
        receipt.provider_commit_identity = receipt_identity(&receipt);
        receipt
    }
}

/// The authoritative external boundary for retained exact evidence. One
/// provider is selected before Start and owned by the exact session; Patchbay
/// and other callers cannot substitute a sink at drain time.
pub trait ExactEvidenceProvider {
    fn binding(&self) -> &ExactEvidenceProviderBinding;

    /// Re-observes the provider grant, store generation, and lease at the
    /// operation boundary. This comes from the installed provider boundary;
    /// a Patchbay/read caller cannot supply or substitute it.
    fn observe_use_authority(
        &self,
        run: &ExactRunIdentity,
    ) -> Result<ExactEvidenceUseAuthority, crate::RuntimeError>;

    fn commit_exact_evidence(
        &mut self,
        request: &ExactEvidenceCommitRequest,
        records: &[crate::ExactEvidenceRecord],
    ) -> Result<ExactEvidenceCommitReceipt, crate::RuntimeError>;
}

/// Failure while draining evidence. Every failure leaves the exact-run cursor
/// and resident scheduler observations unchanged for an explicit retry.
#[derive(Debug)]
pub enum ExactEvidenceDrainError {
    Scheduler(SchedulerError),
    Provider(crate::RuntimeError),
    Authority(crate::RuntimeError),
    Receipt(crate::RuntimeError),
}

fn receipt_identity(receipt: &ExactEvidenceCommitReceipt) -> SemanticHash {
    let fields = [
        MapField {
            name: Id("plan-identity"),
            value: CanonicalValue::Bytes(receipt.plan_identity.as_bytes()),
            disposition: FieldDisposition::Semantic,
        },
        MapField {
            name: Id("plan-epoch"),
            value: CanonicalValue::Integer(i128::from(receipt.plan_epoch)),
            disposition: FieldDisposition::Semantic,
        },
        MapField {
            name: Id("run-id"),
            value: CanonicalValue::Text(&receipt.run_id),
            disposition: FieldDisposition::Semantic,
        },
        MapField {
            name: Id("provider-implementation-id"),
            value: CanonicalValue::Text(&receipt.provider_implementation_id),
            disposition: FieldDisposition::Semantic,
        },
        MapField {
            name: Id("provider-implementation"),
            value: CanonicalValue::Bytes(receipt.provider_implementation_identity.as_bytes()),
            disposition: FieldDisposition::Semantic,
        },
        MapField {
            name: Id("provider-artifact-id"),
            value: CanonicalValue::Text(&receipt.provider_artifact_id),
            disposition: FieldDisposition::Semantic,
        },
        MapField {
            name: Id("provider-artifact"),
            value: CanonicalValue::Bytes(receipt.provider_artifact_digest.as_bytes()),
            disposition: FieldDisposition::Semantic,
        },
        MapField {
            name: Id("host-observation"),
            value: CanonicalValue::Text(&receipt.host_observation_id),
            disposition: FieldDisposition::Semantic,
        },
        MapField {
            name: Id("store-resource-kind"),
            value: CanonicalValue::Text(&receipt.store_resource_kind),
            disposition: FieldDisposition::Semantic,
        },
        MapField {
            name: Id("store-resource"),
            value: CanonicalValue::Text(&receipt.store_resource_id),
            disposition: FieldDisposition::Semantic,
        },
        MapField {
            name: Id("store-generation"),
            value: CanonicalValue::Integer(i128::from(receipt.store_generation)),
            disposition: FieldDisposition::Semantic,
        },
        MapField {
            name: Id("grant"),
            value: CanonicalValue::Bytes(receipt.grant_hash.as_bytes()),
            disposition: FieldDisposition::Semantic,
        },
        MapField {
            name: Id("lease"),
            value: CanonicalValue::Text(&receipt.lease_id),
            disposition: FieldDisposition::Semantic,
        },
        MapField {
            name: Id("lease-epoch"),
            value: CanonicalValue::Integer(i128::from(receipt.lease_epoch)),
            disposition: FieldDisposition::Semantic,
        },
        MapField {
            name: Id("start-cursor"),
            value: CanonicalValue::Integer(i128::from(receipt.start_cursor)),
            disposition: FieldDisposition::Semantic,
        },
        MapField {
            name: Id("end-cursor"),
            value: CanonicalValue::Integer(i128::from(receipt.end_cursor)),
            disposition: FieldDisposition::Semantic,
        },
        MapField {
            name: Id("batch-digest"),
            value: CanonicalValue::Bytes(receipt.batch_digest.as_bytes()),
            disposition: FieldDisposition::Semantic,
        },
    ];
    CanonicalDescriptor {
        kind: Id("conduit/exact-evidence-commit-receipt"),
        schema_version: 0,
        body: CanonicalValue::Map(&fields),
    }
    .semantic_hash()
    .expect("the exact-evidence receipt uses static valid identifiers")
}

fn validate_evidence_authority(
    run: &ExactRunIdentity,
    binding: &ExactEvidenceProviderBinding,
    authority: &ExactEvidenceUseAuthority,
) -> Result<(), crate::RuntimeError> {
    if !authority.grant_active
        || !authority.lease_available
        || authority.grant_hash != binding.grant_hash
        || authority.run_id != run.run_id
        || authority.plan_epoch != run.plan_epoch
        || authority.host_observation_id != binding.host_observation_id
        || authority.store_resource_kind != binding.store_resource_kind
        || authority.store_resource_id != binding.store_resource_id
        || authority.store_generation != binding.store_generation
        || authority.lease_id.is_empty()
        || authority.lease_epoch != run.plan_epoch
        || authority.time_basis != binding.time_basis
        || authority.validated_at_tick >= authority.valid_until_tick
    {
        return Err(crate::RuntimeError::new(
            "CND-EVC-004",
            "evidence provider grant or lease is revoked, expired, stale, or inexact",
        ));
    }
    Ok(())
}

fn validate_evidence_receipt(
    request: &ExactEvidenceCommitRequest,
    receipt: &ExactEvidenceCommitReceipt,
) -> Result<(), crate::RuntimeError> {
    let expected = ExactEvidenceCommitReceipt::acknowledged(request);
    if receipt != &expected || receipt.provider_commit_identity != receipt_identity(receipt) {
        return Err(crate::RuntimeError::new(
            "CND-EVC-005",
            "evidence provider receipt does not match the exact request",
        ));
    }
    Ok(())
}

/// Finite admission controller for concurrently retained exact-run sessions.
///
/// A host creates one registry for its runtime boundary and passes it to each
/// start request. The registry reserves the caller's declared runtime budget
/// before any implementation is prepared or started; releasing a terminal
/// session (or abandoning a failed start) returns that reservation.
#[derive(Clone, Debug)]
pub struct ExactRunSessionRegistry {
    capacity: Rc<RefCell<SessionCapacity>>,
}

#[derive(Debug)]
struct SessionCapacity {
    maximum_sessions: usize,
    maximum_reserved_bytes: u64,
    active_sessions: usize,
    reserved_bytes: u64,
    abandoned_live_session: bool,
}

/// A non-forgeable reservation retained by exactly one exact-run session.
#[derive(Debug)]
pub struct ExactRunSessionAdmission {
    capacity: Rc<RefCell<SessionCapacity>>,
    reserved_bytes: u64,
}

impl ExactRunSessionRegistry {
    /// Creates a finite hosted-session admission boundary.
    pub fn new(
        maximum_sessions: usize,
        maximum_reserved_bytes: u64,
    ) -> Result<Self, SchedulerError> {
        if maximum_sessions == 0 || maximum_reserved_bytes == 0 {
            return Err(SchedulerError::InvalidPolicy);
        }
        Ok(Self {
            capacity: Rc::new(RefCell::new(SessionCapacity {
                maximum_sessions,
                maximum_reserved_bytes,
                active_sessions: 0,
                reserved_bytes: 0,
                abandoned_live_session: false,
            })),
        })
    }

    /// Reserves one concurrent session and its declared runtime budget before
    /// node preparation or execution begins.
    pub fn admit(&self, reserved_bytes: u64) -> Result<ExactRunSessionAdmission, SchedulerError> {
        if reserved_bytes == 0 {
            return Err(SchedulerError::InvalidPolicy);
        }
        let mut capacity = self.capacity.borrow_mut();
        if capacity.abandoned_live_session {
            return Err(SchedulerError::AllocationUnavailable);
        }
        let next_sessions = capacity
            .active_sessions
            .checked_add(1)
            .ok_or(SchedulerError::AllocationUnavailable)?;
        let next_bytes = capacity
            .reserved_bytes
            .checked_add(reserved_bytes)
            .ok_or(SchedulerError::AllocationUnavailable)?;
        if next_sessions > capacity.maximum_sessions || next_bytes > capacity.maximum_reserved_bytes
        {
            return Err(SchedulerError::AllocationUnavailable);
        }
        capacity.active_sessions = next_sessions;
        capacity.reserved_bytes = next_bytes;
        Ok(ExactRunSessionAdmission {
            capacity: Rc::clone(&self.capacity),
            reserved_bytes,
        })
    }

    #[must_use]
    pub fn active_sessions(&self) -> usize {
        self.capacity.borrow().active_sessions
    }

    #[must_use]
    pub fn reserved_bytes(&self) -> u64 {
        self.capacity.borrow().reserved_bytes
    }

    /// Whether a nonterminal session was abandoned. This is distinct from a
    /// requested cancellation, and the registry rejects another Start until
    /// its owning host is replaced or recovered deliberately.
    #[must_use]
    pub fn has_abandoned_live_session(&self) -> bool {
        self.capacity.borrow().abandoned_live_session
    }
}

impl Drop for ExactRunSessionAdmission {
    fn drop(&mut self) {
        let mut capacity = self.capacity.borrow_mut();
        capacity.active_sessions = capacity.active_sessions.saturating_sub(1);
        capacity.reserved_bytes = capacity.reserved_bytes.saturating_sub(self.reserved_bytes);
    }
}

impl ExactRunSessionAdmission {
    fn mark_live_session_abandoned(&self) {
        self.capacity.borrow_mut().abandoned_live_session = true;
    }
}

/// One persistent exact execution session. All mutable scheduler state is
/// owned by this value and is released when it is finalized or dropped.
pub struct ExactRunSession<N: SchedulerNode> {
    identity: ExactRunIdentity,
    executor: Option<DeterministicExecutor<N>>,
    admission: Option<ExactRunSessionAdmission>,
    evidence_provider: Option<Box<dyn ExactEvidenceProvider>>,
    stop: Option<StopPolicy>,
}

impl<N: SchedulerNode> Drop for ExactRunSession<N> {
    fn drop(&mut self) {
        if self
            .executor
            .as_ref()
            .is_some_and(|executor| !is_terminal(executor.status()))
        {
            self.admission
                .as_ref()
                .expect("live exact-run session retains its admission")
                .mark_live_session_abandoned();
        }
    }
}

impl<N: SchedulerNode> ExactRunSession<N> {
    #[must_use]
    pub fn new(
        admission: ExactRunSessionAdmission,
        identity: ExactRunIdentity,
        executor: DeterministicExecutor<N>,
    ) -> Self {
        Self {
            identity,
            executor: Some(executor),
            admission: Some(admission),
            evidence_provider: None,
            stop: None,
        }
    }

    /// Starts one session with the exact plan-selected evidence provider. The
    /// installed provider must identify the same implementation, artifact,
    /// store generation, grant, and lease as the immutable plan binding.
    pub fn new_with_evidence_provider(
        admission: ExactRunSessionAdmission,
        identity: ExactRunIdentity,
        executor: DeterministicExecutor<N>,
        binding: ExactEvidenceProviderBinding,
        provider: Box<dyn ExactEvidenceProvider>,
    ) -> Result<Self, crate::RuntimeError> {
        if provider.binding() != &binding {
            return Err(crate::RuntimeError::new(
                "CND-EVC-001",
                "installed evidence provider does not match the exact plan binding",
            ));
        }
        Ok(Self {
            identity,
            executor: Some(executor),
            admission: Some(admission),
            evidence_provider: Some(provider),
            stop: None,
        })
    }

    #[must_use]
    pub fn identity(&self) -> &ExactRunIdentity {
        &self.identity
    }

    #[must_use]
    pub fn state(&self) -> ExactRunState {
        state_for(self.executor().status(), self.stop)
    }

    #[must_use]
    pub fn scheduler_status(&self) -> SchedulerStatus {
        self.executor().status()
    }

    /// Pump at most `quantum` fair node decisions. Reaching the quantum gives
    /// control back to the host without resetting any run identity, counter,
    /// queue, or timer state.
    pub fn pump(&mut self, quantum: u64) -> Result<ExactRunPump, SchedulerError> {
        self.pump_with_authority(quantum, &[])
    }

    /// Pump with the current host authority facts governing any hosted timer
    /// wake reached during this call.
    pub fn pump_with_authority(
        &mut self,
        quantum: u64,
        grant_observations: &[crate::ExactHostedServiceUseObservation],
    ) -> Result<ExactRunPump, SchedulerError> {
        if quantum == 0 {
            return Err(SchedulerError::InvalidPolicy);
        }
        let start = self.executor().decisions();
        while self.executor().decisions().saturating_sub(start) < quantum {
            let before = self.executor().decisions();
            let status = self
                .executor_mut()
                .run_one_with_authority(grant_observations)?;
            if !matches!(status, SchedulerStatus::Running) || self.executor().decisions() == before
            {
                break;
            }
        }
        Ok(self.snapshot())
    }

    /// Advance only the active run's exact scheduler clock. The caller must
    /// supply an admitted monotonic tick; this never creates a new epoch.
    pub fn advance_to(&mut self, tick: u64) -> Result<ExactRunPump, SchedulerError> {
        self.advance_to_with_authority(tick, &[])
    }

    /// Advance with the current host authority facts governing due hosted
    /// timer waits.
    pub fn advance_to_with_authority(
        &mut self,
        tick: u64,
        grant_observations: &[crate::ExactHostedServiceUseObservation],
    ) -> Result<ExactRunPump, SchedulerError> {
        self.executor_mut()
            .advance_to_with_authority(tick, grant_observations)?;
        Ok(self.snapshot())
    }

    /// Wake one exact named host operation on this session.
    pub fn notify_host_operation(
        &mut self,
        subject: conduit_core::Id<'_>,
    ) -> Result<ExactRunPump, SchedulerError> {
        self.notify_host_operation_with_authority(subject, &[])
    }

    /// Wake one exact named host operation with fresh live authority facts.
    pub fn notify_host_operation_with_authority(
        &mut self,
        subject: conduit_core::Id<'_>,
        grant_observations: &[crate::ExactHostedServiceUseObservation],
    ) -> Result<ExactRunPump, SchedulerError> {
        self.executor_mut()
            .notify_host_operation_with_authority(subject, grant_observations)?;
        Ok(self.snapshot())
    }

    /// Request the active session's exact Drain or Abort path.
    pub fn cancel(&mut self, stop: StopPolicy) -> Result<ExactRunPump, SchedulerError> {
        self.executor_mut().cancel(stop)?;
        self.stop = Some(stop);
        Ok(self.snapshot())
    }

    #[must_use]
    pub fn next_timer_deadline(&self) -> Option<u64> {
        self.executor().next_timer_deadline()
    }

    #[must_use]
    pub fn scheduler_event_count(&self) -> usize {
        self.executor().event_count()
    }

    pub fn scheduler_events(&self) -> impl Iterator<Item = &crate::SchedulerEvent> {
        self.executor().events()
    }

    /// First sequence still retained by this session's fixed event log.
    #[must_use]
    pub fn retained_event_cursor(&self) -> u64 {
        self.executor().retained_event_cursor()
    }

    /// One-past-the-end monotonic event cursor for this exact session.
    #[must_use]
    pub fn next_event_cursor(&self) -> u64 {
        self.executor().next_event_cursor()
    }

    /// Reads one bounded caller-owned batch from the retained event window.
    /// A caller must acknowledge only after its configured evidence provider
    /// has committed the batch.
    pub fn read_scheduler_events(
        &self,
        cursor: u64,
        maximum_events: u32,
    ) -> Result<SchedulerEventBatch, SchedulerError> {
        self.executor().read_events(cursor, maximum_events)
    }

    /// Projects one bounded read-only exact-evidence delta. Unlike
    /// [`Self::drain_exact_evidence`], this neither commits to an external
    /// provider nor acknowledges/reuses the retained scheduler prefix.
    pub fn read_exact_evidence(
        &self,
        cursor: u64,
        maximum_events: u32,
    ) -> Result<ExactEvidenceBatch, SchedulerError> {
        let batch = self.read_scheduler_events(cursor, maximum_events)?;
        let records = self.executor().project_exact_evidence_batch(
            &self.identity.plan_identity.to_string(),
            self.identity.plan_epoch,
            &self.identity.run_id,
            &batch.events,
        );
        Ok(ExactEvidenceBatch {
            status: batch.status,
            next_cursor: batch.next_cursor,
            records,
        })
    }

    /// Releases an externally committed prefix of the fixed event log.
    pub fn acknowledge_scheduler_events_through(
        &mut self,
        cursor: u64,
    ) -> Result<(), SchedulerError> {
        self.executor_mut().acknowledge_events_through(cursor)
    }

    /// Projects and commits one bounded exact-evidence batch. The scheduler
    /// acknowledges the batch only after the external sink succeeds, so a
    /// provider crash or backpressure cannot silently discard observations.
    pub fn drain_exact_evidence(
        &mut self,
        cursor: u64,
        maximum_events: u32,
    ) -> Result<ExactEvidenceBatch, ExactEvidenceDrainError> {
        let batch = self
            .read_scheduler_events(cursor, maximum_events)
            .map_err(ExactEvidenceDrainError::Scheduler)?;
        let records = self.executor().project_exact_evidence_batch(
            &self.identity.plan_identity.to_string(),
            self.identity.plan_epoch,
            &self.identity.run_id,
            &batch.events,
        );
        if batch.status == EvidenceCursorStatus::Available && !batch.events.is_empty() {
            let provider = self.evidence_provider.as_mut().ok_or_else(|| {
                ExactEvidenceDrainError::Provider(crate::RuntimeError::new(
                    "CND-EVC-002",
                    "exact run has no plan-selected evidence provider",
                ))
            })?;
            let binding = provider.binding().clone();
            let authority = provider
                .observe_use_authority(&self.identity)
                .map_err(ExactEvidenceDrainError::Authority)?;
            validate_evidence_authority(&self.identity, &binding, &authority)
                .map_err(ExactEvidenceDrainError::Authority)?;
            let batch_digest = crate::exact_evidence_batch_digest(
                cursor,
                batch.next_cursor,
                &records,
            )
            .map_err(|error| {
                ExactEvidenceDrainError::Receipt(crate::RuntimeError::new(
                    "CND-EVC-003",
                    format!("exact evidence batch could not be canonically encoded: {error}"),
                ))
            })?;
            let request = ExactEvidenceCommitRequest {
                plan_identity: self.identity.plan_identity,
                plan_epoch: self.identity.plan_epoch,
                run_id: self.identity.run_id.clone(),
                provider: binding,
                authority,
                start_cursor: cursor,
                end_cursor: batch.next_cursor,
                batch_digest,
            };
            let receipt = provider
                .commit_exact_evidence(&request, &records)
                .map_err(ExactEvidenceDrainError::Provider)?;
            validate_evidence_receipt(&request, &receipt)
                .map_err(ExactEvidenceDrainError::Receipt)?;
            self.acknowledge_scheduler_events_through(batch.next_cursor)
                .map_err(ExactEvidenceDrainError::Scheduler)?;
        }
        Ok(ExactEvidenceBatch {
            status: batch.status,
            next_cursor: batch.next_cursor,
            records,
        })
    }

    #[must_use]
    pub fn exact_evidence(&self) -> Vec<crate::ExactEvidenceRecord> {
        self.executor().project_exact_evidence(
            &self.identity.plan_identity.to_string(),
            self.identity.plan_epoch,
            &self.identity.run_id,
        )
    }

    #[must_use]
    pub fn allocation(&self) -> crate::SchedulerAllocation {
        self.executor().allocation()
    }

    /// The finite runtime reservation held for this session's complete life.
    #[must_use]
    pub fn reserved_session_bytes(&self) -> u64 {
        self.admission
            .as_ref()
            .map_or(0, |admission| admission.reserved_bytes)
    }

    #[must_use]
    pub fn plan_budget(&self) -> conduit_core::PlanResourceBudget {
        self.executor().plan_budget()
    }

    #[must_use]
    pub fn high_water(&self) -> SchedulerHighWater {
        self.executor().high_water()
    }

    /// Current and high-water payload storage for hosts that expose a fixed
    /// value arena. Portable drivers return no host-specific measurement.
    #[must_use]
    pub fn value_storage_usage(&self) -> Option<ValueStorageUsage> {
        self.executor().value_storage_usage()
    }

    /// Releases the owned scheduler only after it is terminal. A nonterminal
    /// error leaves this same session retained and usable by the caller.
    pub fn finalize(&mut self) -> Result<DeterministicExecutor<N>, ExactRunState> {
        if is_terminal(self.executor().status()) {
            let executor = self.executor.take().expect("terminal executor is retained");
            let admission = self
                .admission
                .take()
                .expect("terminal executor retains its admission");
            drop(admission);
            Ok(executor)
        } else {
            Err(self.state())
        }
    }

    fn executor(&self) -> &DeterministicExecutor<N> {
        self.executor
            .as_ref()
            .expect("exact-run session executor is retained until finalization")
    }

    fn executor_mut(&mut self) -> &mut DeterministicExecutor<N> {
        self.executor
            .as_mut()
            .expect("exact-run session executor is retained until finalization")
    }

    fn snapshot(&self) -> ExactRunPump {
        ExactRunPump {
            state: self.state(),
            decisions: self.executor().decisions(),
            tick: self.executor().tick(),
            event_cursor: self.executor().next_event_cursor(),
            high_water: self.executor().high_water(),
            value_storage: self.executor().value_storage_usage(),
        }
    }
}

const fn is_terminal(status: SchedulerStatus) -> bool {
    matches!(
        status,
        SchedulerStatus::Succeeded
            | SchedulerStatus::Cancelled
            | SchedulerStatus::Disconnected
            | SchedulerStatus::Failed(_)
    )
}

fn state_for(status: SchedulerStatus, stop: Option<StopPolicy>) -> ExactRunState {
    match status {
        SchedulerStatus::Running => match stop {
            Some(StopPolicy::Drain) => ExactRunState::Quiescing,
            Some(StopPolicy::Abort) => ExactRunState::Aborting,
            None => ExactRunState::Active,
        },
        SchedulerStatus::Stalled => match stop {
            Some(StopPolicy::Drain) => ExactRunState::Quiescing,
            Some(StopPolicy::Abort) => ExactRunState::Aborting,
            None => ExactRunState::Waiting,
        },
        SchedulerStatus::Succeeded => ExactRunState::Terminal(TerminalClass::Succeeded),
        SchedulerStatus::Cancelled => ExactRunState::Terminal(TerminalClass::Cancelled),
        SchedulerStatus::Disconnected => ExactRunState::Terminal(TerminalClass::Disconnected),
        SchedulerStatus::Failed(_) => ExactRunState::Terminal(TerminalClass::Failed),
    }
}
