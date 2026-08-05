use conduit_core::{
    kind_id, CapabilityId, CapabilityLimits, CapabilityOffer, ConnectionProvider, FormId,
    HostAdvertisement, HostCommand, HostEvent, HostId, HostProfileId, ImplementationId,
    Observation, ObservationKind, OfferGeneration, Plan, PlanFragment, PlanId, PlatformEffect,
    PROTOCOL_VERSION,
};
use conduit_form::CheckedForm;
use conduit_planner::{default_placements, parse_placements, plan, PlacementChoices};
use conduit_runtime::{HostRuntime, RuntimeOutput};
use conduit_signal::{decode_signal, PULSE_KIND, SHOW_KIND, SIGNAL_VALUE_KIND};
use std::fmt::Write as _;
use std::fs;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

static BOOT_COUNTER: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone)]
pub struct StdHostConfig {
    pub host_id: HostId,
    pub boot_id: conduit_core::BootId,
    pub offer_generation: OfferGeneration,
}

#[derive(Debug, Clone)]
pub struct StdRunReport {
    pub text: String,
    pub observations: Vec<Observation>,
}

pub struct StdHost {
    runtime: HostRuntime,
}

impl Default for StdHost {
    fn default() -> Self {
        Self::new()
    }
}

impl StdHost {
    pub fn new() -> Self {
        Self::new_with_config(StdHostConfig {
            host_id: HostId::from("std-host-1"),
            boot_id: conduit_core::BootId::from(fresh_boot_id()),
            offer_generation: OfferGeneration(1),
        })
    }

    pub fn new_with_config(config: StdHostConfig) -> Self {
        Self {
            runtime: HostRuntime::new(build_advertisement(config), 256),
        }
    }

    pub fn advertisement(&self) -> &HostAdvertisement {
        self.runtime.advertisement()
    }

    pub fn plan_local(
        &self,
        form: &CheckedForm,
        placements: Option<&PlacementChoices>,
    ) -> Result<Plan, Box<dyn std::error::Error>> {
        let realm = vec![self.advertisement().clone()];
        let placements = match placements {
            Some(placements) => placements.clone(),
            None => default_placements(form, &realm)?,
        };
        Ok(plan(
            form,
            &realm,
            &placements,
            &[ConnectionProvider::Local],
        )?)
    }

    pub fn run_fragment(&mut self, fragment: PlanFragment) -> Result<StdRunReport, String> {
        let mut text = String::new();
        write_operator_report(
            &mut text,
            self.advertisement(),
            &fragment.plan_id,
            &fragment.form_id,
            &fragment,
        );

        let prepare = self.runtime.handle(HostCommand::Prepare(fragment.clone()));
        if let Some(reason) = preparation_rejection(&prepare) {
            return Err(reason);
        }
        let output = self
            .runtime
            .handle(HostCommand::Activate(fragment.plan_id.clone()));
        if let Some(reason) = activation_rejection(&output) {
            return Err(reason);
        }

        let mut pending_effects = output.effects;
        while let Some(effect) = pending_effects.pop() {
            let follow_up = match effect {
                PlatformEffect::Wait {
                    plan_id,
                    placement_id,
                    duration_ms,
                } => {
                    thread::sleep(Duration::from_millis(duration_ms));
                    self.runtime.handle(HostCommand::CompleteWait {
                        plan_id,
                        placement_id,
                    })
                }
                PlatformEffect::PresentValue {
                    plan_id,
                    placement_id,
                    value,
                } => {
                    let signal = decode_signal(&value).map_err(|err| err.to_string())?;
                    writeln!(
                        text,
                        "signal {} {}",
                        signal.sequence,
                        if signal.level { "on" } else { "off" }
                    )
                    .expect("rendered output should be writable");
                    self.runtime.handle(HostCommand::CompletePresentation {
                        plan_id,
                        placement_id,
                        value,
                        success: true,
                        message: None,
                    })
                }
                PlatformEffect::TransmitConnection { .. } => {
                    return Err("std host has no in-memory connection driver".to_string());
                }
            };
            pending_effects.extend(follow_up.effects.into_iter().rev());
        }

        let observations = inspect_observations(&mut self.runtime);
        let receipts = observations
            .iter()
            .filter_map(|observation| match &observation.kind {
                ObservationKind::ValuePresented { value } => {
                    Some(decode_signal(value).expect("signal payload must decode"))
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        writeln!(text, "plan {} complete", fragment.plan_id.as_str())
            .expect("rendered output should be writable");
        if let (Some(first), Some(last)) = (receipts.first(), receipts.last()) {
            writeln!(
                text,
                "receipts {} first=({}, {}) last=({}, {})",
                receipts.len(),
                first.sequence,
                first.level,
                last.sequence,
                last.level
            )
            .expect("rendered output should be writable");
        } else {
            writeln!(text, "receipts 0").expect("rendered output should be writable");
        }
        Ok(StdRunReport { text, observations })
    }
}

pub fn load_checked_form(path: &str) -> Result<CheckedForm, Box<dyn std::error::Error>> {
    Ok(conduit_form::parse(&fs::read_to_string(path)?)?)
}

pub fn load_placements(
    path: Option<&str>,
) -> Result<Option<PlacementChoices>, Box<dyn std::error::Error>> {
    match path {
        Some(path) => Ok(Some(parse_placements(&fs::read_to_string(path)?)?)),
        None => Ok(None),
    }
}

fn build_advertisement(config: StdHostConfig) -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: config.host_id,
        boot_id: config.boot_id,
        offer_generation: config.offer_generation,
        profile: HostProfileId::from("rust-std"),
        capabilities: vec![
            CapabilityOffer {
                capability_id: CapabilityId::from("pulse-1"),
                kind_id: kind_id(PULSE_KIND),
                implementation_id: ImplementationId::from("std/pulse-v1"),
                limits: CapabilityLimits {
                    value_kind: kind_id(SIGNAL_VALUE_KIND),
                    max_active_instances: 16,
                    max_queue_items: 4,
                    max_queue_bytes: 64,
                },
            },
            CapabilityOffer {
                capability_id: CapabilityId::from("stdout-show-1"),
                kind_id: kind_id(SHOW_KIND),
                implementation_id: ImplementationId::from("std/stdout-show-signal-v1"),
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

fn write_operator_report(
    out: &mut String,
    advertisement: &HostAdvertisement,
    plan_id: &PlanId,
    form_id: &FormId,
    fragment: &PlanFragment,
) {
    writeln!(
        out,
        "host {} boot {} profile {} protocol {}",
        advertisement.host_id.as_str(),
        advertisement.boot_id.as_str(),
        advertisement.profile.as_str(),
        advertisement.protocol_version
    )
    .expect("rendered output should be writable");
    writeln!(out, "plan {} form {}", plan_id.as_str(), form_id.as_str())
        .expect("rendered output should be writable");
    for placement in &fragment.placements {
        writeln!(
            out,
            "place {} kind={} host={} boot={} capability={} implementation={}",
            placement.operation_id.as_str(),
            placement.kind_id.as_str(),
            placement.host_id.as_str(),
            placement.boot_id.as_str(),
            placement.capability_id.as_str(),
            placement.implementation_id.as_str()
        )
        .expect("rendered output should be writable");
    }
    for connection in &fragment.connections {
        writeln!(
            out,
            "connection {} {}:{} -> {}:{} via {:?} queue={}",
            connection.connection_id.as_str(),
            connection.source_placement_id.as_str(),
            connection.source_port_id.as_str(),
            connection.sink_placement_id.as_str(),
            connection.sink_port_id.as_str(),
            connection.provider,
            connection.item_capacity
        )
        .expect("rendered output should be writable");
    }
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

fn preparation_rejection(output: &RuntimeOutput) -> Option<String> {
    output.events.iter().find_map(|event| match event {
        HostEvent::PreparationRejected { reason, .. } => Some(reason.clone()),
        _ => None,
    })
}

fn activation_rejection(output: &RuntimeOutput) -> Option<String> {
    output.events.iter().find_map(|event| match event {
        HostEvent::ActivationRejected { reason, .. } => Some(reason.clone()),
        _ => None,
    })
}

fn fresh_boot_id() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    let counter = BOOT_COUNTER.fetch_add(1, Ordering::Relaxed);
    format!("boot-{now:x}-{counter:x}")
}

#[cfg(test)]
mod tests {
    use super::{StdHost, StdHostConfig};
    use conduit_core::{BootId, HostId, OfferGeneration};

    #[test]
    fn fresh_starts_get_fresh_boot_ids() {
        let first = StdHost::new();
        let second = StdHost::new();
        assert_ne!(
            first.advertisement().boot_id.as_str(),
            second.advertisement().boot_id.as_str()
        );
    }

    #[test]
    fn deterministic_boot_ids_are_injectable() {
        let host = StdHost::new_with_config(StdHostConfig {
            host_id: HostId::from("test-host"),
            boot_id: BootId::from("boot-test"),
            offer_generation: OfferGeneration(9),
        });
        assert_eq!(host.advertisement().boot_id.as_str(), "boot-test");
        assert_eq!(host.advertisement().offer_generation.0, 9);
    }
}
