use conduit_core::{
    kind_id, verify_plan, ArtifactId, BootId, CapabilityLimits, CapabilityOffer,
    ConnectionEnvelope, ConnectionId, ConnectionOutcome, ConnectionProvider, ExecutionProfileId,
    FailureReason, HostAdvertisement, HostCommand, HostEvent, HostId, HostProfileId,
    ImplementationId, Observation, ObservationKind, OfferGeneration, Plan, PlanFragment, PlanId,
    PlatformEffect, TerminalDisposition, PROTOCOL_VERSION,
};
use conduit_form::CheckedForm;
use conduit_runtime::{
    providers::in_memory::InMemoryConnectionProvider, HostRuntime, RuntimeOutput,
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
    pub source_child: HostId,
    pub sink_child: HostId,
    pub connection_id: ConnectionId,
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
        let placement_for = |operation_id: &conduit_core::OperationId| {
            internal_plan
                .fragments
                .iter()
                .flat_map(|fragment| &fragment.placements)
                .find(|placement| &placement.operation_id == operation_id)
        };
        let source = placement_for(&boundary.source_operation_id).ok_or_else(|| {
            CompositeError::InvalidInternalPlan("export source is absent from plan".into())
        })?;
        let sink = placement_for(&boundary.sink_operation_id).ok_or_else(|| {
            CompositeError::InvalidInternalPlan("export sink is absent from plan".into())
        })?;
        let connection = internal_plan
            .fragments
            .iter()
            .flat_map(|fragment| &fragment.connections)
            .find(|connection| {
                connection.source_placement_id == source.placement_id
                    && connection.source_port_id == boundary.source_port_id
                    && connection.sink_placement_id == sink.placement_id
                    && connection.sink_port_id == boundary.sink_port_id
                    && connection.value_kind == boundary.value_kind
                    && connection.provider == ConnectionProvider::InMemory
            })
            .cloned()
            .ok_or_else(|| {
                CompositeError::InvalidInternalPlan(
                    "authored export does not resolve to an exact in-memory plan boundary".into(),
                )
            })?;
        let source_child = source.host_id.clone();
        let sink_child = sink.host_id.clone();
        let execution_profile_id =
            ExecutionProfileId::from(format!("composite:{}@1", implementation_id.as_str()));
        Ok(Self {
            host_id,
            boot_id,
            offer_generation,
            profile,
            external_capability: CapabilityOffer {
                capability_id: boundary.capability_id,
                kind_id: boundary.kind_id,
                kind_contract_revision: boundary.kind_contract_revision,
                execution_profile_id,
                implementation_id,
                artifact_id,
                inputs: boundary.inputs,
                outputs: boundary.outputs,
                host_operations: Vec::new(),
                resource_requirements: Vec::new(),
                authority_requirements: Vec::new(),
                limits: CapabilityLimits {
                    max_active_instances: 1,
                    max_queue_items: connection.item_capacity,
                    max_queue_bytes: connection.byte_capacity,
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
                source_child,
                sink_child,
                connection_id: connection.connection_id,
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
}

#[derive(Debug)]
struct ChildRuntime {
    runtime: HostRuntime,
    fragment: PlanFragment,
}

#[derive(Debug)]
pub struct CompositeHost {
    advertisement: HostAdvertisement,
    children: BTreeMap<HostId, ChildRuntime>,
    boundary: CompositeBoundary,
    internal_plan_id: PlanId,
    connection_id: ConnectionId,
    provider: InMemoryConnectionProvider,
    external_plans: BTreeMap<PlanId, ExternalPlan>,
    released_plans: BTreeSet<PlanId>,
    observations: Vec<Observation>,
    internal_events: Vec<(HostId, HostEvent)>,
    observation_limit: usize,
    fail_next_presentation: bool,
    delivery_mode: DeliveryMode,
    failure_translation: FailureReason,
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
        if !declared_ids.contains(&definition.boundary.source_child)
            || !declared_ids.contains(&definition.boundary.sink_child)
            || definition.boundary.source_child == definition.boundary.sink_child
        {
            return Err(CompositeError::InvalidInternalPlan(
                "boundary must name two distinct declared children".into(),
            ));
        }
        let source_fragment = definition
            .internal_plan
            .fragments
            .iter()
            .find(|fragment| fragment.host_id == definition.boundary.source_child)
            .expect("declared child has a plan fragment");
        let sink_fragment = definition
            .internal_plan
            .fragments
            .iter()
            .find(|fragment| fragment.host_id == definition.boundary.sink_child)
            .expect("declared child has a plan fragment");
        let connection = source_fragment
            .connections
            .iter()
            .find(|connection| {
                connection.provider == ConnectionProvider::InMemory
                    && connection.connection_id == definition.boundary.connection_id
            })
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
        let source_limits = relevant_limit(
            &definition.boundary.source_child,
            &connection.source_placement_id,
        )
        .ok_or_else(|| {
            CompositeError::InvalidInternalPlan(
                "boundary source placement has no matching child capability".into(),
            )
        })?;
        let sink_limits = relevant_limit(
            &definition.boundary.sink_child,
            &connection.sink_placement_id,
        )
        .ok_or_else(|| {
            CompositeError::InvalidInternalPlan(
                "boundary sink placement has no matching child capability".into(),
            )
        })?;
        let mut external_capability = definition.external_capability;
        external_capability.limits.max_active_instances = external_capability
            .limits
            .max_active_instances
            .min(source_limits.max_active_instances)
            .min(sink_limits.max_active_instances)
            .min(1);
        external_capability.limits.max_queue_items = external_capability
            .limits
            .max_queue_items
            .min(connection.item_capacity);
        external_capability.limits.max_queue_bytes = external_capability
            .limits
            .max_queue_bytes
            .min(connection.byte_capacity);
        if external_capability
            .inputs
            .iter()
            .chain(&external_capability.outputs)
            .any(|port| port.value_kind != connection.value_kind)
        {
            return Err(CompositeError::InvalidInternalPlan(
                "external value kind does not match the internal boundary".to_string(),
            ));
        }
        let advertisement = HostAdvertisement {
            protocol_version: PROTOCOL_VERSION,
            host_id: definition.host_id,
            boot_id: definition.boot_id,
            offer_generation: definition.offer_generation,
            profile: definition.profile,
            resources: Vec::new(),
            capabilities: vec![external_capability],
        };
        let internal_plan_id = definition.internal_plan.plan_id.clone();
        let connection_id = connection.connection_id.clone();
        let provider = InMemoryConnectionProvider::new(internal_plan_id.clone(), &connection);
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
            connection_id,
            provider,
            external_plans: BTreeMap::new(),
            released_plans: BTreeSet::new(),
            observations: Vec::new(),
            internal_events: Vec::new(),
            observation_limit,
            fail_next_presentation: false,
            delivery_mode: DeliveryMode::Immediate,
            failure_translation: definition.failure_translation,
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

    pub fn set_delivery_mode(&mut self, mode: DeliveryMode) {
        self.delivery_mode = mode;
    }

    pub fn provider_status(&self) -> ConnectionOutcome {
        self.provider.status()
    }

    pub fn provider_queued_items(&self) -> usize {
        self.provider.queued_items()
    }

    pub fn provider_queued_bytes(&self) -> u32 {
        self.provider.queued_bytes()
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
        let Some((ConnectionOutcome::Delivered, mut envelope)) = self.provider.deliver() else {
            return external;
        };
        mutate(&mut envelope);
        let sequence = envelope.sequence;
        let source_child = self.boundary.source_child.clone();
        let sink_child = self.boundary.sink_child.clone();
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
                connection_id: self.connection_id.clone(),
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

    pub fn disconnect_provider(&mut self, external_plan_id: &PlanId) -> RuntimeOutput {
        let mut external = RuntimeOutput::default();
        let had_queued_envelopes = self.provider.queued_items() > 0;
        let outcome = self.provider.disconnect();
        if !had_queued_envelopes {
            return external;
        }
        let source_child = self.boundary.source_child.clone();
        let source = self
            .children
            .get_mut(&source_child)
            .expect("validated boundary source exists")
            .runtime
            .handle(HostCommand::CompleteConnectionDelivery {
                plan_id: self.internal_plan_id.clone(),
                connection_id: self.connection_id.clone(),
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
        if self.released_plans.contains(&plan_id)
            || fragment.host_id != self.advertisement.host_id
            || fragment.boot_id != self.advertisement.boot_id
            || fragment.offer_generation != self.advertisement.offer_generation
            || fragment.placements.len() != 1
            || fragment.placements[0].kind_id != self.advertisement.capabilities[0].kind_id
            || fragment.placements[0].capability_id
                != self.advertisement.capabilities[0].capability_id
            || fragment.placements[0].implementation_id
                != self.advertisement.capabilities[0].implementation_id
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
        self.external_plans.insert(
            plan_id.clone(),
            ExternalPlan {
                state: ExternalState::Prepared,
                terminal_emitted: false,
                child_terminals: BTreeMap::new(),
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
                reason: FailureReason::InvalidLifecycleCommand,
                message: Some("unknown external plan".into()),
            });
            return output;
        };
        if plan.state != ExternalState::Prepared {
            output.events.push(HostEvent::ActivationRejected {
                plan_id,
                reason: FailureReason::InvalidLifecycleCommand,
                message: Some("external plan is not prepared".into()),
            });
            return output;
        }
        plan.state = ExternalState::Active;
        self.record(Some(plan_id.clone()), ObservationKind::PlanActivated);
        output.events.push(HostEvent::Activated {
            plan_id: plan_id.clone(),
        });
        let source_child = self.boundary.source_child.clone();
        let mut child_ids = self.children.keys().cloned().collect::<Vec<_>>();
        child_ids.sort_by_key(|host_id| host_id == &source_child);
        let initial = child_ids
            .into_iter()
            .map(|host_id| {
                let child_output = self
                    .children
                    .get_mut(&host_id)
                    .expect("listed child exists")
                    .runtime
                    .handle(HostCommand::Activate(self.internal_plan_id.clone()));
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
                    } if child == self.boundary.source_child
                        && connection_id == self.connection_id =>
                    {
                        let sink_child = self.boundary.sink_child.clone();
                        let closed = self
                            .children
                            .get_mut(&sink_child)
                            .expect("validated boundary sink exists")
                            .runtime
                            .handle(HostCommand::CloseConnection {
                                plan_id: self.internal_plan_id.clone(),
                                connection_id,
                            });
                        pending.push_back((sink_child, closed));
                    }
                    HostEvent::PlanTerminated { disposition, .. } => {
                        if let Some(plan) = self.external_plans.get_mut(external_plan_id) {
                            plan.child_terminals.insert(child.clone(), disposition);
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
                                placement_id,
                                value,
                                success,
                                message: (!success)
                                    .then(|| "injected child presentation failure".into()),
                            });
                        pending.push_back((child.clone(), next));
                    }
                    PlatformEffect::TransmitConnection { envelope } => {
                        let sequence = envelope.sequence;
                        let outcome = self.provider.accept(envelope);
                        if outcome == ConnectionOutcome::Accepted {
                            let accepted = self
                                .children
                                .get_mut(&child)
                                .expect("effect came from a known child")
                                .runtime
                                .handle(HostCommand::CompleteConnectionDelivery {
                                    plan_id: self.internal_plan_id.clone(),
                                    connection_id: self.connection_id.clone(),
                                    sequence,
                                    outcome: ConnectionOutcome::Accepted,
                                });
                            pending.push_back((child.clone(), accepted));
                            if self.delivery_mode == DeliveryMode::Immediate {
                                let (delivery_outcome, delivered) = self
                                    .provider
                                    .deliver()
                                    .expect("accepted envelope must be queued");
                                debug_assert_eq!(delivery_outcome, ConnectionOutcome::Delivered);
                                let sink_child = self.boundary.sink_child.clone();
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
                                    .get_mut(&child)
                                    .expect("effect came from a known child")
                                    .runtime
                                    .handle(HostCommand::CompleteConnectionDelivery {
                                        plan_id: self.internal_plan_id.clone(),
                                        connection_id: self.connection_id.clone(),
                                        sequence,
                                        outcome: if sink_accepted {
                                            ConnectionOutcome::Delivered
                                        } else {
                                            ConnectionOutcome::Malformed
                                        },
                                    });
                                pending.push_back((child.clone(), source_output));
                            }
                        } else {
                            let source_output = self
                                .children
                                .get_mut(&child)
                                .expect("effect came from a known child")
                                .runtime
                                .handle(HostCommand::CompleteConnectionDelivery {
                                    plan_id: self.internal_plan_id.clone(),
                                    connection_id: self.connection_id.clone(),
                                    sequence,
                                    outcome,
                                });
                            pending.push_back((child.clone(), source_output));
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
        if plan.terminal_emitted || plan.child_terminals.len() != self.children.len() {
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

fn preparation_failure(output: &RuntimeOutput) -> Option<(FailureReason, Option<String>)> {
    output.events.iter().find_map(|event| match event {
        HostEvent::PreparationRejected {
            reason, message, ..
        } => Some((*reason, message.clone())),
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::{
        ChildHostBinding, CompositeBoundary, CompositeDefinition, CompositeHost, DeliveryMode,
    };
    use conduit_core::{
        kind_id, process_owned_link_binding, ArtifactId, BootId, CapabilityId, CapabilityLimits,
        CapabilityOffer, CheckedFormId, ConnectionEnvelope, ConnectionOutcome, ConnectionProvider,
        ExecutionProfileId, HostAdvertisement, HostCommand, HostEvent, HostId, HostProfileId,
        ImplementationId, KindContractRevision, KindId, OfferGeneration, OperationId,
        TerminalDisposition, PROTOCOL_VERSION,
    };
    use conduit_form::{parse, CheckedForm, CheckedOperation, ProfileCatalog};
    use conduit_planner::{plan, plan_with_link_bindings, PlacementChoice, PlacementChoices};
    use conduit_runtime::{providers::in_memory::InMemoryConnectionProvider, HostRuntime};
    use conduit_signal::{
        pulse_contract_revision, pulse_execution_profile, pulse_host_operation_requirements,
        pulse_outputs, pulse_resource_requirements, show_contract_revision, show_execution_profile,
        show_host_operation_requirements, show_inputs, show_resource_requirements,
        signal_profile_catalog, signal_registry, signal_resource_offers, PULSE_KIND, SHOW_KIND,
    };
    use std::collections::BTreeMap;

    const COMPOSITE_DEMONSTRATION_KIND: &str = "demonstration/run-signal";

    fn authored_internal_form() -> conduit_form::CheckedForm {
        parse(
            include_str!("../../../examples/signal-composite.form"),
            &signal_profile_catalog(),
        )
        .expect("authored composite form parses")
    }

    fn parent_catalog() -> ProfileCatalog {
        let mut catalog = signal_profile_catalog();
        catalog
            .insert_export(&authored_internal_form(), &CapabilityId::from("run-signal"))
            .expect("authored export installs into the parent catalog");
        catalog
    }

    fn child_advertisement(host: &str, boot: &str, source: bool) -> HostAdvertisement {
        HostAdvertisement {
            protocol_version: PROTOCOL_VERSION,
            host_id: HostId::from(host),
            boot_id: BootId::from(boot),
            offer_generation: OfferGeneration(1),
            profile: HostProfileId::from("rust-std"),
            resources: signal_resource_offers(
                &format!("{host}/timer"),
                &format!("{host}/presentation"),
                2,
            ),
            capabilities: vec![CapabilityOffer {
                capability_id: CapabilityId::from(if source { "pulse" } else { "show" }),
                kind_id: kind_id(if source { PULSE_KIND } else { SHOW_KIND }),
                kind_contract_revision: if source {
                    pulse_contract_revision()
                } else {
                    show_contract_revision()
                },
                execution_profile_id: if source {
                    pulse_execution_profile()
                } else {
                    show_execution_profile()
                },
                implementation_id: ImplementationId::from(if source {
                    "test/pulse-v1"
                } else {
                    "test/show-v1"
                }),
                artifact_id: ArtifactId::from(if source {
                    "conduit-signal/pulse-artifact-v1"
                } else {
                    "conduit-signal/show-artifact-v1"
                }),
                inputs: if source { vec![] } else { show_inputs() },
                outputs: if source { pulse_outputs() } else { vec![] },
                host_operations: if source {
                    pulse_host_operation_requirements()
                } else {
                    show_host_operation_requirements()
                },
                resource_requirements: if source {
                    pulse_resource_requirements()
                } else {
                    show_resource_requirements()
                },
                authority_requirements: vec![],
                limits: CapabilityLimits {
                    max_active_instances: 2,
                    max_queue_items: 8,
                    max_queue_bytes: 128,
                },
            }],
        }
    }

    fn internal_plan(item_capacity: u16, byte_capacity: u32) -> conduit_core::Plan {
        let form = authored_internal_form();
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
        let links = [process_owned_link_binding(
            "link/composite-children",
            ConnectionProvider::InMemory,
            "fixture/in-memory/composite-children",
            &source,
            &sink,
            8,
            128,
        )];
        plan_with_link_bindings(
            &form,
            &[source, sink],
            &placements,
            &[ConnectionProvider::Local, ConnectionProvider::InMemory],
            item_capacity,
            byte_capacity,
            &links,
        )
        .expect("cross-host plan succeeds")
    }

    fn three_child_internal_plan() -> conduit_core::Plan {
        let form = parse(
            "form 0\n\ninternal {\n pulse: flow/pulse\n show: presentation/show\n auxiliary: flow/pulse\n pulse.count = 1\n pulse.period-ms = 0\n pulse.initial = false\n auxiliary.count = 0\n auxiliary.period-ms = 0\n auxiliary.initial = false\n pulse > show\n export run-signal: demonstration/run-signal = pulse.signal -> show.signal\n}\n",
            &signal_profile_catalog(),
        )
        .expect("three-child internal form parses");
        let source = child_advertisement("child-source", "source-boot", true);
        let sink = child_advertisement("child-sink", "sink-boot", false);
        let auxiliary = child_advertisement("child-auxiliary", "auxiliary-boot", true);
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
                (
                    OperationId::from("auxiliary"),
                    PlacementChoice {
                        host_id: auxiliary.host_id.clone(),
                        capability_id: CapabilityId::from("pulse"),
                    },
                ),
            ]),
        };
        let links = [process_owned_link_binding(
            "link/composite-children",
            ConnectionProvider::InMemory,
            "fixture/in-memory/composite-children",
            &source,
            &sink,
            8,
            128,
        )];
        plan_with_link_bindings(
            &form,
            &[source, sink, auxiliary],
            &placements,
            &[ConnectionProvider::Local, ConnectionProvider::InMemory],
            conduit_core::DEFAULT_CONNECTION_ITEM_CAPACITY,
            conduit_core::DEFAULT_CONNECTION_BYTE_CAPACITY,
            &links,
        )
        .expect("three-child plan succeeds")
    }

    fn composite(item_capacity: u16, byte_capacity: u32) -> CompositeHost {
        let definition = composite_definition(item_capacity, byte_capacity);
        CompositeHost::from_definition(definition, child_runtimes(), 64)
            .expect("composite is valid")
    }

    fn child_runtimes() -> Vec<HostRuntime> {
        let source_ad = child_advertisement("child-source", "source-boot", true);
        let sink_ad = child_advertisement("child-sink", "sink-boot", false);
        child_runtimes_with_advertisements(source_ad, sink_ad)
    }

    fn child_runtimes_with_advertisements(
        source_ad: HostAdvertisement,
        sink_ad: HostAdvertisement,
    ) -> Vec<HostRuntime> {
        let link = process_owned_link_binding(
            "link/composite-children",
            ConnectionProvider::InMemory,
            "fixture/in-memory/composite-children",
            &source_ad,
            &sink_ad,
            8,
            128,
        );
        vec![
            HostRuntime::new_with_external_state(
                source_ad,
                signal_registry(
                    ImplementationId::from("test/pulse-v1"),
                    ImplementationId::from("unused/show-v1"),
                )
                .expect("source registry installs"),
                128,
                vec![],
                vec![link.clone()],
            ),
            HostRuntime::new_with_external_state(
                sink_ad,
                signal_registry(
                    ImplementationId::from("unused/pulse-v1"),
                    ImplementationId::from("test/show-v1"),
                )
                .expect("sink registry installs"),
                128,
                vec![],
                vec![link],
            ),
        ]
    }

    fn composite_definition(item_capacity: u16, byte_capacity: u32) -> CompositeDefinition {
        let form = authored_internal_form();
        let plan = internal_plan(item_capacity, byte_capacity);
        CompositeDefinition::from_authored_export(
            HostId::from("composite-host"),
            BootId::from("composite-boot"),
            OfferGeneration(7),
            HostProfileId::from("composite/in-memory-v1"),
            ImplementationId::from("composite/pulse-show-v1"),
            ArtifactId::from("composite/pulse-show-artifact-v1"),
            &form,
            &CapabilityId::from("run-signal"),
            plan,
            conduit_core::FailureReason::CompositeCapabilityFailed,
        )
        .expect("authored export derives the composite definition")
    }

    fn parent_fragment(composite: &CompositeHost) -> conduit_core::PlanFragment {
        let form = parse(
            include_str!("../../../examples/composite-parent.form"),
            &parent_catalog(),
        )
        .expect("authored parent form parses");
        let ordinary = child_advertisement("ordinary-host", "ordinary-boot", true);
        let placements = PlacementChoices {
            by_operation: BTreeMap::from([(
                OperationId::from("run"),
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
    fn authored_parent_consumes_derived_export_through_an_ordinary_planned_cord() {
        let internal = authored_internal_form();
        let boundary = internal
            .export_boundary(&CapabilityId::from("run-signal"))
            .expect("authored export checks");
        let composite = composite(4, 64);
        let sink = child_advertisement("parent-sink", "parent-sink-boot", false);
        let parent = parse(
            "form 0\nparent {\n child: demonstration/run-signal\n sink: presentation/show\n child.signal -> sink.signal\n}\n",
            &parent_catalog(),
        )
        .expect("parent consumes the derived output as an ordinary port");
        let placements = PlacementChoices {
            by_operation: BTreeMap::from([
                (
                    OperationId::from("child"),
                    PlacementChoice {
                        host_id: composite.advertisement().host_id.clone(),
                        capability_id: CapabilityId::from("run-signal"),
                    },
                ),
                (
                    OperationId::from("sink"),
                    PlacementChoice {
                        host_id: sink.host_id.clone(),
                        capability_id: CapabilityId::from("show"),
                    },
                ),
            ]),
        };
        let links = [process_owned_link_binding(
            "link/parent-child",
            ConnectionProvider::InMemory,
            "fixture/in-memory/parent-child",
            composite.advertisement(),
            &sink,
            8,
            128,
        )];
        let plan = plan_with_link_bindings(
            &parent,
            &[composite.advertisement().clone(), sink],
            &placements,
            &[ConnectionProvider::Local, ConnectionProvider::InMemory],
            4,
            64,
            &links,
        )
        .expect("ordinary parent cord plans against the composite offer");
        let connection = plan
            .fragments
            .iter()
            .flat_map(|fragment| &fragment.connections)
            .next()
            .expect("parent plan has the ordinary cord");

        assert_eq!(
            parent.operations[0].kind_contract_revision,
            boundary.kind_contract_revision
        );
        assert_eq!(connection.source_port_id.as_str(), "signal");
        assert_eq!(connection.sink_port_id.as_str(), "signal");
        assert_eq!(connection.value_kind, boundary.value_kind);
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
        assert_eq!(provider.accept(envelope), ConnectionOutcome::Terminal);
    }

    #[test]
    fn definition_rejects_missing_extra_and_mismatched_children() {
        let definition = composite_definition(2, 18);

        let mut duplicate_child = definition.clone();
        duplicate_child
            .children
            .push(duplicate_child.children[0].clone());
        assert!(CompositeHost::from_definition(duplicate_child, child_runtimes(), 16).is_err());

        let mut runtimes = child_runtimes();
        runtimes.pop();
        assert!(CompositeHost::from_definition(definition.clone(), runtimes, 16).is_err());

        let mut missing_declared_child = definition.clone();
        missing_declared_child.children.pop();
        assert!(
            CompositeHost::from_definition(missing_declared_child, child_runtimes(), 16).is_err()
        );

        let mut extra_declared_child = definition.clone();
        extra_declared_child.children.push(ChildHostBinding {
            host_id: HostId::from("unused-child"),
        });
        let mut runtimes = child_runtimes();
        runtimes.push(HostRuntime::new(
            child_advertisement("unused-child", "unused-boot", true),
            signal_registry(
                ImplementationId::from("test/pulse-v1"),
                ImplementationId::from("unused/show-v1"),
            )
            .expect("unused child registry installs"),
            16,
        ));
        assert!(CompositeHost::from_definition(extra_declared_child, runtimes, 16).is_err());

        let mut mismatched_boundary = definition;
        std::mem::swap(
            &mut mismatched_boundary.boundary.source_child,
            &mut mismatched_boundary.boundary.sink_child,
        );
        assert!(CompositeHost::from_definition(mismatched_boundary, child_runtimes(), 16).is_err());

        let mut mismatched_fragment = composite_definition(2, 18);
        mismatched_fragment
            .internal_plan
            .fragments
            .iter_mut()
            .find(|fragment| fragment.host_id.as_str() == "child-source")
            .expect("source fragment exists")
            .boot_id = BootId::from("wrong-child-boot");
        assert!(CompositeHost::from_definition(mismatched_fragment, child_runtimes(), 16).is_err());
    }

    #[test]
    fn definition_runs_three_plan_used_children_without_new_role_fields() {
        let plan = three_child_internal_plan();
        let connection = plan
            .fragments
            .iter()
            .flat_map(|fragment| &fragment.connections)
            .next()
            .expect("three-child plan has its exposed boundary")
            .clone();
        let mut definition = composite_definition(4, 64);
        definition.internal_plan = plan;
        definition.children.push(ChildHostBinding {
            host_id: HostId::from("child-auxiliary"),
        });
        definition.boundary.connection_id = connection.connection_id;

        let mut runtimes = child_runtimes();
        runtimes.push(HostRuntime::new(
            child_advertisement("child-auxiliary", "auxiliary-boot", true),
            signal_registry(
                ImplementationId::from("test/pulse-v1"),
                ImplementationId::from("unused/show-v1"),
            )
            .expect("auxiliary registry installs"),
            32,
        ));
        let mut composite = CompositeHost::from_definition(definition, runtimes, 64)
            .expect("three-child composite is valid");
        let fragment = parent_fragment(&composite);
        composite.handle(HostCommand::Prepare(fragment.clone()));
        let output = composite.handle(HostCommand::Activate(fragment.plan_id));
        assert!(output.events.iter().any(|event| matches!(
            event,
            HostEvent::PlanTerminated {
                disposition: TerminalDisposition::Completed,
                ..
            }
        )));
        assert!(composite
            .internal_observations()
            .contains_key(&HostId::from("child-auxiliary")));
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
        assert!(!format!("{output:?}").contains("child-source"));
        assert!(!format!("{output:?}").contains("child-sink"));
        let observations = composite.internal_observations();
        let source = &observations[&HostId::from("child-source")];
        let sink = &observations[&HostId::from("child-sink")];
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
    fn parent_planning_cannot_address_an_internal_child_identity() {
        let composite = composite(4, 64);
        let form = CheckedForm {
            source_document_id: conduit_core::SourceDocumentId::from("parent-child-leak-source"),
            checked_form_id: CheckedFormId::from("parent-child-leak-form"),
            expanded_form_id: conduit_core::ExpandedFormId::from("parent-child-leak-expanded"),
            name: "parent-child-leak".into(),
            operations: vec![CheckedOperation {
                operation_id: OperationId::from("run"),
                kind_id: KindId::from(COMPOSITE_DEMONSTRATION_KIND),
                kind_contract_revision: KindContractRevision::from(format!(
                    "{COMPOSITE_DEMONSTRATION_KIND}@1"
                )),
                inputs: Vec::new(),
                outputs: Vec::new(),
                configuration: Vec::new(),
            }],
            connections: Vec::new(),
            exports: Vec::new(),
        };
        let placements = PlacementChoices {
            by_operation: BTreeMap::from([(
                OperationId::from("run"),
                PlacementChoice {
                    host_id: HostId::from("child-source"),
                    capability_id: CapabilityId::from("pulse"),
                },
            )]),
        };
        assert!(plan(
            &form,
            std::slice::from_ref(composite.advertisement()),
            &placements,
            &[ConnectionProvider::Local, ConnectionProvider::InMemory],
        )
        .is_err());
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
        assert!(!format!("{output:?}").contains("child-source"));
        assert!(!format!("{output:?}").contains("child-sink"));
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
        let observations = composite.internal_observations();
        let sink = &observations[&HostId::from("child-sink")];
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

        let definition = composite_definition(4, 64);
        let mut source_ad = child_advertisement("child-source", "source-boot", true);
        source_ad.capabilities.push(CapabilityOffer {
            capability_id: CapabilityId::from("unrelated-narrow-capability"),
            kind_id: kind_id("unrelated/kind"),
            kind_contract_revision: KindContractRevision::from("unrelated/kind@1"),
            execution_profile_id: ExecutionProfileId::from("unrelated/profile@1"),
            implementation_id: ImplementationId::from("unrelated/implementation"),
            artifact_id: ArtifactId::from("unrelated/artifact"),
            inputs: vec![],
            outputs: vec![],
            host_operations: vec![],
            resource_requirements: vec![],
            authority_requirements: vec![],
            limits: CapabilityLimits {
                max_active_instances: 0,
                max_queue_items: 0,
                max_queue_bytes: 0,
            },
        });
        let sink_ad = child_advertisement("child-sink", "sink-boot", false);
        let relevant_only = CompositeHost::from_definition(
            definition,
            child_runtimes_with_advertisements(source_ad, sink_ad),
            16,
        )
        .expect("unrelated child capability does not narrow exposed limits");
        assert_eq!(
            relevant_only.advertisement().capabilities[0]
                .limits
                .max_active_instances,
            1
        );
    }

    #[test]
    fn controlled_delivery_fills_then_drains_in_real_composite_flow() {
        let mut composite = composite(2, 18);
        composite.set_delivery_mode(DeliveryMode::Controlled);
        let fragment = parent_fragment(&composite);
        let plan_id = fragment.plan_id.clone();
        composite.handle(HostCommand::Prepare(fragment));
        let activated = composite.handle(HostCommand::Activate(plan_id.clone()));
        assert!(!activated
            .events
            .iter()
            .any(|event| matches!(event, HostEvent::PlanTerminated { .. })));
        assert_eq!(composite.provider_status(), ConnectionOutcome::Full);
        assert_eq!(composite.provider_queued_items(), 2);
        assert_eq!(composite.provider_queued_bytes(), 18);
        let before_delivery = composite.internal_observations();
        assert_eq!(
            before_delivery[&HostId::from("child-sink")]
                .iter()
                .filter(|item| matches!(
                    item.kind,
                    conduit_core::ObservationKind::ValuePresented { .. }
                ))
                .count(),
            0
        );
        assert!(composite.internal_events().iter().any(|(host_id, event)| {
            host_id.as_str() == "child-source"
                && matches!(event, HostEvent::ConnectionBlocked { .. })
        }));

        let first_delivery = composite.deliver_next(&plan_id);
        assert!(!first_delivery
            .events
            .iter()
            .any(|event| matches!(event, HostEvent::PlanTerminated { .. })));
        assert_eq!(composite.provider_queued_items(), 2);
        assert_eq!(composite.provider_queued_bytes(), 18);

        let mut terminal = false;
        for _ in 0..3 {
            let output = composite.deliver_next(&plan_id);
            terminal |= output.events.iter().any(|event| {
                matches!(
                    event,
                    HostEvent::PlanTerminated {
                        disposition: TerminalDisposition::Completed,
                        ..
                    }
                )
            });
            if terminal {
                break;
            }
        }
        assert!(terminal);
        assert_eq!(composite.provider_queued_items(), 0);
        assert_eq!(composite.provider_queued_bytes(), 0);
        let after_delivery = composite.internal_observations();
        assert_eq!(
            after_delivery[&HostId::from("child-sink")]
                .iter()
                .filter(|item| matches!(
                    item.kind,
                    conduit_core::ObservationKind::ValuePresented { .. }
                ))
                .count(),
            3
        );
    }

    #[test]
    fn configured_failure_translation_is_the_only_parent_failure_contract() {
        let mut definition = composite_definition(4, 64);
        definition.failure_translation = conduit_core::FailureReason::RequiredBranchFailed;
        let mut composite = CompositeHost::from_definition(definition, child_runtimes(), 64)
            .expect("configured composite is valid");
        composite.fail_next_presentation();
        let fragment = parent_fragment(&composite);
        composite.handle(HostCommand::Prepare(fragment.clone()));
        let output = composite.handle(HostCommand::Activate(fragment.plan_id));
        assert!(output.events.iter().any(|event| matches!(
            event,
            HostEvent::PlanTerminated {
                disposition: TerminalDisposition::Failed {
                    reason: conduit_core::FailureReason::RequiredBranchFailed
                },
                ..
            }
        )));
        let rendered = format!("{output:?}");
        assert!(!rendered.contains("child-source"));
        assert!(!rendered.contains("child-sink"));
    }

    #[test]
    fn malformed_sink_delivery_becomes_source_failure_without_topology_leakage() {
        let mut composite = composite(2, 18);
        composite.set_delivery_mode(DeliveryMode::Controlled);
        let fragment = parent_fragment(&composite);
        let plan_id = fragment.plan_id.clone();
        composite.handle(HostCommand::Prepare(fragment));
        composite.handle(HostCommand::Activate(plan_id.clone()));

        let failed = composite.deliver_next_malformed(&plan_id);
        assert!(
            failed.events.iter().any(|event| matches!(
                event,
                HostEvent::PlanTerminated {
                    disposition: TerminalDisposition::Failed {
                        reason: conduit_core::FailureReason::CompositeCapabilityFailed
                    },
                    ..
                }
            )),
            "{failed:?}"
        );
        assert!(!format!("{failed:?}").contains("child-source"));
        assert!(!format!("{failed:?}").contains("child-sink"));
        let observations = composite.internal_observations();
        assert!(observations[&HostId::from("child-source")]
            .iter()
            .any(|item| matches!(
                &item.kind,
                conduit_core::ObservationKind::ConnectionTerminal { disposition }
                    if matches!(
                        disposition.disposition,
                        TerminalDisposition::Failed {
                            reason: conduit_core::FailureReason::MalformedConnectionEnvelope
                        }
                    )
            )));
    }

    #[test]
    fn disconnect_with_empty_provider_fails_composite_and_rejects_future_delivery() {
        let mut composite = composite(2, 18);
        composite.set_delivery_mode(DeliveryMode::Controlled);
        let fragment = parent_fragment(&composite);
        let plan_id = fragment.plan_id.clone();
        composite.handle(HostCommand::Prepare(fragment));
        let disconnected = composite.disconnect_provider(&plan_id);
        assert_eq!(composite.provider_status(), ConnectionOutcome::Terminal);
        assert!(disconnected
            .events
            .iter()
            .all(|event| !matches!(event, HostEvent::PlanTerminated { .. })));
        let activated = composite.handle(HostCommand::Activate(plan_id));
        assert!(
            activated.events.iter().any(|event| matches!(
                event,
                HostEvent::PlanTerminated {
                    disposition: TerminalDisposition::Failed {
                        reason: conduit_core::FailureReason::CompositeCapabilityFailed
                    },
                    ..
                }
            )),
            "{activated:?}"
        );
    }

    #[test]
    fn controlled_delivery_releases_bytes_only_on_delivery_or_disconnect() {
        let mut composite = composite(4, 64);
        composite.set_delivery_mode(DeliveryMode::Controlled);
        let fragment = parent_fragment(&composite);
        let plan_id = fragment.plan_id.clone();
        composite.handle(HostCommand::Prepare(fragment));
        composite.handle(HostCommand::Activate(plan_id.clone()));
        assert_eq!(composite.provider_queued_bytes(), 27);
        composite.deliver_next(&plan_id);
        assert_eq!(composite.provider_queued_bytes(), 18);
        let failed = composite.disconnect_provider(&plan_id);
        assert_eq!(composite.provider_queued_bytes(), 0);
        assert_eq!(composite.provider_status(), ConnectionOutcome::Terminal);
        assert!(failed.events.iter().any(|event| matches!(
            event,
            HostEvent::PlanTerminated {
                disposition: TerminalDisposition::Failed {
                    reason: conduit_core::FailureReason::CompositeCapabilityFailed
                },
                ..
            }
        )));
        let observations = composite.internal_observations();
        assert!(observations[&HostId::from("child-source")]
            .iter()
            .any(|item| matches!(
                &item.kind,
                conduit_core::ObservationKind::ConnectionTerminal { disposition }
                    if disposition.undeliverable_items == 2
                        && matches!(
                            disposition.disposition,
                            TerminalDisposition::Failed {
                                reason: conduit_core::FailureReason::ConnectionDisconnected
                            }
                        )
            )));
    }

    #[test]
    fn definition_data_can_expose_a_different_composite_capability() {
        let plan = internal_plan(2, 18);
        let connection = plan.fragments[0].connections[0].clone();
        let source_ad = child_advertisement("child-source", "source-boot", true);
        let sink_ad = child_advertisement("child-sink", "sink-boot", false);
        let definition = CompositeDefinition {
            host_id: HostId::from("alternate-composite"),
            boot_id: BootId::from("alternate-boot"),
            offer_generation: OfferGeneration(3),
            profile: HostProfileId::from("composite/test-alternate"),
            external_capability: CapabilityOffer {
                capability_id: CapabilityId::from("alternate-capability"),
                kind_id: kind_id("demonstration/alternate"),
                kind_contract_revision: KindContractRevision::from("demonstration/alternate@1"),
                execution_profile_id: ExecutionProfileId::from("composite/alternate-hosted@1"),
                implementation_id: ImplementationId::from("composite/alternate-v1"),
                artifact_id: ArtifactId::from("composite/alternate-artifact-v1"),
                inputs: show_inputs(),
                outputs: pulse_outputs(),
                host_operations: vec![],
                resource_requirements: vec![],
                authority_requirements: vec![],
                limits: CapabilityLimits {
                    max_active_instances: 5,
                    max_queue_items: 9,
                    max_queue_bytes: 99,
                },
            },
            children: vec![
                ChildHostBinding {
                    host_id: source_ad.host_id.clone(),
                },
                ChildHostBinding {
                    host_id: sink_ad.host_id.clone(),
                },
            ],
            internal_plan: plan,
            boundary: CompositeBoundary {
                source_child: source_ad.host_id.clone(),
                sink_child: sink_ad.host_id.clone(),
                connection_id: connection.connection_id,
            },
            failure_translation: conduit_core::FailureReason::RequiredBranchFailed,
        };
        let host = CompositeHost::from_definition(
            definition,
            vec![
                HostRuntime::new(
                    source_ad,
                    signal_registry(
                        ImplementationId::from("test/pulse-v1"),
                        ImplementationId::from("unused/show-v1"),
                    )
                    .expect("source registry installs"),
                    64,
                ),
                HostRuntime::new(
                    sink_ad,
                    signal_registry(
                        ImplementationId::from("unused/pulse-v1"),
                        ImplementationId::from("test/show-v1"),
                    )
                    .expect("sink registry installs"),
                    64,
                ),
            ],
            64,
        )
        .expect("data-driven composite builds");
        let capability = &host.advertisement().capabilities[0];
        assert_eq!(capability.kind_id.as_str(), "demonstration/alternate");
        assert_eq!(capability.limits.max_active_instances, 1);
        assert_eq!(capability.limits.max_queue_items, 2);
        assert_eq!(capability.limits.max_queue_bytes, 18);
    }
}
