use conduit_core::{
    kind_id, ArtifactId, BootId, CapabilityId, CapabilityLimits, CapabilityOffer,
    ConnectionEnvelope, ConnectionId, ConnectionOutcome, ConnectionProvider, HostAdvertisement,
    HostCommand, HostEvent, HostId, HostProfileId, ImplementationId, Observation, OfferGeneration,
    PlacementId, Plan, PlanFragment, PlanId, PlatformEffect, TerminalDisposition, PROTOCOL_VERSION,
};
use conduit_form::CheckedForm;
use conduit_planner::{plan_with_connection_limits, PlacementChoice, PlacementChoices};
use conduit_runtime::{HostRuntime, RuntimeOutput};
use conduit_signal::{
    decode_signal, signal_profile_catalog, signal_registry, PULSE_KIND, SHOW_KIND,
    SIGNAL_PRESENTATION_KIND, SIGNAL_VALUE_KIND,
};
use std::collections::{BTreeMap, VecDeque};

#[derive(Debug, Clone)]
pub struct BoundedWebSocketRelay {
    max_payload_bytes: u32,
    max_frame_bytes: usize,
    frames: Vec<Vec<u8>>,
}

impl BoundedWebSocketRelay {
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
            .map_err(|err| format!("websocket relay encode failed: {err:?}"))?;
        if frame.len() > self.max_frame_bytes {
            return Err(format!(
                "websocket relay frame {} exceeds bound {}",
                frame.len(),
                self.max_frame_bytes
            ));
        }
        let decoded = conduit_wire::decode_envelope(&frame, self.max_payload_bytes)
            .map_err(|err| format!("websocket relay decode failed: {err:?}"))?;
        self.frames.push(frame);
        Ok(decoded)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserHostConfig {
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
pub struct BrowserHostSnapshot {
    pub host_id: HostId,
    pub boot_id: BootId,
    pub capabilities: Vec<CapabilityOffer>,
    pub receipts: Vec<BrowserReceipt>,
}

pub struct BrowserHost {
    runtime: HostRuntime,
    receipts: Vec<BrowserReceipt>,
}

impl BrowserHost {
    pub fn new(config: BrowserHostConfig) -> Self {
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

    fn complete_dom_presentation(
        &mut self,
        plan_id: conduit_core::PlanId,
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

    fn snapshot(&self) -> BrowserHostSnapshot {
        BrowserHostSnapshot {
            host_id: self.advertisement().host_id.clone(),
            boot_id: self.advertisement().boot_id.clone(),
            capabilities: self.advertisement().capabilities.clone(),
            receipts: self.receipts.clone(),
        }
    }
}

pub struct BrowserPage {
    hosts: BTreeMap<HostId, BrowserHost>,
    inbound_routes: BTreeMap<(PlanId, ConnectionId), HostId>,
    delivery_ack_routes: BTreeMap<(PlanId, PlacementId), (HostId, ConnectionId)>,
}

impl BrowserPage {
    pub fn with_hosts(configs: impl IntoIterator<Item = BrowserHostConfig>) -> Self {
        Self {
            hosts: configs
                .into_iter()
                .map(|config| {
                    let host = BrowserHost::new(config);
                    (host.advertisement().host_id.clone(), host)
                })
                .collect(),
            inbound_routes: BTreeMap::new(),
            delivery_ack_routes: BTreeMap::new(),
        }
    }

    pub fn host_snapshots(&self) -> Vec<BrowserHostSnapshot> {
        self.hosts
            .values()
            .map(BrowserHost::snapshot)
            .collect::<Vec<_>>()
    }

    pub fn advertisements(&self) -> Vec<HostAdvertisement> {
        self.hosts
            .values()
            .map(|host| host.advertisement().clone())
            .collect()
    }

    pub fn plan_pair(
        &self,
        form: &CheckedForm,
        source_host: &HostId,
        sink_host: &HostId,
    ) -> Result<Plan, Box<dyn std::error::Error>> {
        let placements = PlacementChoices {
            by_operation: BTreeMap::from([
                (
                    conduit_core::OperationId::from("pulse"),
                    PlacementChoice {
                        host_id: source_host.clone(),
                        capability_id: CapabilityId::from("pulse"),
                    },
                ),
                (
                    conduit_core::OperationId::from("show"),
                    PlacementChoice {
                        host_id: sink_host.clone(),
                        capability_id: CapabilityId::from("dom-show"),
                    },
                ),
            ]),
        };
        Ok(plan_with_connection_limits(
            form,
            &self.advertisements(),
            &placements,
            &[ConnectionProvider::InMemory],
            4,
            64,
        )?)
    }

    pub fn plan_std_to_browser(
        &self,
        form: &CheckedForm,
        std_advertisement: &HostAdvertisement,
        browser_host: &HostId,
    ) -> Result<Plan, Box<dyn std::error::Error>> {
        let placements = PlacementChoices {
            by_operation: BTreeMap::from([
                (
                    conduit_core::OperationId::from("pulse"),
                    PlacementChoice {
                        host_id: std_advertisement.host_id.clone(),
                        capability_id: CapabilityId::from("pulse-1"),
                    },
                ),
                (
                    conduit_core::OperationId::from("show"),
                    PlacementChoice {
                        host_id: browser_host.clone(),
                        capability_id: CapabilityId::from("dom-show"),
                    },
                ),
            ]),
        };
        let mut realm = Vec::with_capacity(self.hosts.len() + 1);
        realm.push(std_advertisement.clone());
        realm.extend(self.advertisements());
        Ok(plan_with_connection_limits(
            form,
            &realm,
            &placements,
            &[ConnectionProvider::WebSocket],
            4,
            64,
        )?)
    }

    pub fn run_plan(&mut self, plan: Plan) -> Result<BrowserRunReport, String> {
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
                .handle(HostCommand::Activate(fragment.plan_id.clone()));
            ensure_activated(&output)?;
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
            .flat_map(BrowserHost::inspect)
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
                placement_id,
                presentation_kind,
                value,
            } => {
                if presentation_kind.as_str() != SIGNAL_PRESENTATION_KIND {
                    return Err(format!(
                        "browser host cannot manifest presentation kind '{}'",
                        presentation_kind.as_str()
                    ));
                }
                let output = self.host_mut(host_id)?.complete_dom_presentation(
                    plan_id,
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
                            "no browser host has inbound connection '{}'",
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

    fn host_mut(&mut self, host_id: &HostId) -> Result<&mut BrowserHost, String> {
        self.hosts
            .get_mut(host_id)
            .ok_or_else(|| format!("unknown browser host '{}'", host_id.as_str()))
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
    pub snapshots: Vec<BrowserHostSnapshot>,
}

pub fn load_checked_form(path: &str) -> Result<CheckedForm, Box<dyn std::error::Error>> {
    Ok(conduit_form::parse(
        &std::fs::read_to_string(path)?,
        &signal_profile_catalog(),
    )?)
}

fn browser_advertisement(config: BrowserHostConfig) -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: config.host_id,
        boot_id: config.boot_id,
        offer_generation: config.offer_generation,
        profile: HostProfileId::from("browser/wasm-page-v1"),
        capabilities: vec![
            CapabilityOffer {
                capability_id: CapabilityId::from("pulse"),
                kind_id: kind_id(PULSE_KIND),
                implementation_id: ImplementationId::from("browser/pulse-v1"),
                artifact_id: ArtifactId::from("conduit-signal/pulse-artifact-v1"),
                limits: CapabilityLimits {
                    value_kind: kind_id(SIGNAL_VALUE_KIND),
                    max_active_instances: 16,
                    max_queue_items: 4,
                    max_queue_bytes: 64,
                },
            },
            CapabilityOffer {
                capability_id: CapabilityId::from("dom-show"),
                kind_id: kind_id(SHOW_KIND),
                implementation_id: ImplementationId::from("browser/dom-show-signal-v1"),
                artifact_id: ArtifactId::from("conduit-signal/show-artifact-v1"),
                limits: CapabilityLimits {
                    value_kind: kind_id(SIGNAL_VALUE_KIND),
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
                (is_remote_provider(connection.provider) && has_sink && !has_source).then(|| {
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
                (is_remote_provider(connection.provider) && has_source && !has_sink).then(|| {
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

fn is_remote_provider(provider: ConnectionProvider) -> bool {
    matches!(
        provider,
        ConnectionProvider::InMemory | ConnectionProvider::WebSocket
    )
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
        .ok_or_else(|| format!("remote browser host did not accept sequence {sequence}"))
}

fn ensure_prepared(output: &RuntimeOutput) -> Result<(), String> {
    output
        .events
        .iter()
        .any(|event| matches!(event, HostEvent::Prepared { .. }))
        .then_some(())
        .ok_or_else(|| format!("browser host prepare failed: {:?}", output.events))
}

fn ensure_activated(output: &RuntimeOutput) -> Result<(), String> {
    output
        .events
        .iter()
        .any(|event| matches!(event, HostEvent::Activated { .. }))
        .then_some(())
        .ok_or_else(|| format!("browser host activation failed: {:?}", output.events))
}

#[cfg(test)]
mod tests {
    use super::{BoundedWebSocketRelay, BrowserHostConfig, BrowserPage};
    use conduit_core::{
        kind_id, BootId, ConnectionProvider, HostCommand, HostEvent, HostId, OfferGeneration,
        PlatformEffect, TerminalDisposition,
    };
    use conduit_form::parse;
    use conduit_signal::{signal_profile_catalog, PULSE_KIND, SHOW_KIND};
    use conduit_std_host::{StdHost, StdHostConfig};
    use std::collections::VecDeque;

    fn page() -> BrowserPage {
        BrowserPage::with_hosts([
            BrowserHostConfig {
                host_id: HostId::from("browser-host-a"),
                boot_id: BootId::from("browser-boot-a"),
                offer_generation: OfferGeneration(1),
            },
            BrowserHostConfig {
                host_id: HostId::from("browser-host-b"),
                boot_id: BootId::from("browser-boot-b"),
                offer_generation: OfferGeneration(1),
            },
        ])
    }

    fn pair_form() -> conduit_form::CheckedForm {
        parse(
            "form 0\n\nsignal-demo {\n pulse: flow/pulse\n show: display/show\n pulse.count = 16\n pulse.period-ms = 250\n pulse.initial = false\n pulse > show\n}\n",
            &signal_profile_catalog(),
        )
        .expect("browser pair form parses")
    }

    #[test]
    fn one_page_owns_multiple_independent_browser_host_instances() {
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
                    && offer.implementation_id.as_str() == "browser/pulse-v1"));
            assert!(snapshot
                .capabilities
                .iter()
                .any(|offer| offer.kind_id == kind_id(SHOW_KIND)
                    && offer.implementation_id.as_str() == "browser/dom-show-signal-v1"));
        }
    }

    #[test]
    fn two_browser_hosts_execute_pair_form_over_bounded_memory_link() {
        let mut page = page();
        let source_host = HostId::from("browser-host-a");
        let sink_host = HostId::from("browser-host-b");
        let plan = page
            .plan_pair(&pair_form(), &source_host, &sink_host)
            .expect("browser pair plan resolves");
        assert_eq!(plan.fragments.len(), 2);
        let connection = plan
            .fragments
            .iter()
            .flat_map(|fragment| &fragment.connections)
            .find(|connection| connection.provider == ConnectionProvider::InMemory)
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
    fn std_host_sends_signal_to_browser_over_bounded_websocket_relay() {
        let mut std_host = StdHost::new_with_config(StdHostConfig {
            host_id: HostId::from("std-host-1"),
            boot_id: BootId::from("std-boot-1"),
            offer_generation: OfferGeneration(1),
        });
        let mut page = BrowserPage::with_hosts([BrowserHostConfig {
            host_id: HostId::from("browser-host-web"),
            boot_id: BootId::from("browser-boot-web"),
            offer_generation: OfferGeneration(1),
        }]);
        let browser_host = HostId::from("browser-host-web");
        let plan = page
            .plan_std_to_browser(&pair_form(), std_host.advertisement(), &browser_host)
            .expect("std-to-browser plan resolves");
        let connection = plan
            .fragments
            .iter()
            .flat_map(|fragment| &fragment.connections)
            .find(|connection| connection.provider == ConnectionProvider::WebSocket)
            .expect("websocket connection is planned");
        assert_eq!(connection.item_capacity, 4);
        assert_eq!(connection.byte_capacity, 64);

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
                        .expect("browser host exists")
                        .handle(HostCommand::Prepare(fragment.clone())),
                )
                .expect("browser sink prepares");
            }
        }

        let mut pending = VecDeque::new();
        for fragment in super::sink_fragments_first(&plan.fragments) {
            if fragment.host_id == std_host.advertisement().host_id {
                let output = std_host.handle(HostCommand::Activate(fragment.plan_id.clone()));
                super::ensure_activated(&output).expect("std activates");
                pending.extend(
                    output
                        .effects
                        .into_iter()
                        .map(|effect| (fragment.host_id.clone(), effect)),
                );
            } else {
                let output = page
                    .host_mut(&fragment.host_id)
                    .expect("browser host exists")
                    .handle(HostCommand::Activate(fragment.plan_id.clone()));
                super::ensure_activated(&output).expect("browser activates");
                pending.extend(
                    output
                        .effects
                        .into_iter()
                        .map(|effect| (fragment.host_id.clone(), effect)),
                );
            }
        }

        let mut relay = BoundedWebSocketRelay::new(64, 512);
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
            .expect("browser host exists")
            .inspect();
        assert!(observations.iter().any(|observation| matches!(
            observation.kind,
            conduit_core::ObservationKind::PlanTerminal {
                disposition: TerminalDisposition::Completed
            }
        )));
    }

    fn drive_std_effect(
        std_host: &mut StdHost,
        page: &mut BrowserPage,
        relay: &mut BoundedWebSocketRelay,
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
                    .expect("browser host exists")
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
        std_host: &mut StdHost,
        page: &mut BrowserPage,
        host_id: &HostId,
        effect: PlatformEffect,
    ) -> Vec<(HostId, PlatformEffect)> {
        match effect {
            PlatformEffect::PresentValue {
                plan_id,
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
                    .expect("browser host exists")
                    .complete_dom_presentation(plan_id, placement_id, value)
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
        std_host: &mut StdHost,
        page: &mut BrowserPage,
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
                            .expect("browser host exists")
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
