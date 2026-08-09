use conduit_core::{
    bind_active_play, bind_sign, kind_id, verify_plan, ActivePlayId, ArtifactId, BootId,
    CapabilityLimits, CapabilityOffer, ConnectionEnvelope, ConnectionId, ConnectionOutcome,
    ExecutionProfileId, FailureReason, HostAdvertisement, HostCommand, HostEvent, HostId,
    HostProfileId, ImplementationId, Observation, ObservationKind, OfferGeneration, Plan,
    PlanFragment, PlanId, PlatformEffect, PortDescriptor, PortId, TerminalDisposition,
    PROTOCOL_VERSION,
};
use conduit_form::{CheckedForm, CompositeFaceTerminal};
use conduit_runtime::{
    bases::in_memory::InMemoryConnectionBase, CompositeBoundaryEffect, CompositePortBinding,
    HostRuntime, RuntimeOutput,
};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompositeError {
    InvalidInternalPlan(String),
    ChildPreparationFailed(String),
}

impl std::fmt::Display for CompositeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidInternalPlan(reason) => write!(f, "invalid internal plan: {reason}"),
            Self::ChildPreparationFailed(reason) => write!(f, "child preparation failed: {reason}"),
        }
    }
}

impl std::error::Error for CompositeError {}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum ExternalState {
    Prepared,
    Active,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum DeliveryMode {
    Immediate,
    Controlled,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompositeBoundary {
    pub input_faces: Vec<CompositeFaceBinding>,
    pub output_faces: Vec<CompositeFaceBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompositeFaceBinding {
    pub external_port: PortDescriptor,
    pub internal_child: HostId,
    pub internal_placement_id: conduit_core::PlacementId,
    pub internal_port_id: PortId,
    pub terminal: CompositeFaceTerminal,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChildHostBinding {
    pub host_id: HostId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompositeDefinition {
    pub host_id: HostId,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
    pub profile: HostProfileId,
    pub external_capability: CapabilityOffer,
    pub children: Vec<ChildHostBinding>,
    pub internal_plan: Plan,
    pub boundary: CompositeBoundary,
    pub failure_translation: FailureReason,
}

impl CompositeDefinition {
    #[allow(clippy::too_many_arguments)]
    pub fn from_authored_export(
        host_id: HostId,
        boot_id: BootId,
        offer_generation: OfferGeneration,
        profile: HostProfileId,
        implementation_id: ImplementationId,
        artifact_id: ArtifactId,
        form: &CheckedForm,
        export_capability_id: &conduit_core::CapabilityId,
        internal_plan: Plan,
        failure_translation: FailureReason,
    ) -> Result<Self, CompositeError> {
        if internal_plan.source_document_id != form.source_document_id
            || internal_plan.checked_form_id != form.checked_form_id
            || internal_plan.expanded_form_id != form.expanded_form_id
            || !verify_plan(&internal_plan)
        {
            return Err(CompositeError::InvalidInternalPlan(
                "authored form and exact internal plan do not agree".into(),
            ));
        }
        let boundary = form
            .export_boundary(export_capability_id)
            .map_err(|error| CompositeError::InvalidInternalPlan(error.to_string()))?;
        let placement_for = |gear_id: &conduit_core::GearId| {
            internal_plan
                .fragments
                .iter()
                .flat_map(|fragment| &fragment.placements)
                .find(|placement| &placement.gear_id == gear_id)
        };
        let bind_faces = |faces: &[conduit_form::CheckedCompositeFace]| {
            faces
                .iter()
                .map(|face| {
                    let placement = placement_for(&face.internal_gear_id).ok_or_else(|| {
                        CompositeError::InvalidInternalPlan(format!(
                            "face '{}' internal operation is absent from the exact plan",
                            face.external_port.port_id.as_str()
                        ))
                    })?;
                    let planned_port = match face.external_port.direction {
                        conduit_core::PortDirection::Input => &placement.inputs,
                        conduit_core::PortDirection::Output => &placement.outputs,
                    }
                    .iter()
                    .find(|port| port.port_id == face.internal_port_id)
                    .ok_or_else(|| {
                        CompositeError::InvalidInternalPlan(format!(
                            "face '{}' internal endpoint is absent from the exact plan",
                            face.external_port.port_id.as_str()
                        ))
                    })?;
                    if planned_port.value_kind != face.external_port.value_kind
                        || planned_port.direction != face.external_port.direction
                    {
                        return Err(CompositeError::InvalidInternalPlan(format!(
                            "face '{}' differs from its exact planned endpoint",
                            face.external_port.port_id.as_str()
                        )));
                    }
                    Ok(CompositeFaceBinding {
                        external_port: face.external_port.clone(),
                        internal_child: placement.host_id.clone(),
                        internal_placement_id: placement.placement_id.clone(),
                        internal_port_id: face.internal_port_id.clone(),
                        terminal: face.terminal,
                    })
                })
                .collect::<Result<Vec<_>, CompositeError>>()
        };
        let input_faces = bind_faces(&boundary.input_faces)?;
        let output_faces = bind_faces(&boundary.output_faces)?;
        let queue_items = internal_plan
            .fragments
            .iter()
            .flat_map(|fragment| &fragment.connections)
            .map(|connection| connection.item_capacity)
            .min()
            .unwrap_or(conduit_core::DEFAULT_CONNECTION_ITEM_CAPACITY);
        let queue_bytes = internal_plan
            .fragments
            .iter()
            .flat_map(|fragment| &fragment.connections)
            .map(|connection| connection.byte_capacity)
            .min()
            .unwrap_or(conduit_core::DEFAULT_CONNECTION_BYTE_CAPACITY);
        let execution_profile_id =
            ExecutionProfileId::from(format!("composite:{}@1", implementation_id.as_str()));
        Ok(Self {
            host_id,
            boot_id,
            offer_generation,
            profile,
            external_capability: CapabilityOffer {
                startup_parameters: vec![],
                shorthand: None,
                capability_id: boundary.capability_id,
                kind_id: boundary.kind_id,
                kind_contract_revision: boundary.kind_contract_revision,
                implementation: conduit_core::ImplementationOffer {
                    execution_profile_id,
                    implementation_id,
                    artifact_id,
                },
                inputs: boundary.inputs,
                outputs: boundary.outputs,
                host_operations: Vec::new(),
                resource_requirements: Vec::new(),
                authority_requirements: Vec::new(),
                limits: CapabilityLimits {
                    max_active_instances: 1,
                    max_queue_items: queue_items,
                    max_queue_bytes: queue_bytes,
                },
            },
            children: internal_plan
                .fragments
                .iter()
                .map(|fragment| ChildHostBinding {
                    host_id: fragment.host_id.clone(),
                })
                .collect(),
            internal_plan,
            boundary: CompositeBoundary {
                input_faces,
                output_faces,
            },
            failure_translation,
        })
    }
}

#[derive(Debug)]
struct ExternalPlan {
    state: ExternalState,
    terminal_emitted: bool,
    child_terminals: BTreeMap<HostId, TerminalDisposition>,
    active_play_id: Option<ActivePlayId>,
    connections: BTreeMap<ConnectionId, ExternalConnection>,
    pending_outputs: BTreeMap<(PortId, u64), PendingFaceOutput>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExternalConnectionRole {
    Input,
    Output,
}

#[derive(Debug)]
struct ExternalConnection {
    spec: conduit_core::PlannedConnection,
    role: ExternalConnectionRole,
    face_port_id: PortId,
    next_expected_sequence: u64,
    next_send_sequence: u64,
    terminal: Option<TerminalDisposition>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExternalDeliveryState {
    Offered,
    Accepted,
    Delivered,
}

#[derive(Debug)]
struct ExternalOutputBranch {
    sequence: u64,
    state: ExternalDeliveryState,
    value: conduit_core::ValuePayload,
}

#[derive(Debug)]
struct PendingFaceOutput {
    child: HostId,
    branches: BTreeMap<ConnectionId, ExternalOutputBranch>,
}

#[derive(Debug)]
struct ChildRuntime {
    runtime: HostRuntime,
    fragment: PlanFragment,
}

#[derive(Debug)]
struct InternalConnection {
    source_child: HostId,
    sink_child: HostId,
    base: InMemoryConnectionBase,
}

#[derive(Debug)]
pub struct CompositeHost {
    advertisement: HostAdvertisement,
    children: BTreeMap<HostId, ChildRuntime>,
    boundary: CompositeBoundary,
    internal_plan_id: PlanId,
    internal_connections: BTreeMap<ConnectionId, InternalConnection>,
    external_plans: BTreeMap<PlanId, ExternalPlan>,
    released_plans: BTreeSet<PlanId>,
    observations: Vec<Observation>,
    internal_events: Vec<(HostId, HostEvent)>,
    observation_limit: usize,
    fail_next_presentation: bool,
    delivery_mode: DeliveryMode,
    failure_translation: FailureReason,
    next_active_play_sequence: u64,
    next_sign_sequence: u64,
}

impl CompositeHost {
    pub fn from_definition(
        definition: CompositeDefinition,
        child_runtimes: Vec<HostRuntime>,
        observation_limit: usize,
    ) -> Result<Self, CompositeError> {
        let declared_ids = definition
            .children
            .iter()
            .map(|binding| binding.host_id.clone())
            .collect::<BTreeSet<_>>();
        if declared_ids.len() != definition.children.len() {
            return Err(CompositeError::InvalidInternalPlan(
                "definition contains a duplicate child host".into(),
            ));
        }
        let plan_ids = definition
            .internal_plan
            .fragments
            .iter()
            .map(|fragment| fragment.host_id.clone())
            .collect::<BTreeSet<_>>();
        if plan_ids != declared_ids {
            return Err(CompositeError::InvalidInternalPlan(
                "definition children must exactly match internal plan fragments".into(),
            ));
        }
        let runtime_ids = child_runtimes
            .iter()
            .map(|runtime| runtime.advertisement().host_id.clone())
            .collect::<BTreeSet<_>>();
        if runtime_ids.len() != child_runtimes.len() || runtime_ids != declared_ids {
            return Err(CompositeError::InvalidInternalPlan(
                "supplied child runtimes must exactly match definition children".into(),
            ));
        }
        let boot_ids = child_runtimes
            .iter()
            .map(|runtime| runtime.advertisement().boot_id.clone())
            .collect::<BTreeSet<_>>();
        if boot_ids.len() != child_runtimes.len() {
            return Err(CompositeError::InvalidInternalPlan(
                "child hosts must have distinct boot identities".into(),
            ));
        }
        if child_runtimes.iter().any(|runtime| {
            let advertisement = runtime.advertisement();
            definition
                .internal_plan
                .fragments
                .iter()
                .find(|fragment| fragment.host_id == advertisement.host_id)
                .is_none_or(|fragment| {
                    fragment.boot_id != advertisement.boot_id
                        || fragment.offer_generation != advertisement.offer_generation
                })
        }) {
            return Err(CompositeError::InvalidInternalPlan(
                "child plan fragment identity does not match its runtime advertisement".into(),
            ));
        }
        let mut mapped_ports = BTreeSet::new();
        for face in definition
            .boundary
            .input_faces
            .iter()
            .chain(&definition.boundary.output_faces)
        {
            if !declared_ids.contains(&face.internal_child)
                || !mapped_ports.insert(face.external_port.port_id.clone())
                || face.terminal != CompositeFaceTerminal::Independent
            {
                return Err(CompositeError::InvalidInternalPlan(
                    "boundary contains a duplicate, unknown-child, or unsupported face".into(),
                ));
            }
            let placement = definition
                .internal_plan
                .fragments
                .iter()
                .find(|fragment| fragment.host_id == face.internal_child)
                .and_then(|fragment| {
                    fragment
                        .placements
                        .iter()
                        .find(|placement| placement.placement_id == face.internal_placement_id)
                })
                .ok_or_else(|| {
                    CompositeError::InvalidInternalPlan(
                        "boundary face names a hidden or missing internal placement".into(),
                    )
                })?;
            let ports = match face.external_port.direction {
                conduit_core::PortDirection::Input => &placement.inputs,
                conduit_core::PortDirection::Output => &placement.outputs,
            };
            if ports
                .iter()
                .find(|port| port.port_id == face.internal_port_id)
                != Some(&face.external_port)
            {
                // Internal and external port names may differ; compare the
                // endpoint contract without accidentally exposing its name.
                let Some(port) = ports
                    .iter()
                    .find(|port| port.port_id == face.internal_port_id)
                else {
                    return Err(CompositeError::InvalidInternalPlan(
                        "boundary face names a missing or wrongly directed internal port".into(),
                    ));
                };
                if port.value_kind != face.external_port.value_kind
                    || port.direction != face.external_port.direction
                {
                    return Err(CompositeError::InvalidInternalPlan(
                        "boundary face differs from its internal endpoint contract".into(),
                    ));
                }
            }
        }
        let advertised_inputs = definition
            .boundary
            .input_faces
            .iter()
            .map(|face| face.external_port.clone())
            .collect::<Vec<_>>();
        let advertised_outputs = definition
            .boundary
            .output_faces
            .iter()
            .map(|face| face.external_port.clone())
            .collect::<Vec<_>>();
        if definition.external_capability.inputs != advertised_inputs
            || definition.external_capability.outputs != advertised_outputs
        {
            return Err(CompositeError::InvalidInternalPlan(
                "advertised ports differ from the authored boundary faces".into(),
            ));
        }
        let relevant_limit = |host_id: &HostId, placement_id: &conduit_core::PlacementId| {
            let runtime = child_runtimes
                .iter()
                .find(|runtime| runtime.advertisement().host_id == *host_id)?;
            let fragment = definition
                .internal_plan
                .fragments
                .iter()
                .find(|fragment| fragment.host_id == *host_id)?;
            let placement = fragment
                .placements
                .iter()
                .find(|placement| placement.placement_id == *placement_id)?;
            runtime
                .advertisement()
                .capabilities
                .iter()
                .find(|offer| offer.capability_id == placement.capability_id)
                .map(|offer| offer.limits.clone())
        };
        let mut external_capability = definition.external_capability;
        for face in definition
            .boundary
            .input_faces
            .iter()
            .chain(&definition.boundary.output_faces)
        {
            let limits = relevant_limit(&face.internal_child, &face.internal_placement_id)
                .ok_or_else(|| {
                    CompositeError::InvalidInternalPlan(
                        "boundary placement has no matching child capability".into(),
                    )
                })?;
            external_capability.limits.max_active_instances = external_capability
                .limits
                .max_active_instances
                .min(limits.max_active_instances)
                .min(1);
            external_capability.limits.max_queue_items = external_capability
                .limits
                .max_queue_items
                .min(limits.max_queue_items);
            external_capability.limits.max_queue_bytes = external_capability
                .limits
                .max_queue_bytes
                .min(limits.max_queue_bytes);
        }
        let advertisement = HostAdvertisement {
            protocol_version: PROTOCOL_VERSION,
            host_id: definition.host_id,
            boot_id: definition.boot_id,
            offer_generation: definition.offer_generation,
            profile: definition.profile,
            resources: Vec::new(),
            planner_capabilities: vec![],
            capabilities: vec![external_capability],
        };
        let internal_plan_id = definition.internal_plan.plan_id.clone();
        let mut connection_rows =
            BTreeMap::<ConnectionId, Vec<(HostId, conduit_core::PlannedConnection)>>::new();
        for fragment in &definition.internal_plan.fragments {
            for connection in &fragment.connections {
                if connection
                    .selected_line
                    .as_ref()
                    .is_some_and(|line| line.binding.base == conduit_core::ConnectionBase::InMemory)
                {
                    connection_rows
                        .entry(connection.connection_id.clone())
                        .or_default()
                        .push((fragment.host_id.clone(), connection.clone()));
                }
            }
        }
        let mut internal_connections = BTreeMap::new();
        for (connection_id, rows) in connection_rows {
            if rows.len() != 2 || rows[0].1 != rows[1].1 {
                return Err(CompositeError::InvalidInternalPlan(format!(
                    "in-memory connection '{}' is not shared by two exact child fragments",
                    connection_id.as_str()
                )));
            }
            let connection = &rows[0].1;
            let source_child = definition
                .internal_plan
                .fragments
                .iter()
                .find_map(|fragment| {
                    fragment
                        .placements
                        .iter()
                        .any(|placement| placement.placement_id == connection.source_placement_id)
                        .then(|| fragment.host_id.clone())
                })
                .ok_or_else(|| {
                    CompositeError::InvalidInternalPlan(
                        "in-memory connection source placement is missing".into(),
                    )
                })?;
            let sink_child = definition
                .internal_plan
                .fragments
                .iter()
                .find_map(|fragment| {
                    fragment
                        .placements
                        .iter()
                        .any(|placement| placement.placement_id == connection.sink_placement_id)
                        .then(|| fragment.host_id.clone())
                })
                .ok_or_else(|| {
                    CompositeError::InvalidInternalPlan(
                        "in-memory connection sink placement is missing".into(),
                    )
                })?;
            if source_child == sink_child
                || !rows.iter().any(|(host, _)| host == &source_child)
                || !rows.iter().any(|(host, _)| host == &sink_child)
            {
                return Err(CompositeError::InvalidInternalPlan(
                    "in-memory connection endpoints do not match its child fragments".into(),
                ));
            }
            internal_connections.insert(
                connection_id,
                InternalConnection {
                    source_child,
                    sink_child,
                    base: InMemoryConnectionBase::new(internal_plan_id.clone(), connection),
                },
            );
        }
        let mut children = BTreeMap::new();
        for runtime in child_runtimes {
            let host_id = runtime.advertisement().host_id.clone();
            let fragment = definition
                .internal_plan
                .fragments
                .iter()
                .find(|fragment| fragment.host_id == host_id)
                .expect("validated runtime has a plan fragment")
                .clone();
            children.insert(host_id, ChildRuntime { runtime, fragment });
        }
        let mut host = Self {
            advertisement,
            children,
            boundary: definition.boundary,
            internal_plan_id,
            internal_connections,
            external_plans: BTreeMap::new(),
            released_plans: BTreeSet::new(),
            observations: Vec::new(),
            internal_events: Vec::new(),
            observation_limit,
            fail_next_presentation: false,
            delivery_mode: DeliveryMode::Immediate,
            failure_translation: definition.failure_translation,
            next_active_play_sequence: 0,
            next_sign_sequence: 0,
        };
        host.record(None, ObservationKind::HostStarted);
        host.record(None, ObservationKind::AdvertisementPublished);
        Ok(host)
    }

    pub fn advertisement(&self) -> &HostAdvertisement {
        &self.advertisement
    }

    pub fn boundary(&self) -> &CompositeBoundary {
        &self.boundary
    }

    pub fn fail_next_presentation(&mut self) {
        self.fail_next_presentation = true;
    }

    pub fn set_delivery_mode(&mut self, mode: DeliveryMode) {
        self.delivery_mode = mode;
    }

    pub fn base_status(&self) -> ConnectionOutcome {
        self.internal_connections
            .values()
            .next()
            .map_or(ConnectionOutcome::Terminal, |connection| {
                connection.base.status()
            })
    }

    pub fn base_queued_items(&self) -> usize {
        self.internal_connections
            .values()
            .map(|connection| connection.base.queued_items())
            .sum()
    }

    pub fn base_queued_bytes(&self) -> u32 {
        self.internal_connections
            .values()
            .map(|connection| connection.base.queued_bytes())
            .sum()
    }

    pub fn deliver_next(&mut self, external_plan_id: &PlanId) -> RuntimeOutput {
        self.deliver_next_with_mutation(external_plan_id, |_| {})
    }

    /// Exercises the malformed-envelope translation path without exposing the child transport.
    pub fn deliver_next_malformed(&mut self, external_plan_id: &PlanId) -> RuntimeOutput {
        self.deliver_next_with_mutation(external_plan_id, |envelope| {
            envelope.value_kind = kind_id("composite/test-malformed-value");
        })
    }

    fn deliver_next_with_mutation(
        &mut self,
        external_plan_id: &PlanId,
        mutate: impl FnOnce(&mut ConnectionEnvelope),
    ) -> RuntimeOutput {
        let mut external = RuntimeOutput::default();
        let Some(connection_id) = self
            .internal_connections
            .iter()
            .find(|(_, connection)| connection.base.queued_items() > 0)
            .map(|(connection_id, _)| connection_id.clone())
        else {
            return external;
        };
        let (source_child, sink_child, delivery) = {
            let connection = self
                .internal_connections
                .get_mut(&connection_id)
                .expect("listed internal connection exists");
            (
                connection.source_child.clone(),
                connection.sink_child.clone(),
                connection.base.deliver(),
            )
        };
        let Some((ConnectionOutcome::Delivered, mut envelope)) = delivery else {
            return external;
        };
        mutate(&mut envelope);
        let sequence = envelope.sequence;
        let sink_output = self
            .children
            .get_mut(&sink_child)
            .expect("validated boundary sink exists")
            .runtime
            .handle(HostCommand::AcceptConnectionEnvelope(envelope));
        let sink_accepted = sink_output.events.iter().any(|event| {
            matches!(
                event,
                HostEvent::ConnectionEnvelopeOutcome {
                    outcome: ConnectionOutcome::Accepted,
                    ..
                }
            )
        });
        let source_output = self
            .children
            .get_mut(&source_child)
            .expect("validated boundary source exists")
            .runtime
            .handle(HostCommand::CompleteConnectionDelivery {
                plan_id: self.internal_plan_id.clone(),
                connection_id,
                sequence,
                outcome: if sink_accepted {
                    ConnectionOutcome::Delivered
                } else {
                    ConnectionOutcome::Malformed
                },
            });
        self.drive_internal(
            external_plan_id,
            vec![(sink_child, sink_output), (source_child, source_output)],
            &mut external,
        );
        external
    }

    pub fn disconnect_base(&mut self, external_plan_id: &PlanId) -> RuntimeOutput {
        let mut external = RuntimeOutput::default();
        let Some(connection_id) = self.internal_connections.keys().next().cloned() else {
            return external;
        };
        let (source_child, had_queued_envelopes, outcome) = {
            let connection = self
                .internal_connections
                .get_mut(&connection_id)
                .expect("listed internal connection exists");
            (
                connection.source_child.clone(),
                connection.base.queued_items() > 0,
                connection.base.disconnect(),
            )
        };
        if !had_queued_envelopes {
            return external;
        }
        let source = self
            .children
            .get_mut(&source_child)
            .expect("validated boundary source exists")
            .runtime
            .handle(HostCommand::CompleteConnectionDelivery {
                plan_id: self.internal_plan_id.clone(),
                connection_id,
                sequence: 0,
                outcome,
            });
        self.drive_internal(
            external_plan_id,
            vec![(source_child, source)],
            &mut external,
        );
        external
    }

    pub fn handle(&mut self, command: HostCommand) -> RuntimeOutput {
        match command {
            HostCommand::Prepare(fragment) => self.prepare(fragment),
            HostCommand::StartPlay(plan_id) => self.start_play(plan_id),
            HostCommand::AcceptConnectionEnvelope(envelope) => {
                self.accept_external_envelope(envelope)
            }
            HostCommand::CompleteConnectionDelivery {
                plan_id,
                connection_id,
                sequence,
                outcome,
            } => self.complete_external_delivery(plan_id, connection_id, sequence, outcome),
            HostCommand::CloseConnection {
                plan_id,
                connection_id,
            } => self.close_external_connection(plan_id, connection_id),
            HostCommand::Cancel(plan_id) => self.cancel(plan_id),
            HostCommand::Release(plan_id) => self.release(plan_id),
            HostCommand::Inspect => RuntimeOutput {
                events: vec![HostEvent::Observations {
                    items: self.observations.clone(),
                }],
                effects: Vec::new(),
            },
            HostCommand::PublishAdvertisement(advertisement) => {
                self.advertisement = advertisement;
                self.record(None, ObservationKind::AdvertisementPublished);
                RuntimeOutput::default()
            }
            _ => RuntimeOutput {
                events: vec![HostEvent::CommandRejected {
                    plan_id: None,
                    reason: FailureReason::InvalidLifecycleCommand,
                }],
                effects: Vec::new(),
            },
        }
    }

    fn accept_external_envelope(&mut self, envelope: ConnectionEnvelope) -> RuntimeOutput {
        let mut external = RuntimeOutput::default();
        let plan_id = envelope.plan_id.clone();
        let connection_id = envelope.connection_id.clone();
        let sequence = envelope.sequence;
        let Some(plan) = self.external_plans.get(&plan_id) else {
            external.events.push(HostEvent::ConnectionEnvelopeOutcome {
                plan_id,
                connection_id,
                sequence,
                outcome: ConnectionOutcome::Malformed,
            });
            return external;
        };
        let Some(connection) = plan.connections.get(&connection_id) else {
            external.events.push(HostEvent::ConnectionEnvelopeOutcome {
                plan_id,
                connection_id,
                sequence,
                outcome: ConnectionOutcome::Malformed,
            });
            return external;
        };
        if plan.state != ExternalState::Active || connection.terminal.is_some() {
            external.events.push(HostEvent::ConnectionEnvelopeOutcome {
                plan_id,
                connection_id,
                sequence,
                outcome: ConnectionOutcome::Terminal,
            });
            return external;
        }
        let malformed = envelope.protocol_version != PROTOCOL_VERSION
            || connection.role != ExternalConnectionRole::Input
            || envelope.value_kind != connection.spec.value_kind
            || envelope.encoded_len() > connection.spec.byte_capacity
            || sequence != connection.next_expected_sequence;
        if malformed {
            external.events.push(HostEvent::ConnectionEnvelopeOutcome {
                plan_id,
                connection_id,
                sequence,
                outcome: ConnectionOutcome::Malformed,
            });
            return external;
        }
        let face_port_id = connection.face_port_id.clone();
        let child_id = self
            .boundary
            .input_faces
            .iter()
            .find(|face| face.external_port.port_id == face_port_id)
            .expect("prepared external connection has a checked input face")
            .internal_child
            .clone();
        let (outcome, child_output) = self
            .children
            .get_mut(&child_id)
            .expect("checked input face child exists")
            .runtime
            .accept_composite_input(
                &self.internal_plan_id,
                &face_port_id,
                sequence,
                envelope.into_value(),
            );
        if outcome == ConnectionOutcome::Accepted {
            self.external_plans
                .get_mut(&plan_id)
                .expect("external plan was checked")
                .connections
                .get_mut(&connection_id)
                .expect("external connection was checked")
                .next_expected_sequence += 1;
        }
        external.events.push(HostEvent::ConnectionEnvelopeOutcome {
            plan_id: plan_id.clone(),
            connection_id,
            sequence,
            outcome,
        });
        self.drive_internal(&plan_id, vec![(child_id, child_output)], &mut external);
        external
    }

    fn close_external_connection(
        &mut self,
        plan_id: PlanId,
        connection_id: ConnectionId,
    ) -> RuntimeOutput {
        let mut external = RuntimeOutput::default();
        let Some(plan) = self.external_plans.get_mut(&plan_id) else {
            external.events.push(HostEvent::CommandRejected {
                plan_id: Some(plan_id),
                reason: FailureReason::StalePlan,
            });
            return external;
        };
        if plan.state != ExternalState::Active {
            external.events.push(HostEvent::CommandRejected {
                plan_id: Some(plan_id),
                reason: FailureReason::InvalidLifecycleCommand,
            });
            return external;
        }
        let Some(connection) = plan.connections.get_mut(&connection_id) else {
            external.events.push(HostEvent::CommandRejected {
                plan_id: Some(plan_id),
                reason: FailureReason::MalformedConnectionEnvelope,
            });
            return external;
        };
        if connection.role != ExternalConnectionRole::Input || connection.terminal.is_some() {
            external.events.push(HostEvent::CommandRejected {
                plan_id: Some(plan_id),
                reason: FailureReason::InvalidLifecycleCommand,
            });
            return external;
        }
        connection.terminal = Some(TerminalDisposition::Completed);
        let face_port_id = connection.face_port_id.clone();
        let last_sequence = connection.next_expected_sequence.checked_sub(1);
        let child_id = self
            .boundary
            .input_faces
            .iter()
            .find(|face| face.external_port.port_id == face_port_id)
            .expect("prepared connection has an input face")
            .internal_child
            .clone();
        let child_output = self
            .children
            .get_mut(&child_id)
            .expect("checked input child exists")
            .runtime
            .close_composite_input(&self.internal_plan_id, &face_port_id);
        external.events.push(HostEvent::ConnectionTerminated {
            plan_id: plan_id.clone(),
            connection_id,
            disposition: conduit_core::ConnectionTerminalDisposition {
                disposition: TerminalDisposition::Completed,
                last_accepted_sequence: last_sequence,
                last_manifested_sequence: last_sequence,
                undeliverable_items: 0,
            },
        });
        self.drive_internal(&plan_id, vec![(child_id, child_output)], &mut external);
        external
    }

    fn complete_external_delivery(
        &mut self,
        plan_id: PlanId,
        connection_id: ConnectionId,
        sequence: u64,
        outcome: ConnectionOutcome,
    ) -> RuntimeOutput {
        let mut external = RuntimeOutput::default();
        let pending_key = self.external_plans.get(&plan_id).and_then(|plan| {
            plan.pending_outputs.iter().find_map(|(key, pending)| {
                pending
                    .branches
                    .get(&connection_id)
                    .filter(|branch| branch.sequence == sequence)
                    .map(|_| key.clone())
            })
        });
        let Some((port_id, boundary_sequence)) = pending_key else {
            external.events.push(HostEvent::CommandRejected {
                plan_id: Some(plan_id),
                reason: FailureReason::MalformedConnectionEnvelope,
            });
            return external;
        };
        let mut retry = None;
        let mut finish = None;
        let mut invalid_completion = false;
        {
            let plan = self
                .external_plans
                .get_mut(&plan_id)
                .expect("pending output has an external plan");
            let pending = plan
                .pending_outputs
                .get_mut(&(port_id.clone(), boundary_sequence))
                .expect("pending output key was found");
            let branch = pending
                .branches
                .get_mut(&connection_id)
                .expect("pending output branch was found");
            match outcome {
                ConnectionOutcome::Accepted if branch.state == ExternalDeliveryState::Offered => {
                    branch.state = ExternalDeliveryState::Accepted;
                }
                ConnectionOutcome::Delivered
                    if matches!(
                        branch.state,
                        ExternalDeliveryState::Offered | ExternalDeliveryState::Accepted
                    ) =>
                {
                    branch.state = ExternalDeliveryState::Delivered;
                }
                ConnectionOutcome::Full if branch.state == ExternalDeliveryState::Offered => {
                    retry = Some(ConnectionEnvelope {
                        protocol_version: PROTOCOL_VERSION,
                        plan_id: plan_id.clone(),
                        connection_id: connection_id.clone(),
                        sequence,
                        value_kind: branch.value.value_kind.clone(),
                        payload: branch.value.encoded.clone(),
                    });
                }
                ConnectionOutcome::Malformed
                | ConnectionOutcome::Disconnected
                | ConnectionOutcome::Terminal => finish = Some(outcome),
                _ => invalid_completion = true,
            }
            if finish.is_none()
                && pending
                    .branches
                    .values()
                    .all(|branch| branch.state == ExternalDeliveryState::Delivered)
            {
                finish = Some(ConnectionOutcome::Delivered);
            }
        }
        if invalid_completion {
            external.events.push(HostEvent::CommandRejected {
                plan_id: Some(plan_id),
                reason: FailureReason::InvalidLifecycleCommand,
            });
            return external;
        }
        if let Some(envelope) = retry {
            external
                .effects
                .push(PlatformEffect::TransmitConnection { envelope });
        }
        if let Some(final_outcome) = finish {
            let pending = self
                .external_plans
                .get_mut(&plan_id)
                .expect("pending output has a plan")
                .pending_outputs
                .remove(&(port_id.clone(), boundary_sequence))
                .expect("pending output exists");
            let child_output = self
                .children
                .get_mut(&pending.child)
                .expect("pending output child exists")
                .runtime
                .complete_composite_output(
                    &self.internal_plan_id,
                    &port_id,
                    boundary_sequence,
                    final_outcome,
                );
            self.drive_internal(&plan_id, vec![(pending.child, child_output)], &mut external);
        }
        external
    }

    pub fn internal_observations(&mut self) -> BTreeMap<HostId, Vec<Observation>> {
        fn inspect(runtime: &mut HostRuntime) -> Vec<Observation> {
            runtime
                .handle(HostCommand::Inspect)
                .events
                .into_iter()
                .find_map(|event| match event {
                    HostEvent::Observations { items } => Some(items),
                    _ => None,
                })
                .unwrap_or_default()
        }
        self.children
            .iter_mut()
            .map(|(host_id, child)| (host_id.clone(), inspect(&mut child.runtime)))
            .collect()
    }

    pub fn internal_events(&self) -> &[(HostId, HostEvent)] {
        &self.internal_events
    }

    fn prepare(&mut self, fragment: PlanFragment) -> RuntimeOutput {
        let mut output = RuntimeOutput::default();
        let plan_id = fragment.plan_id.clone();
        if !conduit_core::verify_plan_fragment(&fragment)
            || self.released_plans.contains(&plan_id)
            || fragment.host_id != self.advertisement.host_id
            || fragment.boot_id != self.advertisement.boot_id
            || fragment.offer_generation != self.advertisement.offer_generation
            || fragment.placements.len() != 1
            || fragment.placements[0].kind_id != self.advertisement.capabilities[0].kind_id
            || fragment.placements[0].kind_contract_revision
                != self.advertisement.capabilities[0].kind_contract_revision
            || fragment.placements[0].execution_profile_id
                != self.advertisement.capabilities[0]
                    .implementation
                    .execution_profile_id
            || fragment.placements[0].capability_id
                != self.advertisement.capabilities[0].capability_id
            || fragment.placements[0].implementation_id
                != self.advertisement.capabilities[0]
                    .implementation
                    .implementation_id
            || fragment.placements[0].artifact_id
                != self.advertisement.capabilities[0]
                    .implementation
                    .artifact_id
            || fragment.placements[0].inputs != self.advertisement.capabilities[0].inputs
            || fragment.placements[0].outputs != self.advertisement.capabilities[0].outputs
            || self
                .external_plans
                .values()
                .any(|plan| matches!(plan.state, ExternalState::Prepared | ExternalState::Active))
        {
            output.events.push(HostEvent::PreparationRejected {
                plan_id,
                reason: FailureReason::AdvertisedImplementationMismatch,
                message: Some("external fragment does not match the composite offer".into()),
            });
            return output;
        }
        let placement_id = fragment.placements[0].placement_id.clone();
        let mut external_connections = BTreeMap::new();
        for connection in &fragment.connections {
            let (role, face_port_id) = if connection.sink_placement_id == placement_id
                && connection.source_placement_id != placement_id
            {
                (
                    ExternalConnectionRole::Input,
                    connection.sink_port_id.clone(),
                )
            } else if connection.source_placement_id == placement_id
                && connection.sink_placement_id != placement_id
            {
                (
                    ExternalConnectionRole::Output,
                    connection.source_port_id.clone(),
                )
            } else {
                output.events.push(HostEvent::PreparationRejected {
                    plan_id,
                    reason: FailureReason::InvalidGearConfiguration,
                    message: Some(
                        "external connection must have exactly one composite endpoint".into(),
                    ),
                });
                return output;
            };
            let face = match role {
                ExternalConnectionRole::Input => self
                    .boundary
                    .input_faces
                    .iter()
                    .find(|face| face.external_port.port_id == face_port_id),
                ExternalConnectionRole::Output => self
                    .boundary
                    .output_faces
                    .iter()
                    .find(|face| face.external_port.port_id == face_port_id),
            };
            if connection.selected_line.is_none()
                || face.is_none_or(|face| face.external_port.value_kind != connection.value_kind)
                || external_connections.contains_key(&connection.connection_id)
            {
                output.events.push(HostEvent::PreparationRejected {
                    plan_id,
                    reason: FailureReason::InvalidGearConfiguration,
                    message: Some(
                        "external connection differs from an exact named composite face".into(),
                    ),
                });
                return output;
            }
            external_connections.insert(
                connection.connection_id.clone(),
                ExternalConnection {
                    spec: connection.clone(),
                    role,
                    face_port_id,
                    next_expected_sequence: 0,
                    next_send_sequence: 0,
                    terminal: None,
                },
            );
        }
        let preparation_failure = self.children.values_mut().find_map(|child| {
            let result = child
                .runtime
                .handle(HostCommand::Prepare(child.fragment.clone()));
            preparation_failure(&result)
        });
        if preparation_failure.is_some() {
            output.events.push(HostEvent::PreparationRejected {
                plan_id,
                reason: self.failure_translation,
                message: Some("composite child preparation failed".into()),
            });
            return output;
        }
        let external_limits = &self.advertisement.capabilities[0].limits;
        for (host_id, child) in &mut self.children {
            let bindings = |faces: &[CompositeFaceBinding], role: ExternalConnectionRole| {
                faces
                    .iter()
                    .filter(|face| &face.internal_child == host_id)
                    .map(|face| {
                        let mut item_capacity = external_limits.max_queue_items;
                        let mut byte_capacity = external_limits.max_queue_bytes;
                        for connection in external_connections.values().filter(|connection| {
                            connection.role == role
                                && connection.face_port_id == face.external_port.port_id
                        }) {
                            item_capacity = item_capacity.min(connection.spec.item_capacity);
                            byte_capacity = byte_capacity.min(connection.spec.byte_capacity);
                        }
                        CompositePortBinding {
                            external_port_id: face.external_port.port_id.clone(),
                            placement_id: face.internal_placement_id.clone(),
                            internal_port_id: face.internal_port_id.clone(),
                            value_kind: face.external_port.value_kind.clone(),
                            item_capacity,
                            byte_capacity,
                        }
                    })
                    .collect::<Vec<_>>()
            };
            let inputs = bindings(&self.boundary.input_faces, ExternalConnectionRole::Input);
            let outputs = bindings(&self.boundary.output_faces, ExternalConnectionRole::Output);
            if child
                .runtime
                .configure_composite_boundary(&self.internal_plan_id, inputs, outputs)
                .is_err()
            {
                output.events.push(HostEvent::PreparationRejected {
                    plan_id,
                    reason: self.failure_translation,
                    message: Some("composite child boundary configuration failed".into()),
                });
                return output;
            }
        }
        self.external_plans.insert(
            plan_id.clone(),
            ExternalPlan {
                state: ExternalState::Prepared,
                terminal_emitted: false,
                child_terminals: BTreeMap::new(),
                active_play_id: None,
                connections: external_connections,
                pending_outputs: BTreeMap::new(),
            },
        );
        self.record(Some(plan_id.clone()), ObservationKind::PlanFragmentReceived);
        output.events.push(HostEvent::Prepared { plan_id });
        output
    }

    fn start_play(&mut self, plan_id: PlanId) -> RuntimeOutput {
        let mut output = RuntimeOutput::default();
        let Some(plan) = self.external_plans.get_mut(&plan_id) else {
            output.events.push(HostEvent::PlayStartRejected {
                plan_id,
                reason: FailureReason::InvalidLifecycleCommand,
                message: Some("unknown external plan".into()),
            });
            return output;
        };
        if plan.state != ExternalState::Prepared {
            output.events.push(HostEvent::PlayStartRejected {
                plan_id,
                reason: FailureReason::InvalidLifecycleCommand,
                message: Some("external plan is not prepared".into()),
            });
            return output;
        }
        let Some(next_active_play_sequence) = self.next_active_play_sequence.checked_add(1) else {
            output.events.push(HostEvent::PlayStartRejected {
                plan_id,
                reason: FailureReason::InvalidLifecycleCommand,
                message: Some("active-play identity sequence exhausted".into()),
            });
            return output;
        };
        let active_play = bind_active_play(
            &plan_id,
            &self.advertisement.host_id,
            &self.advertisement.boot_id,
            self.next_active_play_sequence,
        );
        self.next_active_play_sequence = next_active_play_sequence;
        plan.active_play_id = Some(active_play.active_play_id.clone());
        plan.state = ExternalState::Active;
        self.record(Some(plan_id.clone()), ObservationKind::PlanPlayStarted);
        output.events.push(HostEvent::PlayStarted {
            plan_id: plan_id.clone(),
            active_play_id: active_play.active_play_id,
        });
        let connected_inputs = self
            .external_plans
            .get(&plan_id)
            .expect("active external plan exists")
            .connections
            .values()
            .filter(|connection| connection.role == ExternalConnectionRole::Input)
            .map(|connection| connection.face_port_id.clone())
            .collect::<BTreeSet<_>>();
        for face in &self.boundary.input_faces {
            if !connected_inputs.contains(&face.external_port.port_id) {
                let _ = self
                    .children
                    .get_mut(&face.internal_child)
                    .expect("checked input child exists")
                    .runtime
                    .close_composite_input(&self.internal_plan_id, &face.external_port.port_id);
            }
        }
        let source_children = self
            .internal_connections
            .values()
            .map(|connection| connection.source_child.clone())
            .collect::<BTreeSet<_>>();
        let mut child_ids = self.children.keys().cloned().collect::<Vec<_>>();
        child_ids.sort_by_key(|host_id| source_children.contains(host_id));
        let initial = child_ids
            .into_iter()
            .map(|host_id| {
                let child_output = self
                    .children
                    .get_mut(&host_id)
                    .expect("listed child exists")
                    .runtime
                    .handle(HostCommand::StartPlay(self.internal_plan_id.clone()));
                (host_id, child_output)
            })
            .collect();
        self.drive_internal(&plan_id, initial, &mut output);
        output
    }

    fn drive_internal(
        &mut self,
        external_plan_id: &PlanId,
        initial: Vec<(HostId, RuntimeOutput)>,
        external: &mut RuntimeOutput,
    ) {
        let mut pending = VecDeque::from(initial);
        while let Some((child, output)) = pending.pop_front() {
            for event in output.events {
                self.internal_events.push((child.clone(), event.clone()));
                match event {
                    HostEvent::ConnectionTerminated {
                        connection_id,
                        disposition:
                            conduit_core::ConnectionTerminalDisposition {
                                disposition: TerminalDisposition::Completed,
                                ..
                            },
                        ..
                    } => {
                        if let Some(connection) = self.internal_connections.get(&connection_id) {
                            if child == connection.source_child {
                                let sink_child = connection.sink_child.clone();
                                let closed = self
                                    .children
                                    .get_mut(&sink_child)
                                    .expect("validated connection sink exists")
                                    .runtime
                                    .handle(HostCommand::CloseConnection {
                                        plan_id: self.internal_plan_id.clone(),
                                        connection_id,
                                    });
                                pending.push_back((sink_child, closed));
                            }
                        }
                    }
                    HostEvent::PlanTerminated { disposition, .. } => {
                        if let Some(plan) = self.external_plans.get_mut(external_plan_id) {
                            plan.child_terminals.insert(child.clone(), disposition);
                            if matches!(disposition, TerminalDisposition::Failed { .. }) {
                                plan.pending_outputs.clear();
                                for (connection_id, connection) in &mut plan.connections {
                                    if connection.terminal.is_none() {
                                        connection.terminal = Some(TerminalDisposition::Failed {
                                            reason: self.failure_translation,
                                        });
                                        external.events.push(HostEvent::ConnectionTerminated {
                                            plan_id: external_plan_id.clone(),
                                            connection_id: connection_id.clone(),
                                            disposition:
                                                conduit_core::ConnectionTerminalDisposition {
                                                    disposition: TerminalDisposition::Failed {
                                                        reason: self.failure_translation,
                                                    },
                                                    last_accepted_sequence: connection
                                                        .next_expected_sequence
                                                        .checked_sub(1),
                                                    last_manifested_sequence: None,
                                                    undeliverable_items: 0,
                                                },
                                        });
                                    }
                                }
                            }
                        }
                        if matches!(disposition, TerminalDisposition::Failed { .. }) {
                            let other_ids = self
                                .children
                                .keys()
                                .filter(|host_id| *host_id != &child)
                                .cloned()
                                .collect::<Vec<_>>();
                            for other_id in other_ids {
                                let cancellation = self
                                    .children
                                    .get_mut(&other_id)
                                    .expect("listed child exists")
                                    .runtime
                                    .handle(HostCommand::Cancel(self.internal_plan_id.clone()));
                                pending.push_back((other_id, cancellation));
                            }
                        }
                    }
                    _ => {}
                }
            }
            for effect in output.effects {
                match effect {
                    PlatformEffect::Wait {
                        plan_id,
                        placement_id,
                        ..
                    } => {
                        let next = self
                            .children
                            .get_mut(&child)
                            .expect("effect came from a known child")
                            .runtime
                            .handle(HostCommand::CompleteWait {
                                plan_id,
                                placement_id,
                            });
                        pending.push_back((child.clone(), next));
                    }
                    PlatformEffect::PresentValue {
                        plan_id,
                        active_play_id,
                        presentation_id,
                        placement_id,
                        value,
                        ..
                    } => {
                        let success = !self.fail_next_presentation;
                        self.fail_next_presentation = false;
                        let next = self
                            .children
                            .get_mut(&child)
                            .expect("effect came from a known child")
                            .runtime
                            .handle(HostCommand::CompletePresentation {
                                plan_id,
                                active_play_id,
                                presentation_id,
                                placement_id,
                                value,
                                success,
                                message: (!success)
                                    .then(|| "injected child presentation failure".into()),
                            });
                        pending.push_back((child.clone(), next));
                    }
                    PlatformEffect::TransmitConnection { envelope } => {
                        let connection_id = envelope.connection_id.clone();
                        let sequence = envelope.sequence;
                        let (source_child, sink_child, outcome) =
                            match self.internal_connections.get_mut(&connection_id) {
                                Some(connection) if connection.source_child == child => (
                                    connection.source_child.clone(),
                                    connection.sink_child.clone(),
                                    connection.base.accept(envelope),
                                ),
                                _ => {
                                    let source_output = self
                                        .children
                                        .get_mut(&child)
                                        .expect("effect came from a known child")
                                        .runtime
                                        .handle(HostCommand::CompleteConnectionDelivery {
                                            plan_id: self.internal_plan_id.clone(),
                                            connection_id,
                                            sequence,
                                            outcome: ConnectionOutcome::Malformed,
                                        });
                                    pending.push_back((child.clone(), source_output));
                                    continue;
                                }
                            };
                        if outcome == ConnectionOutcome::Accepted {
                            let accepted = self
                                .children
                                .get_mut(&source_child)
                                .expect("effect came from a known child")
                                .runtime
                                .handle(HostCommand::CompleteConnectionDelivery {
                                    plan_id: self.internal_plan_id.clone(),
                                    connection_id: connection_id.clone(),
                                    sequence,
                                    outcome: ConnectionOutcome::Accepted,
                                });
                            pending.push_back((source_child.clone(), accepted));
                            if self.delivery_mode == DeliveryMode::Immediate {
                                let (delivery_outcome, delivered) = self
                                    .internal_connections
                                    .get_mut(&connection_id)
                                    .expect("accepted internal connection exists")
                                    .base
                                    .deliver()
                                    .expect("accepted envelope must be queued");
                                debug_assert_eq!(delivery_outcome, ConnectionOutcome::Delivered);
                                let sink_output = self
                                    .children
                                    .get_mut(&sink_child)
                                    .expect("validated boundary sink exists")
                                    .runtime
                                    .handle(HostCommand::AcceptConnectionEnvelope(delivered));
                                let sink_accepted = sink_output.events.iter().any(|event| {
                                    matches!(
                                        event,
                                        HostEvent::ConnectionEnvelopeOutcome {
                                            outcome: ConnectionOutcome::Accepted,
                                            ..
                                        }
                                    )
                                });
                                pending.push_back((sink_child, sink_output));
                                let source_output = self
                                    .children
                                    .get_mut(&source_child)
                                    .expect("effect came from a known child")
                                    .runtime
                                    .handle(HostCommand::CompleteConnectionDelivery {
                                        plan_id: self.internal_plan_id.clone(),
                                        connection_id: connection_id.clone(),
                                        sequence,
                                        outcome: if sink_accepted {
                                            ConnectionOutcome::Delivered
                                        } else {
                                            ConnectionOutcome::Malformed
                                        },
                                    });
                                pending.push_back((source_child, source_output));
                            }
                        } else {
                            let source_output = self
                                .children
                                .get_mut(&source_child)
                                .expect("effect came from a known child")
                                .runtime
                                .handle(HostCommand::CompleteConnectionDelivery {
                                    plan_id: self.internal_plan_id.clone(),
                                    connection_id,
                                    sequence,
                                    outcome,
                                });
                            pending.push_back((source_child, source_output));
                        }
                    }
                }
            }
            let boundary_effects = self
                .children
                .get_mut(&child)
                .expect("output came from a known child")
                .runtime
                .drain_composite_boundary_effects();
            for effect in boundary_effects {
                self.process_boundary_effect(
                    external_plan_id,
                    &child,
                    effect,
                    &mut pending,
                    external,
                );
            }
        }
        self.finish_external_if_terminal(external_plan_id, external);
    }

    fn process_boundary_effect(
        &mut self,
        external_plan_id: &PlanId,
        child: &HostId,
        effect: CompositeBoundaryEffect,
        pending: &mut VecDeque<(HostId, RuntimeOutput)>,
        external: &mut RuntimeOutput,
    ) {
        match effect {
            CompositeBoundaryEffect::Transmit {
                plan_id,
                port_id,
                sequence,
                value,
            } => {
                if plan_id != self.internal_plan_id {
                    return;
                }
                let key = (port_id.clone(), sequence);
                let mut branches = BTreeMap::new();
                let Some(plan) = self.external_plans.get_mut(external_plan_id) else {
                    return;
                };
                if plan.pending_outputs.contains_key(&key) {
                    return;
                }
                let connection_ids = plan
                    .connections
                    .iter()
                    .filter(|(_, connection)| {
                        connection.role == ExternalConnectionRole::Output
                            && connection.face_port_id == port_id
                            && connection.terminal.is_none()
                    })
                    .map(|(connection_id, _)| connection_id.clone())
                    .collect::<Vec<_>>();
                for connection_id in connection_ids {
                    let connection = plan
                        .connections
                        .get_mut(&connection_id)
                        .expect("listed external output exists");
                    let external_sequence = connection.next_send_sequence;
                    connection.next_send_sequence += 1;
                    external.effects.push(PlatformEffect::TransmitConnection {
                        envelope: ConnectionEnvelope {
                            protocol_version: PROTOCOL_VERSION,
                            plan_id: external_plan_id.clone(),
                            connection_id: connection_id.clone(),
                            sequence: external_sequence,
                            value_kind: value.value_kind.clone(),
                            payload: value.encoded.clone(),
                        },
                    });
                    branches.insert(
                        connection_id,
                        ExternalOutputBranch {
                            sequence: external_sequence,
                            state: ExternalDeliveryState::Offered,
                            value: value.clone(),
                        },
                    );
                }
                if branches.is_empty() {
                    let child_output = self
                        .children
                        .get_mut(child)
                        .expect("boundary effect child exists")
                        .runtime
                        .complete_composite_output(
                            &self.internal_plan_id,
                            &port_id,
                            sequence,
                            ConnectionOutcome::Delivered,
                        );
                    pending.push_back((child.clone(), child_output));
                } else {
                    plan.pending_outputs.insert(
                        key,
                        PendingFaceOutput {
                            child: child.clone(),
                            branches,
                        },
                    );
                }
                self.record(
                    Some(external_plan_id.clone()),
                    ObservationKind::ValueProduced { value },
                );
            }
            CompositeBoundaryEffect::Closed {
                plan_id,
                port_id,
                disposition,
            } => {
                if plan_id != self.internal_plan_id {
                    return;
                }
                let Some(plan) = self.external_plans.get_mut(external_plan_id) else {
                    return;
                };
                let connection_ids = plan
                    .connections
                    .iter()
                    .filter(|(_, connection)| {
                        connection.role == ExternalConnectionRole::Output
                            && connection.face_port_id == port_id
                            && connection.terminal.is_none()
                    })
                    .map(|(connection_id, _)| connection_id.clone())
                    .collect::<Vec<_>>();
                for connection_id in connection_ids {
                    let connection = plan
                        .connections
                        .get_mut(&connection_id)
                        .expect("listed external output exists");
                    connection.terminal = Some(disposition);
                    external.events.push(HostEvent::ConnectionTerminated {
                        plan_id: external_plan_id.clone(),
                        connection_id,
                        disposition: conduit_core::ConnectionTerminalDisposition {
                            disposition,
                            last_accepted_sequence: connection.next_send_sequence.checked_sub(1),
                            last_manifested_sequence: connection.next_send_sequence.checked_sub(1),
                            undeliverable_items: 0,
                        },
                    });
                }
            }
        }
    }

    fn finish_external_if_terminal(&mut self, plan_id: &PlanId, output: &mut RuntimeOutput) {
        let Some(plan) = self.external_plans.get_mut(plan_id) else {
            return;
        };
        if plan.terminal_emitted
            || plan.child_terminals.len() != self.children.len()
            || !plan.pending_outputs.is_empty()
            || plan
                .connections
                .values()
                .any(|connection| connection.terminal.is_none())
        {
            return;
        }
        let failed = plan
            .child_terminals
            .values()
            .any(|disposition| matches!(disposition, TerminalDisposition::Failed { .. }));
        let cancelled = plan
            .child_terminals
            .values()
            .any(|disposition| matches!(disposition, TerminalDisposition::Cancelled { .. }));
        let disposition = if failed {
            plan.state = ExternalState::Failed;
            TerminalDisposition::Failed {
                reason: self.failure_translation,
            }
        } else if cancelled {
            plan.state = ExternalState::Cancelled;
            TerminalDisposition::Cancelled {
                reason: conduit_core::CancellationReason::OperatorRequested,
            }
        } else {
            plan.state = ExternalState::Completed;
            output.events.push(HostEvent::PlanCompleted {
                plan_id: plan_id.clone(),
            });
            TerminalDisposition::Completed
        };
        plan.terminal_emitted = true;
        self.record(
            Some(plan_id.clone()),
            ObservationKind::PlanTerminal { disposition },
        );
        output.events.push(HostEvent::PlanTerminated {
            plan_id: plan_id.clone(),
            disposition,
        });
    }

    fn cancel(&mut self, plan_id: PlanId) -> RuntimeOutput {
        let mut output = RuntimeOutput::default();
        let Some(plan) = self.external_plans.get_mut(&plan_id) else {
            output.events.push(HostEvent::CommandRejected {
                plan_id: Some(plan_id),
                reason: FailureReason::InvalidLifecycleCommand,
            });
            return output;
        };
        if !matches!(plan.state, ExternalState::Prepared | ExternalState::Active) {
            output.events.push(HostEvent::CommandRejected {
                plan_id: Some(plan_id),
                reason: FailureReason::InvalidLifecycleCommand,
            });
            return output;
        }
        plan.state = ExternalState::Cancelled;
        plan.pending_outputs.clear();
        for (connection_id, connection) in &mut plan.connections {
            if connection.terminal.is_none() {
                connection.terminal = Some(TerminalDisposition::Cancelled {
                    reason: conduit_core::CancellationReason::OperatorRequested,
                });
                output.events.push(HostEvent::ConnectionTerminated {
                    plan_id: plan_id.clone(),
                    connection_id: connection_id.clone(),
                    disposition: conduit_core::ConnectionTerminalDisposition {
                        disposition: TerminalDisposition::Cancelled {
                            reason: conduit_core::CancellationReason::OperatorRequested,
                        },
                        last_accepted_sequence: connection.next_expected_sequence.checked_sub(1),
                        last_manifested_sequence: None,
                        undeliverable_items: 0,
                    },
                });
            }
        }
        let child_ids = self.children.keys().cloned().collect::<Vec<_>>();
        let initial = child_ids
            .into_iter()
            .map(|host_id| {
                let child_output = self
                    .children
                    .get_mut(&host_id)
                    .expect("listed child exists")
                    .runtime
                    .handle(HostCommand::Cancel(self.internal_plan_id.clone()));
                (host_id, child_output)
            })
            .collect();
        self.drive_internal(&plan_id, initial, &mut output);
        output.events.push(HostEvent::Cancelled { plan_id });
        output
    }

    fn release(&mut self, plan_id: PlanId) -> RuntimeOutput {
        let mut output = RuntimeOutput::default();
        let Some(plan) = self.external_plans.get(&plan_id) else {
            output.events.push(HostEvent::CommandRejected {
                plan_id: Some(plan_id),
                reason: FailureReason::InvalidLifecycleCommand,
            });
            return output;
        };
        if matches!(plan.state, ExternalState::Prepared | ExternalState::Active) {
            output.events.push(HostEvent::CommandRejected {
                plan_id: Some(plan_id),
                reason: FailureReason::InvalidLifecycleCommand,
            });
            return output;
        }
        self.external_plans.remove(&plan_id);
        self.released_plans.insert(plan_id.clone());
        for child in self.children.values_mut() {
            let _ = child
                .runtime
                .handle(HostCommand::Release(self.internal_plan_id.clone()));
        }
        self.record(Some(plan_id.clone()), ObservationKind::Released);
        output.events.push(HostEvent::Released { plan_id });
        output
    }

    fn record(&mut self, plan_id: Option<PlanId>, kind: ObservationKind) {
        if self.observation_limit == 0 {
            return;
        }
        let active_play_id = plan_id
            .as_ref()
            .and_then(|plan_id| self.external_plans.get(plan_id))
            .and_then(|plan| plan.active_play_id.clone());
        if self.observations.len() == self.observation_limit {
            let mut dropped = 1;
            if let Some(Observation {
                kind: ObservationKind::SignGap { dropped: previous },
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
                let gap_sign_id = self.issue_sign_id(None);
                self.observations.push(Observation {
                    sign_id: gap_sign_id,
                    active_play_id: None,
                    presentation_id: None,
                    host_id: self.advertisement.host_id.clone(),
                    boot_id: self.advertisement.boot_id.clone(),
                    plan_id: None,
                    placement_id: None,
                    connection_id: None,
                    kind: ObservationKind::SignGap { dropped },
                });
                return;
            }
            while self.observations.len() > self.observation_limit - 2 {
                self.observations.remove(0);
                dropped += 1;
            }
            let gap_sign_id = self.issue_sign_id(None);
            self.observations.insert(
                0,
                Observation {
                    sign_id: gap_sign_id,
                    active_play_id: None,
                    presentation_id: None,
                    host_id: self.advertisement.host_id.clone(),
                    boot_id: self.advertisement.boot_id.clone(),
                    plan_id: None,
                    placement_id: None,
                    connection_id: None,
                    kind: ObservationKind::SignGap { dropped },
                },
            );
        }
        let sign_id = self.issue_sign_id(active_play_id.as_ref());
        self.observations.push(Observation {
            sign_id,
            active_play_id,
            presentation_id: None,
            host_id: self.advertisement.host_id.clone(),
            boot_id: self.advertisement.boot_id.clone(),
            plan_id,
            placement_id: None,
            connection_id: None,
            kind,
        });
    }

    fn issue_sign_id(&mut self, active_play_id: Option<&ActivePlayId>) -> conduit_core::SignId {
        let sign = bind_sign(
            &self.advertisement.host_id,
            &self.advertisement.boot_id,
            active_play_id,
            self.next_sign_sequence,
        );
        self.next_sign_sequence = self
            .next_sign_sequence
            .checked_add(1)
            .expect("sign identity sequence exhausted");
        sign.sign_id
    }
}

fn preparation_failure(output: &RuntimeOutput) -> Option<(FailureReason, Option<String>)> {
    output.events.iter().find_map(|event| match event {
        HostEvent::PreparationRejected {
            reason, message, ..
        } => Some((*reason, message.clone())),
        _ => None,
    })
}

#[cfg(test)]
mod tests;
