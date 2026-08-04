use conduit_core::{
    BoundedQueue, ConnectionId, HostAdvertisement, HostCommand, HostEvent, Observation,
    ObservationKind, OperationConfiguration, PlacementId, PlacementLifecycleState, PlanFragment,
    PlanId, PlannedConnection, PlannedOperation, PlatformEffect, Signal,
};
use std::collections::BTreeMap;

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
}

#[derive(Debug)]
struct RuntimePlan {
    fragment: PlanFragment,
    placements: BTreeMap<PlacementId, RuntimePlacement>,
    connections: BTreeMap<ConnectionId, RuntimeConnection>,
    state: PlanState,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
enum PlanState {
    Prepared,
    Active,
    Failed,
    Cancelled,
    Released,
    Completed,
}

#[derive(Debug)]
struct RuntimePlacement {
    spec: PlannedOperation,
    lifecycle: PlacementLifecycleState,
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
    pending: Option<Signal>,
}

#[derive(Debug)]
struct RuntimeConnection {
    spec: PlannedConnection,
    queue: BoundedQueue<Signal>,
    source_done: bool,
    sink_failed: bool,
    blocked: bool,
}

impl HostRuntime {
    pub fn new(advertisement: HostAdvertisement, observation_limit: usize) -> Self {
        let mut runtime = Self {
            advertisement,
            observation_limit,
            observations: Vec::new(),
            plans: BTreeMap::new(),
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
                signal,
                success,
                message,
            } => self.complete_presentation(&plan_id, &placement_id, signal, success, message),
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
                let pulse = match &spec.configuration {
                    OperationConfiguration::Pulse(_) => Some(PulseState {
                        next_sequence: 0,
                        waiting: false,
                        emission_complete: false,
                    }),
                    OperationConfiguration::Show => None,
                };
                let show = match &spec.configuration {
                    OperationConfiguration::Show => Some(ShowState { pending: None }),
                    OperationConfiguration::Pulse(_) => None,
                };
                (
                    spec.placement_id.clone(),
                    RuntimePlacement {
                        spec,
                        lifecycle: PlacementLifecycleState::Prepared,
                        pulse,
                        show,
                    },
                )
            })
            .collect::<BTreeMap<_, _>>();

        let mut connections = BTreeMap::new();
        for connection in &fragment.connections {
            let source_capability = self
                .advertisement
                .capabilities
                .iter()
                .find(|offer| {
                    offer.capability_id
                        == placements[&connection.source_placement_id]
                            .spec
                            .capability_id
                })
                .expect("source capability must exist");
            let sink_capability = self
                .advertisement
                .capabilities
                .iter()
                .find(|offer| {
                    offer.capability_id
                        == placements[&connection.sink_placement_id].spec.capability_id
                })
                .expect("sink capability must exist");
            if connection.item_capacity > source_capability.limits.max_queue_items
                || connection.item_capacity > sink_capability.limits.max_queue_items
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
            connections.insert(
                connection.connection_id.clone(),
                RuntimeConnection {
                    spec: connection.clone(),
                    queue: BoundedQueue::new(connection.item_capacity as usize),
                    source_done: false,
                    sink_failed: false,
                    blocked: false,
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
            },
        );
        output.events.push(HostEvent::Prepared {
            plan_id: fragment.plan_id,
        });
        output
    }

    fn activate(&mut self, plan_id: &PlanId) -> RuntimeOutput {
        let Some(plan) = self.plans.get_mut(plan_id) else {
            return RuntimeOutput {
                events: vec![HostEvent::ActivationRejected {
                    plan_id: plan_id.clone(),
                    reason: "plan was not prepared".to_string(),
                }],
                effects: Vec::new(),
            };
        };
        if plan.state != PlanState::Prepared {
            return RuntimeOutput {
                events: vec![HostEvent::ActivationRejected {
                    plan_id: plan_id.clone(),
                    reason: "plan is not in prepared state".to_string(),
                }],
                effects: Vec::new(),
            };
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

        let mut output = RuntimeOutput {
            events: vec![HostEvent::Activated {
                plan_id: plan_id.clone(),
            }],
            effects: Vec::new(),
        };
        self.pump(plan_id, &mut output);
        output
    }

    fn complete_wait(&mut self, plan_id: &PlanId, placement_id: &PlacementId) -> RuntimeOutput {
        let mut output = RuntimeOutput::default();
        if let Some(plan) = self.plans.get_mut(plan_id) {
            if let Some(placement) = plan.placements.get_mut(placement_id) {
                if let Some(pulse) = placement.pulse.as_mut() {
                    pulse.waiting = false;
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
        signal: Signal,
        success: bool,
        message: Option<String>,
    ) -> RuntimeOutput {
        let mut output = RuntimeOutput::default();
        let mut pending_observations = Vec::new();
        if let Some(plan) = self.plans.get_mut(plan_id) {
            if let Some(placement) = plan.placements.get_mut(placement_id) {
                if let Some(show) = placement.show.as_mut() {
                    show.pending = None;
                    if success {
                        pending_observations.push((
                            Some(plan_id.clone()),
                            Some(placement_id.clone()),
                            None,
                            ObservationKind::SignalPresented {
                                signal: signal.clone(),
                            },
                        ));
                        output.events.push(HostEvent::ManifestationCompleted {
                            plan_id: plan_id.clone(),
                            placement_id: placement_id.clone(),
                            signal,
                        });
                    } else {
                        placement.lifecycle = PlacementLifecycleState::Failed;
                        plan.state = PlanState::Failed;
                        for connection in plan.connections.values_mut() {
                            if connection.spec.sink_placement_id == *placement_id {
                                connection.sink_failed = true;
                            }
                        }
                        let reason = message.unwrap_or_else(|| "presentation failed".to_string());
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
                            signal,
                            reason,
                        });
                    }
                }
            }
        }
        for (plan_id, placement_id, connection_id, kind) in pending_observations {
            self.record_observation(plan_id, placement_id, connection_id, kind);
        }
        self.pump(plan_id, &mut output);
        output
    }

    fn cancel(&mut self, plan_id: &PlanId) -> RuntimeOutput {
        if let Some(plan) = self.plans.get_mut(plan_id) {
            plan.state = PlanState::Cancelled;
            for placement in plan.placements.values_mut() {
                placement.lifecycle = PlacementLifecycleState::Cancelled;
            }
            self.record_observation(
                Some(plan_id.clone()),
                None,
                None,
                ObservationKind::Cancelled,
            );
            return RuntimeOutput {
                events: vec![HostEvent::Cancelled {
                    plan_id: plan_id.clone(),
                }],
                effects: Vec::new(),
            };
        }
        RuntimeOutput::default()
    }

    fn release(&mut self, plan_id: &PlanId) -> RuntimeOutput {
        if let Some(mut plan) = self.plans.remove(plan_id) {
            plan.state = PlanState::Released;
            for placement in plan.placements.values_mut() {
                placement.lifecycle = PlacementLifecycleState::Released;
            }
            self.record_observation(Some(plan_id.clone()), None, None, ObservationKind::Released);
            return RuntimeOutput {
                events: vec![HostEvent::Released {
                    plan_id: plan_id.clone(),
                }],
                effects: Vec::new(),
            };
        }
        RuntimeOutput::default()
    }

    fn pump(&mut self, plan_id: &PlanId, output: &mut RuntimeOutput) {
        loop {
            let mut changed = false;
            let mut pending_observations = Vec::new();
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
                    let OperationConfiguration::Pulse(config) = &placement.spec.configuration
                    else {
                        continue;
                    };
                    if pulse.emission_complete || pulse.waiting {
                        continue;
                    }
                    if pulse.next_sequence >= config.count {
                        pulse.emission_complete = true;
                        mark_source_done(&placement.spec.placement_id, &mut plan.connections);
                        placement.lifecycle = PlacementLifecycleState::Completed;
                        output.events.push(HostEvent::PlacementCompleted {
                            plan_id: plan_id.clone(),
                            placement_id: placement.spec.placement_id.clone(),
                        });
                        pending_observations.push((
                            Some(plan_id.clone()),
                            Some(placement.spec.placement_id.clone()),
                            None,
                            ObservationKind::PlacementCompleted,
                        ));
                        changed = true;
                        continue;
                    }

                    let signal = Signal {
                        sequence: pulse.next_sequence,
                        level: if pulse.next_sequence % 2 == 0 {
                            config.initial_level
                        } else {
                            !config.initial_level
                        },
                    };
                    let outgoing =
                        outgoing_connections(&placement.spec.placement_id, &plan.connections);
                    let mut blocked = None;
                    for connection_id in &outgoing {
                        let connection = &plan.connections[connection_id];
                        if connection.sink_failed {
                            continue;
                        }
                        if connection.queue.len() >= connection.queue.capacity() {
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
                        if connection.sink_failed {
                            continue;
                        }
                        connection.blocked = false;
                        connection
                            .queue
                            .push(signal.clone())
                            .expect("capacity was checked before push");
                        output.events.push(HostEvent::SignalDelivered {
                            plan_id: plan_id.clone(),
                            connection_id: connection_id.clone(),
                            signal: signal.clone(),
                        });
                        pending_observations.push((
                            Some(plan_id.clone()),
                            Some(placement.spec.placement_id.clone()),
                            Some(connection_id),
                            ObservationKind::SignalProduced {
                                signal: signal.clone(),
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
                if connection.queue.is_empty() || connection.sink_failed {
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
                let signal = connection
                    .queue
                    .pop()
                    .expect("queue was checked before pop");
                show.pending = Some(signal.clone());
                output.events.push(HostEvent::PresentSignalRequested {
                    plan_id: plan_id.clone(),
                    placement_id: sink_id.clone(),
                    signal: signal.clone(),
                });
                output.effects.push(PlatformEffect::PresentSignal {
                    plan_id: plan_id.clone(),
                    placement_id: sink_id.clone(),
                    signal: signal.clone(),
                });
                pending_observations.push((
                    Some(plan_id.clone()),
                    Some(sink_id),
                    Some(connection_id),
                    ObservationKind::SignalAccepted { signal },
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
                    (connection.source_done || connection.sink_failed)
                        && connection.queue.is_empty()
                });
                if done {
                    sink.lifecycle = PlacementLifecycleState::Completed;
                    output.events.push(HostEvent::PlacementCompleted {
                        plan_id: plan_id.clone(),
                        placement_id: sink_id.clone(),
                    });
                    pending_observations.push((
                        Some(plan_id.clone()),
                        Some(sink_id),
                        None,
                        ObservationKind::PlacementCompleted,
                    ));
                    changed = true;
                }
            }

            let plan_completed = plan.state == PlanState::Active
                && plan
                    .placements
                    .values()
                    .all(|placement| placement.lifecycle == PlacementLifecycleState::Completed);

            let _ = plan;

            for (plan_id, placement_id, connection_id, kind) in pending_observations {
                self.record_observation(plan_id, placement_id, connection_id, kind);
            }

            if plan_completed {
                if let Some(plan) = self.plans.get_mut(plan_id) {
                    plan.state = PlanState::Completed;
                }
                output.events.push(HostEvent::PlanCompleted {
                    plan_id: plan_id.clone(),
                });
                self.record_observation(
                    Some(plan_id.clone()),
                    None,
                    None,
                    ObservationKind::PlanCompleted,
                );
                return;
            }

            if !changed {
                return;
            }
        }
    }

    fn record_observation(
        &mut self,
        plan_id: Option<PlanId>,
        placement_id: Option<PlacementId>,
        connection_id: Option<ConnectionId>,
        kind: ObservationKind,
    ) {
        self.observations.push(Observation {
            host_id: self.advertisement.host_id.clone(),
            boot_id: self.advertisement.boot_id.clone(),
            plan_id,
            placement_id,
            connection_id,
            kind,
        });
        if self.observations.len() > self.observation_limit {
            let drop_count = self.observations.len() - self.observation_limit;
            self.observations.drain(0..drop_count);
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
mod tests {
    use super::HostRuntime;
    use conduit_core::{
        kind_id, BootId, CapabilityId, CapabilityLimits, CapabilityOffer, ConnectionProvider,
        HostAdvertisement, HostCommand, HostEvent, HostId, HostProfileId, ImplementationId,
        ObservationKind, OfferGeneration, PlatformEffect, PROTOCOL_VERSION, PULSE_KIND, SHOW_KIND,
        SIGNAL_VALUE_KIND,
    };
    use conduit_form::parse;
    use conduit_planner::{default_placements, plan};

    fn advertisement(boot: &str, offer_generation: u64, queue_items: u16) -> HostAdvertisement {
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
                        max_queue_bytes: 64,
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
                        max_queue_bytes: 64,
                    },
                },
            ],
        }
    }

    fn demo_fragment(count: u64) -> conduit_core::PlanFragment {
        let form = parse(&format!(
            "form 0\n\ndemo {{\n    pulse: flow/pulse\n    show: presentation/show\n\n    pulse.count = {count}\n    pulse.period-ms = 0\n    pulse.initial = false\n\n    pulse > show\n}}\n"
        ))
        .expect("form should parse");
        let advertisement = advertisement("boot-1", 1, 4);
        let placements = default_placements(&form, std::slice::from_ref(&advertisement))
            .expect("placements work");
        let plan = plan(
            &form,
            std::slice::from_ref(&advertisement),
            &placements,
            &[ConnectionProvider::Local],
        )
        .expect("plan should succeed");
        plan.fragments.into_iter().next().expect("fragment exists")
    }

    #[test]
    fn preparation_rejects_stale_boot() {
        let fragment = demo_fragment(2);
        let mut runtime = HostRuntime::new(advertisement("boot-2", 1, 4), 128);
        let output = runtime.handle(HostCommand::Prepare(fragment));
        assert!(matches!(
            output.events.first(),
            Some(HostEvent::PreparationRejected { .. })
        ));
        assert!(output.effects.is_empty());
    }

    #[test]
    fn preparation_rejects_stale_offer_generation() {
        let fragment = demo_fragment(2);
        let mut runtime = HostRuntime::new(advertisement("boot-1", 2, 4), 128);
        let output = runtime.handle(HostCommand::Prepare(fragment));
        assert!(matches!(
            output.events.first(),
            Some(HostEvent::PreparationRejected { .. })
        ));
    }

    #[test]
    fn preparation_emits_no_effects() {
        let fragment = demo_fragment(2);
        let mut runtime = HostRuntime::new(advertisement("boot-1", 1, 4), 128);
        let output = runtime.handle(HostCommand::Prepare(fragment));
        assert!(matches!(
            output.events.first(),
            Some(HostEvent::Prepared { .. })
        ));
        assert!(output.effects.is_empty());
    }

    #[test]
    fn activation_requires_prepared_plan() {
        let mut runtime = HostRuntime::new(advertisement("boot-1", 1, 4), 128);
        let output = runtime.handle(HostCommand::Activate(conduit_core::PlanId::from("missing")));
        assert!(matches!(
            output.events.first(),
            Some(HostEvent::ActivationRejected { .. })
        ));
    }

    #[test]
    fn full_queue_applies_backpressure() {
        let mut fragment = demo_fragment(3);
        fragment.connections[0].item_capacity = 1;
        let mut runtime = HostRuntime::new(advertisement("boot-1", 1, 1), 128);
        let prepared = runtime.handle(HostCommand::Prepare(fragment.clone()));
        assert!(matches!(
            prepared.events.first(),
            Some(HostEvent::Prepared { .. })
        ));
        let output = runtime.handle(HostCommand::Activate(fragment.plan_id.clone()));
        assert!(output
            .events
            .iter()
            .any(|event| matches!(event, HostEvent::ConnectionBlocked { .. })));
        assert!(output
            .effects
            .iter()
            .any(|effect| matches!(effect, PlatformEffect::PresentSignal { .. })));
    }

    #[test]
    fn duplicate_activation_is_rejected() {
        let fragment = demo_fragment(2);
        let mut runtime = HostRuntime::new(advertisement("boot-1", 1, 4), 128);
        runtime.handle(HostCommand::Prepare(fragment.clone()));
        let first = runtime.handle(HostCommand::Activate(fragment.plan_id.clone()));
        assert!(matches!(
            first.events.first(),
            Some(HostEvent::Activated { .. })
        ));
        let second = runtime.handle(HostCommand::Activate(fragment.plan_id.clone()));
        assert!(matches!(
            second.events.first(),
            Some(HostEvent::ActivationRejected { .. })
        ));
    }

    #[test]
    fn plan_executes_after_form_is_discarded() {
        let fragment = demo_fragment(3);
        let plan_id = fragment.plan_id.clone();
        let mut runtime = HostRuntime::new(advertisement("boot-1", 1, 4), 128);
        runtime.handle(HostCommand::Prepare(fragment));
        let mut output = runtime.handle(HostCommand::Activate(plan_id.clone()));
        let mut presented = Vec::new();
        while let Some(effect) = output.effects.pop() {
            output = match effect {
                PlatformEffect::Wait {
                    plan_id,
                    placement_id,
                    ..
                } => runtime.handle(HostCommand::CompleteWait {
                    plan_id,
                    placement_id,
                }),
                PlatformEffect::PresentSignal {
                    plan_id,
                    placement_id,
                    signal,
                } => {
                    presented.push(signal.clone());
                    runtime.handle(HostCommand::CompletePresentation {
                        plan_id,
                        placement_id,
                        signal,
                        success: true,
                        message: None,
                    })
                }
            };
        }

        let inspected = runtime.handle(HostCommand::Inspect);
        let observations = inspected
            .events
            .into_iter()
            .find_map(|event| match event {
                HostEvent::Observations { items } => Some(items),
                _ => None,
            })
            .expect("observations must exist");
        assert_eq!(presented.len(), 3);
        assert!(observations
            .iter()
            .any(|item| item.plan_id.as_ref() == Some(&plan_id)));
        assert!(observations
            .iter()
            .any(|item| matches!(item.kind, ObservationKind::PlanCompleted)));
    }
}
