#![cfg_attr(not(feature = "compatibility-fixture"), no_std)]

extern crate alloc;

use alloc::format;
use alloc::string::String;
#[cfg(feature = "compatibility-fixture")]
use alloc::vec;
use alloc::vec::Vec;
use conduit_core::ConnectionEnvelope;

#[cfg(feature = "compatibility-fixture")]
use conduit_core::{
    kind_id, ArtifactId, BootId, CapabilityId, CapabilityLimits, CapabilityOffer,
    HostAdvertisement, HostId, HostProfileId, ImplementationId, OfferGeneration, PlacementId,
    PROTOCOL_VERSION,
};
#[cfg(feature = "compatibility-fixture")]
use conduit_signal::{
    decode_signal, pulse_contract_revision, pulse_execution_profile,
    pulse_host_operation_requirements, pulse_outputs, pulse_resource_requirements,
    show_contract_revision, show_execution_profile, show_host_operation_requirements, show_inputs,
    show_resource_requirements, signal_resource_offers, PULSE_KIND, SHOW_KIND,
};

#[cfg(feature = "compatibility-fixture")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PicoSimConfig {
    pub host_id: HostId,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
}

#[cfg(feature = "compatibility-fixture")]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LedReceipt {
    pub host_id: HostId,
    pub placement_id: PlacementId,
    pub sequence: u64,
    pub level: bool,
    pub led_on: bool,
}

#[derive(Debug, Clone)]
pub struct BoundedDatagramRelayFixture {
    max_payload_bytes: u32,
    max_datagram_bytes: usize,
    datagrams: Vec<Vec<u8>>,
}

impl BoundedDatagramRelayFixture {
    pub fn new(max_payload_bytes: u32, max_datagram_bytes: usize) -> Self {
        Self {
            max_payload_bytes,
            max_datagram_bytes,
            datagrams: Vec::new(),
        }
    }

    pub fn datagrams(&self) -> &[Vec<u8>] {
        &self.datagrams
    }

    pub fn transmit(
        &mut self,
        envelope: &ConnectionEnvelope,
    ) -> Result<ConnectionEnvelope, String> {
        let datagram = conduit_wire::encode_envelope(envelope, self.max_payload_bytes)
            .map_err(|err| format!("datagram fixture encode failed: {err:?}"))?;
        if datagram.len() > self.max_datagram_bytes {
            return Err(format!(
                "datagram fixture datagram {} exceeds bound {}",
                datagram.len(),
                self.max_datagram_bytes
            ));
        }
        let decoded = conduit_wire::decode_envelope(&datagram, self.max_payload_bytes)
            .map_err(|err| format!("datagram fixture decode failed: {err:?}"))?;
        self.datagrams.push(datagram);
        Ok(decoded)
    }
}

#[cfg(feature = "compatibility-fixture")]
pub fn pico_advertisement(config: PicoSimConfig) -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: config.host_id,
        boot_id: config.boot_id,
        offer_generation: config.offer_generation,
        profile: HostProfileId::from("pico-w"),
        resources: signal_resource_offers("pico/timer", "pico/presentation", 1),
        planner_capabilities: vec![],
        capabilities: vec![
            CapabilityOffer {
                startup_parameters: conduit_signal::pulse_face_startup_parameters(),
                shorthand: None,
                capability_id: CapabilityId::from("pico-pulse"),
                kind_id: kind_id(PULSE_KIND),
                kind_contract_revision: pulse_contract_revision(),
                implementation: conduit_core::ImplementationOffer {
                    execution_profile_id: pulse_execution_profile(),
                    implementation_id: ImplementationId::from("pico/pulse-v1"),
                    artifact_id: ArtifactId::from("conduit-signal/pulse-artifact-v1"),
                },
                inputs: vec![],
                outputs: pulse_outputs(),
                host_operations: pulse_host_operation_requirements(),
                resource_requirements: pulse_resource_requirements(),
                authority_requirements: vec![],
                limits: CapabilityLimits {
                    max_active_instances: 1,
                    max_queue_items: 4,
                    max_queue_bytes: 64,
                },
            },
            CapabilityOffer {
                startup_parameters: vec![],
                shorthand: None,
                capability_id: CapabilityId::from("onboard-led"),
                kind_id: kind_id(SHOW_KIND),
                kind_contract_revision: show_contract_revision(),
                implementation: conduit_core::ImplementationOffer {
                    execution_profile_id: show_execution_profile(),
                    implementation_id: ImplementationId::from("pico/onboard-led-show-signal-v1"),
                    artifact_id: ArtifactId::from("conduit-signal/show-artifact-v1"),
                },
                inputs: show_inputs(),
                outputs: vec![],
                host_operations: show_host_operation_requirements(),
                resource_requirements: show_resource_requirements(),
                authority_requirements: vec![],
                limits: CapabilityLimits {
                    max_active_instances: 1,
                    max_queue_items: 4,
                    max_queue_bytes: 64,
                },
            },
        ],
    }
}

#[cfg(feature = "compatibility-fixture")]
pub fn led_receipt(
    host_id: HostId,
    placement_id: PlacementId,
    value: &conduit_core::ValuePayload,
) -> Result<LedReceipt, conduit_signal::SignalProfileError> {
    let signal = decode_signal(value)?;
    Ok(LedReceipt {
        host_id,
        placement_id,
        sequence: signal.sequence,
        level: signal.level,
        led_on: signal.level,
    })
}

#[cfg(feature = "compatibility-fixture")]
mod std_fixture {
    use super::{led_receipt, pico_advertisement, LedReceipt, PicoSimConfig};
    use alloc::vec::Vec;
    use conduit_core::{
        process_owned_line_offer, CapabilityId, ConnectionBase, HostAdvertisement, HostCommand,
        HostEvent, HostId, ImplementationId, Observation, PlacementId, Plan, PlanFragment,
        PlatformEffect,
    };
    use conduit_form::CheckedForm;
    use conduit_planner::{plan, plan_with_line_offers, PlacementChoice, PlacementChoices};
    use conduit_runtime::{HostRuntime, RuntimeOutput};
    use conduit_signal::{signal_registry, SIGNAL_PRESENTATION_KIND};
    use std::collections::BTreeMap;

    pub struct PicoSim {
        runtime: HostRuntime,
        receipts: Vec<LedReceipt>,
    }

    impl PicoSim {
        pub fn new(config: PicoSimConfig) -> Self {
            let advertisement = pico_advertisement(config);
            let registry = signal_registry(
                ImplementationId::from("pico/pulse-v1"),
                ImplementationId::from("pico/onboard-led-show-signal-v1"),
            )
            .expect("pico signal implementations have unique identities");
            Self {
                runtime: HostRuntime::new(advertisement, registry, 64),
                receipts: Vec::new(),
            }
        }

        pub fn advertisement(&self) -> &HostAdvertisement {
            self.runtime.advertisement()
        }

        pub fn receipts(&self) -> &[LedReceipt] {
            &self.receipts
        }

        pub fn handle(&mut self, command: HostCommand) -> RuntimeOutput {
            self.runtime.handle(command)
        }

        pub fn replace_line_offers(&mut self, lines: Vec<conduit_core::LineOffer>) {
            self.runtime.replace_line_offers(lines);
        }

        pub fn plan_local(&self, form: &CheckedForm) -> Result<Plan, Box<dyn std::error::Error>> {
            let placements = PlacementChoices {
                by_gear: BTreeMap::from([
                    (
                        conduit_core::GearId::from("signal-demo/pulse"),
                        PlacementChoice {
                            host_id: self.advertisement().host_id.clone(),
                            capability_id: CapabilityId::from("pico-pulse"),
                        },
                    ),
                    (
                        conduit_core::GearId::from("signal-demo/show"),
                        PlacementChoice {
                            host_id: self.advertisement().host_id.clone(),
                            capability_id: CapabilityId::from("onboard-led"),
                        },
                    ),
                ]),
            };
            Ok(plan(
                form,
                &[self.advertisement().clone()],
                &placements,
                &[ConnectionBase::Local],
            )?)
        }

        pub fn plan_std_to_pico(
            &self,
            form: &CheckedForm,
            std_advertisement: &HostAdvertisement,
        ) -> Result<Plan, Box<dyn std::error::Error>> {
            let placements = PlacementChoices {
                by_gear: BTreeMap::from([
                    (
                        conduit_core::GearId::from("signal-demo/pulse"),
                        PlacementChoice {
                            host_id: std_advertisement.host_id.clone(),
                            capability_id: CapabilityId::from("pulse-1"),
                        },
                    ),
                    (
                        conduit_core::GearId::from("signal-demo/show"),
                        PlacementChoice {
                            host_id: self.advertisement().host_id.clone(),
                            capability_id: CapabilityId::from("onboard-led"),
                        },
                    ),
                ]),
            };
            let lines = [process_owned_line_offer(
                "line/std-pico",
                "link/std-pico",
                ConnectionBase::FixtureDatagram,
                "fixture/datagram/std-pico",
                std_advertisement,
                self.advertisement(),
                4,
                64,
            )];
            Ok(plan_with_line_offers(
                form,
                &[std_advertisement.clone(), self.advertisement().clone()],
                &placements,
                &[ConnectionBase::FixtureDatagram],
                4,
                64,
                &lines,
            )?)
        }

        pub fn complete_led_presentation(
            &mut self,
            plan_id: conduit_core::PlanId,
            active_play_id: conduit_core::ActivePlayId,
            presentation_id: conduit_core::PresentationId,
            placement_id: PlacementId,
            value: conduit_core::ValuePayload,
        ) -> Result<RuntimeOutput, String> {
            let receipt = led_receipt(
                self.advertisement().host_id.clone(),
                placement_id.clone(),
                &value,
            )
            .map_err(|err| err.to_string())?;
            self.receipts.push(receipt);
            Ok(self.runtime.handle(HostCommand::CompletePresentation {
                plan_id,
                active_play_id,
                presentation_id,
                placement_id,
                value,
                success: true,
                message: None,
            }))
        }

        pub fn run_fragment(&mut self, fragment: PlanFragment) -> Result<Vec<Observation>, String> {
            let prepare = self.runtime.handle(HostCommand::Prepare(fragment.clone()));
            ensure_prepared(&prepare)?;
            let triggerd = self
                .runtime
                .handle(HostCommand::StartPlay(fragment.plan_id.clone()));
            ensure_triggerd(&triggerd)?;

            let mut pending = triggerd.effects;
            while let Some(effect) = pending.pop() {
                let follow_up = match effect {
                    PlatformEffect::Wait {
                        plan_id,
                        placement_id,
                        ..
                    } => self.runtime.handle(HostCommand::CompleteWait {
                        plan_id,
                        placement_id,
                    }),
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
                                "Pico simulation cannot manifest presentation kind '{}'",
                                presentation_kind.as_str()
                            ));
                        }
                        self.complete_led_presentation(
                            plan_id,
                            active_play_id,
                            presentation_id,
                            placement_id,
                            value,
                        )?
                    }
                    PlatformEffect::TransmitConnection { .. } => {
                        return Err("pico local fixture must not use remote transport".to_string());
                    }
                };
                pending.extend(follow_up.effects.into_iter().rev());
            }

            Ok(inspect_observations(&mut self.runtime))
        }
    }

    fn ensure_prepared(output: &RuntimeOutput) -> Result<(), String> {
        output
            .events
            .iter()
            .find_map(|event| match event {
                HostEvent::PreparationRejected {
                    reason, message, ..
                } => Some(message.clone().unwrap_or_else(|| format!("{reason:?}"))),
                _ => None,
            })
            .map_or(Ok(()), Err)
    }

    fn ensure_triggerd(output: &RuntimeOutput) -> Result<(), String> {
        output
            .events
            .iter()
            .find_map(|event| match event {
                HostEvent::PlayStartRejected {
                    reason, message, ..
                } => Some(message.clone().unwrap_or_else(|| format!("{reason:?}"))),
                _ => None,
            })
            .map_or(Ok(()), Err)
    }

    fn inspect_observations(runtime: &mut HostRuntime) -> Vec<Observation> {
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

    #[allow(dead_code)]
    fn _assert_ids_are_distinct(host_id: &HostId, placement_id: &PlacementId) {
        let _ = (host_id, placement_id);
    }
}

#[cfg(feature = "compatibility-fixture")]
pub use std_fixture::PicoSim;

#[cfg(all(test, feature = "compatibility-fixture"))]
mod tests {
    use super::{pico_advertisement, BoundedDatagramRelayFixture, PicoSim, PicoSimConfig};
    use conduit_core::{
        BootId, ConnectionBase, ConnectionId, ConnectionOutcome, HostCommand, HostEvent, HostId,
        OfferGeneration, PlatformEffect, TerminalDisposition,
    };
    use conduit_form::parse_with_startup;
    use conduit_runtime::RuntimeOutput;
    use conduit_signal::signal_profile_catalog;
    use conduit_std_host::{LegacyStdFixtureHost, StdHostConfig};
    use std::collections::VecDeque;

    #[test]
    fn constrained_advertisement_names_pico_led_without_transport_claims() {
        let advertisement = pico_advertisement(PicoSimConfig {
            host_id: HostId::from("pico-1"),
            boot_id: BootId::from("boot-pico-1"),
            offer_generation: OfferGeneration(1),
        });
        assert_eq!(advertisement.profile.as_str(), "pico-w");
        assert_eq!(advertisement.capabilities.len(), 2);
        assert!(advertisement
            .capabilities
            .iter()
            .any(|offer| offer.capability_id.as_str() == "onboard-led"));
        assert!(advertisement
            .capabilities
            .iter()
            .all(|offer| offer.limits.max_active_instances == 1));
    }

    #[test]
    fn pico_simulation_runs_pair_form_to_onboard_led_receipts() {
        let form = parse_with_startup(
            include_str!("../../../fixtures/forms/signal-demo.conduit"),
            &conduit_signal::signal_startup_catalog(),
            &signal_profile_catalog(),
        )
        .expect("signal form parses");
        let mut host = PicoSim::new(PicoSimConfig {
            host_id: HostId::from("pico-1"),
            boot_id: BootId::from("boot-pico-1"),
            offer_generation: OfferGeneration(1),
        });
        let plan = host.plan_local(&form).expect("pico local plan resolves");
        let fragment = plan
            .fragments
            .iter()
            .find(|fragment| fragment.host_id == host.advertisement().host_id)
            .expect("pico fragment exists")
            .clone();
        let connection = fragment
            .connections
            .iter()
            .find(|connection| connection.selected_line.is_none())
            .expect("pico local connection is planned");
        assert_eq!(connection.item_capacity, 4);
        assert_eq!(connection.byte_capacity, 64);
        let observations = host.run_fragment(fragment).expect("pico run completes");
        assert_eq!(host.receipts().len(), 16);
        assert_eq!(host.receipts()[0].sequence, 0);
        assert!(!host.receipts()[0].level);
        assert!(!host.receipts()[0].led_on);
        assert_eq!(host.receipts()[15].sequence, 15);
        assert!(host.receipts()[15].level);
        assert!(host.receipts()[15].led_on);
        assert!(observations.iter().any(|observation| matches!(
            observation.kind,
            conduit_core::ObservationKind::PlanTerminal {
                disposition: TerminalDisposition::Completed
            }
        )));
    }

    #[test]
    fn std_host_sends_signal_to_pico_through_bounded_datagram_fixture() {
        let form = parse_with_startup(
            include_str!("../../../fixtures/forms/signal-demo.conduit"),
            &conduit_signal::signal_startup_catalog(),
            &signal_profile_catalog(),
        )
        .expect("signal form parses");
        let mut std_host = LegacyStdFixtureHost::new_with_config(StdHostConfig {
            host_id: HostId::from("std-host-1"),
            boot_id: BootId::from("std-boot-1"),
            offer_generation: OfferGeneration(1),
        });
        let mut pico = PicoSim::new(PicoSimConfig {
            host_id: HostId::from("pico-sim-datagram-1"),
            boot_id: BootId::from("pico-sim-boot-datagram-1"),
            offer_generation: OfferGeneration(1),
        });
        let plan = pico
            .plan_std_to_pico(&form, std_host.advertisement())
            .expect("std-to-pico plan resolves");
        let connection = plan
            .fragments
            .iter()
            .flat_map(|fragment| &fragment.connections)
            .find(|connection| {
                connection
                    .selected_line
                    .as_ref()
                    .map(|line| line.binding.base)
                    == Some(ConnectionBase::FixtureDatagram)
            })
            .expect("datagram fixture connection is planned");
        assert_eq!(connection.item_capacity, 4);
        assert_eq!(connection.byte_capacity, 64);
        let connection_id = connection.connection_id.clone();
        let line = connection
            .selected_line
            .clone()
            .expect("remote datagram connection seals a Line");
        let line_offer = conduit_core::LineOffer {
            line_id: line.line_id.clone(),
            binding: conduit_core::LinkBinding {
                binding_id: line.binding.binding_id.clone(),
                source: line.binding.source.clone(),
                sink: line.binding.sink.clone(),
                base: line.binding.base,
                base_instance_id: line.binding.base_instance_id.clone(),
                credential: line.binding.credential.clone(),
                authority: line.binding.authority.clone(),
                limits: line.binding.limits,
            },
            contract: line.contract,
            availability: conduit_core::LineAvailabilitySign {
                line_id: line.line_id,
                binding_id: line.binding.binding_id,
                availability: conduit_core::LineAvailability::Ready,
                sign_id: conduit_core::SignId::from("fixture/pico-line-ready"),
            },
        };
        std_host.replace_line_offers(vec![line_offer.clone()]);
        pico.replace_line_offers(vec![line_offer]);

        for fragment in &plan.fragments {
            if fragment.host_id == std_host.advertisement().host_id {
                ensure_prepared(&std_host.handle(HostCommand::Prepare(fragment.clone())))
                    .expect("std source prepares");
            } else {
                ensure_prepared(&pico.handle(HostCommand::Prepare(fragment.clone())))
                    .expect("pico sink prepares");
            }
        }

        let mut pending = VecDeque::new();
        for fragment in sink_fragments_first(&plan.fragments) {
            let output = if fragment.host_id == std_host.advertisement().host_id {
                std_host.handle(HostCommand::StartPlay(fragment.plan_id.clone()))
            } else {
                pico.handle(HostCommand::StartPlay(fragment.plan_id.clone()))
            };
            ensure_triggerd(&output).expect("fragment triggers");
            pending.extend(
                output
                    .effects
                    .into_iter()
                    .map(|effect| (fragment.host_id.clone(), effect)),
            );
        }

        let mut relay = BoundedDatagramRelayFixture::new(64, 512);
        while let Some((host_id, effect)) = pending.pop_front() {
            if host_id == std_host.advertisement().host_id {
                pending.extend(drive_std_effect(
                    &mut std_host,
                    &mut pico,
                    &mut relay,
                    effect,
                ));
            } else {
                pending.extend(drive_pico_effect_with_std_ack(
                    &mut std_host,
                    &mut pico,
                    &connection_id,
                    effect,
                ));
            }
        }

        assert_eq!(relay.datagrams().len(), 16);
        assert_eq!(pico.receipts().len(), 16);
        assert_eq!(pico.receipts()[0].sequence, 0);
        assert!(!pico.receipts()[0].level);
        assert!(!pico.receipts()[0].led_on);
        assert_eq!(pico.receipts()[15].sequence, 15);
        assert!(pico.receipts()[15].level);
        assert!(pico.receipts()[15].led_on);
        let observations = inspect(&mut pico);
        assert!(observations.iter().any(|observation| matches!(
            observation.kind,
            conduit_core::ObservationKind::PlanTerminal {
                disposition: TerminalDisposition::Completed
            }
        )));
    }

    fn drive_std_effect(
        std_host: &mut LegacyStdFixtureHost,
        pico: &mut PicoSim,
        relay: &mut BoundedDatagramRelayFixture,
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
                std_output_effects(std_host, pico, output)
            }
            PlatformEffect::TransmitConnection { envelope } => {
                let decoded = relay.transmit(&envelope).expect("relay accepts datagram");
                let accepted = pico.handle(HostCommand::AcceptConnectionEnvelope(decoded.clone()));
                pending_success(&accepted, decoded.sequence).expect("pico accepts datagram");
                let source_accepted = std_host.handle(HostCommand::CompleteConnectionDelivery {
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
                pending.extend(std_output_effects(std_host, pico, source_accepted));
                pending
            }
            PlatformEffect::PresentValue { .. } => {
                panic!("std source-only fragment must not request presentation")
            }
        }
    }

    fn drive_pico_effect_with_std_ack(
        std_host: &mut LegacyStdFixtureHost,
        pico: &mut PicoSim,
        connection_id: &ConnectionId,
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
                    connection_id: connection_id.clone(),
                    sequence: signal.sequence,
                    outcome: ConnectionOutcome::Delivered,
                });
                let mut pending = output
                    .effects
                    .into_iter()
                    .map(|effect| (pico.advertisement().host_id.clone(), effect))
                    .collect::<Vec<_>>();
                pending.extend(std_output_effects(std_host, pico, delivered));
                pending
            }
            PlatformEffect::Wait { .. } | PlatformEffect::TransmitConnection { .. } => {
                panic!("pico sink fragment should only present received values")
            }
        }
    }

    fn std_output_effects(
        std_host: &mut LegacyStdFixtureHost,
        pico: &mut PicoSim,
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
            }
        }
        pending
    }

    fn sink_fragments_first(
        fragments: &[conduit_core::PlanFragment],
    ) -> Vec<conduit_core::PlanFragment> {
        let mut sorted = fragments.to_vec();
        sorted.sort_by_key(|fragment| {
            fragment.connections.iter().any(|connection| {
                connection
                    .selected_line
                    .as_ref()
                    .map(|line| line.binding.base)
                    == Some(ConnectionBase::FixtureDatagram)
            }) && fragment
                .placements
                .iter()
                .all(|placement| placement.outputs.is_empty())
        });
        sorted.reverse();
        sorted
    }

    fn ensure_prepared(output: &RuntimeOutput) -> Result<(), String> {
        output
            .events
            .iter()
            .find_map(|event| match event {
                HostEvent::PreparationRejected {
                    reason, message, ..
                } => Some(message.clone().unwrap_or_else(|| format!("{reason:?}"))),
                _ => None,
            })
            .map_or(Ok(()), Err)
    }

    fn ensure_triggerd(output: &RuntimeOutput) -> Result<(), String> {
        output
            .events
            .iter()
            .find_map(|event| match event {
                HostEvent::PlayStartRejected {
                    reason, message, ..
                } => Some(message.clone().unwrap_or_else(|| format!("{reason:?}"))),
                _ => None,
            })
            .map_or(Ok(()), Err)
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

    fn inspect(pico: &mut PicoSim) -> Vec<conduit_core::Observation> {
        pico.handle(HostCommand::Inspect)
            .events
            .into_iter()
            .find_map(|event| match event {
                HostEvent::Observations { items } => Some(items),
                _ => None,
            })
            .unwrap_or_default()
    }
}
