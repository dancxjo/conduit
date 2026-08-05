use conduit_core::{
    kind_id, ArtifactId, CapabilityId, CapabilityLimits, CapabilityOffer, ConnectionProvider,
    FormId, HostAdvertisement, HostCommand, HostEvent, HostId, HostProfileId, ImplementationId,
    Observation, OfferGeneration, Plan, PlanFragment, PlanId, PlatformEffect, PROTOCOL_VERSION,
};
use conduit_form::CheckedForm;
use conduit_planner::{default_placements, parse_placements, plan, PlacementChoices};
use conduit_runtime::{HostRuntime, RuntimeOutput};
use conduit_signal::{
    decode_signal, signal_profile_catalog, signal_registry, PULSE_KIND, SHOW_KIND,
    SIGNAL_PRESENTATION_KIND, SIGNAL_VALUE_KIND,
};
use std::fs;
use std::io::Write;
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
    pub observations: Vec<Observation>,
    pub receipts: Vec<SignalReceipt>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignalReceipt {
    pub placement_id: conduit_core::PlacementId,
    pub sequence: u64,
    pub level: bool,
}

pub trait TimerAdapter {
    fn wait(&mut self, duration: Duration);
}

pub struct ThreadTimer;

impl TimerAdapter for ThreadTimer {
    fn wait(&mut self, duration: Duration) {
        thread::sleep(duration);
    }
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
        let advertisement = build_advertisement(config);
        let registry = signal_registry(
            ImplementationId::from("std/pulse-v1"),
            ImplementationId::from("std/stdout-show-signal-v1"),
        )
        .expect("std signal implementations have unique identities");
        Self {
            runtime: HostRuntime::new(advertisement, registry, 256),
        }
    }

    pub fn advertisement(&self) -> &HostAdvertisement {
        self.runtime.advertisement()
    }

    pub fn handle(&mut self, command: HostCommand) -> RuntimeOutput {
        self.runtime.handle(command)
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

    pub fn run_fragment_to<W: Write, T: TimerAdapter>(
        &mut self,
        fragment: PlanFragment,
        output: &mut W,
        timer: &mut T,
    ) -> Result<StdRunReport, String> {
        write_operator_report(
            output,
            self.advertisement(),
            &fragment.plan_id,
            &fragment.form_id,
            &fragment,
        )?;

        let prepare = self.runtime.handle(HostCommand::Prepare(fragment.clone()));
        if let Some(reason) = preparation_rejection(&prepare) {
            return Err(reason);
        }
        let activated_output = self
            .runtime
            .handle(HostCommand::Activate(fragment.plan_id.clone()));
        if let Some(reason) = activation_rejection(&activated_output) {
            return Err(reason);
        }

        let mut pending_effects = activated_output.effects;
        let mut receipts = Vec::new();
        while let Some(effect) = pending_effects.pop() {
            let follow_up = match effect {
                PlatformEffect::Wait {
                    plan_id,
                    placement_id,
                    duration_ms,
                } => {
                    timer.wait(Duration::from_millis(duration_ms));
                    self.runtime.handle(HostCommand::CompleteWait {
                        plan_id,
                        placement_id,
                    })
                }
                PlatformEffect::PresentValue {
                    plan_id,
                    placement_id,
                    presentation_kind,
                    value,
                } => {
                    if presentation_kind.as_str() != SIGNAL_PRESENTATION_KIND {
                        return Err(format!(
                            "std host cannot manifest presentation kind '{}'",
                            presentation_kind.as_str()
                        ));
                    }
                    let signal = decode_signal(&value).map_err(|err| err.to_string())?;
                    writeln!(
                        output,
                        "signal {} {}",
                        signal.sequence,
                        if signal.level { "on" } else { "off" }
                    )
                    .map_err(|error| error.to_string())?;
                    writeln!(
                        output,
                        "receipt signal placement={} sequence={} level={}",
                        placement_id.as_str(),
                        signal.sequence,
                        signal.level
                    )
                    .map_err(|error| error.to_string())?;
                    receipts.push(SignalReceipt {
                        placement_id: placement_id.clone(),
                        sequence: signal.sequence,
                        level: signal.level,
                    });
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
        writeln!(output, "plan {} complete", fragment.plan_id.as_str())
            .map_err(|error| error.to_string())?;
        if let (Some(first), Some(last)) = (receipts.first(), receipts.last()) {
            writeln!(
                output,
                "receipts {} first=({}, {}) last=({}, {})",
                receipts.len(),
                first.sequence,
                first.level,
                last.sequence,
                last.level
            )
            .map_err(|error| error.to_string())?;
        } else {
            writeln!(output, "receipts 0").map_err(|error| error.to_string())?;
        }
        Ok(StdRunReport {
            observations,
            receipts,
        })
    }
}

pub fn load_checked_form(path: &str) -> Result<CheckedForm, Box<dyn std::error::Error>> {
    Ok(conduit_form::parse(
        &fs::read_to_string(path)?,
        &signal_profile_catalog(),
    )?)
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
                artifact_id: ArtifactId::from("conduit-signal/pulse-artifact-v1"),
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

fn write_operator_report<W: Write>(
    out: &mut W,
    advertisement: &HostAdvertisement,
    plan_id: &PlanId,
    form_id: &FormId,
    fragment: &PlanFragment,
) -> Result<(), String> {
    writeln!(
        out,
        "host {} boot {} profile {} protocol {}",
        advertisement.host_id.as_str(),
        advertisement.boot_id.as_str(),
        advertisement.profile.as_str(),
        advertisement.protocol_version
    )
    .map_err(|error| error.to_string())?;
    writeln!(out, "plan {} form {}", plan_id.as_str(), form_id.as_str())
        .map_err(|error| error.to_string())?;
    for placement in &fragment.placements {
        writeln!(
            out,
            "place {} kind={} host={} boot={} capability={} implementation={} artifact={}",
            placement.operation_id.as_str(),
            placement.kind_id.as_str(),
            placement.host_id.as_str(),
            placement.boot_id.as_str(),
            placement.capability_id.as_str(),
            placement.implementation_id.as_str(),
            placement.artifact_id.as_str()
        )
        .map_err(|error| error.to_string())?;
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
        .map_err(|error| error.to_string())?;
    }
    Ok(())
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
        HostEvent::PreparationRejected {
            reason, message, ..
        } => Some(message.clone().unwrap_or_else(|| format!("{reason:?}"))),
        _ => None,
    })
}

fn activation_rejection(output: &RuntimeOutput) -> Option<String> {
    output.events.iter().find_map(|event| match event {
        HostEvent::ActivationRejected {
            reason, message, ..
        } => Some(message.clone().unwrap_or_else(|| format!("{reason:?}"))),
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
    use super::{StdHost, StdHostConfig, TimerAdapter};
    use conduit_core::{BootId, HostId, OfferGeneration};
    use conduit_form::parse;
    use conduit_signal::signal_profile_catalog;
    use std::time::Duration;

    #[derive(Default)]
    struct VirtualTimer {
        waits: Vec<Duration>,
    }

    impl TimerAdapter for VirtualTimer {
        fn wait(&mut self, duration: Duration) {
            self.waits.push(duration);
        }
    }

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

    #[test]
    fn streamed_output_uses_a_virtual_clock_and_retains_terminal_evidence() {
        let mut host = StdHost::new_with_config(StdHostConfig {
            host_id: HostId::from("test-host"),
            boot_id: BootId::from("virtual-clock-boot"),
            offer_generation: OfferGeneration(1),
        });
        let form = parse(
            "form 0\n\nvirtual {\n pulse: flow/pulse\n show: presentation/show\n pulse.count = 3\n pulse.period-ms = 7\n pulse.initial = false\n pulse > show\n}\n",
            &signal_profile_catalog(),
        )
        .expect("virtual-clock form parses");
        let plan = host.plan_local(&form, None).expect("local plan resolves");
        let fragment = plan.fragments[0].clone();
        let mut output = Vec::new();
        let mut timer = VirtualTimer::default();
        let report = host
            .run_fragment_to(fragment, &mut output, &mut timer)
            .expect("streamed run completes");

        assert_eq!(timer.waits, vec![Duration::from_millis(7); 2]);
        let output = String::from_utf8(output).expect("stream is utf-8");
        assert!(output.lines().any(|line| line == "signal 0 off"));
        assert!(output.lines().any(|line| line == "signal 1 on"));
        assert!(output.lines().any(|line| line == "signal 2 off"));
        assert!(output
            .lines()
            .any(|line| line.starts_with("receipt signal placement=")
                && line.ends_with(" sequence=0 level=false")));
        assert!(output
            .lines()
            .any(|line| line.starts_with("receipt signal placement=")
                && line.ends_with(" sequence=2 level=false")));
        assert!(output.contains("receipts 3 first=(0, false) last=(2, false)"));
        assert_eq!(report.receipts.len(), 3);
        assert_eq!(report.receipts[0].sequence, 0);
        assert!(!report.receipts[0].level);
        assert_eq!(report.receipts[2].sequence, 2);
        assert!(!report.receipts[2].level);
        assert!(report.observations.iter().any(|observation| matches!(
            observation.kind,
            conduit_core::ObservationKind::PlanTerminal {
                disposition: conduit_core::TerminalDisposition::Completed
            }
        )));
    }
}
