use conduit_core::{
    kind_id, process_owned_line_offer, ArtifactId, BootId, CapabilityId, CapabilityLimits,
    CapabilityOffer, ConnectionBase, ConnectionEnvelope, ConnectionId, ConnectionOutcome,
    HostAdvertisement, HostCommand, HostEvent, HostId, HostProfileId, ImplementationId,
    Observation, OfferGeneration, PlacementId, Plan, PlanFragment, PlanId, PlatformEffect,
    TerminalDisposition, PROTOCOL_VERSION,
};
use conduit_form::CheckedForm;
use conduit_planner::{plan_with_line_offers, PlacementChoice, PlacementChoices};
use conduit_runtime::{HostRuntime, RuntimeOutput};
use conduit_signal::{
    decode_signal, pulse_contract_revision, pulse_execution_profile,
    pulse_host_operation_requirements, pulse_outputs, pulse_resource_requirements,
    show_contract_revision, show_execution_profile, show_host_operation_requirements, show_inputs,
    show_resource_requirements, signal_profile_catalog, signal_registry, signal_resource_offers,
    PULSE_KIND, SHOW_KIND, SIGNAL_PRESENTATION_KIND,
};
use std::collections::{BTreeMap, VecDeque};

#[derive(Debug, Clone)]
pub struct BoundedFrameRelayFixture {
    max_payload_bytes: u32,
    max_frame_bytes: usize,
    frames: Vec<Vec<u8>>,
}

impl BoundedFrameRelayFixture {
    pub fn new(max_payload_bytes: u32, max_frame_bytes: usize) -> Self {
        Self {
            max_payload_bytes,
            max_frame_bytes,
            frames: Vec::new(),
        }
    }

    pub fn frames(&self) -> &[Vec<u8>] {
        &self.frames
    }

    pub fn transmit(
        &mut self,
        envelope: &ConnectionEnvelope,
    ) -> Result<ConnectionEnvelope, String> {
        let frame = conduit_wire::encode_envelope(envelope, self.max_payload_bytes)
            .map_err(|err| format!("frame fixture encode failed: {err:?}"))?;
        if frame.len() > self.max_frame_bytes {
            return Err(format!(
                "frame fixture frame {} exceeds bound {}",
                frame.len(),
                self.max_frame_bytes
            ));
        }
        let decoded = conduit_wire::decode_envelope(&frame, self.max_payload_bytes)
            .map_err(|err| format!("frame fixture decode failed: {err:?}"))?;
        self.frames.push(frame);
        Ok(decoded)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserSimConfig {
    pub host_id: HostId,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserReceipt {
    pub host_id: HostId,
    pub placement_id: PlacementId,
    pub sequence: u64,
    pub level: bool,
    pub indicator_on: bool,
    pub text_state: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserSimSnapshot {
    pub host_id: HostId,
    pub boot_id: BootId,
    pub capabilities: Vec<CapabilityOffer>,
    pub receipts: Vec<BrowserReceipt>,
}

pub struct BrowserSim {
    runtime: HostRuntime,
    receipts: Vec<BrowserReceipt>,
}

impl BrowserSim {
    pub fn new(config: BrowserSimConfig) -> Self {
        let advertisement = browser_advertisement(config);
        let registry = signal_registry(
            ImplementationId::from("browser/pulse-v1"),
            ImplementationId::from("browser/dom-show-signal-v1"),
        )
        .expect("browser signal implementations have unique identities");
        Self {
            runtime: HostRuntime::new(advertisement, registry, 256),
            receipts: Vec::new(),
        }
    }

    pub fn advertisement(&self) -> &HostAdvertisement {
        self.runtime.advertisement()
    }

    pub fn receipts(&self) -> &[BrowserReceipt] {
        &self.receipts
    }

    fn handle(&mut self, command: HostCommand) -> RuntimeOutput {
        self.runtime.handle(command)
    }

    fn replace_line_offers(&mut self, lines: Vec<conduit_core::LineOffer>) {
        self.runtime.replace_line_offers(lines);
    }

    fn complete_dom_presentation(
        &mut self,
        plan_id: conduit_core::PlanId,
        active_play_id: conduit_core::ActivePlayId,
        presentation_id: conduit_core::PresentationId,
        placement_id: PlacementId,
        value: conduit_core::ValuePayload,
    ) -> Result<RuntimeOutput, String> {
        let signal = decode_signal(&value).map_err(|err| err.to_string())?;
        self.receipts.push(BrowserReceipt {
            host_id: self.advertisement().host_id.clone(),
            placement_id: placement_id.clone(),
            sequence: signal.sequence,
            level: signal.level,
            indicator_on: signal.level,
            text_state: format!(
                "signal {} {}",
                signal.sequence,
                if signal.level { "on" } else { "off" }
            ),
        });
        Ok(self.handle(HostCommand::CompletePresentation {
            plan_id,
            active_play_id,
            presentation_id,
            placement_id,
            value,
            success: true,
            message: None,
        }))
    }

    fn inspect(&mut self) -> Vec<Observation> {
        self.handle(HostCommand::Inspect)
            .events
            .into_iter()
            .find_map(|event| match event {
                HostEvent::Observations { items } => Some(items),
                _ => None,
            })
            .unwrap_or_default()
    }

    fn snapshot(&self) -> BrowserSimSnapshot {
        BrowserSimSnapshot {
            host_id: self.advertisement().host_id.clone(),
            boot_id: self.advertisement().boot_id.clone(),
            capabilities: self.advertisement().capabilities.clone(),
            receipts: self.receipts.clone(),
        }
    }
}

pub struct BrowserSimPage {
    hosts: BTreeMap<HostId, BrowserSim>,
    inbound_routes: BTreeMap<(PlanId, ConnectionId), HostId>,
    delivery_ack_routes: BTreeMap<(PlanId, PlacementId), (HostId, ConnectionId)>,
}

impl BrowserSimPage {
    pub fn with_hosts(configs: impl IntoIterator<Item = BrowserSimConfig>) -> Self {
        Self {
            hosts: configs
                .into_iter()
                .map(|config| {
                    let host = BrowserSim::new(config);
                    (host.advertisement().host_id.clone(), host)
                })
                .collect(),
            inbound_routes: BTreeMap::new(),
            delivery_ack_routes: BTreeMap::new(),
        }
    }

    pub fn host_snapshots(&self) -> Vec<BrowserSimSnapshot> {
        self.hosts
            .values()
            .map(BrowserSim::snapshot)
            .collect::<Vec<_>>()
    }

    pub fn advertisements(&self) -> Vec<HostAdvertisement> {
        self.hosts
            .values()
            .map(|host| host.advertisement().clone())
            .collect()
    }

    pub fn replace_line_offers(&mut self, lines: &[conduit_core::LineOffer]) {
        for host in self.hosts.values_mut() {
            let host_id = &host.advertisement().host_id;
            let relevant = lines
                .iter()
                .filter(|line| {
                    &line.binding.source.host_id == host_id || &line.binding.sink.host_id == host_id
                })
                .cloned()
                .collect();
            host.replace_line_offers(relevant);
        }
    }

    pub fn plan_pair(
        &self,
        form: &CheckedForm,
        source_host: &HostId,
        sink_host: &HostId,
    ) -> Result<Plan, Box<dyn std::error::Error>> {
        let placements = PlacementChoices {
            by_gear: BTreeMap::from([
                (
                    conduit_core::GearId::from("pulse"),
                    PlacementChoice {
                        host_id: source_host.clone(),
                        capability_id: CapabilityId::from("pulse"),
                    },
                ),
                (
                    conduit_core::GearId::from("show"),
                    PlacementChoice {
                        host_id: sink_host.clone(),
                        capability_id: CapabilityId::from("dom-show"),
                    },
                ),
            ]),
        };
        let advertisements = self.advertisements();
        let source = advertisements
            .iter()
            .find(|advertisement| &advertisement.host_id == source_host)
            .ok_or("source browser advertisement missing")?;
        let sink = advertisements
            .iter()
            .find(|advertisement| &advertisement.host_id == sink_host)
            .ok_or("sink browser advertisement missing")?;
        let lines = [process_owned_line_offer(
            "line/browser-pair",
            "link/browser-pair",
            ConnectionBase::InMemory,
            "fixture/in-memory/browser-pair",
            source,
            sink,
            4,
            64,
        )];
        Ok(plan_with_line_offers(
            form,
            &advertisements,
            &placements,
            &[ConnectionBase::InMemory],
            4,
            64,
            &lines,
        )?)
    }

    pub fn plan_std_to_browser(
        &self,
        form: &CheckedForm,
        std_advertisement: &HostAdvertisement,
        browser_host: &HostId,
    ) -> Result<Plan, Box<dyn std::error::Error>> {
        let placements = PlacementChoices {
            by_gear: BTreeMap::from([
                (
                    conduit_core::GearId::from("pulse"),
                    PlacementChoice {
                        host_id: std_advertisement.host_id.clone(),
                        capability_id: CapabilityId::from("pulse-1"),
                    },
                ),
                (
                    conduit_core::GearId::from("show"),
                    PlacementChoice {
                        host_id: browser_host.clone(),
                        capability_id: CapabilityId::from("dom-show"),
                    },
                ),
            ]),
        };
        let mut hosts = Vec::with_capacity(self.hosts.len() + 1);
        hosts.push(std_advertisement.clone());
        hosts.extend(self.advertisements());
        let browser_advertisement = hosts
            .iter()
            .find(|advertisement| &advertisement.host_id == browser_host)
            .ok_or("browser advertisement missing")?;
        let lines = [process_owned_line_offer(
            "line/std-browser",
            "link/std-browser",
            ConnectionBase::FixtureFrame,
            "fixture/frame/std-browser",
            std_advertisement,
            browser_advertisement,
            4,
            64,
        )];
        Ok(plan_with_line_offers(
            form,
            &hosts,
            &placements,
            &[ConnectionBase::FixtureFrame],
            4,
            64,
            &lines,
        )?)
    }

    pub fn run_plan(&mut self, plan: Plan) -> Result<BrowserRunReport, String> {
        let line_offers = plan
            .fragments
            .iter()
            .flat_map(|fragment| &fragment.connections)
            .flat_map(|connection| connection.admitted_lines.iter().cloned())
            .fold(BTreeMap::new(), |mut lines, line| {
                lines.entry(line.line_id.clone()).or_insert(line);
                lines
            })
            .into_values()
            .collect::<Vec<_>>();
        let line_offers = line_offers
            .into_iter()
            .map(|line| conduit_core::LineOffer {
                availability: conduit_core::LineAvailabilitySign {
                    line_id: line.line_id.clone(),
                    binding_id: line.binding.binding_id.clone(),
                    availability: conduit_core::LineAvailability::Ready,
                    sign_id: conduit_core::ClueId::from(format!(
                        "fixture/line/{}/ready",
                        line.line_id.as_str()
                    )),
                },
                line_id: line.line_id,
                binding: conduit_core::LinkBinding {
                    binding_id: line.binding.binding_id,
                    source: line.binding.source,
                    sink: line.binding.sink,
                    base: line.binding.base,
                    base_instance_id: line.binding.base_instance_id,
                    credential: line.binding.credential,
                    authority: line.binding.authority,
                    limits: line.binding.limits,
                },
                contract: line.contract,
            })
            .collect::<Vec<_>>();
        self.replace_line_offers(&line_offers);
        self.inbound_routes = inbound_routes(&plan.fragments);
        self.delivery_ack_routes = delivery_ack_routes(&plan.fragments);
        for fragment in &plan.fragments {
            let output = self
                .host_mut(&fragment.host_id)?
                .handle(HostCommand::Prepare(fragment.clone()));
            ensure_prepared(&output)?;
        }

        let mut pending = VecDeque::new();
        for fragment in sink_fragments_first(&plan.fragments) {
            let output = self
                .host_mut(&fragment.host_id)?
                .handle(HostCommand::StartPlay(fragment.plan_id.clone()));
            ensure_triggerd(&output)?;
            pending.extend(
                output
                    .effects
                    .into_iter()
                    .map(|effect| (fragment.host_id.clone(), effect)),
            );
        }

        while let Some((host_id, effect)) = pending.pop_front() {
            pending.extend(self.handle_effect(&host_id, effect)?);
        }

        let receipts = self
            .hosts
            .values()
            .flat_map(|host| host.receipts.clone())
            .collect::<Vec<_>>();
        let observations = self
            .hosts
            .values_mut()
            .flat_map(BrowserSim::inspect)
            .collect::<Vec<_>>();
        Ok(BrowserRunReport {
            plan_id: plan.plan_id,
            receipts,
            observations,
            snapshots: self.host_snapshots(),
        })
    }

    fn handle_effect(
        &mut self,
        host_id: &HostId,
        effect: PlatformEffect,
    ) -> Result<Vec<(HostId, PlatformEffect)>, String> {
        match effect {
            PlatformEffect::Wait {
                plan_id,
                placement_id,
                ..
            } => {
                let output = self.host_mut(host_id)?.handle(HostCommand::CompleteWait {
                    plan_id,
                    placement_id,
                });
                self.effects_for(host_id, output)
            }
            PlatformEffect::PresentValue {
                plan_id,
                active_play_id,
                presentation_id,
                placement_id,
                presentation_kind,
                value,
            } => {
                if presentation_kind.as_str() != SIGNAL_PRESENTATION_KIND {
                    return Err(format!(
                        "browser simulation cannot manifest presentation kind '{}'",
                        presentation_kind.as_str()
                    ));
                }
                let output = self.host_mut(host_id)?.complete_dom_presentation(
                    plan_id,
                    active_play_id,
                    presentation_id,
                    placement_id,
                    value,
                )?;
                self.effects_for(host_id, output)
            }
            PlatformEffect::TransmitConnection { envelope } => {
                let sink_host_id = self
                    .host_for_inbound_connection(&envelope.plan_id, &envelope.connection_id)
                    .ok_or_else(|| {
                        format!(
                            "no browser simulation has inbound connection '{}'",
                            envelope.connection_id.as_str()
                        )
                    })?;
                let outcome = self
                    .host_mut(&sink_host_id)?
                    .handle(HostCommand::AcceptConnectionEnvelope(envelope.clone()));
                pending_success(&outcome, envelope.sequence)?;
                let accepted =
                    self.host_mut(host_id)?
                        .handle(HostCommand::CompleteConnectionDelivery {
                            plan_id: envelope.plan_id,
                            connection_id: envelope.connection_id,
                            sequence: envelope.sequence,
                            outcome: ConnectionOutcome::Accepted,
                        });
                let mut pending = self.effects_for(&sink_host_id, outcome)?;
                pending.extend(self.effects_for(host_id, accepted)?);
                Ok(pending)
            }
        }
    }

    fn effects_for(
        &mut self,
        host_id: &HostId,
        output: RuntimeOutput,
    ) -> Result<Vec<(HostId, PlatformEffect)>, String> {
        let mut pending = output
            .effects
            .into_iter()
            .map(|effect| (host_id.clone(), effect))
            .collect::<Vec<_>>();
        for event in output.events {
            match event {
                HostEvent::ConnectionTerminated {
                    plan_id,
                    connection_id,
                    disposition,
                } if matches!(disposition.disposition, TerminalDisposition::Completed) => {
                    if let Some(sink_host_id) =
                        self.host_for_inbound_connection(&plan_id, &connection_id)
                    {
                        let close =
                            self.host_mut(&sink_host_id)?
                                .handle(HostCommand::CloseConnection {
                                    plan_id,
                                    connection_id,
                                });
                        pending.extend(
                            close
                                .effects
                                .into_iter()
                                .map(|effect| (sink_host_id.clone(), effect)),
                        );
                    }
                }
                HostEvent::ManifestationCompleted {
                    plan_id,
                    placement_id,
                    value,
                    ..
                } => {
                    if let Some((source_host_id, connection_id)) = self
                        .delivery_ack_routes
                        .get(&(plan_id.clone(), placement_id))
                        .cloned()
                    {
                        let signal = decode_signal(&value).map_err(|err| err.to_string())?;
                        let delivered = self.host_mut(&source_host_id)?.handle(
                            HostCommand::CompleteConnectionDelivery {
                                plan_id,
                                connection_id,
                                sequence: signal.sequence,
                                outcome: ConnectionOutcome::Delivered,
                            },
                        );
                        pending.extend(
                            delivered
                                .effects
                                .into_iter()
                                .map(|effect| (source_host_id.clone(), effect)),
                        );
                    }
                }
                _ => {}
            }
        }
        Ok(pending)
    }

    fn host_mut(&mut self, host_id: &HostId) -> Result<&mut BrowserSim, String> {
        self.hosts
            .get_mut(host_id)
            .ok_or_else(|| format!("unknown browser simulation '{}'", host_id.as_str()))
    }

    fn host_for_inbound_connection(
        &self,
        plan_id: &conduit_core::PlanId,
        connection_id: &conduit_core::ConnectionId,
    ) -> Option<HostId> {
        self.inbound_routes
            .get(&(plan_id.clone(), connection_id.clone()))
            .cloned()
    }
}

#[derive(Debug, Clone)]
pub struct BrowserRunReport {
    pub plan_id: conduit_core::PlanId,
    pub receipts: Vec<BrowserReceipt>,
    pub observations: Vec<Observation>,
    pub snapshots: Vec<BrowserSimSnapshot>,
}

pub fn load_checked_form(path: &str) -> Result<CheckedForm, Box<dyn std::error::Error>> {
    Ok(conduit_form::parse(
        &std::fs::read_to_string(path)?,
        &signal_profile_catalog(),
    )?)
}

fn browser_advertisement(config: BrowserSimConfig) -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: config.host_id,
        boot_id: config.boot_id,
        offer_generation: config.offer_generation,
        profile: HostProfileId::from("browser/wasm-page-v1"),
        resources: signal_resource_offers("browser/timer", "browser/presentation", 16),
        planner_capabilities: vec![],
        capabilities: vec![
            CapabilityOffer {
                startup_parameters: conduit_signal::pulse_face_startup_parameters(),
                shorthand: None,
                capability_id: CapabilityId::from("pulse"),
                kind_id: kind_id(PULSE_KIND),
                kind_contract_revision: pulse_contract_revision(),
                implementation: conduit_core::ImplementationOffer {
                    execution_profile_id: pulse_execution_profile(),
                    implementation_id: ImplementationId::from("browser/pulse-v1"),
                    artifact_id: ArtifactId::from("conduit-signal/pulse-artifact-v1"),
                },
                inputs: vec![],
                outputs: pulse_outputs(),
                host_operations: pulse_host_operation_requirements(),
                resource_requirements: pulse_resource_requirements(),
                authority_requirements: vec![],
                limits: CapabilityLimits {
                    max_active_instances: 16,
                    max_queue_items: 4,
                    max_queue_bytes: 64,
                },
            },
            CapabilityOffer {
                startup_parameters: vec![],
                shorthand: None,
                capability_id: CapabilityId::from("dom-show"),
                kind_id: kind_id(SHOW_KIND),
                kind_contract_revision: show_contract_revision(),
                implementation: conduit_core::ImplementationOffer {
                    execution_profile_id: show_execution_profile(),
                    implementation_id: ImplementationId::from("browser/dom-show-signal-v1"),
                    artifact_id: ArtifactId::from("conduit-signal/show-artifact-v1"),
                },
                inputs: show_inputs(),
                outputs: vec![],
                host_operations: show_host_operation_requirements(),
                resource_requirements: show_resource_requirements(),
                authority_requirements: vec![],
                limits: CapabilityLimits {
                    max_active_instances: 16,
                    max_queue_items: 4,
                    max_queue_bytes: 64,
                },
            },
        ],
    }
}

fn inbound_routes(fragments: &[PlanFragment]) -> BTreeMap<(PlanId, ConnectionId), HostId> {
    fragments
        .iter()
        .flat_map(|fragment| {
            fragment.connections.iter().filter_map(|connection| {
                let has_sink = fragment
                    .placements
                    .iter()
                    .any(|placement| placement.placement_id == connection.sink_placement_id);
                let has_source = fragment
                    .placements
                    .iter()
                    .any(|placement| placement.placement_id == connection.source_placement_id);
                (connection.selected_line.is_some() && has_sink && !has_source).then(|| {
                    (
                        (fragment.plan_id.clone(), connection.connection_id.clone()),
                        fragment.host_id.clone(),
                    )
                })
            })
        })
        .collect()
}

fn delivery_ack_routes(
    fragments: &[PlanFragment],
) -> BTreeMap<(PlanId, PlacementId), (HostId, ConnectionId)> {
    fragments
        .iter()
        .flat_map(|fragment| {
            fragment.connections.iter().filter_map(|connection| {
                let has_source = fragment
                    .placements
                    .iter()
                    .any(|placement| placement.placement_id == connection.source_placement_id);
                let has_sink = fragment
                    .placements
                    .iter()
                    .any(|placement| placement.placement_id == connection.sink_placement_id);
                (connection.selected_line.is_some() && has_source && !has_sink).then(|| {
                    (
                        (
                            fragment.plan_id.clone(),
                            connection.sink_placement_id.clone(),
                        ),
                        (fragment.host_id.clone(), connection.connection_id.clone()),
                    )
                })
            })
        })
        .collect()
}

fn sink_fragments_first(fragments: &[PlanFragment]) -> Vec<&PlanFragment> {
    let mut ordered = fragments.iter().collect::<Vec<_>>();
    ordered.sort_by_key(|fragment| {
        fragment
            .placements
            .iter()
            .any(|placement| !placement.inputs.is_empty())
            && !fragment
                .placements
                .iter()
                .any(|placement| !placement.outputs.is_empty())
    });
    ordered.reverse();
    ordered
}

fn pending_success(output: &RuntimeOutput, sequence: u64) -> Result<(), String> {
    output
        .events
        .iter()
        .find_map(|event| match event {
            HostEvent::ConnectionEnvelopeOutcome {
                sequence: event_sequence,
                outcome,
                ..
            } if *event_sequence == sequence => Some(*outcome),
            _ => None,
        })
        .filter(|outcome| *outcome == ConnectionOutcome::Accepted)
        .map(|_| ())
        .ok_or_else(|| format!("remote browser simulation did not accept sequence {sequence}"))
}

fn ensure_prepared(output: &RuntimeOutput) -> Result<(), String> {
    output
        .events
        .iter()
        .any(|event| matches!(event, HostEvent::Prepared { .. }))
        .then_some(())
        .ok_or_else(|| format!("browser simulation prepare failed: {:?}", output.events))
}

fn ensure_triggerd(output: &RuntimeOutput) -> Result<(), String> {
    output
        .events
        .iter()
        .any(|event| matches!(event, HostEvent::PlayStarted { .. }))
        .then_some(())
        .ok_or_else(|| format!("browser simulation trigger failed: {:?}", output.events))
}

#[cfg(test)]
mod tests {
    use super::{BoundedFrameRelayFixture, BrowserSimConfig, BrowserSimPage};
    use conduit_core::{
        kind_id, process_owned_line_offer, BootId, ConnectionBase, ConnectionId, ConnectionOutcome,
        HostCommand, HostEvent, HostId, OfferGeneration, PlacementId, PlatformEffect,
        TerminalDisposition,
    };
    use conduit_form::parse;
    use conduit_pico_sim::{BoundedDatagramRelayFixture, PicoSim, PicoSimConfig};
    use conduit_planner::{plan_with_options, PlacementChoice, PlacementChoices, PlanningOptions};
    use conduit_runtime::RuntimeOutput;
    use conduit_signal::{signal_profile_catalog, PULSE_KIND, SHOW_KIND};
    use conduit_std_host::{LegacyStdFixtureHost, SignalReceipt, StdHostConfig};
    use std::collections::{BTreeMap, VecDeque};

    fn page() -> BrowserSimPage {
        BrowserSimPage::with_hosts([
            BrowserSimConfig {
                host_id: HostId::from("browser-sim-a"),
                boot_id: BootId::from("browser-sim-boot-a"),
                offer_generation: OfferGeneration(1),
            },
            BrowserSimConfig {
                host_id: HostId::from("browser-sim-b"),
                boot_id: BootId::from("browser-sim-boot-b"),
                offer_generation: OfferGeneration(1),
            },
        ])
    }

    fn pair_form() -> conduit_form::CheckedForm {
        parse(
            "form 0\n\nsignal-demo {\n pulse: flow/pulse\n show: presentation/show\n pulse.count = 16\n pulse.period-ms = 250\n pulse.initial = false\n pulse > show\n}\n",
            &signal_profile_catalog(),
        )
        .expect("browser pair form parses")
    }

    fn triple_form() -> conduit_form::CheckedForm {
        parse(
            include_str!("../../../examples/triple-signal.form"),
            &signal_profile_catalog(),
        )
        .expect("triple signal form parses")
    }

    fn planned_lines(plan: &conduit_core::Plan) -> Vec<conduit_core::LineOffer> {
        plan.fragments
            .iter()
            .flat_map(|fragment| &fragment.connections)
            .flat_map(|connection| connection.admitted_lines.iter().cloned())
            .fold(BTreeMap::new(), |mut lines, line| {
                lines.entry(line.line_id.clone()).or_insert(line);
                lines
            })
            .into_values()
            .map(|line| conduit_core::LineOffer {
                availability: conduit_core::LineAvailabilitySign {
                    line_id: line.line_id.clone(),
                    binding_id: line.binding.binding_id.clone(),
                    availability: conduit_core::LineAvailability::Ready,
                    sign_id: conduit_core::ClueId::from(format!(
                        "fixture/{}/ready",
                        line.line_id.as_str()
                    )),
                },
                line_id: line.line_id,
                binding: conduit_core::LinkBinding {
                    binding_id: line.binding.binding_id,
                    source: line.binding.source,
                    sink: line.binding.sink,
                    base: line.binding.base,
                    base_instance_id: line.binding.base_instance_id,
                    credential: line.binding.credential,
                    authority: line.binding.authority,
                    limits: line.binding.limits,
                },
                contract: line.contract,
            })
            .collect()
    }

    #[test]
    fn one_page_owns_multiple_independent_browser_simulations() {
        let page = page();
        let snapshots = page.host_snapshots();
        assert_eq!(snapshots.len(), 2);
        assert_ne!(snapshots[0].host_id, snapshots[1].host_id);
        assert_ne!(snapshots[0].boot_id, snapshots[1].boot_id);
        for snapshot in snapshots {
            assert_eq!(snapshot.capabilities.len(), 2);
            assert!(snapshot
                .capabilities
                .iter()
                .any(|offer| offer.kind_id == kind_id(PULSE_KIND)
                    && offer.implementation.implementation_id.as_str() == "browser/pulse-v1"));
            assert!(snapshot
                .capabilities
                .iter()
                .any(|offer| offer.kind_id == kind_id(SHOW_KIND)
                    && offer.implementation.implementation_id.as_str()
                        == "browser/dom-show-signal-v1"));
        }
    }

    #[test]
    fn two_browser_simulations_execute_pair_form_over_bounded_memory_link() {
        let mut page = page();
        let source_host = HostId::from("browser-sim-a");
        let sink_host = HostId::from("browser-sim-b");
        let plan = page
            .plan_pair(&pair_form(), &source_host, &sink_host)
            .expect("browser pair plan resolves");
        assert_eq!(plan.fragments.len(), 2);
        let connection = plan
            .fragments
            .iter()
            .flat_map(|fragment| &fragment.connections)
            .find(|connection| connection.selected_line.is_some())
            .expect("browser-memory connection is planned");
        assert_eq!(connection.item_capacity, 4);
        assert_eq!(connection.byte_capacity, 64);

        let report = page.run_plan(plan).expect("browser pair run completes");
        let sink_receipts = report
            .receipts
            .iter()
            .filter(|receipt| receipt.host_id == sink_host)
            .collect::<Vec<_>>();
        assert_eq!(sink_receipts.len(), 16);
        assert_eq!(sink_receipts[0].sequence, 0);
        assert!(!sink_receipts[0].level);
        assert_eq!(sink_receipts[0].text_state, "signal 0 off");
        assert_eq!(sink_receipts[15].sequence, 15);
        assert!(sink_receipts[15].level);
        assert!(sink_receipts[15].indicator_on);
        assert!(!report
            .receipts
            .iter()
            .any(|receipt| receipt.host_id == source_host));
        assert!(report.observations.iter().any(|observation| matches!(
            observation.kind,
            conduit_core::ObservationKind::PlanTerminal {
                disposition: TerminalDisposition::Completed
            }
        )));
        let sink_snapshot = report
            .snapshots
            .iter()
            .find(|snapshot| snapshot.host_id == sink_host)
            .expect("sink snapshot exists");
        assert_eq!(sink_snapshot.receipts.len(), 16);
    }

    #[test]
    fn std_host_sends_signal_to_browser_through_bounded_frame_fixture() {
        let mut std_host = LegacyStdFixtureHost::new_with_config(StdHostConfig {
            host_id: HostId::from("std-host-1"),
            boot_id: BootId::from("std-boot-1"),
            offer_generation: OfferGeneration(1),
        });
        let mut page = BrowserSimPage::with_hosts([BrowserSimConfig {
            host_id: HostId::from("browser-sim-web"),
            boot_id: BootId::from("browser-sim-boot-web"),
            offer_generation: OfferGeneration(1),
        }]);
        let browser_host = HostId::from("browser-sim-web");
        let plan = page
            .plan_std_to_browser(&pair_form(), std_host.advertisement(), &browser_host)
            .expect("std-to-browser plan resolves");
        let connection = plan
            .fragments
            .iter()
            .flat_map(|fragment| &fragment.connections)
            .find(|connection| {
                connection
                    .selected_line
                    .as_ref()
                    .map(|line| line.binding.base)
                    == Some(ConnectionBase::FixtureFrame)
            })
            .expect("frame fixture connection is planned");
        assert_eq!(connection.item_capacity, 4);
        assert_eq!(connection.byte_capacity, 64);

        let lines = planned_lines(&plan);
        std_host.replace_line_offers(lines.clone());
        page.replace_line_offers(&lines);

        page.inbound_routes = super::inbound_routes(&plan.fragments);
        page.delivery_ack_routes = super::delivery_ack_routes(&plan.fragments);
        for fragment in &plan.fragments {
            if fragment.host_id == std_host.advertisement().host_id {
                super::ensure_prepared(&std_host.handle(HostCommand::Prepare(fragment.clone())))
                    .expect("std source prepares");
            } else {
                super::ensure_prepared(
                    &page
                        .host_mut(&fragment.host_id)
                        .expect("browser simulation exists")
                        .handle(HostCommand::Prepare(fragment.clone())),
                )
                .expect("browser sink prepares");
            }
        }

        let mut pending = VecDeque::new();
        for fragment in super::sink_fragments_first(&plan.fragments) {
            if fragment.host_id == std_host.advertisement().host_id {
                let output = std_host.handle(HostCommand::StartPlay(fragment.plan_id.clone()));
                super::ensure_triggerd(&output).expect("std triggers");
                pending.extend(
                    output
                        .effects
                        .into_iter()
                        .map(|effect| (fragment.host_id.clone(), effect)),
                );
            } else {
                let output = page
                    .host_mut(&fragment.host_id)
                    .expect("browser simulation exists")
                    .handle(HostCommand::StartPlay(fragment.plan_id.clone()));
                super::ensure_triggerd(&output).expect("browser triggers");
                pending.extend(
                    output
                        .effects
                        .into_iter()
                        .map(|effect| (fragment.host_id.clone(), effect)),
                );
            }
        }

        let mut relay = BoundedFrameRelayFixture::new(64, 512);
        while let Some((host_id, effect)) = pending.pop_front() {
            if host_id == std_host.advertisement().host_id {
                pending.extend(drive_std_effect(
                    &mut std_host,
                    &mut page,
                    &mut relay,
                    effect,
                ));
            } else {
                pending.extend(drive_browser_effect_with_std_ack(
                    &mut std_host,
                    &mut page,
                    &host_id,
                    effect,
                ));
            }
        }

        assert_eq!(relay.frames().len(), 16);
        let report = page.host_snapshots();
        let sink = report
            .iter()
            .find(|snapshot| snapshot.host_id == browser_host)
            .expect("browser sink snapshot exists");
        assert_eq!(sink.receipts.len(), 16);
        assert_eq!(sink.receipts[0].sequence, 0);
        assert!(!sink.receipts[0].level);
        assert_eq!(sink.receipts[15].sequence, 15);
        assert!(sink.receipts[15].level);
        let observations = page
            .host_mut(&browser_host)
            .expect("browser simulation exists")
            .inspect();
        assert!(observations.iter().any(|observation| matches!(
            observation.kind,
            conduit_core::ObservationKind::PlanTerminal {
                disposition: TerminalDisposition::Completed
            }
        )));
    }

    #[test]
    fn triple_signal_form_fans_out_to_std_and_simulated_receipts() {
        let mut std_host = LegacyStdFixtureHost::new_with_config(StdHostConfig {
            host_id: HostId::from("std-host-triple"),
            boot_id: BootId::from("std-boot-triple"),
            offer_generation: OfferGeneration(1),
        });
        let mut page = BrowserSimPage::with_hosts([BrowserSimConfig {
            host_id: HostId::from("browser-sim-triple"),
            boot_id: BootId::from("browser-sim-boot-triple"),
            offer_generation: OfferGeneration(1),
        }]);
        let mut pico = PicoSim::new(PicoSimConfig {
            host_id: HostId::from("pico-sim-triple"),
            boot_id: BootId::from("pico-sim-boot-triple"),
            offer_generation: OfferGeneration(1),
        });

        let form = triple_form();
        let browser_host = HostId::from("browser-sim-triple");
        let placements = PlacementChoices {
            by_gear: BTreeMap::from([
                (
                    conduit_core::GearId::from("pulse"),
                    PlacementChoice {
                        host_id: std_host.advertisement().host_id.clone(),
                        capability_id: conduit_core::CapabilityId::from("pulse-1"),
                    },
                ),
                (
                    conduit_core::GearId::from("local"),
                    PlacementChoice {
                        host_id: std_host.advertisement().host_id.clone(),
                        capability_id: conduit_core::CapabilityId::from("stdout-show-1"),
                    },
                ),
                (
                    conduit_core::GearId::from("web"),
                    PlacementChoice {
                        host_id: browser_host.clone(),
                        capability_id: conduit_core::CapabilityId::from("dom-show"),
                    },
                ),
                (
                    conduit_core::GearId::from("light"),
                    PlacementChoice {
                        host_id: pico.advertisement().host_id.clone(),
                        capability_id: conduit_core::CapabilityId::from("onboard-led"),
                    },
                ),
            ]),
        };
        let connection_bases = BTreeMap::from([
            (
                (
                    conduit_core::GearId::from("pulse"),
                    conduit_core::GearId::from("local"),
                ),
                ConnectionBase::Local,
            ),
            (
                (
                    conduit_core::GearId::from("pulse"),
                    conduit_core::GearId::from("web"),
                ),
                ConnectionBase::FixtureFrame,
            ),
            (
                (
                    conduit_core::GearId::from("pulse"),
                    conduit_core::GearId::from("light"),
                ),
                ConnectionBase::FixtureDatagram,
            ),
        ]);
        let hosts = [
            std_host.advertisement().clone(),
            page.advertisements()
                .into_iter()
                .next()
                .expect("browser advertisement exists"),
            pico.advertisement().clone(),
        ];
        let line_offers = [
            process_owned_line_offer(
                "line/std-browser",
                "link/std-browser",
                ConnectionBase::FixtureFrame,
                "fixture/frame/std-browser",
                &hosts[0],
                &hosts[1],
                4,
                64,
            ),
            process_owned_line_offer(
                "line/std-pico",
                "link/std-pico",
                ConnectionBase::FixtureDatagram,
                "fixture/datagram/std-pico",
                &hosts[0],
                &hosts[2],
                4,
                64,
            ),
        ];
        let plan = plan_with_options(
            &form,
            &hosts,
            &placements,
            &[
                ConnectionBase::Local,
                ConnectionBase::FixtureFrame,
                ConnectionBase::FixtureDatagram,
            ],
            PlanningOptions {
                connection_bases: &connection_bases,
                line_candidates: &BTreeMap::new(),
                connection_item_capacity: 4,
                connection_byte_capacity: 64,
                authority_grants: &[],
                protected_resource_grants: &[],
                line_offers: &line_offers,
            },
        )
        .expect("triple-simulation plan resolves");
        let connection_base_by_id = plan
            .fragments
            .iter()
            .flat_map(|fragment| &fragment.connections)
            .map(|connection| {
                (
                    connection.connection_id.clone(),
                    connection
                        .selected_line
                        .as_ref()
                        .map(|line| line.binding.base)
                        .unwrap_or(ConnectionBase::Local),
                )
            })
            .collect::<BTreeMap<_, _>>();
        assert_eq!(
            connection_base_by_id
                .values()
                .filter(|base| **base == ConnectionBase::Local)
                .count(),
            1
        );
        assert_eq!(
            connection_base_by_id
                .values()
                .filter(|base| **base == ConnectionBase::FixtureFrame)
                .count(),
            1
        );
        assert_eq!(
            connection_base_by_id
                .values()
                .filter(|base| **base == ConnectionBase::FixtureDatagram)
                .count(),
            1
        );
        let source_connection_by_sink = plan
            .fragments
            .iter()
            .flat_map(|fragment| &fragment.connections)
            .filter(|connection| connection.selected_line.is_some())
            .map(|connection| {
                (
                    connection.sink_placement_id.clone(),
                    connection.connection_id.clone(),
                )
            })
            .collect::<BTreeMap<_, _>>();

        let lines = planned_lines(&plan);
        std_host.replace_line_offers(lines.clone());
        page.replace_line_offers(&lines);
        pico.replace_line_offers(lines);

        page.inbound_routes = super::inbound_routes(&plan.fragments);
        page.delivery_ack_routes = super::delivery_ack_routes(&plan.fragments);
        for fragment in &plan.fragments {
            let output = if fragment.host_id == std_host.advertisement().host_id {
                std_host.handle(HostCommand::Prepare(fragment.clone()))
            } else if fragment.host_id == browser_host {
                page.host_mut(&fragment.host_id)
                    .expect("browser simulation exists")
                    .handle(HostCommand::Prepare(fragment.clone()))
            } else {
                pico.handle(HostCommand::Prepare(fragment.clone()))
            };
            super::ensure_prepared(&output).expect("fragment prepares");
        }

        let mut pending = VecDeque::new();
        for fragment in super::sink_fragments_first(&plan.fragments) {
            let output = if fragment.host_id == std_host.advertisement().host_id {
                std_host.handle(HostCommand::StartPlay(fragment.plan_id.clone()))
            } else if fragment.host_id == browser_host {
                page.host_mut(&fragment.host_id)
                    .expect("browser simulation exists")
                    .handle(HostCommand::StartPlay(fragment.plan_id.clone()))
            } else {
                pico.handle(HostCommand::StartPlay(fragment.plan_id.clone()))
            };
            super::ensure_triggerd(&output).expect("fragment triggers");
            pending.extend(
                output
                    .effects
                    .into_iter()
                    .map(|effect| (fragment.host_id.clone(), effect)),
            );
        }

        let mut frame_fixture = BoundedFrameRelayFixture::new(64, 512);
        let mut datagram_fixture = BoundedDatagramRelayFixture::new(64, 512);
        let mut stdout_receipts = Vec::new();
        while let Some((host_id, effect)) = pending.pop_front() {
            if host_id == std_host.advertisement().host_id {
                pending.extend(drive_triple_std_effect(
                    &mut std_host,
                    &mut page,
                    &mut pico,
                    &connection_base_by_id,
                    &mut frame_fixture,
                    &mut datagram_fixture,
                    &mut stdout_receipts,
                    effect,
                ));
            } else if host_id == browser_host {
                pending.extend(drive_triple_browser_effect(
                    &mut std_host,
                    &mut page,
                    &mut pico,
                    &connection_base_by_id,
                    &source_connection_by_sink,
                    &host_id,
                    effect,
                ));
            } else {
                pending.extend(drive_triple_pico_effect(
                    &mut std_host,
                    &mut page,
                    &mut pico,
                    &connection_base_by_id,
                    &source_connection_by_sink,
                    effect,
                ));
            }
        }

        assert_eq!(stdout_receipts.len(), 16);
        assert_eq!(frame_fixture.frames().len(), 16);
        assert_eq!(datagram_fixture.datagrams().len(), 16);
        let browser_receipts = page
            .host_snapshots()
            .into_iter()
            .find(|snapshot| snapshot.host_id == browser_host)
            .expect("browser sink snapshot exists")
            .receipts;
        assert_eq!(browser_receipts.len(), 16);
        assert_eq!(pico.receipts().len(), 16);
        for sequence in 0..16 {
            let expected_level = sequence % 2 == 1;
            assert_eq!(stdout_receipts[sequence].sequence, sequence as u64);
            assert_eq!(browser_receipts[sequence].sequence, sequence as u64);
            assert_eq!(pico.receipts()[sequence].sequence, sequence as u64);
            assert_eq!(stdout_receipts[sequence].level, expected_level);
            assert_eq!(browser_receipts[sequence].level, expected_level);
            assert_eq!(pico.receipts()[sequence].level, expected_level);
            assert_eq!(browser_receipts[sequence].indicator_on, expected_level);
            assert_eq!(pico.receipts()[sequence].led_on, expected_level);
        }
        assert!(std_inspect(&mut std_host)
            .iter()
            .any(|observation| matches!(
                observation.kind,
                conduit_core::ObservationKind::PlanTerminal {
                    disposition: TerminalDisposition::Completed
                }
            )));
        assert!(page
            .host_mut(&browser_host)
            .expect("browser simulation exists")
            .inspect()
            .iter()
            .any(|observation| matches!(
                observation.kind,
                conduit_core::ObservationKind::PlanTerminal {
                    disposition: TerminalDisposition::Completed
                }
            )));
        assert!(pico_inspect(&mut pico).iter().any(|observation| matches!(
            observation.kind,
            conduit_core::ObservationKind::PlanTerminal {
                disposition: TerminalDisposition::Completed
            }
        )));
    }

    #[allow(clippy::too_many_arguments)]
    fn drive_triple_std_effect(
        std_host: &mut LegacyStdFixtureHost,
        page: &mut BrowserSimPage,
        pico: &mut PicoSim,
        connection_base_by_id: &BTreeMap<ConnectionId, ConnectionBase>,
        frame_fixture: &mut BoundedFrameRelayFixture,
        datagram_fixture: &mut BoundedDatagramRelayFixture,
        stdout_receipts: &mut Vec<SignalReceipt>,
        effect: PlatformEffect,
    ) -> Vec<(HostId, PlatformEffect)> {
        match effect {
            PlatformEffect::Wait {
                plan_id,
                placement_id,
                ..
            } => {
                let output = std_host.handle(HostCommand::CompleteWait {
                    plan_id,
                    placement_id,
                });
                triple_std_output_effects(std_host, page, pico, connection_base_by_id, output)
            }
            PlatformEffect::PresentValue {
                plan_id,
                active_play_id,
                presentation_id,
                placement_id,
                presentation_kind,
                value,
            } => {
                assert_eq!(
                    presentation_kind.as_str(),
                    conduit_signal::SIGNAL_PRESENTATION_KIND
                );
                let signal = conduit_signal::decode_signal(&value).expect("signal decodes");
                stdout_receipts.push(SignalReceipt {
                    placement_id: placement_id.clone(),
                    sequence: signal.sequence,
                    level: signal.level,
                });
                let output = std_host.handle(HostCommand::CompletePresentation {
                    plan_id,
                    active_play_id,
                    presentation_id,
                    placement_id,
                    value,
                    success: true,
                    message: None,
                });
                triple_std_output_effects(std_host, page, pico, connection_base_by_id, output)
            }
            PlatformEffect::TransmitConnection { envelope } => {
                match connection_base_by_id
                    .get(&envelope.connection_id)
                    .copied()
                    .expect("connection base exists")
                {
                    ConnectionBase::FixtureFrame => {
                        let decoded = frame_fixture
                            .transmit(&envelope)
                            .expect("relay accepts frame");
                        let sink_host_id = page
                            .host_for_inbound_connection(&decoded.plan_id, &decoded.connection_id)
                            .expect("browser inbound route exists");
                        let accepted = page
                            .host_mut(&sink_host_id)
                            .expect("browser simulation exists")
                            .handle(HostCommand::AcceptConnectionEnvelope(decoded.clone()));
                        super::pending_success(&accepted, decoded.sequence)
                            .expect("browser accepts frame");
                        let source_accepted =
                            std_host.handle(HostCommand::CompleteConnectionDelivery {
                                plan_id: decoded.plan_id,
                                connection_id: decoded.connection_id,
                                sequence: decoded.sequence,
                                outcome: ConnectionOutcome::Accepted,
                            });
                        let mut pending = accepted
                            .effects
                            .into_iter()
                            .map(|effect| (sink_host_id.clone(), effect))
                            .collect::<Vec<_>>();
                        pending.extend(triple_std_output_effects(
                            std_host,
                            page,
                            pico,
                            connection_base_by_id,
                            source_accepted,
                        ));
                        pending
                    }
                    ConnectionBase::FixtureDatagram => {
                        let decoded = datagram_fixture
                            .transmit(&envelope)
                            .expect("relay accepts datagram");
                        let accepted =
                            pico.handle(HostCommand::AcceptConnectionEnvelope(decoded.clone()));
                        pending_success(&accepted, decoded.sequence)
                            .expect("pico accepts datagram");
                        let source_accepted =
                            std_host.handle(HostCommand::CompleteConnectionDelivery {
                                plan_id: decoded.plan_id,
                                connection_id: decoded.connection_id,
                                sequence: decoded.sequence,
                                outcome: ConnectionOutcome::Accepted,
                            });
                        let mut pending = accepted
                            .effects
                            .into_iter()
                            .map(|effect| (pico.advertisement().host_id.clone(), effect))
                            .collect::<Vec<_>>();
                        pending.extend(triple_std_output_effects(
                            std_host,
                            page,
                            pico,
                            connection_base_by_id,
                            source_accepted,
                        ));
                        pending
                    }
                    ConnectionBase::Local
                    | ConnectionBase::InMemory
                    | ConnectionBase::WebSocket
                    | ConnectionBase::UsbCdc => {
                        panic!("triple remote transmit used unsupported base")
                    }
                }
            }
        }
    }

    fn drive_triple_browser_effect(
        std_host: &mut LegacyStdFixtureHost,
        page: &mut BrowserSimPage,
        pico: &mut PicoSim,
        connection_base_by_id: &BTreeMap<ConnectionId, ConnectionBase>,
        source_connection_by_sink: &BTreeMap<PlacementId, ConnectionId>,
        host_id: &HostId,
        effect: PlatformEffect,
    ) -> Vec<(HostId, PlatformEffect)> {
        match effect {
            PlatformEffect::PresentValue {
                plan_id,
                active_play_id,
                presentation_id,
                placement_id,
                presentation_kind,
                value,
            } => {
                assert_eq!(
                    presentation_kind.as_str(),
                    conduit_signal::SIGNAL_PRESENTATION_KIND
                );
                let signal = conduit_signal::decode_signal(&value).expect("signal decodes");
                let connection_id = source_connection_by_sink
                    .get(&placement_id)
                    .cloned()
                    .expect("browser sink connection exists");
                let output = page
                    .host_mut(host_id)
                    .expect("browser simulation exists")
                    .complete_dom_presentation(
                        plan_id.clone(),
                        active_play_id,
                        presentation_id,
                        placement_id,
                        value,
                    )
                    .expect("browser presentation completes");
                let delivered = std_host.handle(HostCommand::CompleteConnectionDelivery {
                    plan_id,
                    connection_id,
                    sequence: signal.sequence,
                    outcome: ConnectionOutcome::Delivered,
                });
                let mut pending = output
                    .effects
                    .into_iter()
                    .map(|effect| (host_id.clone(), effect))
                    .collect::<Vec<_>>();
                pending.extend(triple_std_output_effects(
                    std_host,
                    page,
                    pico,
                    connection_base_by_id,
                    delivered,
                ));
                pending
            }
            PlatformEffect::Wait { .. } | PlatformEffect::TransmitConnection { .. } => {
                panic!("browser triple sink should only present received values")
            }
        }
    }

    fn drive_triple_pico_effect(
        std_host: &mut LegacyStdFixtureHost,
        page: &mut BrowserSimPage,
        pico: &mut PicoSim,
        connection_base_by_id: &BTreeMap<ConnectionId, ConnectionBase>,
        source_connection_by_sink: &BTreeMap<PlacementId, ConnectionId>,
        effect: PlatformEffect,
    ) -> Vec<(HostId, PlatformEffect)> {
        match effect {
            PlatformEffect::PresentValue {
                plan_id,
                active_play_id,
                presentation_id,
                placement_id,
                presentation_kind,
                value,
            } => {
                assert_eq!(
                    presentation_kind.as_str(),
                    conduit_signal::SIGNAL_PRESENTATION_KIND
                );
                let signal = conduit_signal::decode_signal(&value).expect("signal decodes");
                let connection_id = source_connection_by_sink
                    .get(&placement_id)
                    .cloned()
                    .expect("pico sink connection exists");
                let output = pico
                    .complete_led_presentation(
                        plan_id.clone(),
                        active_play_id,
                        presentation_id,
                        placement_id,
                        value,
                    )
                    .expect("pico led presentation completes");
                let delivered = std_host.handle(HostCommand::CompleteConnectionDelivery {
                    plan_id,
                    connection_id,
                    sequence: signal.sequence,
                    outcome: ConnectionOutcome::Delivered,
                });
                let mut pending = output
                    .effects
                    .into_iter()
                    .map(|effect| (pico.advertisement().host_id.clone(), effect))
                    .collect::<Vec<_>>();
                pending.extend(triple_std_output_effects(
                    std_host,
                    page,
                    pico,
                    connection_base_by_id,
                    delivered,
                ));
                pending
            }
            PlatformEffect::Wait { .. } | PlatformEffect::TransmitConnection { .. } => {
                panic!("pico triple sink should only present received values")
            }
        }
    }

    fn triple_std_output_effects(
        std_host: &mut LegacyStdFixtureHost,
        page: &mut BrowserSimPage,
        pico: &mut PicoSim,
        connection_base_by_id: &BTreeMap<ConnectionId, ConnectionBase>,
        output: RuntimeOutput,
    ) -> Vec<(HostId, PlatformEffect)> {
        let mut pending = output
            .effects
            .into_iter()
            .map(|effect| (std_host.advertisement().host_id.clone(), effect))
            .collect::<Vec<_>>();
        for event in output.events {
            if let HostEvent::ConnectionTerminated {
                plan_id,
                connection_id,
                disposition,
            } = event
            {
                if matches!(disposition.disposition, TerminalDisposition::Completed) {
                    match connection_base_by_id.get(&connection_id).copied() {
                        Some(ConnectionBase::FixtureFrame) => {
                            if let Some(sink_host_id) =
                                page.host_for_inbound_connection(&plan_id, &connection_id)
                            {
                                let close = page
                                    .host_mut(&sink_host_id)
                                    .expect("browser simulation exists")
                                    .handle(HostCommand::CloseConnection {
                                        plan_id,
                                        connection_id,
                                    });
                                pending.extend(
                                    close
                                        .effects
                                        .into_iter()
                                        .map(|effect| (sink_host_id.clone(), effect)),
                                );
                            }
                        }
                        Some(ConnectionBase::FixtureDatagram) => {
                            let close = pico.handle(HostCommand::CloseConnection {
                                plan_id,
                                connection_id,
                            });
                            pending.extend(
                                close
                                    .effects
                                    .into_iter()
                                    .map(|effect| (pico.advertisement().host_id.clone(), effect)),
                            );
                        }
                        _ => {}
                    }
                }
            }
        }
        pending
    }

    fn pending_success(output: &RuntimeOutput, sequence: u64) -> Result<(), String> {
        output
            .events
            .iter()
            .find_map(|event| match event {
                HostEvent::ConnectionEnvelopeOutcome {
                    sequence: event_sequence,
                    outcome,
                    ..
                } if *event_sequence == sequence => Some(*outcome),
                _ => None,
            })
            .filter(|outcome| matches!(outcome, ConnectionOutcome::Accepted))
            .map_or_else(
                || Err(format!("missing accepted delivery for sequence {sequence}")),
                |_| Ok(()),
            )
    }

    fn std_inspect(std_host: &mut LegacyStdFixtureHost) -> Vec<conduit_core::Observation> {
        std_host
            .handle(HostCommand::Inspect)
            .events
            .into_iter()
            .find_map(|event| match event {
                HostEvent::Observations { items } => Some(items),
                _ => None,
            })
            .unwrap_or_default()
    }

    fn pico_inspect(pico: &mut PicoSim) -> Vec<conduit_core::Observation> {
        pico.handle(HostCommand::Inspect)
            .events
            .into_iter()
            .find_map(|event| match event {
                HostEvent::Observations { items } => Some(items),
                _ => None,
            })
            .unwrap_or_default()
    }

    fn drive_std_effect(
        std_host: &mut LegacyStdFixtureHost,
        page: &mut BrowserSimPage,
        relay: &mut BoundedFrameRelayFixture,
        effect: PlatformEffect,
    ) -> Vec<(HostId, PlatformEffect)> {
        match effect {
            PlatformEffect::Wait {
                plan_id,
                placement_id,
                ..
            } => {
                let output = std_host.handle(HostCommand::CompleteWait {
                    plan_id,
                    placement_id,
                });
                std_output_effects(std_host, page, output)
            }
            PlatformEffect::TransmitConnection { envelope } => {
                let decoded = relay.transmit(&envelope).expect("relay accepts frame");
                let sink_host_id = page
                    .host_for_inbound_connection(&decoded.plan_id, &decoded.connection_id)
                    .expect("browser inbound route exists");
                let accepted = page
                    .host_mut(&sink_host_id)
                    .expect("browser simulation exists")
                    .handle(HostCommand::AcceptConnectionEnvelope(decoded.clone()));
                super::pending_success(&accepted, decoded.sequence).expect("browser accepts frame");
                let source_accepted = std_host.handle(HostCommand::CompleteConnectionDelivery {
                    plan_id: decoded.plan_id,
                    connection_id: decoded.connection_id,
                    sequence: decoded.sequence,
                    outcome: conduit_core::ConnectionOutcome::Accepted,
                });
                let mut pending = accepted
                    .effects
                    .into_iter()
                    .map(|effect| (sink_host_id.clone(), effect))
                    .collect::<Vec<_>>();
                pending.extend(std_output_effects(std_host, page, source_accepted));
                pending
            }
            PlatformEffect::PresentValue { .. } => {
                panic!("std source-only fragment must not request presentation")
            }
        }
    }

    fn drive_browser_effect_with_std_ack(
        std_host: &mut LegacyStdFixtureHost,
        page: &mut BrowserSimPage,
        host_id: &HostId,
        effect: PlatformEffect,
    ) -> Vec<(HostId, PlatformEffect)> {
        match effect {
            PlatformEffect::PresentValue {
                plan_id,
                active_play_id,
                presentation_id,
                placement_id,
                presentation_kind,
                value,
            } => {
                assert_eq!(
                    presentation_kind.as_str(),
                    conduit_signal::SIGNAL_PRESENTATION_KIND
                );
                let output = page
                    .host_mut(host_id)
                    .expect("browser simulation exists")
                    .complete_dom_presentation(
                        plan_id,
                        active_play_id,
                        presentation_id,
                        placement_id,
                        value,
                    )
                    .expect("browser presentation completes");
                let mut pending = output
                    .effects
                    .into_iter()
                    .map(|effect| (host_id.clone(), effect))
                    .collect::<Vec<_>>();
                for event in output.events {
                    if let HostEvent::ManifestationCompleted {
                        plan_id,
                        placement_id,
                        value,
                        ..
                    } = event
                    {
                        let (source_host_id, connection_id) = page
                            .delivery_ack_routes
                            .get(&(plan_id.clone(), placement_id))
                            .cloned()
                            .expect("delivery ack route exists");
                        assert_eq!(source_host_id, std_host.advertisement().host_id);
                        let signal = conduit_signal::decode_signal(&value).expect("signal decodes");
                        let delivered = std_host.handle(HostCommand::CompleteConnectionDelivery {
                            plan_id,
                            connection_id,
                            sequence: signal.sequence,
                            outcome: conduit_core::ConnectionOutcome::Delivered,
                        });
                        pending.extend(std_output_effects(std_host, page, delivered));
                    }
                }
                pending
            }
            PlatformEffect::Wait { .. } | PlatformEffect::TransmitConnection { .. } => {
                panic!("browser sink fragment should only present received values")
            }
        }
    }

    fn std_output_effects(
        std_host: &mut LegacyStdFixtureHost,
        page: &mut BrowserSimPage,
        output: conduit_runtime::RuntimeOutput,
    ) -> Vec<(HostId, PlatformEffect)> {
        let mut pending = output
            .effects
            .into_iter()
            .map(|effect| (std_host.advertisement().host_id.clone(), effect))
            .collect::<Vec<_>>();
        for event in output.events {
            if let HostEvent::ConnectionTerminated {
                plan_id,
                connection_id,
                disposition,
            } = event
            {
                if matches!(disposition.disposition, TerminalDisposition::Completed) {
                    if let Some(sink_host_id) =
                        page.host_for_inbound_connection(&plan_id, &connection_id)
                    {
                        let close = page
                            .host_mut(&sink_host_id)
                            .expect("browser simulation exists")
                            .handle(HostCommand::CloseConnection {
                                plan_id,
                                connection_id,
                            });
                        pending.extend(
                            close
                                .effects
                                .into_iter()
                                .map(|effect| (sink_host_id.clone(), effect)),
                        );
                    }
                }
            }
        }
        pending
    }
}
