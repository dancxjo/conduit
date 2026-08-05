use conduit_core::{
    mandatory_evidence_storage_requirement, verify_plan_fragment, BoundedQueue, CancellationPolicy,
    CancellationReason, ConnectionEnvelope, ConnectionId, ConnectionOutcome, ConnectionProvider,
    ConnectionTerminalDisposition, EvidenceStorageBudget, ExpectedEvidence, ExpectedTerminal,
    FailureReason, HostAdvertisement, HostCommand, HostEvent, MandatoryEvidenceReport, Observation,
    ObservationKind, PlacementId, PlacementLifecycleState, PlanFragment, PlanId, PlannedConnection,
    PlannedOperation, PlatformEffect, StartupDependency, TerminalDisposition, TerminalPolicy,
    ValuePayload, PROTOCOL_VERSION,
};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

pub mod providers;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImplementationFailure {
    pub reason: FailureReason,
    pub message: Option<String>,
}

impl ImplementationFailure {
    pub fn new(reason: FailureReason, message: impl Into<String>) -> Self {
        Self {
            reason,
            message: Some(message.into()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperationCompletion {
    Emitted,
    TimerElapsed,
    Value(ValuePayload),
    InputsClosed,
    PresentationCompleted {
        success: bool,
        message: Option<String>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OperationAction {
    Idle,
    Emit(ValuePayload),
    Wait {
        duration_ms: u64,
    },
    Present {
        presentation_kind: conduit_core::KindId,
        value: ValuePayload,
    },
    Complete,
    Fail(ImplementationFailure),
}

/// Profile-owned operation state expressed only in host-neutral runtime actions.
///
/// `start`/`resume` cover emit, wait, present, complete, and fail. Cancellation and release are
/// separate lifecycle callbacks so implementations can discard or relinquish opaque state without
/// introducing platform concepts into the runtime contract.
pub trait OperationState {
    fn start(&mut self) -> OperationAction;
    fn resume(&mut self, completion: OperationCompletion) -> OperationAction;

    fn cancel(&mut self) {}

    fn release(&mut self) {}
}

/// The complete installed semantic-operation boundary used by every host adapter.
///
/// The registry identifies an implementation by kind and implementation ID, asks it to validate
/// and prepare a planned operation, and receives opaque [`OperationState`]. The runtime then drives
/// that state with activation and input completions; requested platform work is returned as generic
/// [`OperationAction`] values and translated to [`PlatformEffect`]. Adding a semantic kind must only
/// require installing another implementation, never adding a kind-name match to the runtime.
pub trait OperationImplementation {
    fn kind_id(&self) -> &conduit_core::KindId;
    fn kind_contract_revision(&self) -> conduit_core::KindContractRevision;
    fn execution_profile_id(&self) -> conduit_core::ExecutionProfileId;
    fn implementation_id(&self) -> &conduit_core::ImplementationId;
    fn artifact_id(&self) -> &conduit_core::ArtifactId;
    fn host_operation_requirements(&self) -> Vec<conduit_core::HostOperationRequirement> {
        Vec::new()
    }
    fn prepare(
        &self,
        placement: &PlannedOperation,
    ) -> Result<Box<dyn OperationState>, ImplementationFailure>;

    fn minimum_value_size(&self, _value_kind: &conduit_core::KindId) -> Option<u32> {
        None
    }
}

#[derive(Default, Clone)]
pub struct ImplementationRegistry {
    implementations: BTreeMap<conduit_core::ImplementationId, Arc<dyn OperationImplementation>>,
}

impl core::fmt::Debug for ImplementationRegistry {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("ImplementationRegistry")
            .field(
                "implementation_ids",
                &self.implementations.keys().collect::<Vec<_>>(),
            )
            .finish()
    }
}

impl ImplementationRegistry {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn install<I>(&mut self, implementation: I) -> Result<(), ImplementationFailure>
    where
        I: OperationImplementation + 'static,
    {
        let implementation_id = implementation.implementation_id().clone();
        if self.implementations.contains_key(&implementation_id) {
            return Err(ImplementationFailure::new(
                FailureReason::UnknownImplementation,
                format!(
                    "implementation '{}' is already installed",
                    implementation_id.as_str()
                ),
            ));
        }
        self.implementations
            .insert(implementation_id, Arc::new(implementation));
        Ok(())
    }

    fn get(
        &self,
        implementation_id: &conduit_core::ImplementationId,
    ) -> Option<&Arc<dyn OperationImplementation>> {
        self.implementations.get(implementation_id)
    }
}

type PendingObservation = (
    Option<PlanId>,
    Option<PlacementId>,
    Option<ConnectionId>,
    ObservationKind,
);

#[derive(Debug, Default)]
pub struct RuntimeOutput {
    pub events: Vec<HostEvent>,
    pub effects: Vec<PlatformEffect>,
}

pub struct HostRuntime {
    advertisement: HostAdvertisement,
    observation_limit: usize,
    observations: Vec<Observation>,
    plans: BTreeMap<PlanId, RuntimePlan>,
    released_plans: BTreeSet<PlanId>,
    implementations: ImplementationRegistry,
}

impl core::fmt::Debug for HostRuntime {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("HostRuntime")
            .field("advertisement", &self.advertisement)
            .field("observation_limit", &self.observation_limit)
            .field("observations", &self.observations.len())
            .field("plans", &self.plans.keys().collect::<Vec<_>>())
            .field("released_plans", &self.released_plans)
            .field("implementations", &self.implementations)
            .finish()
    }
}

struct RuntimePlan {
    fragment: PlanFragment,
    mandatory_evidence: MandatoryEvidenceLog,
    placements: BTreeMap<PlacementId, RuntimePlacement>,
    connections: BTreeMap<ConnectionId, RuntimeConnection>,
    state: PlanState,
    terminal: Option<TerminalDisposition>,
    terminal_emitted: bool,
}

#[derive(Debug)]
struct MandatoryEvidenceLog {
    recorded_indices: Vec<u16>,
    allocated_item_slots: u32,
    storage_budget: EvidenceStorageBudget,
    used_bytes: u32,
    overflowed: bool,
}

impl MandatoryEvidenceLog {
    fn new(fragment: &PlanFragment) -> Self {
        let recorded_indices =
            Vec::with_capacity(usize::from(fragment.evidence_storage_budget.item_capacity));
        Self {
            allocated_item_slots: u32::try_from(recorded_indices.capacity()).unwrap_or(u32::MAX),
            recorded_indices,
            storage_budget: fragment.evidence_storage_budget,
            used_bytes: 0,
            overflowed: false,
        }
    }

    fn record(&mut self, expected: &[ExpectedEvidence], evidence: ExpectedEvidence) {
        let Some(index) = expected.iter().position(|item| item == &evidence) else {
            self.overflowed = true;
            return;
        };
        let Ok(index) = u16::try_from(index) else {
            self.overflowed = true;
            return;
        };
        if self.recorded_indices.contains(&index) {
            return;
        }
        let Some(charge) = mandatory_evidence_storage_requirement(core::slice::from_ref(&evidence))
        else {
            self.overflowed = true;
            return;
        };
        let Some(used_bytes) = self.used_bytes.checked_add(charge.byte_capacity) else {
            self.overflowed = true;
            return;
        };
        if self.recorded_indices.len() >= usize::from(self.storage_budget.item_capacity)
            || used_bytes > self.storage_budget.byte_capacity
        {
            self.overflowed = true;
            return;
        }
        self.recorded_indices.push(index);
        self.used_bytes = used_bytes;
    }

    fn report(&self, plan_id: PlanId, expected: &[ExpectedEvidence]) -> MandatoryEvidenceReport {
        MandatoryEvidenceReport {
            plan_id,
            expected: expected.to_vec(),
            recorded: self
                .recorded_indices
                .iter()
                .map(|index| expected[usize::from(*index)].clone())
                .collect(),
            storage_budget: self.storage_budget,
            allocated_item_slots: self.allocated_item_slots,
            used_bytes: self.used_bytes,
            overflowed: self.overflowed,
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum PlanState {
    Prepared,
    Active,
    Failed,
    Cancelled,
    Completed,
}

struct RuntimePlacement {
    spec: PlannedOperation,
    lifecycle: PlacementLifecycleState,
    terminal: Option<TerminalDisposition>,
    implementation_state: Box<dyn OperationState>,
    action: OperationAction,
    effect_issued: bool,
    pending_input_connection: Option<ConnectionId>,
    inputs_closed_notified: bool,
}

#[derive(Debug)]
struct RuntimeConnection {
    spec: PlannedConnection,
    queue: BoundedQueue<QueuedValue>,
    queued_bytes: u32,
    source_done: bool,
    sink_failed: bool,
    blocked: bool,
    last_accepted_sequence: Option<u64>,
    last_manifested_sequence: Option<u64>,
    terminal: Option<ConnectionTerminalDisposition>,
    role: ConnectionRole,
    transmission_in_flight: bool,
    next_expected_sequence: u64,
    next_send_sequence: u64,
    accepted_remote_sequences: BTreeSet<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct QueuedValue {
    sequence: u64,
    value: ValuePayload,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum ConnectionRole {
    Local,
    Outbound,
    Inbound,
}

fn validate_fragment_execution_contract(
    fragment: &PlanFragment,
) -> Option<(FailureReason, String)> {
    if fragment.placements.iter().any(|placement| {
        placement.host_operations.iter().any(|requirement| {
            requirement.contract_id.as_str().is_empty()
                || requirement
                    .target_kind
                    .as_ref()
                    .is_some_and(|target| target.as_str().is_empty())
                || requirement.maximum_in_flight == 0
        }) || placement
            .host_operations
            .windows(2)
            .any(|pair| pair[0] >= pair[1])
    }) {
        return Some((
            FailureReason::HostOperationContractMismatch,
            "host-operation requirements must have non-empty identities, unique canonical ordering, and nonzero in-flight bounds".to_string(),
        ));
    }
    if fragment.cancellation_policy != CancellationPolicy::CancelAllAndRejectLateCompletion {
        return Some((
            FailureReason::UnsupportedCancellationPolicy,
            "host supports only cancel-all with late-completion rejection".to_string(),
        ));
    }
    if fragment.terminal_policy != TerminalPolicy::RequireAllPlacementsAndConnections {
        return Some((
            FailureReason::UnsupportedTerminalPolicy,
            "host requires terminal evidence for every placement and connection".to_string(),
        ));
    }

    let expected_dependencies = fragment
        .connections
        .iter()
        .map(|connection| StartupDependency {
            prerequisite_placement_id: connection.sink_placement_id.clone(),
            dependent_placement_id: connection.source_placement_id.clone(),
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    if fragment.startup_dependencies != expected_dependencies {
        return Some((
            FailureReason::InvalidStartupDependencies,
            "startup dependencies do not match the exact cord endpoints".to_string(),
        ));
    }

    let local_placements = fragment
        .placements
        .iter()
        .map(|placement| placement.placement_id.clone())
        .collect::<BTreeSet<_>>();
    let ordered_placements = fragment
        .startup_order
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    if ordered_placements != local_placements
        || fragment.startup_order.len() != local_placements.len()
    {
        return Some((
            FailureReason::InvalidStartupDependencies,
            "startup order must name every local placement exactly once".to_string(),
        ));
    }
    let positions = fragment
        .startup_order
        .iter()
        .enumerate()
        .map(|(index, placement_id)| (placement_id.clone(), index))
        .collect::<BTreeMap<_, _>>();
    if fragment.startup_dependencies.iter().any(|dependency| {
        let prerequisite = positions.get(&dependency.prerequisite_placement_id);
        let dependent = positions.get(&dependency.dependent_placement_id);
        matches!((prerequisite, dependent), (Some(before), Some(after)) if before >= after)
    }) {
        return Some((
            FailureReason::InvalidStartupDependencies,
            "startup order violates a local prerequisite".to_string(),
        ));
    }

    let expected_terminals = fragment
        .placements
        .iter()
        .map(|placement| ExpectedTerminal::PlacementCompleted(placement.placement_id.clone()))
        .chain(fragment.connections.iter().map(|connection| {
            ExpectedTerminal::ConnectionCompleted(connection.connection_id.clone())
        }))
        .chain(core::iter::once(ExpectedTerminal::PlanCompleted))
        .collect::<Vec<_>>();
    if fragment.expected_terminals != expected_terminals {
        return Some((
            FailureReason::UnsupportedTerminalPolicy,
            "terminal requirements do not cover every planned placement and connection".to_string(),
        ));
    }

    let expected_evidence =
        core::iter::once(ExpectedEvidence::PlanFragmentReceived)
            .chain(fragment.placements.iter().map(|placement| {
                ExpectedEvidence::PlacementPrepared(placement.placement_id.clone())
            }))
            .chain(fragment.placements.iter().map(|placement| {
                ExpectedEvidence::PlacementTerminal(placement.placement_id.clone())
            }))
            .chain(fragment.connections.iter().map(|connection| {
                ExpectedEvidence::ConnectionTerminal(connection.connection_id.clone())
            }))
            .chain(core::iter::once(ExpectedEvidence::PlanTerminal))
            .collect::<Vec<_>>();
    if fragment.expected_evidence != expected_evidence {
        return Some((
            FailureReason::EvidenceBudgetExceeded,
            "mandatory evidence descriptors do not cover the exact fragment".to_string(),
        ));
    }
    let Some(required) = mandatory_evidence_storage_requirement(&fragment.expected_evidence) else {
        return Some((
            FailureReason::EvidenceBudgetExceeded,
            "mandatory evidence cannot be represented by the public budget types".to_string(),
        ));
    };
    if fragment.evidence_storage_budget.item_capacity < required.item_capacity
        || fragment.evidence_storage_budget.byte_capacity < required.byte_capacity
    {
        return Some((
            FailureReason::EvidenceBudgetExceeded,
            "mandatory evidence exceeds its planned item or byte budget".to_string(),
        ));
    }
    None
}

fn validate_host_operation_action(
    placement: &PlannedOperation,
    action: &OperationAction,
) -> Result<(), ImplementationFailure> {
    let (contract, target_kind, input_bytes) = match action {
        OperationAction::Wait { .. } => (
            conduit_core::WAIT_HOST_OPERATION_CONTRACT,
            None,
            core::mem::size_of::<u64>() as u32,
        ),
        OperationAction::Present {
            presentation_kind,
            value,
        } => (
            conduit_core::PRESENT_HOST_OPERATION_CONTRACT,
            Some(presentation_kind),
            value.encoded_len(),
        ),
        _ => return Ok(()),
    };
    let Some(requirement) = placement.host_operations.iter().find(|requirement| {
        requirement.contract_id.as_str() == contract
            && requirement.target_kind.as_ref() == target_kind
    }) else {
        return Err(ImplementationFailure::new(
            FailureReason::HostOperationNotPlanned,
            format!(
                "placement '{}' requested unplanned host operation '{}'",
                placement.placement_id.as_str(),
                contract
            ),
        ));
    };
    if requirement.maximum_in_flight == 0 || input_bytes > requirement.maximum_input_bytes {
        return Err(ImplementationFailure::new(
            FailureReason::HostOperationInputExceeded,
            format!(
                "placement '{}' host operation '{}' input requires {} bytes above bound {}",
                placement.placement_id.as_str(),
                contract,
                input_bytes,
                requirement.maximum_input_bytes
            ),
        ));
    }
    Ok(())
}

impl HostRuntime {
    pub fn new(
        advertisement: HostAdvertisement,
        implementations: ImplementationRegistry,
        observation_limit: usize,
    ) -> Self {
        let mut runtime = Self {
            advertisement,
            observation_limit,
            observations: Vec::new(),
            plans: BTreeMap::new(),
            released_plans: BTreeSet::new(),
            implementations,
        };
        runtime.record_observation(None, None, None, ObservationKind::HostStarted);
        runtime.record_observation(None, None, None, ObservationKind::AdvertisementPublished);
        runtime
    }

    pub fn advertisement(&self) -> &HostAdvertisement {
        &self.advertisement
    }

    pub fn handle(&mut self, command: HostCommand) -> RuntimeOutput {
        match command {
            HostCommand::PublishAdvertisement(advertisement) => {
                self.advertisement = advertisement;
                self.record_observation(None, None, None, ObservationKind::AdvertisementPublished);
                RuntimeOutput::default()
            }
            HostCommand::Prepare(fragment) => self.prepare(fragment),
            HostCommand::Activate(plan_id) => self.activate(&plan_id),
            HostCommand::CompleteWait {
                plan_id,
                placement_id,
            } => self.complete_wait(&plan_id, &placement_id),
            HostCommand::CompletePresentation {
                plan_id,
                placement_id,
                value,
                success,
                message,
            } => self.complete_presentation(&plan_id, &placement_id, value, success, message),
            HostCommand::AcceptConnectionEnvelope(envelope) => {
                self.accept_connection_envelope(envelope)
            }
            HostCommand::CompleteConnectionDelivery {
                plan_id,
                connection_id,
                sequence,
                outcome,
            } => self.complete_connection_delivery(&plan_id, &connection_id, sequence, outcome),
            HostCommand::CloseConnection {
                plan_id,
                connection_id,
            } => self.close_connection(&plan_id, &connection_id),
            HostCommand::Cancel(plan_id) => self.cancel(&plan_id),
            HostCommand::Release(plan_id) => self.release(&plan_id),
            HostCommand::Inspect => RuntimeOutput {
                events: vec![
                    HostEvent::Observations {
                        items: self.observations.clone(),
                    },
                    HostEvent::MandatoryEvidenceReports {
                        items: self
                            .plans
                            .iter()
                            .map(|(plan_id, plan)| {
                                plan.mandatory_evidence
                                    .report(plan_id.clone(), &plan.fragment.expected_evidence)
                            })
                            .collect(),
                    },
                ],
                effects: Vec::new(),
            },
        }
    }

    fn prepare(&mut self, fragment: PlanFragment) -> RuntimeOutput {
        let mut output = RuntimeOutput::default();
        if !verify_plan_fragment(&fragment) {
            output.events.push(HostEvent::PreparationRejected {
                plan_id: fragment.plan_id,
                reason: FailureReason::PlanIdentityMismatch,
                message: Some("plan fragment does not match its exact identity".to_string()),
            });
            return output;
        }
        if let Some((reason, message)) = validate_fragment_execution_contract(&fragment) {
            output.events.push(HostEvent::PreparationRejected {
                plan_id: fragment.plan_id,
                reason,
                message: Some(message),
            });
            return output;
        }
        if self.released_plans.contains(&fragment.plan_id) {
            output.events.push(HostEvent::CommandRejected {
                plan_id: Some(fragment.plan_id),
                reason: FailureReason::InvalidLifecycleCommand,
            });
            return output;
        }
        if fragment.host_id != self.advertisement.host_id {
            output.events.push(HostEvent::PreparationRejected {
                plan_id: fragment.plan_id,
                reason: FailureReason::WrongHostIdentity,
                message: Some("wrong host identity".to_string()),
            });
            return output;
        }
        if fragment.boot_id != self.advertisement.boot_id {
            output.events.push(HostEvent::PreparationRejected {
                plan_id: fragment.plan_id,
                reason: FailureReason::StaleBootIdentity,
                message: Some("stale boot identity".to_string()),
            });
            return output;
        }
        if fragment.offer_generation != self.advertisement.offer_generation {
            output.events.push(HostEvent::PreparationRejected {
                plan_id: fragment.plan_id,
                reason: FailureReason::StaleOfferGeneration,
                message: Some("stale offer generation".to_string()),
            });
            return output;
        }

        let mut counts = BTreeMap::<_, u16>::new();
        for placement in &fragment.placements {
            if !self
                .advertisement
                .capabilities
                .iter()
                .any(|offer| offer.kind_id == placement.kind_id)
            {
                output.events.push(HostEvent::PreparationRejected {
                    plan_id: fragment.plan_id,
                    reason: FailureReason::UnsupportedKind,
                    message: Some(format!(
                        "operation kind '{}' is not advertised by this host",
                        placement.kind_id.as_str()
                    )),
                });
                return output;
            }
            let capability = match self
                .advertisement
                .capabilities
                .iter()
                .find(|offer| offer.capability_id == placement.capability_id)
            {
                Some(capability) => capability,
                None => {
                    output.events.push(HostEvent::PreparationRejected {
                        plan_id: fragment.plan_id,
                        reason: FailureReason::UnknownCapability,
                        message: Some(format!(
                            "unknown capability '{}'",
                            placement.capability_id.as_str()
                        )),
                    });
                    return output;
                }
            };
            if capability.kind_id != placement.kind_id {
                output.events.push(HostEvent::PreparationRejected {
                    plan_id: fragment.plan_id,
                    reason: FailureReason::ImplementationKindMismatch,
                    message: Some(format!(
                        "capability '{}' advertises kind '{}' but placement requires '{}'",
                        capability.capability_id.as_str(),
                        capability.kind_id.as_str(),
                        placement.kind_id.as_str()
                    )),
                });
                return output;
            }
            if capability.kind_contract_revision != placement.kind_contract_revision {
                output.events.push(HostEvent::PreparationRejected {
                    plan_id: fragment.plan_id,
                    reason: FailureReason::KindContractRevisionMismatch,
                    message: Some(format!(
                        "capability '{}' advertises contract '{}' but placement pins '{}'",
                        capability.capability_id.as_str(),
                        capability.kind_contract_revision.as_str(),
                        placement.kind_contract_revision.as_str()
                    )),
                });
                return output;
            }
            if capability.execution_profile_id != placement.execution_profile_id {
                output.events.push(HostEvent::PreparationRejected {
                    plan_id: fragment.plan_id,
                    reason: FailureReason::ExecutionProfileMismatch,
                    message: Some(format!(
                        "capability '{}' advertises execution profile '{}' but placement pins '{}'",
                        capability.capability_id.as_str(),
                        capability.execution_profile_id.as_str(),
                        placement.execution_profile_id.as_str()
                    )),
                });
                return output;
            }
            if capability.inputs != placement.inputs || capability.outputs != placement.outputs {
                output.events.push(HostEvent::PreparationRejected {
                    plan_id: fragment.plan_id,
                    reason: FailureReason::PortContractMismatch,
                    message: Some(format!(
                        "capability '{}' port contracts differ from placement '{}'",
                        capability.capability_id.as_str(),
                        placement.placement_id.as_str()
                    )),
                });
                return output;
            }
            if capability.host_operations != placement.host_operations {
                output.events.push(HostEvent::PreparationRejected {
                    plan_id: fragment.plan_id,
                    reason: FailureReason::HostOperationContractMismatch,
                    message: Some(format!(
                        "capability '{}' host-operation requirements differ from placement '{}'",
                        capability.capability_id.as_str(),
                        placement.placement_id.as_str()
                    )),
                });
                return output;
            }
            if capability.implementation_id != placement.implementation_id {
                output.events.push(HostEvent::PreparationRejected {
                    plan_id: fragment.plan_id,
                    reason: FailureReason::AdvertisedImplementationMismatch,
                    message: Some(format!(
                        "capability '{}' advertises implementation '{}' but placement pins '{}'",
                        capability.capability_id.as_str(),
                        capability.implementation_id.as_str(),
                        placement.implementation_id.as_str()
                    )),
                });
                return output;
            }
            if capability.artifact_id != placement.artifact_id {
                output.events.push(HostEvent::PreparationRejected {
                    plan_id: fragment.plan_id,
                    reason: FailureReason::ArtifactIdentityMismatch,
                    message: Some(format!(
                        "capability '{}' advertises artifact '{}' but placement pins '{}'",
                        capability.capability_id.as_str(),
                        capability.artifact_id.as_str(),
                        placement.artifact_id.as_str()
                    )),
                });
                return output;
            }
            let Some(implementation) = self.implementations.get(&placement.implementation_id)
            else {
                output.events.push(HostEvent::PreparationRejected {
                    plan_id: fragment.plan_id,
                    reason: FailureReason::UnknownImplementation,
                    message: Some(format!(
                        "implementation '{}' is not installed",
                        placement.implementation_id.as_str()
                    )),
                });
                return output;
            };
            if implementation.kind_id() != &placement.kind_id {
                output.events.push(HostEvent::PreparationRejected {
                    plan_id: fragment.plan_id,
                    reason: FailureReason::ImplementationKindMismatch,
                    message: Some(format!(
                        "installed implementation '{}' realizes '{}' rather than '{}'",
                        placement.implementation_id.as_str(),
                        implementation.kind_id().as_str(),
                        placement.kind_id.as_str()
                    )),
                });
                return output;
            }
            if implementation.kind_contract_revision() != placement.kind_contract_revision {
                output.events.push(HostEvent::PreparationRejected {
                    plan_id: fragment.plan_id,
                    reason: FailureReason::KindContractRevisionMismatch,
                    message: Some(format!(
                        "installed implementation '{}' realizes contract '{}' rather than '{}'",
                        placement.implementation_id.as_str(),
                        implementation.kind_contract_revision().as_str(),
                        placement.kind_contract_revision.as_str()
                    )),
                });
                return output;
            }
            if implementation.execution_profile_id() != placement.execution_profile_id {
                output.events.push(HostEvent::PreparationRejected {
                    plan_id: fragment.plan_id,
                    reason: FailureReason::ExecutionProfileMismatch,
                    message: Some(format!(
                        "installed implementation '{}' uses execution profile '{}' rather than '{}'",
                        placement.implementation_id.as_str(),
                        implementation.execution_profile_id().as_str(),
                        placement.execution_profile_id.as_str()
                    )),
                });
                return output;
            }
            if implementation.host_operation_requirements() != placement.host_operations {
                output.events.push(HostEvent::PreparationRejected {
                    plan_id: fragment.plan_id,
                    reason: FailureReason::HostOperationContractMismatch,
                    message: Some(format!(
                        "installed implementation '{}' host-operation requirements differ from placement '{}'",
                        placement.implementation_id.as_str(),
                        placement.placement_id.as_str()
                    )),
                });
                return output;
            }
            if implementation.artifact_id() != &placement.artifact_id {
                output.events.push(HostEvent::PreparationRejected {
                    plan_id: fragment.plan_id,
                    reason: FailureReason::ArtifactIdentityMismatch,
                    message: Some(format!(
                        "installed implementation '{}' uses artifact '{}' rather than '{}'",
                        placement.implementation_id.as_str(),
                        implementation.artifact_id().as_str(),
                        placement.artifact_id.as_str()
                    )),
                });
                return output;
            }
            if placement
                .inputs
                .iter()
                .chain(&placement.outputs)
                .any(|port| {
                    implementation
                        .minimum_value_size(&port.value_kind)
                        .is_none()
                })
            {
                output.events.push(HostEvent::PreparationRejected {
                    plan_id: fragment.plan_id,
                    reason: FailureReason::UnsupportedValueKind,
                    message: Some(format!(
                        "implementation '{}' does not support every planned value kind",
                        placement.implementation_id.as_str()
                    )),
                });
                return output;
            }
            let count = counts.entry(placement.capability_id.clone()).or_insert(0);
            *count += 1;
            if *count > capability.limits.max_active_instances {
                output.events.push(HostEvent::PreparationRejected {
                    plan_id: fragment.plan_id,
                    reason: FailureReason::CapabilityInstanceLimitExceeded,
                    message: Some(format!(
                        "capability '{}' instance limit exceeded",
                        placement.capability_id.as_str()
                    )),
                });
                return output;
            }
        }

        let mut placements = BTreeMap::new();
        for spec in fragment.placements.iter().cloned() {
            let implementation = self
                .implementations
                .get(&spec.implementation_id)
                .expect("implementation validation already succeeded");
            let implementation_state = match implementation.prepare(&spec) {
                Ok(state) => state,
                Err(failure) => {
                    output.events.push(HostEvent::PreparationRejected {
                        plan_id: fragment.plan_id,
                        reason: failure.reason,
                        message: failure.message,
                    });
                    return output;
                }
            };
            placements.insert(
                spec.placement_id.clone(),
                RuntimePlacement {
                    spec,
                    lifecycle: PlacementLifecycleState::Prepared,
                    terminal: None,
                    implementation_state,
                    action: OperationAction::Idle,
                    effect_issued: false,
                    pending_input_connection: None,
                    inputs_closed_notified: false,
                },
            );
        }

        let mut connections = BTreeMap::new();
        for connection in &fragment.connections {
            let source = placements.get(&connection.source_placement_id);
            let sink = placements.get(&connection.sink_placement_id);
            let role = match (connection.provider, source.is_some(), sink.is_some()) {
                (ConnectionProvider::Local, true, true) => ConnectionRole::Local,
                (ConnectionProvider::InMemory, true, false) => ConnectionRole::Outbound,
                (ConnectionProvider::InMemory, false, true) => ConnectionRole::Inbound,
                (ConnectionProvider::FixtureFrame, true, false) => ConnectionRole::Outbound,
                (ConnectionProvider::FixtureFrame, false, true) => ConnectionRole::Inbound,
                (ConnectionProvider::FixtureDatagram, true, false) => ConnectionRole::Outbound,
                (ConnectionProvider::FixtureDatagram, false, true) => ConnectionRole::Inbound,
                _ => {
                    output.events.push(HostEvent::PreparationRejected {
                        plan_id: fragment.plan_id,
                        reason: FailureReason::InvalidOperationConfiguration,
                        message: Some(format!(
                            "connection '{}' has invalid local endpoints for {:?}",
                            connection.connection_id.as_str(),
                            connection.provider
                        )),
                    });
                    return output;
                }
            };
            let local_capabilities = source
                .into_iter()
                .chain(sink)
                .map(|placement| {
                    self.advertisement
                        .capabilities
                        .iter()
                        .find(|offer| offer.capability_id == placement.spec.capability_id)
                        .expect("placement capability must exist")
                })
                .collect::<Vec<_>>();
            if local_capabilities
                .iter()
                .any(|capability| connection.item_capacity > capability.limits.max_queue_items)
            {
                output.events.push(HostEvent::PreparationRejected {
                    plan_id: fragment.plan_id,
                    reason: FailureReason::QueueCapacityExceeded,
                    message: Some(format!(
                        "connection '{}' exceeds queue limits",
                        connection.connection_id.as_str()
                    )),
                });
                return output;
            }
            if local_capabilities
                .iter()
                .any(|capability| connection.byte_capacity > capability.limits.max_queue_bytes)
            {
                output.events.push(HostEvent::PreparationRejected {
                    plan_id: fragment.plan_id,
                    reason: FailureReason::ByteCapacityExceeded,
                    message: Some(format!(
                        "connection '{}' exceeds byte limits",
                        connection.connection_id.as_str()
                    )),
                });
                return output;
            }
            if source
                .into_iter()
                .chain(sink)
                .filter_map(|placement| {
                    self.implementations
                        .get(&placement.spec.implementation_id)
                        .and_then(|implementation| {
                            implementation.minimum_value_size(&connection.value_kind)
                        })
                })
                .max()
                .is_some_and(|minimum| connection.byte_capacity < minimum)
            {
                output.events.push(HostEvent::PreparationRejected {
                    plan_id: fragment.plan_id,
                    reason: FailureReason::ByteCapacityExceeded,
                    message: Some(format!(
                        "connection '{}' byte capacity is too small for value kind '{}'",
                        connection.connection_id.as_str(),
                        connection.value_kind.as_str()
                    )),
                });
                return output;
            }
            connections.insert(
                connection.connection_id.clone(),
                RuntimeConnection {
                    spec: connection.clone(),
                    queue: BoundedQueue::new(connection.item_capacity as usize),
                    queued_bytes: 0,
                    source_done: false,
                    sink_failed: false,
                    blocked: false,
                    last_accepted_sequence: None,
                    last_manifested_sequence: None,
                    terminal: None,
                    role,
                    transmission_in_flight: false,
                    next_expected_sequence: 0,
                    next_send_sequence: 0,
                    accepted_remote_sequences: BTreeSet::new(),
                },
            );
        }

        self.plans.insert(
            fragment.plan_id.clone(),
            RuntimePlan {
                mandatory_evidence: MandatoryEvidenceLog::new(&fragment),
                fragment: fragment.clone(),
                placements,
                connections,
                state: PlanState::Prepared,
                terminal: None,
                terminal_emitted: false,
            },
        );
        self.record_observation(
            Some(fragment.plan_id.clone()),
            None,
            None,
            ObservationKind::PlanFragmentReceived,
        );
        for placement in &fragment.placements {
            self.record_observation(
                Some(fragment.plan_id.clone()),
                Some(placement.placement_id.clone()),
                None,
                ObservationKind::PlacementPrepared,
            );
        }
        output.events.push(HostEvent::Prepared {
            plan_id: fragment.plan_id,
        });
        output
    }

    fn activate(&mut self, plan_id: &PlanId) -> RuntimeOutput {
        let mut output = RuntimeOutput::default();
        if self.released_plans.contains(plan_id) {
            output.events.push(HostEvent::CommandRejected {
                plan_id: Some(plan_id.clone()),
                reason: FailureReason::InvalidLifecycleCommand,
            });
            return output;
        }
        let Some(plan) = self.plans.get_mut(plan_id) else {
            output.events.push(HostEvent::ActivationRejected {
                plan_id: plan_id.clone(),
                reason: FailureReason::InvalidLifecycleCommand,
                message: Some("plan was not prepared".to_string()),
            });
            return output;
        };
        if plan.state != PlanState::Prepared {
            output.events.push(HostEvent::ActivationRejected {
                plan_id: plan_id.clone(),
                reason: FailureReason::InvalidLifecycleCommand,
                message: Some("plan is not in prepared state".to_string()),
            });
            return output;
        }
        plan.state = PlanState::Active;
        for placement_id in &plan.fragment.startup_order {
            if let Some(placement) = plan.placements.get_mut(placement_id) {
                placement.lifecycle = PlacementLifecycleState::Active;
                placement.action = placement.implementation_state.start();
                placement.effect_issued = false;
            }
        }
        self.record_observation(
            Some(plan_id.clone()),
            None,
            None,
            ObservationKind::PlanActivated,
        );
        output.events.push(HostEvent::Activated {
            plan_id: plan_id.clone(),
        });
        self.pump(plan_id, &mut output);
        output
    }

    fn complete_wait(&mut self, plan_id: &PlanId, placement_id: &PlacementId) -> RuntimeOutput {
        let mut output = RuntimeOutput::default();
        if self.released_plans.contains(plan_id) {
            output.events.push(HostEvent::CommandRejected {
                plan_id: Some(plan_id.clone()),
                reason: FailureReason::LatePlatformCompletion,
            });
            return output;
        }
        let Some(plan) = self.plans.get_mut(plan_id) else {
            output.events.push(HostEvent::CommandRejected {
                plan_id: Some(plan_id.clone()),
                reason: FailureReason::InvalidLifecycleCommand,
            });
            return output;
        };
        if plan.state != PlanState::Active && plan.state != PlanState::Failed {
            output.events.push(HostEvent::CommandRejected {
                plan_id: Some(plan_id.clone()),
                reason: FailureReason::LatePlatformCompletion,
            });
            return output;
        }
        let Some(placement) = plan.placements.get_mut(placement_id) else {
            output.events.push(HostEvent::CommandRejected {
                plan_id: Some(plan_id.clone()),
                reason: FailureReason::InvalidLifecycleCommand,
            });
            return output;
        };
        if placement.lifecycle != PlacementLifecycleState::Active
            || !placement.effect_issued
            || !matches!(placement.action, OperationAction::Wait { .. })
        {
            output.events.push(HostEvent::CommandRejected {
                plan_id: Some(plan_id.clone()),
                reason: FailureReason::LatePlatformCompletion,
            });
            return output;
        }
        placement.action = placement
            .implementation_state
            .resume(OperationCompletion::TimerElapsed);
        placement.effect_issued = false;
        self.pump(plan_id, &mut output);
        output
    }

    fn complete_presentation(
        &mut self,
        plan_id: &PlanId,
        placement_id: &PlacementId,
        value: ValuePayload,
        success: bool,
        message: Option<String>,
    ) -> RuntimeOutput {
        let mut output = RuntimeOutput::default();
        if self.released_plans.contains(plan_id) {
            output.events.push(HostEvent::CommandRejected {
                plan_id: Some(plan_id.clone()),
                reason: FailureReason::LatePlatformCompletion,
            });
            return output;
        }
        let Some(plan) = self.plans.get_mut(plan_id) else {
            output.events.push(HostEvent::CommandRejected {
                plan_id: Some(plan_id.clone()),
                reason: FailureReason::InvalidLifecycleCommand,
            });
            return output;
        };
        if plan.state != PlanState::Active && plan.state != PlanState::Failed {
            output.events.push(HostEvent::CommandRejected {
                plan_id: Some(plan_id.clone()),
                reason: FailureReason::LatePlatformCompletion,
            });
            return output;
        }
        let Some(placement) = plan.placements.get_mut(placement_id) else {
            output.events.push(HostEvent::CommandRejected {
                plan_id: Some(plan_id.clone()),
                reason: FailureReason::InvalidLifecycleCommand,
            });
            return output;
        };
        if placement.lifecycle != PlacementLifecycleState::Active {
            output.events.push(HostEvent::CommandRejected {
                plan_id: Some(plan_id.clone()),
                reason: FailureReason::LatePlatformCompletion,
            });
            return output;
        }
        let (presented_value, presentation_kind) = match &placement.action {
            OperationAction::Present {
                presentation_kind,
                value: action_value,
            } if placement.effect_issued && action_value == &value => {
                (action_value.clone(), presentation_kind)
            }
            _ => {
                output.events.push(HostEvent::CommandRejected {
                    plan_id: Some(plan_id.clone()),
                    reason: FailureReason::LatePlatformCompletion,
                });
                return output;
            }
        };
        let completion_bytes = match &message {
            Some(value) => u32::try_from(value.len()).unwrap_or(u32::MAX),
            None => 0,
        };
        let output_bound = placement
            .spec
            .host_operations
            .iter()
            .find(|requirement| {
                requirement.contract_id.as_str() == conduit_core::PRESENT_HOST_OPERATION_CONTRACT
                    && requirement.target_kind.as_ref() == Some(presentation_kind)
            })
            .map(|requirement| requirement.maximum_output_bytes);
        if output_bound.is_none_or(|bound| completion_bytes > bound) {
            output.events.push(HostEvent::CommandRejected {
                plan_id: Some(plan_id.clone()),
                reason: FailureReason::HostOperationOutputExceeded,
            });
            return output;
        }
        if success {
            if let Some(connection_id) = placement.pending_input_connection.take() {
                if let Some(connection) = plan.connections.get_mut(&connection_id) {
                    connection.last_manifested_sequence = connection.last_accepted_sequence;
                }
            }
            output.events.push(HostEvent::ManifestationCompleted {
                plan_id: plan_id.clone(),
                placement_id: placement_id.clone(),
                value: presented_value.clone(),
            });
        } else {
            output.events.push(HostEvent::ManifestationFailed {
                plan_id: plan_id.clone(),
                placement_id: placement_id.clone(),
                value: presented_value.clone(),
                reason: FailureReason::ManifestationFailed,
                message: message.clone(),
            });
        }
        placement.action = placement
            .implementation_state
            .resume(OperationCompletion::PresentationCompleted { success, message });
        placement.effect_issued = false;
        let _ = plan;
        if success {
            self.record_observation(
                Some(plan_id.clone()),
                Some(placement_id.clone()),
                None,
                ObservationKind::ValuePresented {
                    value: presented_value,
                },
            );
        }
        self.pump(plan_id, &mut output);
        output
    }

    fn accept_connection_envelope(&mut self, envelope: ConnectionEnvelope) -> RuntimeOutput {
        let mut output = RuntimeOutput::default();
        let plan_id = envelope.plan_id.clone();
        let connection_id = envelope.connection_id.clone();
        let sequence = envelope.sequence;
        if envelope.protocol_version != PROTOCOL_VERSION {
            output.events.push(HostEvent::ConnectionEnvelopeOutcome {
                plan_id,
                connection_id,
                sequence,
                outcome: ConnectionOutcome::Malformed,
            });
            return output;
        }
        if self.released_plans.contains(&plan_id) {
            output.events.push(HostEvent::ConnectionEnvelopeOutcome {
                plan_id,
                connection_id,
                sequence,
                outcome: ConnectionOutcome::Terminal,
            });
            return output;
        }
        let Some(plan) = self.plans.get_mut(&plan_id) else {
            output.events.push(HostEvent::ConnectionEnvelopeOutcome {
                plan_id,
                connection_id,
                sequence,
                outcome: ConnectionOutcome::Malformed,
            });
            return output;
        };
        if plan.state != PlanState::Active {
            output.events.push(HostEvent::ConnectionEnvelopeOutcome {
                plan_id,
                connection_id,
                sequence,
                outcome: ConnectionOutcome::Terminal,
            });
            return output;
        }
        let Some(connection) = plan.connections.get_mut(&connection_id) else {
            output.events.push(HostEvent::ConnectionEnvelopeOutcome {
                plan_id,
                connection_id,
                sequence,
                outcome: ConnectionOutcome::Malformed,
            });
            return output;
        };
        let malformed = connection.role != ConnectionRole::Inbound
            || connection.terminal.is_some()
            || envelope.value_kind != connection.spec.value_kind
            || envelope.encoded_len() > connection.spec.byte_capacity
            || sequence != connection.next_expected_sequence;
        if malformed {
            output.events.push(HostEvent::ConnectionEnvelopeOutcome {
                plan_id,
                connection_id,
                sequence,
                outcome: ConnectionOutcome::Malformed,
            });
            return output;
        }
        if connection.queue.len() >= connection.queue.capacity()
            || connection.queued_bytes + envelope.encoded_len() > connection.spec.byte_capacity
        {
            output.events.push(HostEvent::ConnectionEnvelopeOutcome {
                plan_id,
                connection_id,
                sequence,
                outcome: ConnectionOutcome::Full,
            });
            return output;
        }
        let value = envelope.into_value();
        connection.queued_bytes += value.encoded_len();
        connection
            .queue
            .push(QueuedValue { sequence, value })
            .expect("connection capacity was checked");
        connection.next_expected_sequence += 1;
        output.events.push(HostEvent::ConnectionEnvelopeOutcome {
            plan_id: plan_id.clone(),
            connection_id,
            sequence,
            outcome: ConnectionOutcome::Accepted,
        });
        self.pump(&plan_id, &mut output);
        output
    }

    fn complete_connection_delivery(
        &mut self,
        plan_id: &PlanId,
        connection_id: &ConnectionId,
        sequence: u64,
        outcome: ConnectionOutcome,
    ) -> RuntimeOutput {
        let mut output = RuntimeOutput::default();
        let mut pending_observations = Vec::new();
        let mut pending_terminal_events = Vec::new();
        let Some(plan) = self.plans.get_mut(plan_id) else {
            output.events.push(HostEvent::CommandRejected {
                plan_id: Some(plan_id.clone()),
                reason: FailureReason::StalePlan,
            });
            return output;
        };
        let Some(connection) = plan.connections.get_mut(connection_id) else {
            output.events.push(HostEvent::CommandRejected {
                plan_id: Some(plan_id.clone()),
                reason: FailureReason::MalformedConnectionEnvelope,
            });
            return output;
        };
        if connection.role != ConnectionRole::Outbound {
            output.events.push(HostEvent::CommandRejected {
                plan_id: Some(plan_id.clone()),
                reason: FailureReason::MalformedConnectionEnvelope,
            });
            return output;
        }
        let front_sequence = connection.queue.front().map(|queued| queued.sequence);
        let requires_in_flight = matches!(
            outcome,
            ConnectionOutcome::Accepted | ConnectionOutcome::Full
        );
        if requires_in_flight
            && (!connection.transmission_in_flight || front_sequence != Some(sequence))
        {
            output.events.push(HostEvent::CommandRejected {
                plan_id: Some(plan_id.clone()),
                reason: FailureReason::MalformedConnectionEnvelope,
            });
            return output;
        }
        if outcome == ConnectionOutcome::Malformed
            && !((connection.transmission_in_flight && front_sequence == Some(sequence))
                || connection.accepted_remote_sequences.contains(&sequence))
        {
            output.events.push(HostEvent::CommandRejected {
                plan_id: Some(plan_id.clone()),
                reason: FailureReason::MalformedConnectionEnvelope,
            });
            return output;
        }
        let mut should_pump = true;
        match outcome {
            ConnectionOutcome::Accepted => {
                let value = connection.queue.pop().expect("accepted value must exist");
                connection.queued_bytes -= value.value.encoded_len();
                connection.accepted_remote_sequences.insert(sequence);
                connection.transmission_in_flight = false;
                connection.blocked = false;
                output.events.push(HostEvent::ConnectionEnvelopeOutcome {
                    plan_id: plan_id.clone(),
                    connection_id: connection_id.clone(),
                    sequence,
                    outcome: ConnectionOutcome::Accepted,
                });
            }
            ConnectionOutcome::Delivered => {
                if !connection.accepted_remote_sequences.remove(&sequence) {
                    output.events.push(HostEvent::CommandRejected {
                        plan_id: Some(plan_id.clone()),
                        reason: FailureReason::MalformedConnectionEnvelope,
                    });
                    return output;
                }
                connection.last_accepted_sequence = Some(sequence);
                output.events.push(HostEvent::ConnectionEnvelopeOutcome {
                    plan_id: plan_id.clone(),
                    connection_id: connection_id.clone(),
                    sequence,
                    outcome: ConnectionOutcome::Delivered,
                });
                if connection.source_done
                    && connection.queue.is_empty()
                    && connection.accepted_remote_sequences.is_empty()
                    && !connection.transmission_in_flight
                {
                    terminate_connection(
                        connection,
                        TerminalDisposition::Completed,
                        &mut pending_observations,
                        &mut pending_terminal_events,
                        plan_id,
                    );
                }
            }
            ConnectionOutcome::Full | ConnectionOutcome::Ready => {
                connection.transmission_in_flight = false;
                connection.blocked = true;
                should_pump = false;
                output.events.push(HostEvent::ConnectionBlocked {
                    plan_id: plan_id.clone(),
                    connection_id: connection_id.clone(),
                });
            }
            ConnectionOutcome::Disconnected
            | ConnectionOutcome::Malformed
            | ConnectionOutcome::Terminal => {
                connection.transmission_in_flight = false;
                connection.accepted_remote_sequences.remove(&sequence);
                terminate_connection(
                    connection,
                    TerminalDisposition::Failed {
                        reason: if outcome == ConnectionOutcome::Malformed {
                            FailureReason::MalformedConnectionEnvelope
                        } else {
                            FailureReason::ConnectionDisconnected
                        },
                    },
                    &mut pending_observations,
                    &mut pending_terminal_events,
                    plan_id,
                );
                plan.state = PlanState::Failed;
                plan.terminal = Some(TerminalDisposition::Failed {
                    reason: FailureReason::RequiredBranchFailed,
                });
                cancel_active_sources(
                    plan,
                    CancellationReason::RequiredPlanFailed,
                    &mut pending_observations,
                    &mut pending_terminal_events,
                    plan_id,
                );
            }
        }
        let _ = plan;
        for item in pending_observations {
            self.record_observation(item.0, item.1, item.2, item.3);
        }
        output.events.extend(pending_terminal_events);
        if should_pump {
            self.pump(plan_id, &mut output);
        }
        output
    }

    fn close_connection(
        &mut self,
        plan_id: &PlanId,
        connection_id: &ConnectionId,
    ) -> RuntimeOutput {
        let mut output = RuntimeOutput::default();
        let Some(plan) = self.plans.get_mut(plan_id) else {
            output.events.push(HostEvent::CommandRejected {
                plan_id: Some(plan_id.clone()),
                reason: FailureReason::StalePlan,
            });
            return output;
        };
        let Some(connection) = plan.connections.get_mut(connection_id) else {
            output.events.push(HostEvent::CommandRejected {
                plan_id: Some(plan_id.clone()),
                reason: FailureReason::MalformedConnectionEnvelope,
            });
            return output;
        };
        if connection.role != ConnectionRole::Inbound || connection.terminal.is_some() {
            output.events.push(HostEvent::CommandRejected {
                plan_id: Some(plan_id.clone()),
                reason: FailureReason::InvalidLifecycleCommand,
            });
            return output;
        }
        connection.source_done = true;
        self.pump(plan_id, &mut output);
        output
    }

    fn cancel(&mut self, plan_id: &PlanId) -> RuntimeOutput {
        let mut output = RuntimeOutput::default();
        if self.released_plans.contains(plan_id) {
            output.events.push(HostEvent::CommandRejected {
                plan_id: Some(plan_id.clone()),
                reason: FailureReason::InvalidLifecycleCommand,
            });
            return output;
        }
        let Some(plan) = self.plans.get_mut(plan_id) else {
            output.events.push(HostEvent::CommandRejected {
                plan_id: Some(plan_id.clone()),
                reason: FailureReason::InvalidLifecycleCommand,
            });
            return output;
        };
        if plan.state == PlanState::Cancelled || plan.state == PlanState::Completed {
            output.events.push(HostEvent::CommandRejected {
                plan_id: Some(plan_id.clone()),
                reason: FailureReason::InvalidLifecycleCommand,
            });
            return output;
        }

        let mut pending_observations = Vec::new();
        let mut pending_terminal_events = Vec::new();
        plan.state = PlanState::Cancelled;
        plan.terminal = Some(TerminalDisposition::Cancelled {
            reason: CancellationReason::OperatorRequested,
        });
        cancel_all_placements_and_connections(
            plan,
            CancellationReason::OperatorRequested,
            &mut pending_observations,
            &mut pending_terminal_events,
            plan_id,
        );
        let _ = plan;
        for item in pending_observations {
            self.record_observation(item.0, item.1, item.2, item.3);
        }
        self.record_observation(
            Some(plan_id.clone()),
            None,
            None,
            ObservationKind::Cancelled,
        );
        output.events.push(HostEvent::Cancelled {
            plan_id: plan_id.clone(),
        });
        output.events.extend(pending_terminal_events);
        self.finalize_terminal_plan(plan_id, &mut output);
        output
    }

    fn release(&mut self, plan_id: &PlanId) -> RuntimeOutput {
        let mut output = RuntimeOutput::default();
        if self.released_plans.contains(plan_id) {
            output.events.push(HostEvent::CommandRejected {
                plan_id: Some(plan_id.clone()),
                reason: FailureReason::InvalidLifecycleCommand,
            });
            return output;
        }
        let Some(plan) = self.plans.get(plan_id) else {
            output.events.push(HostEvent::CommandRejected {
                plan_id: Some(plan_id.clone()),
                reason: FailureReason::InvalidLifecycleCommand,
            });
            return output;
        };
        if plan.state != PlanState::Completed
            && plan.state != PlanState::Failed
            && plan.state != PlanState::Cancelled
        {
            output.events.push(HostEvent::CommandRejected {
                plan_id: Some(plan_id.clone()),
                reason: FailureReason::InvalidLifecycleCommand,
            });
            return output;
        }
        if let Some(plan) = self.plans.get_mut(plan_id) {
            for placement in plan.placements.values_mut() {
                placement.implementation_state.release();
                placement.lifecycle = PlacementLifecycleState::Released;
            }
        }
        self.plans.remove(plan_id);
        self.released_plans.insert(plan_id.clone());
        self.record_observation(Some(plan_id.clone()), None, None, ObservationKind::Released);
        output.events.push(HostEvent::Released {
            plan_id: plan_id.clone(),
        });
        output
    }

    fn pump(&mut self, plan_id: &PlanId, output: &mut RuntimeOutput) {
        loop {
            let mut changed = false;
            let mut pending_observations = Vec::new();
            let mut pending_terminal_events = Vec::new();
            let Some(plan) = self.plans.get_mut(plan_id) else {
                return;
            };
            if plan.state != PlanState::Active && plan.state != PlanState::Failed {
                return;
            }

            let placement_ids = plan.placements.keys().cloned().collect::<Vec<_>>();
            for placement_id in placement_ids {
                let action = match plan.placements.get(&placement_id) {
                    Some(placement) if placement.lifecycle == PlacementLifecycleState::Active => {
                        placement.action.clone()
                    }
                    _ => continue,
                };
                let host_operation_failure =
                    plan.placements.get(&placement_id).and_then(|placement| {
                        validate_host_operation_action(&placement.spec, &action).err()
                    });
                if let Some(failure) = host_operation_failure {
                    fail_operation(
                        plan,
                        &placement_id,
                        failure,
                        &mut pending_observations,
                        &mut pending_terminal_events,
                        plan_id,
                    );
                    changed = true;
                    continue;
                }
                match action {
                    OperationAction::Idle => {}
                    OperationAction::Wait { duration_ms } => {
                        let placement = plan
                            .placements
                            .get_mut(&placement_id)
                            .expect("placement exists");
                        if !placement.effect_issued {
                            placement.effect_issued = true;
                            output.events.push(HostEvent::TimerRequested {
                                plan_id: plan_id.clone(),
                                placement_id: placement_id.clone(),
                                duration_ms,
                            });
                            output.effects.push(PlatformEffect::Wait {
                                plan_id: plan_id.clone(),
                                placement_id: placement_id.clone(),
                                duration_ms,
                            });
                        }
                    }
                    OperationAction::Present {
                        presentation_kind,
                        value,
                    } => {
                        let placement = plan
                            .placements
                            .get_mut(&placement_id)
                            .expect("placement exists");
                        if !placement.effect_issued {
                            placement.effect_issued = true;
                            output.events.push(HostEvent::PresentValueRequested {
                                plan_id: plan_id.clone(),
                                placement_id: placement_id.clone(),
                                presentation_kind: presentation_kind.clone(),
                                value: value.clone(),
                            });
                            output.effects.push(PlatformEffect::PresentValue {
                                plan_id: plan_id.clone(),
                                placement_id: placement_id.clone(),
                                presentation_kind,
                                value,
                            });
                        }
                    }
                    OperationAction::Emit(value) => {
                        let outgoing = outgoing_connections(&placement_id, &plan.connections);
                        let blocked = outgoing.iter().find(|connection_id| {
                            let connection = &plan.connections[*connection_id];
                            connection.terminal.is_none()
                                && !connection.sink_failed
                                && (connection.queue.len() >= connection.queue.capacity()
                                    || connection.queued_bytes + value.encoded_len()
                                        > connection.spec.byte_capacity)
                        });
                        if let Some(connection_id) = blocked.cloned() {
                            let connection = plan
                                .connections
                                .get_mut(&connection_id)
                                .expect("connection exists");
                            if !connection.blocked {
                                connection.blocked = true;
                                output.events.push(HostEvent::ConnectionBlocked {
                                    plan_id: plan_id.clone(),
                                    connection_id,
                                });
                            }
                            continue;
                        }
                        for connection_id in outgoing {
                            let connection = plan
                                .connections
                                .get_mut(&connection_id)
                                .expect("connection exists");
                            if connection.terminal.is_some() || connection.sink_failed {
                                continue;
                            }
                            connection.blocked = false;
                            connection.queued_bytes += value.encoded_len();
                            let sequence = connection.next_send_sequence;
                            connection.next_send_sequence += 1;
                            connection
                                .queue
                                .push(QueuedValue {
                                    sequence,
                                    value: value.clone(),
                                })
                                .expect("capacity was checked before push");
                            output.events.push(HostEvent::ValueDelivered {
                                plan_id: plan_id.clone(),
                                connection_id: connection_id.clone(),
                                value: value.clone(),
                            });
                            pending_observations.push((
                                Some(plan_id.clone()),
                                Some(placement_id.clone()),
                                Some(connection_id),
                                ObservationKind::ValueProduced {
                                    value: value.clone(),
                                },
                            ));
                        }
                        let placement = plan
                            .placements
                            .get_mut(&placement_id)
                            .expect("placement exists");
                        placement.action = placement
                            .implementation_state
                            .resume(OperationCompletion::Emitted);
                        placement.effect_issued = false;
                        changed = true;
                    }
                    OperationAction::Complete => {
                        let has_outputs = plan
                            .placements
                            .get(&placement_id)
                            .is_some_and(|placement| !placement.spec.outputs.is_empty());
                        if has_outputs {
                            mark_source_done(&placement_id, &mut plan.connections);
                        }
                        let incoming = incoming_connections(&placement_id, &plan.connections);
                        let placement = plan
                            .placements
                            .get_mut(&placement_id)
                            .expect("placement exists");
                        terminate_placement(
                            placement,
                            TerminalDisposition::Completed,
                            &mut pending_observations,
                            &mut pending_terminal_events,
                            plan_id,
                        );
                        output.events.push(HostEvent::PlacementCompleted {
                            plan_id: plan_id.clone(),
                            placement_id: placement_id.clone(),
                        });
                        for connection_id in incoming {
                            if let Some(connection) = plan.connections.get_mut(&connection_id) {
                                if connection.terminal.is_none() {
                                    terminate_connection(
                                        connection,
                                        TerminalDisposition::Completed,
                                        &mut pending_observations,
                                        &mut pending_terminal_events,
                                        plan_id,
                                    );
                                }
                            }
                        }
                        changed = true;
                    }
                    OperationAction::Fail(failure) => {
                        fail_operation(
                            plan,
                            &placement_id,
                            failure,
                            &mut pending_observations,
                            &mut pending_terminal_events,
                            plan_id,
                        );
                        changed = true;
                    }
                }
            }

            let connection_ids = plan.connections.keys().cloned().collect::<Vec<_>>();
            for connection_id in connection_ids {
                let Some(connection) = plan.connections.get_mut(&connection_id) else {
                    continue;
                };
                if connection.role == ConnectionRole::Outbound {
                    if connection.terminal.is_none()
                        && connection.source_done
                        && connection.queue.is_empty()
                        && connection.accepted_remote_sequences.is_empty()
                        && !connection.transmission_in_flight
                    {
                        terminate_connection(
                            connection,
                            TerminalDisposition::Completed,
                            &mut pending_observations,
                            &mut pending_terminal_events,
                            plan_id,
                        );
                        changed = true;
                    } else if connection.terminal.is_none()
                        && !connection.transmission_in_flight
                        && !connection.queue.is_empty()
                    {
                        let queued = connection
                            .queue
                            .front()
                            .expect("non-empty outbound queue has a front value");
                        connection.transmission_in_flight = true;
                        output.effects.push(PlatformEffect::TransmitConnection {
                            envelope: ConnectionEnvelope {
                                protocol_version: PROTOCOL_VERSION,
                                plan_id: plan_id.clone(),
                                connection_id: connection_id.clone(),
                                sequence: queued.sequence,
                                value_kind: queued.value.value_kind.clone(),
                                payload: queued.value.encoded.clone(),
                            },
                        });
                    }
                    continue;
                }
                if connection.queue.is_empty()
                    || connection.sink_failed
                    || connection.terminal.is_some()
                {
                    continue;
                }
                let sink_id = connection.spec.sink_placement_id.clone();
                let Some(sink) = plan.placements.get_mut(&sink_id) else {
                    continue;
                };
                if sink.lifecycle != PlacementLifecycleState::Active
                    || !matches!(sink.action, OperationAction::Idle)
                    || sink.pending_input_connection.is_some()
                {
                    continue;
                }
                let queued = connection
                    .queue
                    .pop()
                    .expect("queue was checked before pop");
                connection.queued_bytes -= queued.value.encoded_len();
                connection.last_accepted_sequence = Some(queued.sequence);
                sink.pending_input_connection = Some(connection_id.clone());
                sink.action = sink
                    .implementation_state
                    .resume(OperationCompletion::Value(queued.value.clone()));
                sink.effect_issued = false;
                pending_observations.push((
                    Some(plan_id.clone()),
                    Some(sink_id),
                    Some(connection_id),
                    ObservationKind::ValueAccepted {
                        value: queued.value,
                    },
                ));
                changed = true;
            }

            let consumer_ids = plan
                .placements
                .iter()
                .filter(|(_, placement)| !placement.spec.inputs.is_empty())
                .map(|(placement_id, _)| placement_id.clone())
                .collect::<Vec<_>>();
            for consumer_id in consumer_ids {
                let Some(consumer) = plan.placements.get(&consumer_id) else {
                    continue;
                };
                if consumer.lifecycle != PlacementLifecycleState::Active
                    || consumer.inputs_closed_notified
                    || consumer.pending_input_connection.is_some()
                    || !matches!(consumer.action, OperationAction::Idle)
                {
                    continue;
                }
                let incoming = incoming_connections(&consumer_id, &plan.connections);
                let done = incoming.iter().all(|connection_id| {
                    let connection = &plan.connections[connection_id];
                    (connection.source_done
                        || connection.sink_failed
                        || connection.terminal.is_some())
                        && connection.queue.is_empty()
                });
                if done {
                    let consumer = plan
                        .placements
                        .get_mut(&consumer_id)
                        .expect("consumer exists");
                    consumer.inputs_closed_notified = true;
                    consumer.action = consumer
                        .implementation_state
                        .resume(OperationCompletion::InputsClosed);
                    consumer.effect_issued = false;
                    changed = true;
                }
            }

            let all_terminal = plan
                .placements
                .values()
                .all(|placement| placement.terminal.is_some())
                && plan
                    .connections
                    .values()
                    .all(|connection| connection.terminal.is_some());
            let should_emit_completed = plan.state == PlanState::Active && all_terminal;
            let should_emit_failed = plan.state == PlanState::Failed && all_terminal;

            let _ = plan;

            for item in pending_observations {
                self.record_observation(item.0, item.1, item.2, item.3);
            }
            output.events.extend(pending_terminal_events);

            if should_emit_completed {
                if let Some(plan) = self.plans.get_mut(plan_id) {
                    plan.state = PlanState::Completed;
                    plan.terminal = Some(TerminalDisposition::Completed);
                }
                output.events.push(HostEvent::PlanCompleted {
                    plan_id: plan_id.clone(),
                });
                self.finalize_terminal_plan(plan_id, output);
                return;
            }

            if should_emit_failed {
                self.finalize_terminal_plan(plan_id, output);
                return;
            }

            if !changed {
                return;
            }
        }
    }

    fn finalize_terminal_plan(&mut self, plan_id: &PlanId, output: &mut RuntimeOutput) {
        let Some(plan) = self.plans.get_mut(plan_id) else {
            return;
        };
        if plan.terminal_emitted {
            return;
        }
        let disposition = plan.terminal.unwrap_or(TerminalDisposition::Completed);
        plan.terminal_emitted = true;
        let _ = plan;
        self.record_observation(
            Some(plan_id.clone()),
            None,
            None,
            ObservationKind::PlanTerminal { disposition },
        );
        output.events.push(HostEvent::PlanTerminated {
            plan_id: plan_id.clone(),
            disposition,
        });
    }

    fn record_observation(
        &mut self,
        plan_id: Option<PlanId>,
        placement_id: Option<PlacementId>,
        connection_id: Option<ConnectionId>,
        kind: ObservationKind,
    ) {
        let mandatory_evidence = match (&kind, &placement_id, &connection_id) {
            (ObservationKind::PlanFragmentReceived, _, _) => {
                Some(ExpectedEvidence::PlanFragmentReceived)
            }
            (ObservationKind::PlacementPrepared, Some(placement_id), _) => {
                Some(ExpectedEvidence::PlacementPrepared(placement_id.clone()))
            }
            (ObservationKind::PlacementTerminal { .. }, Some(placement_id), _) => {
                Some(ExpectedEvidence::PlacementTerminal(placement_id.clone()))
            }
            (ObservationKind::ConnectionTerminal { .. }, _, Some(connection_id)) => {
                Some(ExpectedEvidence::ConnectionTerminal(connection_id.clone()))
            }
            (ObservationKind::PlanTerminal { .. }, _, _) => Some(ExpectedEvidence::PlanTerminal),
            _ => None,
        };
        if let (Some(plan_id), Some(evidence)) = (&plan_id, mandatory_evidence) {
            if let Some(plan) = self.plans.get_mut(plan_id) {
                plan.mandatory_evidence
                    .record(&plan.fragment.expected_evidence, evidence);
            }
        }
        if self.observation_limit == 0 {
            return;
        }
        if self.observations.len() < self.observation_limit {
            self.observations.push(Observation {
                host_id: self.advertisement.host_id.clone(),
                boot_id: self.advertisement.boot_id.clone(),
                plan_id,
                placement_id,
                connection_id,
                kind,
            });
            return;
        }

        let mut dropped = 1u64;
        if let Some(Observation {
            kind: ObservationKind::EvidenceGap { dropped: previous },
            ..
        }) = self.observations.first()
        {
            dropped += *previous;
            self.observations.remove(0);
        } else {
            self.observations.remove(0);
        }
        if self.observation_limit == 1 {
            self.observations.clear();
            self.observations.push(Observation {
                host_id: self.advertisement.host_id.clone(),
                boot_id: self.advertisement.boot_id.clone(),
                plan_id: None,
                placement_id: None,
                connection_id: None,
                kind: ObservationKind::EvidenceGap { dropped },
            });
            return;
        }
        while self.observations.len() > self.observation_limit - 2 {
            self.observations.remove(0);
            dropped += 1;
        }
        self.observations.insert(
            0,
            Observation {
                host_id: self.advertisement.host_id.clone(),
                boot_id: self.advertisement.boot_id.clone(),
                plan_id: None,
                placement_id: None,
                connection_id: None,
                kind: ObservationKind::EvidenceGap { dropped },
            },
        );
        self.observations.push(Observation {
            host_id: self.advertisement.host_id.clone(),
            boot_id: self.advertisement.boot_id.clone(),
            plan_id,
            placement_id,
            connection_id,
            kind,
        });
    }
}

fn terminate_placement(
    placement: &mut RuntimePlacement,
    disposition: TerminalDisposition,
    observations: &mut Vec<PendingObservation>,
    events: &mut Vec<HostEvent>,
    plan_id: &PlanId,
) {
    if placement.terminal.is_some() {
        return;
    }
    placement.terminal = Some(disposition);
    placement.lifecycle = match disposition {
        TerminalDisposition::Completed => PlacementLifecycleState::Completed,
        TerminalDisposition::Failed { .. } => PlacementLifecycleState::Failed,
        TerminalDisposition::Cancelled { .. } => PlacementLifecycleState::Cancelled,
    };
    observations.push((
        Some(plan_id.clone()),
        Some(placement.spec.placement_id.clone()),
        None,
        ObservationKind::PlacementTerminal { disposition },
    ));
    events.push(HostEvent::PlacementTerminated {
        plan_id: plan_id.clone(),
        placement_id: placement.spec.placement_id.clone(),
        disposition,
    });
}

fn terminate_connection(
    connection: &mut RuntimeConnection,
    disposition: TerminalDisposition,
    observations: &mut Vec<PendingObservation>,
    events: &mut Vec<HostEvent>,
    plan_id: &PlanId,
) {
    if connection.terminal.is_some() {
        return;
    }
    let report = ConnectionTerminalDisposition {
        disposition,
        last_accepted_sequence: connection.last_accepted_sequence,
        last_manifested_sequence: connection.last_manifested_sequence,
        undeliverable_items: connection
            .queue
            .len()
            .saturating_add(connection.accepted_remote_sequences.len())
            as u16,
    };
    connection.queued_bytes = 0;
    while connection.queue.pop().is_some() {}
    connection.accepted_remote_sequences.clear();
    connection.terminal = Some(report.clone());
    observations.push((
        Some(plan_id.clone()),
        None,
        Some(connection.spec.connection_id.clone()),
        ObservationKind::ConnectionTerminal {
            disposition: report.clone(),
        },
    ));
    events.push(HostEvent::ConnectionTerminated {
        plan_id: plan_id.clone(),
        connection_id: connection.spec.connection_id.clone(),
        disposition: report,
    });
}

fn fail_operation(
    plan: &mut RuntimePlan,
    placement_id: &PlacementId,
    failure: ImplementationFailure,
    observations: &mut Vec<PendingObservation>,
    events: &mut Vec<HostEvent>,
    plan_id: &PlanId,
) {
    if let Some(placement) = plan.placements.get_mut(placement_id) {
        terminate_placement(
            placement,
            TerminalDisposition::Failed {
                reason: failure.reason,
            },
            observations,
            events,
            plan_id,
        );
    }
    for connection_id in incoming_connections(placement_id, &plan.connections) {
        if let Some(connection) = plan.connections.get_mut(&connection_id) {
            connection.sink_failed = true;
            terminate_connection(
                connection,
                TerminalDisposition::Failed {
                    reason: failure.reason,
                },
                observations,
                events,
                plan_id,
            );
        }
    }
    for connection_id in outgoing_connections(placement_id, &plan.connections) {
        if let Some(connection) = plan.connections.get_mut(&connection_id) {
            terminate_connection(
                connection,
                TerminalDisposition::Failed {
                    reason: failure.reason,
                },
                observations,
                events,
                plan_id,
            );
        }
    }
    observations.push((
        Some(plan_id.clone()),
        Some(placement_id.clone()),
        None,
        ObservationKind::Failure {
            reason: failure.reason,
            message: failure.message,
        },
    ));
    if plan.state != PlanState::Failed {
        plan.state = PlanState::Failed;
        plan.terminal = Some(TerminalDisposition::Failed {
            reason: FailureReason::RequiredBranchFailed,
        });
        cancel_active_sources(
            plan,
            CancellationReason::RequiredPlanFailed,
            observations,
            events,
            plan_id,
        );
    }
}

fn cancel_active_sources(
    plan: &mut RuntimePlan,
    reason: CancellationReason,
    observations: &mut Vec<PendingObservation>,
    events: &mut Vec<HostEvent>,
    plan_id: &PlanId,
) {
    let placement_ids = plan.placements.keys().cloned().collect::<Vec<_>>();
    for placement_id in placement_ids {
        let Some(placement) = plan.placements.get_mut(&placement_id) else {
            continue;
        };
        if !placement.spec.outputs.is_empty()
            && placement.lifecycle == PlacementLifecycleState::Active
        {
            placement.implementation_state.cancel();
            terminate_placement(
                placement,
                TerminalDisposition::Cancelled { reason },
                observations,
                events,
                plan_id,
            );
            mark_source_done(&placement_id, &mut plan.connections);
        }
    }
}

fn cancel_all_placements_and_connections(
    plan: &mut RuntimePlan,
    reason: CancellationReason,
    observations: &mut Vec<PendingObservation>,
    events: &mut Vec<HostEvent>,
    plan_id: &PlanId,
) {
    for placement in plan.placements.values_mut() {
        if placement.terminal.is_none() {
            placement.implementation_state.cancel();
            terminate_placement(
                placement,
                TerminalDisposition::Cancelled { reason },
                observations,
                events,
                plan_id,
            );
        }
        placement.action = OperationAction::Idle;
        placement.effect_issued = false;
        placement.pending_input_connection = None;
    }
    for connection in plan.connections.values_mut() {
        if connection.terminal.is_none() {
            terminate_connection(
                connection,
                TerminalDisposition::Cancelled { reason },
                observations,
                events,
                plan_id,
            );
        }
    }
}

fn outgoing_connections(
    placement_id: &PlacementId,
    connections: &BTreeMap<ConnectionId, RuntimeConnection>,
) -> Vec<ConnectionId> {
    connections
        .iter()
        .filter_map(|(connection_id, connection)| {
            if &connection.spec.source_placement_id == placement_id {
                Some(connection_id.clone())
            } else {
                None
            }
        })
        .collect()
}

fn incoming_connections(
    placement_id: &PlacementId,
    connections: &BTreeMap<ConnectionId, RuntimeConnection>,
) -> Vec<ConnectionId> {
    connections
        .iter()
        .filter_map(|(connection_id, connection)| {
            if &connection.spec.sink_placement_id == placement_id {
                Some(connection_id.clone())
            } else {
                None
            }
        })
        .collect()
}

fn mark_source_done(
    placement_id: &PlacementId,
    connections: &mut BTreeMap<ConnectionId, RuntimeConnection>,
) {
    for connection in connections.values_mut() {
        if &connection.spec.source_placement_id == placement_id {
            connection.source_done = true;
        }
    }
}

#[cfg(test)]
mod conformance {
    use super::{
        HostRuntime, ImplementationFailure, ImplementationRegistry, OperationAction,
        OperationCompletion, OperationImplementation, OperationState,
    };
    use conduit_core::{
        kind_id, port_id, present_host_operation_requirement, wait_host_operation_requirement,
        ArtifactId, BootId, CancellationReason, CapabilityId, CapabilityLimits, CapabilityOffer,
        ConfigurationEntry, ConfigurationValue, ConnectionProvider, ExecutionProfileId,
        FailureReason, HostAdvertisement, HostCommand, HostEvent, HostId, HostProfileId,
        ImplementationId, KindContractRevision, ObservationKind, OfferGeneration, PlatformEffect,
        PortDescriptor, PortDirection, TerminalDisposition, ValuePayload, PROTOCOL_VERSION,
    };
    use conduit_form::{
        parse, ConfigurationField, ConfigurationRule, KindDefinition, ProfileCatalog,
    };
    use conduit_planner::{default_placements, plan_with_connection_limits};
    use std::collections::{BTreeMap, VecDeque};

    const PULSE_KIND: &str = "flow/pulse";
    const SHOW_KIND: &str = "presentation/show";
    const SIGNAL_VALUE_KIND: &str = "value/signal";
    const SIGNAL_PRESENTATION_KIND: &str = "test/presentation";
    const SIGNAL_ENCODED_LEN: u32 = 9;
    const PULSE_CONTRACT: &str = "test/flow-pulse@1";
    const SHOW_CONTRACT: &str = "test/presentation-show@1";
    const PULSE_PROFILE: &str = "test/pulse-hosted@1";
    const SHOW_PROFILE: &str = "test/show-hosted@1";

    fn pulse_outputs() -> Vec<PortDescriptor> {
        vec![PortDescriptor {
            port_id: port_id("signal"),
            value_kind: kind_id(SIGNAL_VALUE_KIND),
            direction: PortDirection::Output,
        }]
    }

    fn show_inputs() -> Vec<PortDescriptor> {
        vec![PortDescriptor {
            port_id: port_id("signal"),
            value_kind: kind_id(SIGNAL_VALUE_KIND),
            direction: PortDirection::Input,
        }]
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct Signal {
        sequence: u64,
        level: bool,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    struct TestPulseConfiguration {
        count: u64,
        period_ms: u64,
        initial_level: bool,
    }

    fn encode_signal(signal: &Signal) -> ValuePayload {
        let mut encoded = signal.sequence.to_le_bytes().to_vec();
        encoded.push(u8::from(signal.level));
        ValuePayload {
            value_kind: kind_id(SIGNAL_VALUE_KIND),
            encoded,
        }
    }

    fn decode_signal(value: &ValuePayload) -> Result<Signal, String> {
        if value.value_kind.as_str() != SIGNAL_VALUE_KIND || value.encoded.len() != 9 {
            return Err("invalid test signal".to_string());
        }
        let mut sequence = [0; 8];
        sequence.copy_from_slice(&value.encoded[..8]);
        Ok(Signal {
            sequence: u64::from_le_bytes(sequence),
            level: value.encoded[8] != 0,
        })
    }

    fn parse_pulse_configuration(
        entries: &[ConfigurationEntry],
    ) -> Result<TestPulseConfiguration, String> {
        let get_u64 = |key: &str| {
            entries
                .iter()
                .find(|entry| entry.key == key)
                .and_then(|entry| match entry.value {
                    ConfigurationValue::U64(value) => Some(value),
                    ConfigurationValue::Bool(_) => None,
                })
                .ok_or_else(|| format!("missing integer '{key}'"))
        };
        let initial_level = entries
            .iter()
            .find(|entry| entry.key == "initial")
            .and_then(|entry| match entry.value {
                ConfigurationValue::Bool(value) => Some(value),
                ConfigurationValue::U64(_) => None,
            })
            .ok_or_else(|| "missing boolean 'initial'".to_string())?;
        Ok(TestPulseConfiguration {
            count: get_u64("count")?,
            period_ms: get_u64("period-ms")?,
            initial_level,
        })
    }

    fn signal_profile_catalog() -> ProfileCatalog {
        let mut catalog = ProfileCatalog::new();
        catalog
            .insert(KindDefinition {
                kind_id: kind_id(PULSE_KIND),
                kind_contract_revision: KindContractRevision::from(PULSE_CONTRACT),
                inputs: Vec::new(),
                outputs: pulse_outputs(),
                configuration: vec![
                    ConfigurationField {
                        key: "count".to_string(),
                        default_value: ConfigurationValue::U64(16),
                        validation: ConfigurationRule::Any,
                    },
                    ConfigurationField {
                        key: "period-ms".to_string(),
                        default_value: ConfigurationValue::U64(250),
                        validation: ConfigurationRule::Any,
                    },
                    ConfigurationField {
                        key: "initial".to_string(),
                        default_value: ConfigurationValue::Bool(false),
                        validation: ConfigurationRule::Any,
                    },
                ],
            })
            .expect("test pulse kind installs");
        catalog
            .insert(KindDefinition {
                kind_id: kind_id(SHOW_KIND),
                kind_contract_revision: KindContractRevision::from(SHOW_CONTRACT),
                inputs: show_inputs(),
                outputs: Vec::new(),
                configuration: Vec::new(),
            })
            .expect("test show kind installs");
        catalog
    }

    fn advertisement(
        boot: &str,
        offer_generation: u64,
        queue_items: u16,
        queue_bytes: u32,
    ) -> HostAdvertisement {
        HostAdvertisement {
            protocol_version: PROTOCOL_VERSION,
            host_id: HostId::from("std-host-1"),
            boot_id: BootId::from(boot),
            offer_generation: OfferGeneration(offer_generation),
            profile: HostProfileId::from("rust-std"),
            capabilities: vec![
                CapabilityOffer {
                    capability_id: CapabilityId::from("pulse-1"),
                    kind_id: kind_id(PULSE_KIND),
                    kind_contract_revision: KindContractRevision::from(PULSE_CONTRACT),
                    execution_profile_id: ExecutionProfileId::from(PULSE_PROFILE),
                    implementation_id: ImplementationId::from("std/pulse-v1"),
                    artifact_id: ArtifactId::from("test/pulse-artifact-v1"),
                    inputs: vec![],
                    outputs: pulse_outputs(),
                    host_operations: vec![wait_host_operation_requirement()],
                    limits: CapabilityLimits {
                        max_active_instances: 8,
                        max_queue_items: queue_items,
                        max_queue_bytes: queue_bytes,
                    },
                },
                CapabilityOffer {
                    capability_id: CapabilityId::from("stdout-show-1"),
                    kind_id: kind_id(SHOW_KIND),
                    kind_contract_revision: KindContractRevision::from(SHOW_CONTRACT),
                    execution_profile_id: ExecutionProfileId::from(SHOW_PROFILE),
                    implementation_id: ImplementationId::from("std/stdout-show-signal-v1"),
                    artifact_id: ArtifactId::from("test/show-artifact-v1"),
                    inputs: show_inputs(),
                    outputs: vec![],
                    host_operations: vec![present_host_operation_requirement(
                        kind_id(SIGNAL_PRESENTATION_KIND),
                        SIGNAL_ENCODED_LEN,
                    )],
                    limits: CapabilityLimits {
                        max_active_instances: 8,
                        max_queue_items: queue_items,
                        max_queue_bytes: queue_bytes,
                    },
                },
            ],
        }
    }

    fn test_runtime(advertisement: HostAdvertisement, observation_limit: usize) -> HostRuntime {
        let mut registry = ImplementationRegistry::new();
        registry
            .install(TestPulseImplementation {
                kind_id: kind_id(PULSE_KIND),
                implementation_id: ImplementationId::from("std/pulse-v1"),
                artifact_id: ArtifactId::from("test/pulse-artifact-v1"),
            })
            .expect("pulse implementation installs");
        registry
            .install(TestShowImplementation {
                kind_id: kind_id(SHOW_KIND),
                implementation_id: ImplementationId::from("std/stdout-show-signal-v1"),
                artifact_id: ArtifactId::from("test/show-artifact-v1"),
            })
            .expect("show implementation installs");
        HostRuntime::new(advertisement, registry, observation_limit)
    }

    struct TestPulseImplementation {
        kind_id: conduit_core::KindId,
        implementation_id: ImplementationId,
        artifact_id: ArtifactId,
    }

    impl OperationImplementation for TestPulseImplementation {
        fn kind_id(&self) -> &conduit_core::KindId {
            &self.kind_id
        }

        fn kind_contract_revision(&self) -> KindContractRevision {
            KindContractRevision::from(PULSE_CONTRACT)
        }

        fn execution_profile_id(&self) -> ExecutionProfileId {
            ExecutionProfileId::from(PULSE_PROFILE)
        }

        fn implementation_id(&self) -> &ImplementationId {
            &self.implementation_id
        }

        fn artifact_id(&self) -> &ArtifactId {
            &self.artifact_id
        }

        fn host_operation_requirements(&self) -> Vec<conduit_core::HostOperationRequirement> {
            vec![wait_host_operation_requirement()]
        }

        fn prepare(
            &self,
            placement: &conduit_core::PlannedOperation,
        ) -> Result<Box<dyn OperationState>, ImplementationFailure> {
            Ok(Box::new(TestPulseState {
                configuration: parse_pulse_configuration(&placement.configuration).map_err(
                    |error| {
                        ImplementationFailure::new(
                            FailureReason::InvalidOperationConfiguration,
                            error.to_string(),
                        )
                    },
                )?,
                next_sequence: 0,
            }))
        }

        fn minimum_value_size(&self, value_kind: &conduit_core::KindId) -> Option<u32> {
            (value_kind.as_str() == SIGNAL_VALUE_KIND).then_some(SIGNAL_ENCODED_LEN)
        }
    }

    struct TestPulseState {
        configuration: TestPulseConfiguration,
        next_sequence: u64,
    }

    impl TestPulseState {
        fn next(&self) -> OperationAction {
            if self.next_sequence >= self.configuration.count {
                OperationAction::Complete
            } else {
                OperationAction::Emit(encode_signal(&Signal {
                    sequence: self.next_sequence,
                    level: if self.next_sequence.is_multiple_of(2) {
                        self.configuration.initial_level
                    } else {
                        !self.configuration.initial_level
                    },
                }))
            }
        }
    }

    impl OperationState for TestPulseState {
        fn start(&mut self) -> OperationAction {
            self.next()
        }

        fn resume(&mut self, completion: OperationCompletion) -> OperationAction {
            match completion {
                OperationCompletion::Emitted => {
                    self.next_sequence += 1;
                    if self.next_sequence >= self.configuration.count {
                        OperationAction::Complete
                    } else if self.configuration.period_ms > 0 {
                        OperationAction::Wait {
                            duration_ms: self.configuration.period_ms,
                        }
                    } else {
                        self.next()
                    }
                }
                OperationCompletion::TimerElapsed => self.next(),
                _ => OperationAction::Fail(ImplementationFailure::new(
                    FailureReason::InvalidLifecycleCommand,
                    "unexpected pulse completion",
                )),
            }
        }
    }

    struct TestShowImplementation {
        kind_id: conduit_core::KindId,
        implementation_id: ImplementationId,
        artifact_id: ArtifactId,
    }

    impl OperationImplementation for TestShowImplementation {
        fn kind_id(&self) -> &conduit_core::KindId {
            &self.kind_id
        }

        fn kind_contract_revision(&self) -> KindContractRevision {
            KindContractRevision::from(SHOW_CONTRACT)
        }

        fn execution_profile_id(&self) -> ExecutionProfileId {
            ExecutionProfileId::from(SHOW_PROFILE)
        }

        fn implementation_id(&self) -> &ImplementationId {
            &self.implementation_id
        }

        fn artifact_id(&self) -> &ArtifactId {
            &self.artifact_id
        }

        fn host_operation_requirements(&self) -> Vec<conduit_core::HostOperationRequirement> {
            vec![present_host_operation_requirement(
                kind_id(SIGNAL_PRESENTATION_KIND),
                SIGNAL_ENCODED_LEN,
            )]
        }

        fn prepare(
            &self,
            _placement: &conduit_core::PlannedOperation,
        ) -> Result<Box<dyn OperationState>, ImplementationFailure> {
            Ok(Box::new(TestShowState { expected: 0 }))
        }

        fn minimum_value_size(&self, value_kind: &conduit_core::KindId) -> Option<u32> {
            (value_kind.as_str() == SIGNAL_VALUE_KIND).then_some(SIGNAL_ENCODED_LEN)
        }
    }

    struct TestShowState {
        expected: u64,
    }

    impl OperationState for TestShowState {
        fn start(&mut self) -> OperationAction {
            OperationAction::Idle
        }

        fn resume(&mut self, completion: OperationCompletion) -> OperationAction {
            match completion {
                OperationCompletion::Value(value) => {
                    let signal = decode_signal(&value).expect("test signal decodes");
                    if signal.sequence != self.expected {
                        return OperationAction::Fail(ImplementationFailure::new(
                            FailureReason::MalformedConnectionEnvelope,
                            "out-of-order signal",
                        ));
                    }
                    OperationAction::Present {
                        presentation_kind: kind_id(SIGNAL_PRESENTATION_KIND),
                        value,
                    }
                }
                OperationCompletion::PresentationCompleted { success: true, .. } => {
                    self.expected += 1;
                    OperationAction::Idle
                }
                OperationCompletion::PresentationCompleted {
                    success: false,
                    message,
                } => OperationAction::Fail(ImplementationFailure {
                    reason: FailureReason::ManifestationFailed,
                    message,
                }),
                OperationCompletion::InputsClosed => OperationAction::Complete,
                _ => OperationAction::Fail(ImplementationFailure::new(
                    FailureReason::InvalidLifecycleCommand,
                    "unexpected show completion",
                )),
            }
        }
    }

    fn demo_fragment(
        form_source: &str,
        queue_items: u16,
        queue_bytes: u32,
    ) -> conduit_core::PlanFragment {
        let form = parse(form_source, &signal_profile_catalog()).expect("form should parse");
        let advertisement = advertisement("boot-1", 1, 8, 256);
        let placements = default_placements(&form, std::slice::from_ref(&advertisement))
            .expect("placements work");
        let plan = plan_with_connection_limits(
            &form,
            std::slice::from_ref(&advertisement),
            &placements,
            &[ConnectionProvider::Local],
            queue_items,
            queue_bytes,
        )
        .expect("plan should succeed");
        plan.fragments.first().expect("fragment exists").clone()
    }

    fn inspect(runtime: &mut HostRuntime) -> Vec<conduit_core::Observation> {
        runtime
            .handle(HostCommand::Inspect)
            .events
            .into_iter()
            .find_map(|event| match event {
                HostEvent::Observations { items } => Some(items),
                _ => None,
            })
            .expect("observations must exist")
    }

    fn drive_success(runtime: &mut HostRuntime, plan_id: conduit_core::PlanId) -> Vec<Signal> {
        let output = runtime.handle(HostCommand::Activate(plan_id));
        let mut presented = Vec::new();
        let mut pending_effects = output.effects;
        while let Some(effect) = pending_effects.pop() {
            let follow_up = match effect {
                PlatformEffect::Wait {
                    plan_id,
                    placement_id,
                    ..
                } => runtime.handle(HostCommand::CompleteWait {
                    plan_id,
                    placement_id,
                }),
                PlatformEffect::PresentValue {
                    plan_id,
                    placement_id,
                    value,
                    ..
                } => {
                    presented.push(decode_signal(&value).expect("signal payload must decode"));
                    runtime.handle(HostCommand::CompletePresentation {
                        plan_id,
                        placement_id,
                        value,
                        success: true,
                        message: None,
                    })
                }
                PlatformEffect::TransmitConnection { .. } => {
                    panic!("local test plan must not transmit remotely")
                }
            };
            pending_effects.extend(follow_up.effects.into_iter().rev());
        }
        presented
    }

    fn drive_with_failure(
        runtime: &mut HostRuntime,
        plan_id: conduit_core::PlanId,
        failed_placement: &conduit_core::PlacementId,
        failed_sequence: u64,
    ) -> Vec<HostEvent> {
        let mut all_events = Vec::new();
        let initial = runtime.handle(HostCommand::Activate(plan_id));
        all_events.extend(initial.events);
        let mut pending = VecDeque::from(initial.effects);
        while let Some(effect) = pending.pop_front() {
            let follow_up = match effect {
                PlatformEffect::Wait {
                    plan_id,
                    placement_id,
                    ..
                } => runtime.handle(HostCommand::CompleteWait {
                    plan_id,
                    placement_id,
                }),
                PlatformEffect::PresentValue {
                    plan_id,
                    placement_id,
                    value,
                    ..
                } => {
                    let signal = decode_signal(&value).expect("signal payload must decode");
                    let fail =
                        &placement_id == failed_placement && signal.sequence == failed_sequence;
                    runtime.handle(HostCommand::CompletePresentation {
                        plan_id,
                        placement_id,
                        value,
                        success: !fail,
                        message: fail.then(|| "injected failure".to_string()),
                    })
                }
                PlatformEffect::TransmitConnection { .. } => {
                    panic!("local test plan must not transmit remotely")
                }
            };
            all_events.extend(follow_up.events);
            pending.extend(follow_up.effects);
        }
        all_events
    }

    #[test]
    fn preparation_rejects_stale_boot() {
        let fragment = demo_fragment("form 0\n\ndemo {\n    pulse: flow/pulse\n    show: presentation/show\n\n    pulse.count = 2\n    pulse.period-ms = 0\n    pulse.initial = false\n\n    pulse > show\n}\n", 4, 64);
        let mut runtime = test_runtime(advertisement("boot-2", 1, 4, 64), 128);
        let output = runtime.handle(HostCommand::Prepare(fragment));
        assert!(matches!(
            output.events.first(),
            Some(HostEvent::PreparationRejected { .. })
        ));
    }

    #[test]
    fn preparation_rejects_stale_offer_generation() {
        let fragment = demo_fragment("form 0\n\ndemo {\n    pulse: flow/pulse\n    show: presentation/show\n\n    pulse.count = 2\n    pulse.period-ms = 0\n    pulse.initial = false\n\n    pulse > show\n}\n", 4, 64);
        let mut runtime = test_runtime(advertisement("boot-1", 2, 4, 64), 128);
        let output = runtime.handle(HostCommand::Prepare(fragment));
        assert!(matches!(
            output.events.first(),
            Some(HostEvent::PreparationRejected { .. })
        ));
    }

    #[test]
    fn preparation_rejects_too_small_byte_capacity() {
        let fragment = demo_fragment("form 0\n\ndemo {\n    pulse: flow/pulse\n    show: presentation/show\n\n    pulse.count = 1\n    pulse.period-ms = 0\n    pulse.initial = false\n\n    pulse > show\n}\n", 4, 8);
        let mut runtime = test_runtime(advertisement("boot-1", 1, 4, 64), 128);
        let output = runtime.handle(HostCommand::Prepare(fragment));
        assert!(matches!(
            output.events.first(),
            Some(HostEvent::PreparationRejected { .. })
        ));
    }

    #[test]
    fn full_queue_applies_backpressure() {
        let fragment = demo_fragment("form 0\n\ndemo {\n    pulse: flow/pulse\n    show: presentation/show\n\n    pulse.count = 3\n    pulse.period-ms = 0\n    pulse.initial = false\n\n    pulse > show\n}\n", 1, 64);
        let mut runtime = test_runtime(advertisement("boot-1", 1, 1, 64), 128);
        runtime.handle(HostCommand::Prepare(fragment.clone()));
        let output = runtime.handle(HostCommand::Activate(fragment.plan_id.clone()));
        assert!(output
            .events
            .iter()
            .any(|event| matches!(event, HostEvent::ConnectionBlocked { .. })));
    }

    #[test]
    fn byte_capacity_applies_backpressure() {
        let fragment = demo_fragment("form 0\n\ndemo {\n    pulse: flow/pulse\n    show: presentation/show\n\n    pulse.count = 3\n    pulse.period-ms = 0\n    pulse.initial = false\n\n    pulse > show\n}\n", 4, 9);
        let mut runtime = test_runtime(advertisement("boot-1", 1, 4, 64), 128);
        runtime.handle(HostCommand::Prepare(fragment.clone()));
        let output = runtime.handle(HostCommand::Activate(fragment.plan_id.clone()));
        assert!(output
            .events
            .iter()
            .any(|event| matches!(event, HostEvent::ConnectionBlocked { .. })));
    }

    #[test]
    fn multiple_sources_remain_independent() {
        let fragment = demo_fragment("form 0\n\ndouble-demo {\n    pulse-a: flow/pulse\n    show-a: presentation/show\n    pulse-b: flow/pulse\n    show-b: presentation/show\n\n    pulse-a.count = 3\n    pulse-a.period-ms = 0\n    pulse-a.initial = false\n    pulse-b.count = 5\n    pulse-b.period-ms = 0\n    pulse-b.initial = true\n\n    pulse-a > show-a\n    pulse-b > show-b\n}\n", 4, 64);
        let placement_by_operation = fragment
            .placements
            .iter()
            .map(|placement| {
                (
                    placement.operation_id.as_str().to_string(),
                    placement.placement_id.clone(),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let connection_by_source = fragment
            .connections
            .iter()
            .map(|connection| {
                (
                    connection.source_placement_id.clone(),
                    connection.connection_id.clone(),
                )
            })
            .collect::<BTreeMap<_, _>>();
        let plan_id = fragment.plan_id.clone();
        let mut runtime = test_runtime(advertisement("boot-1", 1, 4, 64), 256);
        runtime.handle(HostCommand::Prepare(fragment));
        let presented = drive_success(&mut runtime, plan_id.clone());
        assert_eq!(presented.len(), 8);
        let observations = inspect(&mut runtime);
        let pulse_a = placement_by_operation["pulse-a"].clone();
        let pulse_b = placement_by_operation["pulse-b"].clone();
        let show_a = placement_by_operation["show-a"].clone();
        let show_b = placement_by_operation["show-b"].clone();
        let conn_a = connection_by_source[&pulse_a].clone();
        let conn_b = connection_by_source[&pulse_b].clone();
        let produced_a = observations
            .iter()
            .filter_map(|item| match &item.kind {
                ObservationKind::ValueProduced { value }
                    if item.placement_id.as_ref() == Some(&pulse_a)
                        && item.connection_id.as_ref() == Some(&conn_a) =>
                {
                    Some(decode_signal(value).expect("signal payload must decode"))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        let produced_b = observations
            .iter()
            .filter_map(|item| match &item.kind {
                ObservationKind::ValueProduced { value }
                    if item.placement_id.as_ref() == Some(&pulse_b)
                        && item.connection_id.as_ref() == Some(&conn_b) =>
                {
                    Some(decode_signal(value).expect("signal payload must decode"))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        let shown_a = observations
            .iter()
            .filter_map(|item| match &item.kind {
                ObservationKind::ValuePresented { value }
                    if item.placement_id.as_ref() == Some(&show_a) =>
                {
                    Some(decode_signal(value).expect("signal payload must decode"))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        let shown_b = observations
            .iter()
            .filter_map(|item| match &item.kind {
                ObservationKind::ValuePresented { value }
                    if item.placement_id.as_ref() == Some(&show_b) =>
                {
                    Some(decode_signal(value).expect("signal payload must decode"))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            produced_a
                .iter()
                .map(|value| value.sequence)
                .collect::<Vec<_>>(),
            vec![0, 1, 2]
        );
        assert_eq!(
            produced_b
                .iter()
                .map(|value| value.sequence)
                .collect::<Vec<_>>(),
            vec![0, 1, 2, 3, 4]
        );
        assert_eq!(
            shown_a.iter().map(|value| value.level).collect::<Vec<_>>(),
            vec![false, true, false]
        );
        assert_eq!(
            shown_b.iter().map(|value| value.level).collect::<Vec<_>>(),
            vec![true, false, true, false, true]
        );
        assert!(observations.iter().any(|item| matches!(
            item.kind,
            ObservationKind::PlanTerminal {
                disposition: TerminalDisposition::Completed
            }
        ) && item.plan_id.as_ref() == Some(&plan_id)));
    }

    #[test]
    fn cancellation_before_activation_is_terminal() {
        let fragment = demo_fragment("form 0\n\ndemo {\n    pulse: flow/pulse\n    show: presentation/show\n\n    pulse.count = 2\n    pulse.period-ms = 0\n    pulse.initial = false\n\n    pulse > show\n}\n", 4, 64);
        let mut runtime = test_runtime(advertisement("boot-1", 1, 4, 64), 128);
        runtime.handle(HostCommand::Prepare(fragment.clone()));
        let output = runtime.handle(HostCommand::Cancel(fragment.plan_id.clone()));
        assert!(output.events.iter().any(|event| matches!(
            event,
            HostEvent::PlanTerminated {
                disposition: TerminalDisposition::Cancelled {
                    reason: CancellationReason::OperatorRequested
                },
                ..
            }
        )));
    }

    #[test]
    fn late_presentation_completion_after_cancel_is_rejected() {
        let fragment = demo_fragment("form 0\n\ndemo {\n    pulse: flow/pulse\n    show: presentation/show\n\n    pulse.count = 1\n    pulse.period-ms = 0\n    pulse.initial = false\n\n    pulse > show\n}\n", 4, 64);
        let show = fragment
            .placements
            .iter()
            .find(|placement| placement.kind_id.as_str() == SHOW_KIND)
            .expect("show placement exists")
            .placement_id
            .clone();
        let mut runtime = test_runtime(advertisement("boot-1", 1, 4, 64), 128);
        runtime.handle(HostCommand::Prepare(fragment.clone()));
        let output = runtime.handle(HostCommand::Activate(fragment.plan_id.clone()));
        let value = output
            .effects
            .into_iter()
            .find_map(|effect| match effect {
                PlatformEffect::PresentValue { value, .. } => Some(value),
                _ => None,
            })
            .expect("present effect must exist");
        runtime.handle(HostCommand::Cancel(fragment.plan_id.clone()));
        let late = runtime.handle(HostCommand::CompletePresentation {
            plan_id: fragment.plan_id,
            placement_id: show,
            value,
            success: true,
            message: None,
        });
        assert!(late.events.iter().any(|event| matches!(
            event,
            HostEvent::CommandRejected {
                reason: FailureReason::LatePlatformCompletion,
                ..
            }
        )));
    }

    #[test]
    fn repeated_release_is_rejected() {
        let fragment = demo_fragment("form 0\n\ndemo {\n    pulse: flow/pulse\n    show: presentation/show\n\n    pulse.count = 1\n    pulse.period-ms = 0\n    pulse.initial = false\n\n    pulse > show\n}\n", 4, 64);
        let plan_id = fragment.plan_id.clone();
        let mut runtime = test_runtime(advertisement("boot-1", 1, 4, 64), 128);
        runtime.handle(HostCommand::Prepare(fragment));
        let _ = drive_success(&mut runtime, plan_id.clone());
        let first = runtime.handle(HostCommand::Release(plan_id.clone()));
        assert!(matches!(
            first.events.first(),
            Some(HostEvent::Released { .. })
        ));
        let second = runtime.handle(HostCommand::Release(plan_id));
        assert!(second.events.iter().any(|event| matches!(
            event,
            HostEvent::CommandRejected {
                reason: FailureReason::InvalidLifecycleCommand,
                ..
            }
        )));
    }

    #[test]
    fn observation_overflow_records_gap() {
        let fragment = demo_fragment("form 0\n\ndemo {\n    pulse: flow/pulse\n    show: presentation/show\n\n    pulse.count = 6\n    pulse.period-ms = 0\n    pulse.initial = false\n\n    pulse > show\n}\n", 4, 64);
        let plan_id = fragment.plan_id.clone();
        let mut runtime = test_runtime(advertisement("boot-1", 1, 4, 64), 4);
        runtime.handle(HostCommand::Prepare(fragment));
        let _ = drive_success(&mut runtime, plan_id);
        let observations = inspect(&mut runtime);
        assert!(observations
            .iter()
            .any(|item| matches!(item.kind, ObservationKind::EvidenceGap { .. })));
    }

    #[test]
    fn fanout_failure_before_first_manifestation_disposes_every_branch() {
        let fragment = demo_fragment("form 0\n\nfanout {\n pulse: flow/pulse\n show-a: presentation/show\n show-b: presentation/show\n show-c: presentation/show\n pulse.count = 8\n pulse.period-ms = 0\n pulse.initial = false\n pulse > show-a\n pulse > show-b\n pulse > show-c\n}\n", 4, 64);
        let failed_sink = fragment
            .placements
            .iter()
            .find(|placement| placement.operation_id.as_str() == "show-b")
            .expect("failed sink exists")
            .placement_id
            .clone();
        let failed_connection = fragment
            .connections
            .iter()
            .find(|connection| connection.sink_placement_id == failed_sink)
            .expect("failed connection exists")
            .connection_id
            .clone();
        let plan_id = fragment.plan_id.clone();
        let mut runtime = test_runtime(advertisement("boot-1", 1, 4, 64), 512);
        runtime.handle(HostCommand::Prepare(fragment));
        let events = drive_with_failure(&mut runtime, plan_id.clone(), &failed_sink, 0);
        let terminal_connections = events
            .iter()
            .filter_map(|event| match event {
                HostEvent::ConnectionTerminated {
                    connection_id,
                    disposition,
                    ..
                } => Some((connection_id, disposition)),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(terminal_connections.len(), 3);
        let failed = terminal_connections
            .iter()
            .find(|(connection_id, _)| **connection_id == failed_connection)
            .expect("failed branch has a disposition")
            .1;
        assert!(matches!(
            failed.disposition,
            TerminalDisposition::Failed {
                reason: FailureReason::ManifestationFailed
            }
        ));
        assert_eq!(failed.last_manifested_sequence, None);
        assert!(failed.undeliverable_items > 0);
        assert!(terminal_connections
            .iter()
            .any(|(connection_id, disposition)| {
                **connection_id != failed_connection
                    && matches!(disposition.disposition, TerminalDisposition::Completed)
            }));
        assert_eq!(
            events
                .iter()
                .filter(|event| matches!(
                    event,
                    HostEvent::PlanTerminated {
                        disposition: TerminalDisposition::Failed { .. },
                        ..
                    }
                ))
                .count(),
            1
        );
        assert!(!events
            .iter()
            .any(|event| matches!(event, HostEvent::PlanCompleted { .. })));
        let observations = inspect(&mut runtime);
        assert!(observations.iter().any(|item| {
            item.plan_id.as_ref() == Some(&plan_id)
                && item.connection_id.as_ref() == Some(&failed_connection)
                && matches!(item.kind, ObservationKind::ConnectionTerminal { .. })
        }));
    }

    #[test]
    fn fanout_failure_after_sequence_seven_retains_last_manifestation() {
        let fragment = demo_fragment("form 0\n\nfanout {\n pulse: flow/pulse\n show-a: presentation/show\n show-b: presentation/show\n show-c: presentation/show\n pulse.count = 10\n pulse.period-ms = 0\n pulse.initial = false\n pulse > show-a\n pulse > show-b\n pulse > show-c\n}\n", 4, 64);
        let failed_sink = fragment
            .placements
            .iter()
            .find(|placement| placement.operation_id.as_str() == "show-b")
            .expect("failed sink exists")
            .placement_id
            .clone();
        let failed_connection = fragment
            .connections
            .iter()
            .find(|connection| connection.sink_placement_id == failed_sink)
            .expect("failed connection exists")
            .connection_id
            .clone();
        let mut runtime = test_runtime(advertisement("boot-1", 1, 4, 64), 768);
        runtime.handle(HostCommand::Prepare(fragment.clone()));
        let events = drive_with_failure(&mut runtime, fragment.plan_id, &failed_sink, 8);
        let disposition = events
            .iter()
            .find_map(|event| match event {
                HostEvent::ConnectionTerminated {
                    connection_id,
                    disposition,
                    ..
                } if connection_id == &failed_connection => Some(disposition),
                _ => None,
            })
            .expect("failed branch terminates");
        assert_eq!(disposition.last_accepted_sequence, Some(8));
        assert_eq!(disposition.last_manifested_sequence, Some(7));
        assert!(matches!(
            disposition.disposition,
            TerminalDisposition::Failed { .. }
        ));
    }

    #[test]
    fn cancellation_while_waiting_rejects_late_timer_and_is_idempotently_rejected() {
        let fragment = demo_fragment("form 0\n\ndemo {\n pulse: flow/pulse\n show: presentation/show\n pulse.count = 3\n pulse.period-ms = 10\n pulse.initial = false\n pulse > show\n}\n", 4, 64);
        let pulse = fragment
            .placements
            .iter()
            .find(|placement| placement.kind_id.as_str() == PULSE_KIND)
            .expect("pulse exists")
            .placement_id
            .clone();
        let mut runtime = test_runtime(advertisement("boot-1", 1, 4, 64), 256);
        runtime.handle(HostCommand::Prepare(fragment.clone()));
        let activated = runtime.handle(HostCommand::Activate(fragment.plan_id.clone()));
        assert!(activated
            .effects
            .iter()
            .any(|effect| matches!(effect, PlatformEffect::Wait { .. })));
        let cancelled = runtime.handle(HostCommand::Cancel(fragment.plan_id.clone()));
        assert!(cancelled.events.iter().any(|event| matches!(
            event,
            HostEvent::PlanTerminated {
                disposition: TerminalDisposition::Cancelled { .. },
                ..
            }
        )));
        let repeated = runtime.handle(HostCommand::Cancel(fragment.plan_id.clone()));
        assert!(repeated.events.iter().any(|event| matches!(
            event,
            HostEvent::CommandRejected {
                reason: FailureReason::InvalidLifecycleCommand,
                ..
            }
        )));
        let late = runtime.handle(HostCommand::CompleteWait {
            plan_id: fragment.plan_id,
            placement_id: pulse,
        });
        assert!(late.events.iter().any(|event| matches!(
            event,
            HostEvent::CommandRejected {
                reason: FailureReason::LatePlatformCompletion,
                ..
            }
        )));
    }

    #[test]
    fn fanout_cancellation_releases_all_queued_items() {
        let fragment = demo_fragment("form 0\n\nfanout {\n pulse: flow/pulse\n show-a: presentation/show\n show-b: presentation/show\n pulse.count = 8\n pulse.period-ms = 0\n pulse.initial = false\n pulse > show-a\n pulse > show-b\n}\n", 4, 64);
        let mut runtime = test_runtime(advertisement("boot-1", 1, 4, 64), 256);
        runtime.handle(HostCommand::Prepare(fragment.clone()));
        runtime.handle(HostCommand::Activate(fragment.plan_id.clone()));
        let cancelled = runtime.handle(HostCommand::Cancel(fragment.plan_id));
        let dispositions = cancelled
            .events
            .iter()
            .filter_map(|event| match event {
                HostEvent::ConnectionTerminated { disposition, .. } => Some(disposition),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(dispositions.len(), 2);
        assert!(dispositions.iter().all(|disposition| matches!(
            disposition.disposition,
            TerminalDisposition::Cancelled {
                reason: CancellationReason::OperatorRequested
            }
        )));
        assert!(dispositions
            .iter()
            .all(|disposition| disposition.undeliverable_items > 0));
    }

    #[test]
    fn fanout_accounts_for_each_branches_byte_capacity_independently() {
        let fragment = demo_fragment("form 0\n\nfanout {\n pulse: flow/pulse\n show-a: presentation/show\n show-b: presentation/show\n show-c: presentation/show\n pulse.count = 3\n pulse.period-ms = 0\n pulse.initial = false\n pulse > show-a\n pulse > show-b\n pulse > show-c\n}\n", 4, 9);
        let connection_ids = fragment
            .connections
            .iter()
            .map(|connection| connection.connection_id.clone())
            .collect::<Vec<_>>();
        let plan_id = fragment.plan_id.clone();
        let mut runtime = test_runtime(advertisement("boot-1", 1, 4, 64), 512);
        runtime.handle(HostCommand::Prepare(fragment));
        let presented = drive_success(&mut runtime, plan_id);
        assert_eq!(presented.len(), 9);
        let observations = inspect(&mut runtime);
        for connection_id in connection_ids {
            let sequences = observations
                .iter()
                .filter_map(|item| match &item.kind {
                    ObservationKind::ValueProduced { value }
                        if item.connection_id.as_ref() == Some(&connection_id) =>
                    {
                        Some(decode_signal(value).expect("signal decodes").sequence)
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(sequences, vec![0, 1, 2]);
        }
    }

    #[test]
    fn release_after_failure_and_cancellation_preserves_terminal_evidence() {
        for fail in [false, true] {
            let fragment = demo_fragment("form 0\n\ndemo {\n pulse: flow/pulse\n show: presentation/show\n pulse.count = 2\n pulse.period-ms = 0\n pulse.initial = false\n pulse > show\n}\n", 4, 64);
            let plan_id = fragment.plan_id.clone();
            let show = fragment
                .placements
                .iter()
                .find(|placement| placement.kind_id.as_str() == SHOW_KIND)
                .expect("show exists")
                .placement_id
                .clone();
            let mut runtime = test_runtime(advertisement("boot-1", 1, 4, 64), 256);
            runtime.handle(HostCommand::Prepare(fragment.clone()));
            if fail {
                let _ = drive_with_failure(&mut runtime, plan_id.clone(), &show, 0);
            } else {
                runtime.handle(HostCommand::Cancel(plan_id.clone()));
            }
            assert!(matches!(
                runtime
                    .handle(HostCommand::Release(plan_id.clone()))
                    .events
                    .first(),
                Some(HostEvent::Released { .. })
            ));
            let observations = inspect(&mut runtime);
            assert!(observations.iter().any(|item| {
                item.plan_id.as_ref() == Some(&plan_id)
                    && matches!(item.kind, ObservationKind::PlanTerminal { .. })
            }));
            assert!(observations.iter().any(|item| {
                item.plan_id.as_ref() == Some(&plan_id)
                    && matches!(item.kind, ObservationKind::Released)
            }));
            let after_release = runtime.handle(HostCommand::Activate(plan_id.clone()));
            assert!(after_release.events.iter().any(|event| matches!(
                event,
                HostEvent::CommandRejected {
                    reason: FailureReason::InvalidLifecycleCommand,
                    ..
                }
            )));
            for rejected in [
                runtime.handle(HostCommand::Cancel(plan_id.clone())),
                runtime.handle(HostCommand::Prepare(fragment.clone())),
            ] {
                assert!(rejected.events.iter().any(|event| matches!(
                    event,
                    HostEvent::CommandRejected {
                        reason: FailureReason::InvalidLifecycleCommand,
                        ..
                    }
                )));
            }
            let late = runtime.handle(HostCommand::CompletePresentation {
                plan_id,
                placement_id: show,
                value: encode_signal(&Signal {
                    sequence: 0,
                    level: false,
                }),
                success: true,
                message: None,
            });
            assert!(late.events.iter().any(|event| matches!(
                event,
                HostEvent::CommandRejected {
                    reason: FailureReason::LatePlatformCompletion,
                    ..
                }
            )));
        }
    }
}
