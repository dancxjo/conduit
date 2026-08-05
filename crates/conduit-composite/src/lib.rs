use conduit_core::{
    kind_id, BootId, CapabilityId, CapabilityLimits, CapabilityOffer, ConnectionEnvelope,
    ConnectionId, ConnectionOutcome, ConnectionProvider, FailureReason, HostAdvertisement,
    HostCommand, HostEvent, HostId, HostProfileId, ImplementationId, Observation, ObservationKind,
    OfferGeneration, Plan, PlanFragment, PlanId, PlatformEffect, TerminalDisposition,
    PROTOCOL_VERSION,
};
use conduit_runtime::{HostRuntime, RuntimeOutput};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

pub const COMPOSITE_DEMONSTRATION_KIND: &str = "demonstration/run-signal";

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

#[derive(Debug)]
pub struct InMemoryConnectionProvider {
    plan_id: PlanId,
    connection_id: ConnectionId,
    value_kind: conduit_core::KindId,
    item_capacity: usize,
    byte_capacity: u32,
    queued_bytes: u32,
    next_sequence: u64,
    terminal: bool,
    queue: VecDeque<ConnectionEnvelope>,
}

impl InMemoryConnectionProvider {
    pub fn new(plan_id: PlanId, connection: &conduit_core::PlannedConnection) -> Self {
        Self {
            plan_id,
            connection_id: connection.connection_id.clone(),
            value_kind: connection.value_kind.clone(),
            item_capacity: connection.item_capacity as usize,
            byte_capacity: connection.byte_capacity,
            queued_bytes: 0,
            next_sequence: 0,
            terminal: false,
            queue: VecDeque::new(),
        }
    }

    pub fn status(&self) -> ConnectionOutcome {
        if self.terminal {
            ConnectionOutcome::Terminal
        } else if self.queue.len() >= self.item_capacity || self.queued_bytes >= self.byte_capacity
        {
            ConnectionOutcome::Full
        } else {
            ConnectionOutcome::Ready
        }
    }

    pub fn accept(&mut self, envelope: ConnectionEnvelope) -> ConnectionOutcome {
        if self.terminal {
            return ConnectionOutcome::Terminal;
        }
        if envelope.protocol_version != PROTOCOL_VERSION
            || envelope.plan_id != self.plan_id
            || envelope.connection_id != self.connection_id
            || envelope.value_kind != self.value_kind
            || envelope.sequence != self.next_sequence
            || envelope.encoded_len() > self.byte_capacity
        {
            return ConnectionOutcome::Malformed;
        }
        if self.queue.len() >= self.item_capacity
            || self.queued_bytes + envelope.encoded_len() > self.byte_capacity
        {
            return ConnectionOutcome::Full;
        }
        self.queued_bytes += envelope.encoded_len();
        self.next_sequence += 1;
        self.queue.push_back(envelope);
        ConnectionOutcome::Accepted
    }

    pub fn deliver(&mut self) -> Option<(ConnectionOutcome, ConnectionEnvelope)> {
        let envelope = self.queue.pop_front()?;
        self.queued_bytes -= envelope.encoded_len();
        Some((ConnectionOutcome::Delivered, envelope))
    }

    pub fn disconnect(&mut self) -> ConnectionOutcome {
        self.terminal = true;
        ConnectionOutcome::Disconnected
    }

    pub fn queued_items(&self) -> usize {
        self.queue.len()
    }

    pub fn queued_bytes(&self) -> u32 {
        self.queued_bytes
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum Child {
    Source,
    Sink,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum ExternalState {
    Prepared,
    Active,
    Completed,
    Failed,
    Cancelled,
}

#[derive(Debug)]
struct ExternalPlan {
    state: ExternalState,
    terminal_emitted: bool,
    source_terminal: Option<TerminalDisposition>,
    sink_terminal: Option<TerminalDisposition>,
}

#[derive(Debug)]
pub struct CompositeHost {
    advertisement: HostAdvertisement,
    source: HostRuntime,
    sink: HostRuntime,
    source_fragment: PlanFragment,
    sink_fragment: PlanFragment,
    internal_plan_id: PlanId,
    connection_id: ConnectionId,
    provider: InMemoryConnectionProvider,
    external_plans: BTreeMap<PlanId, ExternalPlan>,
    released_plans: BTreeSet<PlanId>,
    observations: Vec<Observation>,
    observation_limit: usize,
    fail_next_presentation: bool,
}

impl CompositeHost {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        host_id: HostId,
        boot_id: BootId,
        offer_generation: OfferGeneration,
        capability_id: CapabilityId,
        source: HostRuntime,
        sink: HostRuntime,
        internal_plan: Plan,
        observation_limit: usize,
    ) -> Result<Self, CompositeError> {
        let source_host_id = source.advertisement().host_id.clone();
        let sink_host_id = sink.advertisement().host_id.clone();
        if source_host_id == sink_host_id {
            return Err(CompositeError::InvalidInternalPlan(
                "child hosts must have distinct identities".to_string(),
            ));
        }
        if source.advertisement().boot_id == sink.advertisement().boot_id {
            return Err(CompositeError::InvalidInternalPlan(
                "child hosts must have distinct boot identities".to_string(),
            ));
        }
        let source_fragment = internal_plan
            .fragments
            .iter()
            .find(|fragment| fragment.host_id == source_host_id)
            .cloned()
            .ok_or_else(|| CompositeError::InvalidInternalPlan("missing source fragment".into()))?;
        let sink_fragment = internal_plan
            .fragments
            .iter()
            .find(|fragment| fragment.host_id == sink_host_id)
            .cloned()
            .ok_or_else(|| CompositeError::InvalidInternalPlan("missing sink fragment".into()))?;
        let connection = source_fragment
            .connections
            .iter()
            .find(|connection| connection.provider == ConnectionProvider::InMemory)
            .cloned()
            .ok_or_else(|| {
                CompositeError::InvalidInternalPlan("missing in-memory boundary".into())
            })?;
        if !sink_fragment
            .connections
            .iter()
            .any(|candidate| candidate == &connection)
        {
            return Err(CompositeError::InvalidInternalPlan(
                "child fragments do not share an exact boundary".into(),
            ));
        }
        let source_instances = source
            .advertisement()
            .capabilities
            .iter()
            .map(|offer| offer.limits.max_active_instances)
            .min()
            .unwrap_or(0);
        let sink_instances = sink
            .advertisement()
            .capabilities
            .iter()
            .map(|offer| offer.limits.max_active_instances)
            .min()
            .unwrap_or(0);
        let advertisement = HostAdvertisement {
            protocol_version: PROTOCOL_VERSION,
            host_id,
            boot_id,
            offer_generation,
            profile: HostProfileId::from("composite/in-memory-v1"),
            capabilities: vec![CapabilityOffer {
                capability_id,
                kind_id: kind_id(COMPOSITE_DEMONSTRATION_KIND),
                implementation_id: ImplementationId::from("composite/pulse-show-v1"),
                limits: CapabilityLimits {
                    value_kind: connection.value_kind.clone(),
                    max_active_instances: source_instances.min(sink_instances).min(1),
                    max_queue_items: connection.item_capacity,
                    max_queue_bytes: connection.byte_capacity,
                },
            }],
        };
        let internal_plan_id = internal_plan.plan_id.clone();
        let connection_id = connection.connection_id.clone();
        let provider = InMemoryConnectionProvider::new(internal_plan_id.clone(), &connection);
        let mut host = Self {
            advertisement,
            source,
            sink,
            source_fragment,
            sink_fragment,
            internal_plan_id,
            connection_id,
            provider,
            external_plans: BTreeMap::new(),
            released_plans: BTreeSet::new(),
            observations: Vec::new(),
            observation_limit,
            fail_next_presentation: false,
        };
        host.record(None, ObservationKind::HostStarted);
        host.record(None, ObservationKind::AdvertisementPublished);
        Ok(host)
    }

    pub fn advertisement(&self) -> &HostAdvertisement {
        &self.advertisement
    }

    pub fn fail_next_presentation(&mut self) {
        self.fail_next_presentation = true;
    }

    pub fn handle(&mut self, command: HostCommand) -> RuntimeOutput {
        match command {
            HostCommand::Prepare(fragment) => self.prepare(fragment),
            HostCommand::Activate(plan_id) => self.activate(plan_id),
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

    pub fn internal_observations(&mut self) -> (Vec<Observation>, Vec<Observation>) {
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
        (inspect(&mut self.source), inspect(&mut self.sink))
    }

    fn prepare(&mut self, fragment: PlanFragment) -> RuntimeOutput {
        let mut output = RuntimeOutput::default();
        let plan_id = fragment.plan_id.clone();
        if self.released_plans.contains(&plan_id)
            || fragment.host_id != self.advertisement.host_id
            || fragment.boot_id != self.advertisement.boot_id
            || fragment.offer_generation != self.advertisement.offer_generation
            || fragment.placements.len() != 1
            || fragment.placements[0].kind_id.as_str() != COMPOSITE_DEMONSTRATION_KIND
            || fragment.placements[0].capability_id
                != self.advertisement.capabilities[0].capability_id
            || self
                .external_plans
                .values()
                .any(|plan| matches!(plan.state, ExternalState::Prepared | ExternalState::Active))
        {
            output.events.push(HostEvent::PreparationRejected {
                plan_id,
                reason: "external fragment does not match the composite offer".into(),
            });
            return output;
        }
        let sink_prepare = self
            .sink
            .handle(HostCommand::Prepare(self.sink_fragment.clone()));
        let source_prepare = self
            .source
            .handle(HostCommand::Prepare(self.source_fragment.clone()));
        if let Some(reason) =
            preparation_failure(&sink_prepare).or_else(|| preparation_failure(&source_prepare))
        {
            output
                .events
                .push(HostEvent::PreparationRejected { plan_id, reason });
            return output;
        }
        self.external_plans.insert(
            plan_id.clone(),
            ExternalPlan {
                state: ExternalState::Prepared,
                terminal_emitted: false,
                source_terminal: None,
                sink_terminal: None,
            },
        );
        self.record(Some(plan_id.clone()), ObservationKind::PlanFragmentReceived);
        output.events.push(HostEvent::Prepared { plan_id });
        output
    }

    fn activate(&mut self, plan_id: PlanId) -> RuntimeOutput {
        let mut output = RuntimeOutput::default();
        let Some(plan) = self.external_plans.get_mut(&plan_id) else {
            output.events.push(HostEvent::ActivationRejected {
                plan_id,
                reason: "unknown external plan".into(),
            });
            return output;
        };
        if plan.state != ExternalState::Prepared {
            output.events.push(HostEvent::ActivationRejected {
                plan_id,
                reason: "external plan is not prepared".into(),
            });
            return output;
        }
        plan.state = ExternalState::Active;
        self.record(Some(plan_id.clone()), ObservationKind::PlanActivated);
        output.events.push(HostEvent::Activated {
            plan_id: plan_id.clone(),
        });
        let sink = self
            .sink
            .handle(HostCommand::Activate(self.internal_plan_id.clone()));
        let source = self
            .source
            .handle(HostCommand::Activate(self.internal_plan_id.clone()));
        self.drive_internal(
            &plan_id,
            vec![(Child::Sink, sink), (Child::Source, source)],
            &mut output,
        );
        output
    }

    fn drive_internal(
        &mut self,
        external_plan_id: &PlanId,
        initial: Vec<(Child, RuntimeOutput)>,
        external: &mut RuntimeOutput,
    ) {
        let mut pending = VecDeque::from(initial);
        while let Some((child, output)) = pending.pop_front() {
            for event in output.events {
                match event {
                    HostEvent::ConnectionTerminated {
                        connection_id,
                        disposition:
                            conduit_core::ConnectionTerminalDisposition {
                                disposition: TerminalDisposition::Completed,
                                ..
                            },
                        ..
                    } if child == Child::Source && connection_id == self.connection_id => {
                        let closed = self.sink.handle(HostCommand::CloseConnection {
                            plan_id: self.internal_plan_id.clone(),
                            connection_id,
                        });
                        pending.push_back((Child::Sink, closed));
                    }
                    HostEvent::PlanTerminated { disposition, .. } => {
                        if let Some(plan) = self.external_plans.get_mut(external_plan_id) {
                            match child {
                                Child::Source => plan.source_terminal = Some(disposition),
                                Child::Sink => plan.sink_terminal = Some(disposition),
                            }
                        }
                        if matches!(disposition, TerminalDisposition::Failed { .. }) {
                            let other = match child {
                                Child::Source => Child::Sink,
                                Child::Sink => Child::Source,
                            };
                            let cancellation = match other {
                                Child::Source => self
                                    .source
                                    .handle(HostCommand::Cancel(self.internal_plan_id.clone())),
                                Child::Sink => self
                                    .sink
                                    .handle(HostCommand::Cancel(self.internal_plan_id.clone())),
                            };
                            pending.push_back((other, cancellation));
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
                        let next = match child {
                            Child::Source => self.source.handle(HostCommand::CompleteWait {
                                plan_id,
                                placement_id,
                            }),
                            Child::Sink => self.sink.handle(HostCommand::CompleteWait {
                                plan_id,
                                placement_id,
                            }),
                        };
                        pending.push_back((child, next));
                    }
                    PlatformEffect::PresentValue {
                        plan_id,
                        placement_id,
                        value,
                    } => {
                        let success = !self.fail_next_presentation;
                        self.fail_next_presentation = false;
                        let next = self.sink.handle(HostCommand::CompletePresentation {
                            plan_id,
                            placement_id,
                            value,
                            success,
                            message: (!success).then(|| "injected child sink failure".into()),
                        });
                        pending.push_back((Child::Sink, next));
                    }
                    PlatformEffect::TransmitConnection { envelope } => {
                        let sequence = envelope.sequence;
                        let outcome = self.provider.accept(envelope);
                        if outcome == ConnectionOutcome::Accepted {
                            let accepted =
                                self.source.handle(HostCommand::CompleteConnectionDelivery {
                                    plan_id: self.internal_plan_id.clone(),
                                    connection_id: self.connection_id.clone(),
                                    sequence,
                                    outcome: ConnectionOutcome::Accepted,
                                });
                            pending.push_back((Child::Source, accepted));
                            let (delivery_outcome, delivered) = self
                                .provider
                                .deliver()
                                .expect("accepted envelope must be queued");
                            debug_assert_eq!(delivery_outcome, ConnectionOutcome::Delivered);
                            let sink_output = self
                                .sink
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
                            pending.push_back((Child::Sink, sink_output));
                            let source_output =
                                self.source.handle(HostCommand::CompleteConnectionDelivery {
                                    plan_id: self.internal_plan_id.clone(),
                                    connection_id: self.connection_id.clone(),
                                    sequence,
                                    outcome: if sink_accepted {
                                        ConnectionOutcome::Delivered
                                    } else {
                                        ConnectionOutcome::Malformed
                                    },
                                });
                            pending.push_back((Child::Source, source_output));
                        } else {
                            let source_output =
                                self.source.handle(HostCommand::CompleteConnectionDelivery {
                                    plan_id: self.internal_plan_id.clone(),
                                    connection_id: self.connection_id.clone(),
                                    sequence,
                                    outcome,
                                });
                            pending.push_back((Child::Source, source_output));
                        }
                    }
                }
            }
        }
        self.finish_external_if_terminal(external_plan_id, external);
    }

    fn finish_external_if_terminal(&mut self, plan_id: &PlanId, output: &mut RuntimeOutput) {
        let Some(plan) = self.external_plans.get_mut(plan_id) else {
            return;
        };
        if plan.terminal_emitted || plan.source_terminal.is_none() || plan.sink_terminal.is_none() {
            return;
        }
        let failed = matches!(
            plan.source_terminal,
            Some(TerminalDisposition::Failed { .. })
        ) || matches!(plan.sink_terminal, Some(TerminalDisposition::Failed { .. }));
        let cancelled = matches!(
            plan.source_terminal,
            Some(TerminalDisposition::Cancelled { .. })
        ) || matches!(
            plan.sink_terminal,
            Some(TerminalDisposition::Cancelled { .. })
        );
        let disposition = if failed {
            plan.state = ExternalState::Failed;
            TerminalDisposition::Failed {
                reason: FailureReason::CompositeCapabilityFailed,
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
        let source = self
            .source
            .handle(HostCommand::Cancel(self.internal_plan_id.clone()));
        let sink = self
            .sink
            .handle(HostCommand::Cancel(self.internal_plan_id.clone()));
        self.drive_internal(
            &plan_id,
            vec![(Child::Source, source), (Child::Sink, sink)],
            &mut output,
        );
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
        let _ = self
            .source
            .handle(HostCommand::Release(self.internal_plan_id.clone()));
        let _ = self
            .sink
            .handle(HostCommand::Release(self.internal_plan_id.clone()));
        self.record(Some(plan_id.clone()), ObservationKind::Released);
        output.events.push(HostEvent::Released { plan_id });
        output
    }

    fn record(&mut self, plan_id: Option<PlanId>, kind: ObservationKind) {
        if self.observation_limit == 0 {
            return;
        }
        if self.observations.len() == self.observation_limit {
            let mut dropped = 1;
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
        }
        self.observations.push(Observation {
            host_id: self.advertisement.host_id.clone(),
            boot_id: self.advertisement.boot_id.clone(),
            plan_id,
            placement_id: None,
            connection_id: None,
            kind,
        });
    }
}

fn preparation_failure(output: &RuntimeOutput) -> Option<String> {
    output.events.iter().find_map(|event| match event {
        HostEvent::PreparationRejected { reason, .. } => Some(reason.clone()),
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::{CompositeHost, InMemoryConnectionProvider, COMPOSITE_DEMONSTRATION_KIND};
    use conduit_core::{
        kind_id, port_id, BootId, CapabilityId, CapabilityLimits, CapabilityOffer,
        ConnectionEnvelope, ConnectionOutcome, ConnectionProvider, FormId, HostAdvertisement,
        HostCommand, HostEvent, HostId, HostProfileId, ImplementationId, KindId, OfferGeneration,
        OperationId, PortDescriptor, PortDirection, TerminalDisposition, PROTOCOL_VERSION,
    };
    use conduit_form::{parse, CheckedForm, CheckedOperation};
    use conduit_planner::{plan, PlacementChoice, PlacementChoices};
    use conduit_runtime::HostRuntime;
    use conduit_signal::{PULSE_KIND, SHOW_KIND, SIGNAL_VALUE_KIND};
    use std::collections::BTreeMap;

    fn child_advertisement(host: &str, boot: &str, source: bool) -> HostAdvertisement {
        HostAdvertisement {
            protocol_version: PROTOCOL_VERSION,
            host_id: HostId::from(host),
            boot_id: BootId::from(boot),
            offer_generation: OfferGeneration(1),
            profile: HostProfileId::from("rust-std"),
            capabilities: vec![CapabilityOffer {
                capability_id: CapabilityId::from(if source { "pulse" } else { "show" }),
                kind_id: kind_id(if source { PULSE_KIND } else { SHOW_KIND }),
                implementation_id: ImplementationId::from(if source {
                    "test/pulse-v1"
                } else {
                    "test/show-v1"
                }),
                limits: CapabilityLimits {
                    value_kind: kind_id(SIGNAL_VALUE_KIND),
                    max_active_instances: 2,
                    max_queue_items: 8,
                    max_queue_bytes: 128,
                },
            }],
        }
    }

    fn internal_plan(item_capacity: u16, byte_capacity: u32) -> conduit_core::Plan {
        let form = parse(
            "form 0\n\ninternal {\n pulse: flow/pulse\n show: presentation/show\n pulse.count = 3\n pulse.period-ms = 0\n pulse.initial = false\n pulse > show\n}\n",
        )
        .expect("internal form parses");
        let source = child_advertisement("child-source", "source-boot", true);
        let sink = child_advertisement("child-sink", "sink-boot", false);
        let placements = PlacementChoices {
            by_operation: BTreeMap::from([
                (
                    OperationId::from("pulse"),
                    PlacementChoice {
                        host_id: source.host_id.clone(),
                        capability_id: CapabilityId::from("pulse"),
                    },
                ),
                (
                    OperationId::from("show"),
                    PlacementChoice {
                        host_id: sink.host_id.clone(),
                        capability_id: CapabilityId::from("show"),
                    },
                ),
            ]),
        };
        let mut plan = plan(
            &form,
            &[source, sink],
            &placements,
            &[ConnectionProvider::Local, ConnectionProvider::InMemory],
        )
        .expect("cross-host plan succeeds");
        for fragment in &mut plan.fragments {
            for connection in &mut fragment.connections {
                connection.item_capacity = item_capacity;
                connection.byte_capacity = byte_capacity;
            }
        }
        plan
    }

    fn composite(item_capacity: u16, byte_capacity: u32) -> CompositeHost {
        let source_ad = child_advertisement("child-source", "source-boot", true);
        let sink_ad = child_advertisement("child-sink", "sink-boot", false);
        CompositeHost::new(
            HostId::from("composite-host"),
            BootId::from("composite-boot"),
            OfferGeneration(7),
            CapabilityId::from("run-signal"),
            HostRuntime::new(source_ad, 128),
            HostRuntime::new(sink_ad, 128),
            internal_plan(item_capacity, byte_capacity),
            64,
        )
        .expect("composite is valid")
    }

    fn parent_fragment(composite: &CompositeHost) -> conduit_core::PlanFragment {
        let operation = CheckedOperation {
            operation_id: OperationId::from("demonstration"),
            kind_id: KindId::from(COMPOSITE_DEMONSTRATION_KIND),
            inputs: vec![PortDescriptor {
                port_id: port_id("signal"),
                value_kind: kind_id(SIGNAL_VALUE_KIND),
                direction: PortDirection::Input,
            }],
            outputs: Vec::new(),
            configuration: Vec::new(),
        };
        let form = CheckedForm {
            form_id: FormId::from("parent-form"),
            name: "parent".into(),
            operations: vec![operation],
            connections: Vec::new(),
        };
        let ordinary = child_advertisement("ordinary-host", "ordinary-boot", true);
        let placements = PlacementChoices {
            by_operation: BTreeMap::from([(
                OperationId::from("demonstration"),
                PlacementChoice {
                    host_id: composite.advertisement().host_id.clone(),
                    capability_id: CapabilityId::from("run-signal"),
                },
            )]),
        };
        plan(
            &form,
            &[composite.advertisement().clone(), ordinary],
            &placements,
            &[ConnectionProvider::Local, ConnectionProvider::InMemory],
        )
        .expect("parent planner treats composite as one host")
        .fragments
        .into_iter()
        .find(|fragment| fragment.host_id == composite.advertisement().host_id)
        .expect("composite fragment exists")
    }

    #[test]
    fn provider_enforces_identity_order_and_bounds() {
        let plan = internal_plan(1, 9);
        let connection = &plan.fragments[0].connections[0];
        let mut provider = InMemoryConnectionProvider::new(plan.plan_id.clone(), connection);
        let envelope = ConnectionEnvelope {
            protocol_version: PROTOCOL_VERSION,
            plan_id: plan.plan_id.clone(),
            connection_id: connection.connection_id.clone(),
            sequence: 0,
            value_kind: connection.value_kind.clone(),
            payload: vec![0; 9],
        };
        assert_eq!(provider.status(), ConnectionOutcome::Ready);
        assert_eq!(
            provider.accept(envelope.clone()),
            ConnectionOutcome::Accepted
        );
        assert_eq!(provider.queued_items(), 1);
        assert_eq!(provider.queued_bytes(), 9);
        assert_eq!(provider.status(), ConnectionOutcome::Full);
        let mut next = envelope.clone();
        next.sequence = 1;
        assert_eq!(provider.accept(next), ConnectionOutcome::Full);
        let mut stale = envelope.clone();
        stale.plan_id = conduit_core::PlanId::from("stale");
        stale.sequence = 1;
        assert_eq!(provider.accept(stale), ConnectionOutcome::Malformed);
        assert!(matches!(
            provider.deliver(),
            Some((ConnectionOutcome::Delivered, _))
        ));
        assert_eq!(provider.queued_bytes(), 0);
        assert_eq!(
            provider.accept(envelope.clone()),
            ConnectionOutcome::Malformed
        );
        let mut out_of_order = envelope.clone();
        out_of_order.sequence = 2;
        assert_eq!(provider.accept(out_of_order), ConnectionOutcome::Malformed);
        let oversized = ConnectionEnvelope {
            protocol_version: PROTOCOL_VERSION,
            plan_id: plan.plan_id.clone(),
            connection_id: connection.connection_id.clone(),
            sequence: 1,
            value_kind: connection.value_kind.clone(),
            payload: vec![0; 10],
        };
        assert_eq!(provider.accept(oversized), ConnectionOutcome::Malformed);
        assert_eq!(provider.queued_items(), 0);
        assert_eq!(provider.disconnect(), ConnectionOutcome::Disconnected);
        assert_eq!(provider.status(), ConnectionOutcome::Terminal);
    }

    #[test]
    fn two_child_hosts_compose_and_parent_sees_one_host() {
        let mut composite = composite(4, 64);
        let fragment = parent_fragment(&composite);
        let plan_id = fragment.plan_id.clone();
        assert!(matches!(
            composite
                .handle(HostCommand::Prepare(fragment))
                .events
                .first(),
            Some(HostEvent::Prepared { .. })
        ));
        let output = composite.handle(HostCommand::Activate(plan_id));
        assert!(output.events.iter().any(|event| matches!(
            event,
            HostEvent::PlanTerminated {
                disposition: TerminalDisposition::Completed,
                ..
            }
        )));
        let (source, sink) = composite.internal_observations();
        assert!(source
            .iter()
            .all(|item| item.host_id.as_str() == "child-source"));
        assert!(sink
            .iter()
            .all(|item| item.host_id.as_str() == "child-sink"));
        assert_eq!(
            sink.iter()
                .filter(|item| matches!(
                    item.kind,
                    conduit_core::ObservationKind::ValuePresented { .. }
                ))
                .count(),
            3
        );
    }

    #[test]
    fn child_failure_is_translated_without_topology_leakage() {
        let mut composite = composite(4, 64);
        composite.fail_next_presentation();
        let fragment = parent_fragment(&composite);
        let output = composite.handle(HostCommand::Prepare(fragment.clone()));
        assert!(matches!(
            output.events.first(),
            Some(HostEvent::Prepared { .. })
        ));
        let output = composite.handle(HostCommand::Activate(fragment.plan_id));
        let failures = output
            .events
            .iter()
            .filter(|event| {
                matches!(
                    event,
                    HostEvent::PlanTerminated {
                        disposition: TerminalDisposition::Failed {
                            reason: conduit_core::FailureReason::CompositeCapabilityFailed
                        },
                        ..
                    }
                )
            })
            .count();
        assert_eq!(failures, 1);
        assert!(!format!("{output:?}").contains("child-sink"));
        let (_, sink) = composite.internal_observations();
        assert!(sink
            .iter()
            .any(|item| matches!(item.kind, conduit_core::ObservationKind::Failure { .. })));
    }

    #[test]
    fn external_limits_are_conservatively_derived() {
        let narrow = composite(2, 18);
        let wide = composite(4, 64);
        let narrow_limits = &narrow.advertisement().capabilities[0].limits;
        let wide_limits = &wide.advertisement().capabilities[0].limits;
        assert_eq!(narrow_limits.max_active_instances, 1);
        assert_eq!(narrow_limits.max_queue_items, 2);
        assert_eq!(narrow_limits.max_queue_bytes, 18);
        assert_eq!(wide_limits.max_queue_items, 4);
        assert_eq!(wide_limits.max_queue_bytes, 64);
    }
}
