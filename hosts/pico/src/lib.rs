#![cfg_attr(not(feature = "std"), no_std)]

extern crate alloc;

use alloc::vec;
use conduit_core::{
    kind_id, ArtifactId, BootId, CapabilityId, CapabilityLimits, CapabilityOffer,
    HostAdvertisement, HostId, HostProfileId, ImplementationId, OfferGeneration, PlacementId,
    PROTOCOL_VERSION,
};
use conduit_signal::{decode_signal, PULSE_KIND, SHOW_KIND, SIGNAL_VALUE_KIND};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PicoHostConfig {
    pub host_id: HostId,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LedReceipt {
    pub host_id: HostId,
    pub placement_id: PlacementId,
    pub sequence: u64,
    pub level: bool,
    pub led_on: bool,
}

pub fn pico_advertisement(config: PicoHostConfig) -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: config.host_id,
        boot_id: config.boot_id,
        offer_generation: config.offer_generation,
        profile: HostProfileId::from("pico-w"),
        capabilities: vec![
            CapabilityOffer {
                capability_id: CapabilityId::from("pico-pulse"),
                kind_id: kind_id(PULSE_KIND),
                implementation_id: ImplementationId::from("pico/pulse-v1"),
                artifact_id: ArtifactId::from("conduit-signal/pulse-artifact-v1"),
                limits: CapabilityLimits {
                    value_kind: kind_id(SIGNAL_VALUE_KIND),
                    max_active_instances: 1,
                    max_queue_items: 4,
                    max_queue_bytes: 64,
                },
            },
            CapabilityOffer {
                capability_id: CapabilityId::from("onboard-led"),
                kind_id: kind_id(SHOW_KIND),
                implementation_id: ImplementationId::from("pico/onboard-led-show-signal-v1"),
                artifact_id: ArtifactId::from("conduit-signal/show-artifact-v1"),
                limits: CapabilityLimits {
                    value_kind: kind_id(SIGNAL_VALUE_KIND),
                    max_active_instances: 1,
                    max_queue_items: 4,
                    max_queue_bytes: 64,
                },
            },
        ],
    }
}

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

#[cfg(feature = "std")]
mod std_fixture {
    use super::{led_receipt, pico_advertisement, LedReceipt, PicoHostConfig};
    use alloc::vec::Vec;
    use conduit_core::{
        CapabilityId, ConnectionProvider, HostAdvertisement, HostCommand, HostEvent, HostId,
        ImplementationId, Observation, PlacementId, Plan, PlanFragment, PlatformEffect,
    };
    use conduit_form::CheckedForm;
    use conduit_planner::{plan, PlacementChoice, PlacementChoices};
    use conduit_runtime::{HostRuntime, RuntimeOutput};
    use conduit_signal::{signal_registry, SIGNAL_PRESENTATION_KIND};
    use std::collections::BTreeMap;

    pub struct PicoHost {
        runtime: HostRuntime,
        receipts: Vec<LedReceipt>,
    }

    impl PicoHost {
        pub fn new(config: PicoHostConfig) -> Self {
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

        pub fn plan_local(&self, form: &CheckedForm) -> Result<Plan, Box<dyn std::error::Error>> {
            let placements = PlacementChoices {
                by_operation: BTreeMap::from([
                    (
                        conduit_core::OperationId::from("pulse"),
                        PlacementChoice {
                            host_id: self.advertisement().host_id.clone(),
                            capability_id: CapabilityId::from("pico-pulse"),
                        },
                    ),
                    (
                        conduit_core::OperationId::from("show"),
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
                &[ConnectionProvider::Local],
            )?)
        }

        pub fn run_fragment(&mut self, fragment: PlanFragment) -> Result<Vec<Observation>, String> {
            let prepare = self.runtime.handle(HostCommand::Prepare(fragment.clone()));
            ensure_prepared(&prepare)?;
            let activated = self
                .runtime
                .handle(HostCommand::Activate(fragment.plan_id.clone()));
            ensure_activated(&activated)?;

            let mut pending = activated.effects;
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
                        placement_id,
                        presentation_kind,
                        value,
                    } => {
                        if presentation_kind.as_str() != SIGNAL_PRESENTATION_KIND {
                            return Err(format!(
                                "pico host cannot manifest presentation kind '{}'",
                                presentation_kind.as_str()
                            ));
                        }
                        let receipt = led_receipt(
                            self.advertisement().host_id.clone(),
                            placement_id.clone(),
                            &value,
                        )
                        .map_err(|err| err.to_string())?;
                        self.receipts.push(receipt);
                        self.runtime.handle(HostCommand::CompletePresentation {
                            plan_id,
                            placement_id,
                            value,
                            success: true,
                            message: None,
                        })
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

    fn ensure_activated(output: &RuntimeOutput) -> Result<(), String> {
        output
            .events
            .iter()
            .find_map(|event| match event {
                HostEvent::ActivationRejected {
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

#[cfg(feature = "std")]
pub use std_fixture::PicoHost;

#[cfg(test)]
mod tests {
    use super::{pico_advertisement, PicoHost, PicoHostConfig};
    use conduit_core::{BootId, ConnectionProvider, HostId, OfferGeneration, TerminalDisposition};
    use conduit_form::parse;
    use conduit_signal::signal_profile_catalog;

    #[test]
    fn constrained_advertisement_names_pico_led_without_transport_claims() {
        let advertisement = pico_advertisement(PicoHostConfig {
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
    fn pico_host_runs_pair_form_to_onboard_led_receipts() {
        let form = parse(
            include_str!("../../../examples/signal-demo.form"),
            &signal_profile_catalog(),
        )
        .expect("signal form parses");
        let mut host = PicoHost::new(PicoHostConfig {
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
            .find(|connection| connection.provider == ConnectionProvider::Local)
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
}
