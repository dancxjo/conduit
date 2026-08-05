use conduit_core::{
    BoundedQueue, CancellationReason, ConnectionEnvelope, ConnectionId, ConnectionOutcome,
    ConnectionProvider, ConnectionTerminalDisposition, FailureReason, HostAdvertisement,
    HostCommand, HostEvent, Observation, ObservationKind, PlacementId, PlacementLifecycleState,
    PlanFragment, PlanId, PlannedConnection, PlannedOperation, PlatformEffect, TerminalDisposition,
    ValuePayload, PROTOCOL_VERSION,
};
use conduit_signal::{
    decode_signal, encode_signal, parse_pulse_configuration, signal_payload_size, Signal,
    PULSE_KIND, SHOW_KIND, SIGNAL_VALUE_KIND,
};
use std::collections::{BTreeMap, BTreeSet};

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

#[derive(Debug)]
pub struct HostRuntime {
    advertisement: HostAdvertisement,
    observation_limit: usize,
    observations: Vec<Observation>,
    plans: BTreeMap<PlanId, RuntimePlan>,
    released_plans: BTreeSet<PlanId>,
}

#[derive(Debug)]
struct RuntimePlan {
    fragment: PlanFragment,
    placements: BTreeMap<PlacementId, RuntimePlacement>,
    connections: BTreeMap<ConnectionId, RuntimeConnection>,
    state: PlanState,
    terminal: Option<TerminalDisposition>,
    terminal_emitted: bool,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum PlanState {
    Prepared,
    Active,
    Failed,
    Cancelled,
    Completed,
}

#[derive(Debug)]
struct RuntimePlacement {
    spec: PlannedOperation,
    lifecycle: PlacementLifecycleState,
    terminal: Option<TerminalDisposition>,
    pulse: Option<PulseState>,
    show: Option<ShowState>,
}

#[derive(Debug)]
struct PulseState {
    next_sequence: u64,
    waiting: bool,
    emission_complete: bool,
}

#[derive(Debug)]
struct ShowState {
    pending: Option<(ConnectionId, ValuePayload)>,
}

#[derive(Debug)]
struct RuntimeConnection {
    spec: PlannedConnection,
    queue: BoundedQueue<ValuePayload>,
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
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum ConnectionRole {
    Local,
    Outbound,
    Inbound,
}

impl HostRuntime {
    pub fn new(advertisement: HostAdvertisement, observation_limit: usize) -> Self {
        let mut runtime = Self {
            advertisement,
            observation_limit,
            observations: Vec::new(),
            plans: BTreeMap::new(),
            released_plans: BTreeSet::new(),
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
                events: vec![HostEvent::Observations {
                    items: self.observations.clone(),
                }],
                effects: Vec::new(),
            },
        }
    }

    fn prepare(&mut self, fragment: PlanFragment) -> RuntimeOutput {
        let mut output = RuntimeOutput::default();
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
                reason: "wrong host identity".to_string(),
            });
            return output;
        }
        if fragment.boot_id != self.advertisement.boot_id {
            output.events.push(HostEvent::PreparationRejected {
                plan_id: fragment.plan_id,
                reason: "stale boot identity".to_string(),
            });
            return output;
        }
        if fragment.offer_generation != self.advertisement.offer_generation {
            output.events.push(HostEvent::PreparationRejected {
                plan_id: fragment.plan_id,
                reason: "stale offer generation".to_string(),
            });
            return output;
        }

        let mut counts = BTreeMap::<_, u16>::new();
        for placement in &fragment.placements {
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
                        reason: format!(
                            "unknown capability '{}'",
                            placement.capability_id.as_str()
                        ),
                    });
                    return output;
                }
            };
            let count = counts.entry(placement.capability_id.clone()).or_insert(0);
            *count += 1;
            if *count > capability.limits.max_active_instances {
                output.events.push(HostEvent::PreparationRejected {
                    plan_id: fragment.plan_id,
                    reason: format!(
                        "capability '{}' instance limit exceeded",
                        placement.capability_id.as_str()
                    ),
                });
                return output;
            }
        }

        let placements = fragment
            .placements
            .iter()
            .cloned()
            .map(|spec| {
                let pulse = if spec.kind_id.as_str() == PULSE_KIND {
                    Some(PulseState {
                        next_sequence: 0,
                        waiting: false,
                        emission_complete: false,
                    })
                } else {
                    None
                };
                let show = if spec.kind_id.as_str() == SHOW_KIND {
                    Some(ShowState { pending: None })
                } else {
                    None
                };
                (
                    spec.placement_id.clone(),
                    RuntimePlacement {
                        spec,
                        lifecycle: PlacementLifecycleState::Prepared,
                        terminal: None,
                        pulse,
                        show,
                    },
                )
            })
            .collect::<BTreeMap<_, _>>();

        let mut connections = BTreeMap::new();
        for connection in &fragment.connections {
            let source = placements.get(&connection.source_placement_id);
            let sink = placements.get(&connection.sink_placement_id);
            let role = match (connection.provider, source.is_some(), sink.is_some()) {
                (ConnectionProvider::Local, true, true) => ConnectionRole::Local,
                (ConnectionProvider::InMemory, true, false) => ConnectionRole::Outbound,
                (ConnectionProvider::InMemory, false, true) => ConnectionRole::Inbound,
                _ => {
                    output.events.push(HostEvent::PreparationRejected {
                        plan_id: fragment.plan_id,
                        reason: format!(
                            "connection '{}' has invalid local endpoints for {:?}",
                            connection.connection_id.as_str(),
                            connection.provider
                        ),
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
                    reason: format!(
                        "connection '{}' exceeds queue limits",
                        connection.connection_id.as_str()
                    ),
                });
                return output;
            }
            if local_capabilities
                .iter()
                .any(|capability| connection.byte_capacity > capability.limits.max_queue_bytes)
            {
                output.events.push(HostEvent::PreparationRejected {
                    plan_id: fragment.plan_id,
                    reason: format!(
                        "connection '{}' exceeds byte limits",
                        connection.connection_id.as_str()
                    ),
                });
                return output;
            }
            if connection.value_kind.as_str() == SIGNAL_VALUE_KIND
                && connection.byte_capacity < signal_payload_size()
            {
                output.events.push(HostEvent::PreparationRejected {
                    plan_id: fragment.plan_id,
                    reason: format!(
                        "connection '{}' byte capacity is too small for one signal",
                        connection.connection_id.as_str()
                    ),
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
                },
            );
        }

        for placement in placements.values() {
            self.record_observation(
                Some(fragment.plan_id.clone()),
                Some(placement.spec.placement_id.clone()),
                None,
                ObservationKind::PlacementPrepared,
            );
        }
        self.record_observation(
            Some(fragment.plan_id.clone()),
            None,
            None,
            ObservationKind::PlanFragmentReceived,
        );
        self.plans.insert(
            fragment.plan_id.clone(),
            RuntimePlan {
                fragment: fragment.clone(),
                placements,
                connections,
                state: PlanState::Prepared,
                terminal: None,
                terminal_emitted: false,
            },
        );
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
                reason: "plan was not prepared".to_string(),
            });
            return output;
        };
        if plan.state != PlanState::Prepared {
            output.events.push(HostEvent::ActivationRejected {
                plan_id: plan_id.clone(),
                reason: "plan is not in prepared state".to_string(),
            });
            return output;
        }
        plan.state = PlanState::Active;
        for placement_id in &plan.fragment.startup_order {
            if let Some(placement) = plan.placements.get_mut(placement_id) {
                placement.lifecycle = PlacementLifecycleState::Active;
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
        if let Some(placement) = plan.placements.get_mut(placement_id) {
            if let Some(pulse) = placement.pulse.as_mut() {
                if placement.lifecycle == PlacementLifecycleState::Active {
                    pulse.waiting = false;
                } else {
                    output.events.push(HostEvent::CommandRejected {
                        plan_id: Some(plan_id.clone()),
                        reason: FailureReason::LatePlatformCompletion,
                    });
                    return output;
                }
            }
        }
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
        let mut pending_observations = Vec::new();
        let mut pending_terminal_events = Vec::new();
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
        if let Some(show) = placement.show.as_mut() {
            let pending_connection = show.pending.take().map(|(connection_id, _)| connection_id);
            if success {
                if let Some(connection_id) = pending_connection {
                    if let Some(connection) = plan.connections.get_mut(&connection_id) {
                        connection.last_manifested_sequence = payload_sequence(&value);
                    }
                }
                pending_observations.push((
                    Some(plan_id.clone()),
                    Some(placement_id.clone()),
                    None,
                    ObservationKind::ValuePresented {
                        value: value.clone(),
                    },
                ));
                output.events.push(HostEvent::ManifestationCompleted {
                    plan_id: plan_id.clone(),
                    placement_id: placement_id.clone(),
                    value,
                });
            } else {
                let reason = message.unwrap_or_else(|| "presentation failed".to_string());
                fail_placement(
                    plan,
                    placement_id,
                    TerminalDisposition::Failed {
                        reason: FailureReason::ManifestationFailed,
                    },
                    &mut pending_observations,
                    &mut pending_terminal_events,
                    plan_id,
                );
                fail_incoming_connections(
                    plan,
                    placement_id,
                    TerminalDisposition::Failed {
                        reason: FailureReason::ManifestationFailed,
                    },
                    &mut pending_observations,
                    &mut pending_terminal_events,
                    plan_id,
                );
                if plan.state != PlanState::Failed {
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
                pending_observations.push((
                    Some(plan_id.clone()),
                    Some(placement_id.clone()),
                    None,
                    ObservationKind::Failure {
                        reason: reason.clone(),
                    },
                ));
                output.events.push(HostEvent::ManifestationFailed {
                    plan_id: plan_id.clone(),
                    placement_id: placement_id.clone(),
                    value,
                    reason,
                });
            }
        }
        let _ = plan;
        for item in pending_observations {
            self.record_observation(item.0, item.1, item.2, item.3);
        }
        output.events.extend(pending_terminal_events);
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
            .push(value)
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
        let front_sequence = connection.queue.front().and_then(payload_sequence);
        if connection.role != ConnectionRole::Outbound
            || !connection.transmission_in_flight
            || front_sequence != Some(sequence)
        {
            output.events.push(HostEvent::CommandRejected {
                plan_id: Some(plan_id.clone()),
                reason: FailureReason::MalformedConnectionEnvelope,
            });
            return output;
        }
        match outcome {
            ConnectionOutcome::Accepted => {
                output.events.push(HostEvent::ConnectionEnvelopeOutcome {
                    plan_id: plan_id.clone(),
                    connection_id: connection_id.clone(),
                    sequence,
                    outcome: ConnectionOutcome::Accepted,
                });
            }
            ConnectionOutcome::Delivered => {
                let value = connection.queue.pop().expect("in-flight value must exist");
                connection.queued_bytes -= value.encoded_len();
                connection.last_accepted_sequence = Some(sequence);
                connection.transmission_in_flight = false;
                output.events.push(HostEvent::ConnectionEnvelopeOutcome {
                    plan_id: plan_id.clone(),
                    connection_id: connection_id.clone(),
                    sequence,
                    outcome: ConnectionOutcome::Delivered,
                });
                if connection.source_done && connection.queue.is_empty() {
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
                output.events.push(HostEvent::ConnectionBlocked {
                    plan_id: plan_id.clone(),
                    connection_id: connection_id.clone(),
                });
            }
            ConnectionOutcome::Disconnected
            | ConnectionOutcome::Malformed
            | ConnectionOutcome::Terminal => {
                connection.transmission_in_flight = false;
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
        self.pump(plan_id, &mut output);
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
                let Some(placement) = plan.placements.get_mut(&placement_id) else {
                    continue;
                };
                if placement.lifecycle != PlacementLifecycleState::Active {
                    continue;
                }
                if let Some(pulse) = placement.pulse.as_mut() {
                    let config = parse_pulse_configuration(&placement.spec.configuration)
                        .expect("planned pulse configuration must be valid");
                    if pulse.emission_complete || pulse.waiting {
                        continue;
                    }
                    if pulse.next_sequence >= config.count {
                        pulse.emission_complete = true;
                        mark_source_done(&placement.spec.placement_id, &mut plan.connections);
                        terminate_placement(
                            placement,
                            TerminalDisposition::Completed,
                            &mut pending_observations,
                            &mut pending_terminal_events,
                            plan_id,
                        );
                        output.events.push(HostEvent::PlacementCompleted {
                            plan_id: plan_id.clone(),
                            placement_id: placement.spec.placement_id.clone(),
                        });
                        changed = true;
                        continue;
                    }

                    let value = encode_signal(&Signal {
                        sequence: pulse.next_sequence,
                        level: if pulse.next_sequence % 2 == 0 {
                            config.initial_level
                        } else {
                            !config.initial_level
                        },
                    });
                    let outgoing =
                        outgoing_connections(&placement.spec.placement_id, &plan.connections);
                    let mut blocked = None;
                    for connection_id in &outgoing {
                        let connection = &plan.connections[connection_id];
                        if connection.terminal.is_some() || connection.sink_failed {
                            continue;
                        }
                        if connection.queue.len() >= connection.queue.capacity()
                            || connection.queued_bytes + value.encoded_len()
                                > connection.spec.byte_capacity
                        {
                            blocked = Some(connection_id.clone());
                            break;
                        }
                    }
                    if let Some(connection_id) = blocked {
                        if let Some(connection) = plan.connections.get_mut(&connection_id) {
                            if !connection.blocked {
                                connection.blocked = true;
                                output.events.push(HostEvent::ConnectionBlocked {
                                    plan_id: plan_id.clone(),
                                    connection_id: connection_id.clone(),
                                });
                            }
                        }
                        continue;
                    }
                    for connection_id in outgoing {
                        let connection = plan
                            .connections
                            .get_mut(&connection_id)
                            .expect("connection must exist");
                        if connection.terminal.is_some() || connection.sink_failed {
                            continue;
                        }
                        connection.blocked = false;
                        connection.queued_bytes += value.encoded_len();
                        connection
                            .queue
                            .push(value.clone())
                            .expect("capacity was checked before push");
                        output.events.push(HostEvent::ValueDelivered {
                            plan_id: plan_id.clone(),
                            connection_id: connection_id.clone(),
                            value: value.clone(),
                        });
                        pending_observations.push((
                            Some(plan_id.clone()),
                            Some(placement.spec.placement_id.clone()),
                            Some(connection_id),
                            ObservationKind::ValueProduced {
                                value: value.clone(),
                            },
                        ));
                    }
                    pulse.next_sequence += 1;
                    changed = true;
                    if pulse.next_sequence < config.count && config.period_ms > 0 {
                        pulse.waiting = true;
                        output.events.push(HostEvent::TimerRequested {
                            plan_id: plan_id.clone(),
                            placement_id: placement.spec.placement_id.clone(),
                            duration_ms: config.period_ms,
                        });
                        output.effects.push(PlatformEffect::Wait {
                            plan_id: plan_id.clone(),
                            placement_id: placement.spec.placement_id.clone(),
                            duration_ms: config.period_ms,
                        });
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
                        let value = connection
                            .queue
                            .front()
                            .expect("non-empty outbound queue has a front value");
                        let sequence = payload_sequence(value).unwrap_or(0);
                        connection.transmission_in_flight = true;
                        output.effects.push(PlatformEffect::TransmitConnection {
                            envelope: ConnectionEnvelope {
                                protocol_version: PROTOCOL_VERSION,
                                plan_id: plan_id.clone(),
                                connection_id: connection_id.clone(),
                                sequence,
                                value_kind: value.value_kind.clone(),
                                payload: value.encoded.clone(),
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
                let Some(show) = sink.show.as_mut() else {
                    continue;
                };
                if show.pending.is_some() || sink.lifecycle != PlacementLifecycleState::Active {
                    continue;
                }
                let value = connection
                    .queue
                    .pop()
                    .expect("queue was checked before pop");
                connection.queued_bytes -= value.encoded_len();
                connection.last_accepted_sequence = payload_sequence(&value);
                show.pending = Some((connection_id.clone(), value.clone()));
                output.events.push(HostEvent::PresentValueRequested {
                    plan_id: plan_id.clone(),
                    placement_id: sink_id.clone(),
                    value: value.clone(),
                });
                output.effects.push(PlatformEffect::PresentValue {
                    plan_id: plan_id.clone(),
                    placement_id: sink_id.clone(),
                    value: value.clone(),
                });
                pending_observations.push((
                    Some(plan_id.clone()),
                    Some(sink_id),
                    Some(connection_id),
                    ObservationKind::ValueAccepted { value },
                ));
                changed = true;
            }

            let sink_ids = plan
                .placements
                .iter()
                .filter_map(|(placement_id, placement)| {
                    placement.show.as_ref().map(|_| placement_id.clone())
                })
                .collect::<Vec<_>>();
            for sink_id in sink_ids {
                let Some(sink) = plan.placements.get_mut(&sink_id) else {
                    continue;
                };
                if sink.lifecycle != PlacementLifecycleState::Active {
                    continue;
                }
                let Some(show) = sink.show.as_ref() else {
                    continue;
                };
                if show.pending.is_some() {
                    continue;
                }
                let incoming = incoming_connections(&sink_id, &plan.connections);
                let done = incoming.iter().all(|connection_id| {
                    let connection = &plan.connections[connection_id];
                    (connection.source_done
                        || connection.sink_failed
                        || connection.terminal.is_some())
                        && connection.queue.is_empty()
                });
                if done {
                    terminate_placement(
                        sink,
                        TerminalDisposition::Completed,
                        &mut pending_observations,
                        &mut pending_terminal_events,
                        plan_id,
                    );
                    output.events.push(HostEvent::PlacementCompleted {
                        plan_id: plan_id.clone(),
                        placement_id: sink_id.clone(),
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
        undeliverable_items: connection.queue.len() as u16,
    };
    connection.queued_bytes = 0;
    while connection.queue.pop().is_some() {}
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

fn fail_placement(
    plan: &mut RuntimePlan,
    placement_id: &PlacementId,
    disposition: TerminalDisposition,
    observations: &mut Vec<PendingObservation>,
    events: &mut Vec<HostEvent>,
    plan_id: &PlanId,
) {
    if let Some(placement) = plan.placements.get_mut(placement_id) {
        terminate_placement(placement, disposition, observations, events, plan_id);
    }
}

fn fail_incoming_connections(
    plan: &mut RuntimePlan,
    placement_id: &PlacementId,
    disposition: TerminalDisposition,
    observations: &mut Vec<PendingObservation>,
    events: &mut Vec<HostEvent>,
    plan_id: &PlanId,
) {
    for connection_id in incoming_connections(placement_id, &plan.connections) {
        if let Some(connection) = plan.connections.get_mut(&connection_id) {
            connection.sink_failed = true;
            terminate_connection(connection, disposition, observations, events, plan_id);
        }
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
        if placement.pulse.is_some() && placement.lifecycle == PlacementLifecycleState::Active {
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
            terminate_placement(
                placement,
                TerminalDisposition::Cancelled { reason },
                observations,
                events,
                plan_id,
            );
        }
        if let Some(show) = placement.show.as_mut() {
            show.pending = None;
        }
        if let Some(pulse) = placement.pulse.as_mut() {
            pulse.waiting = false;
            pulse.emission_complete = true;
        }
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

fn payload_sequence(payload: &ValuePayload) -> Option<u64> {
    decode_signal(payload).ok().map(|signal| signal.sequence)
}

#[cfg(test)]
mod tests {
    use super::HostRuntime;
    use conduit_core::{
        kind_id, BootId, CancellationReason, CapabilityId, CapabilityLimits, CapabilityOffer,
        ConnectionProvider, FailureReason, HostAdvertisement, HostCommand, HostEvent, HostId,
        HostProfileId, ImplementationId, ObservationKind, OfferGeneration, PlatformEffect,
        TerminalDisposition, PROTOCOL_VERSION,
    };
    use conduit_form::parse;
    use conduit_planner::{default_placements, plan};
    use conduit_signal::{
        decode_signal, encode_signal, Signal, PULSE_KIND, SHOW_KIND, SIGNAL_VALUE_KIND,
    };
    use std::collections::{BTreeMap, VecDeque};

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
                    implementation_id: ImplementationId::from("std/pulse-v1"),
                    limits: CapabilityLimits {
                        value_kind: kind_id(SIGNAL_VALUE_KIND),
                        max_active_instances: 8,
                        max_queue_items: queue_items,
                        max_queue_bytes: queue_bytes,
                    },
                },
                CapabilityOffer {
                    capability_id: CapabilityId::from("stdout-show-1"),
                    kind_id: kind_id(SHOW_KIND),
                    implementation_id: ImplementationId::from("std/stdout-show-signal-v1"),
                    limits: CapabilityLimits {
                        value_kind: kind_id(SIGNAL_VALUE_KIND),
                        max_active_instances: 8,
                        max_queue_items: queue_items,
                        max_queue_bytes: queue_bytes,
                    },
                },
            ],
        }
    }

    fn demo_fragment(
        form_source: &str,
        queue_items: u16,
        queue_bytes: u32,
    ) -> conduit_core::PlanFragment {
        let form = parse(form_source).expect("form should parse");
        let advertisement = advertisement("boot-1", 1, 8, 256);
        let placements = default_placements(&form, std::slice::from_ref(&advertisement))
            .expect("placements work");
        let mut plan = plan(
            &form,
            std::slice::from_ref(&advertisement),
            &placements,
            &[ConnectionProvider::Local],
        )
        .expect("plan should succeed");
        let fragment = plan.fragments.get_mut(0).expect("fragment exists");
        for connection in &mut fragment.connections {
            connection.item_capacity = queue_items;
            connection.byte_capacity = queue_bytes;
        }
        fragment.clone()
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
        let mut runtime = HostRuntime::new(advertisement("boot-2", 1, 4, 64), 128);
        let output = runtime.handle(HostCommand::Prepare(fragment));
        assert!(matches!(
            output.events.first(),
            Some(HostEvent::PreparationRejected { .. })
        ));
    }

    #[test]
    fn preparation_rejects_stale_offer_generation() {
        let fragment = demo_fragment("form 0\n\ndemo {\n    pulse: flow/pulse\n    show: presentation/show\n\n    pulse.count = 2\n    pulse.period-ms = 0\n    pulse.initial = false\n\n    pulse > show\n}\n", 4, 64);
        let mut runtime = HostRuntime::new(advertisement("boot-1", 2, 4, 64), 128);
        let output = runtime.handle(HostCommand::Prepare(fragment));
        assert!(matches!(
            output.events.first(),
            Some(HostEvent::PreparationRejected { .. })
        ));
    }

    #[test]
    fn preparation_rejects_too_small_byte_capacity() {
        let fragment = demo_fragment("form 0\n\ndemo {\n    pulse: flow/pulse\n    show: presentation/show\n\n    pulse.count = 1\n    pulse.period-ms = 0\n    pulse.initial = false\n\n    pulse > show\n}\n", 4, 8);
        let mut runtime = HostRuntime::new(advertisement("boot-1", 1, 4, 64), 128);
        let output = runtime.handle(HostCommand::Prepare(fragment));
        assert!(matches!(
            output.events.first(),
            Some(HostEvent::PreparationRejected { .. })
        ));
    }

    #[test]
    fn full_queue_applies_backpressure() {
        let fragment = demo_fragment("form 0\n\ndemo {\n    pulse: flow/pulse\n    show: presentation/show\n\n    pulse.count = 3\n    pulse.period-ms = 0\n    pulse.initial = false\n\n    pulse > show\n}\n", 1, 64);
        let mut runtime = HostRuntime::new(advertisement("boot-1", 1, 1, 64), 128);
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
        let mut runtime = HostRuntime::new(advertisement("boot-1", 1, 4, 64), 128);
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
        let mut runtime = HostRuntime::new(advertisement("boot-1", 1, 4, 64), 256);
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
        let mut runtime = HostRuntime::new(advertisement("boot-1", 1, 4, 64), 128);
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
        let mut runtime = HostRuntime::new(advertisement("boot-1", 1, 4, 64), 128);
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
        let mut runtime = HostRuntime::new(advertisement("boot-1", 1, 4, 64), 128);
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
        let mut runtime = HostRuntime::new(advertisement("boot-1", 1, 4, 64), 4);
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
        let mut runtime = HostRuntime::new(advertisement("boot-1", 1, 4, 64), 512);
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
        let mut runtime = HostRuntime::new(advertisement("boot-1", 1, 4, 64), 768);
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
        let mut runtime = HostRuntime::new(advertisement("boot-1", 1, 4, 64), 256);
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
        let mut runtime = HostRuntime::new(advertisement("boot-1", 1, 4, 64), 256);
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
        let mut runtime = HostRuntime::new(advertisement("boot-1", 1, 4, 64), 512);
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
            let mut runtime = HostRuntime::new(advertisement("boot-1", 1, 4, 64), 256);
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
