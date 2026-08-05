use conduit_core::{
    kind_id, ArtifactId, CapabilityId, CapabilityLimits, CapabilityOffer, ConnectionProvider,
    HostAdvertisement, HostCommand, HostEvent, HostId, HostProfileId, ImplementationId,
    Observation, OfferGeneration, Plan, PlanFragment, PlanId, PlatformEffect, PROTOCOL_VERSION,
};
use conduit_form::CheckedForm;
use conduit_planner::{default_placements, parse_placements, plan, PlacementChoices};
use conduit_runtime::{HostRuntime, RuntimeOutput};
use conduit_signal::{
    decode_signal, pulse_contract_revision, pulse_execution_profile,
    pulse_host_operation_requirements, pulse_outputs, pulse_resource_requirements,
    show_contract_revision, show_execution_profile, show_host_operation_requirements, show_inputs,
    show_resource_requirements, signal_profile_catalog, signal_registry, signal_resource_offers,
    PULSE_KIND, SHOW_KIND, SIGNAL_PRESENTATION_KIND,
};
use std::fs;
use std::io::Write;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub mod kernel_multivalue;
mod kernel_signal;

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
    pub kernel: Option<StdKernelExecutionReport>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StdKernelExecutionReport {
    pub active_play_id: conduit_core::ActivePlayId,
    pub decisions: u32,
    pub kernel_events: u16,
    pub value_allocation_capacity_before: (usize, usize),
    pub value_allocation_capacity_after: (usize, usize),
    pub presentation_ids: Vec<conduit_core::PresentationId>,
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
    next_kernel_activation_sequence: u64,
    next_kernel_evidence_sequence: u64,
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
            next_kernel_activation_sequence: 0,
            next_kernel_evidence_sequence: 0,
        }
    }

    pub fn advertisement(&self) -> &HostAdvertisement {
        self.runtime.advertisement()
    }

    pub fn handle(&mut self, command: HostCommand) -> RuntimeOutput {
        self.runtime.handle(command)
    }

    pub fn replace_link_bindings(&mut self, bindings: Vec<conduit_core::LinkBinding>) {
        self.runtime.replace_link_bindings(bindings);
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
        write_operator_report(output, self.advertisement(), &fragment.plan_id, &fragment)?;

        if !is_installed_kernel_signal_pair(&fragment) {
            return self.run_fragment_legacy_to(fragment, output, timer);
        }

        // The old runtime is used only as an effect-free S2 preparation
        // validator. Dropping this temporary facade cannot leave an alternate
        // operation pump or a resource reservation alive beside the kernel.
        let registry = signal_registry(
            ImplementationId::from("std/pulse-v1"),
            ImplementationId::from("std/stdout-show-signal-v1"),
        )
        .map_err(|error| format!("signal registry: {error:?}"))?;
        let mut preparation = HostRuntime::new(self.advertisement().clone(), registry, 256);
        let prepare = preparation.handle(HostCommand::Prepare(fragment.clone()));
        if let Some(reason) = preparation_rejection(&prepare) {
            return Err(reason);
        }
        drop(preparation);
        let activation_sequence = self.next_kernel_activation_sequence;
        self.next_kernel_activation_sequence = activation_sequence
            .checked_add(1)
            .ok_or_else(|| "kernel activation sequence exhausted".to_string())?;
        let advertisement = self.advertisement().clone();
        let report = kernel_signal::run_signal_fragment(
            &advertisement,
            &fragment,
            activation_sequence,
            &mut self.next_kernel_evidence_sequence,
            output,
            timer,
        )?;
        writeln!(output, "plan {} complete", fragment.plan_id.as_str())
            .map_err(|error| error.to_string())?;
        if let (Some(first), Some(last)) = (report.receipts.first(), report.receipts.last()) {
            writeln!(
                output,
                "receipts {} first=({}, {}) last=({}, {})",
                report.receipts.len(),
                first.sequence,
                first.level,
                last.sequence,
                last.level
            )
            .map_err(|error| error.to_string())?;
        } else {
            writeln!(output, "receipts 0").map_err(|error| error.to_string())?;
        }
        Ok(report)
    }

    fn run_fragment_legacy_to<W: Write, T: TimerAdapter>(
        &mut self,
        fragment: PlanFragment,
        output: &mut W,
        timer: &mut T,
    ) -> Result<StdRunReport, String> {
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
                    active_play_id,
                    presentation_id,
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
                        active_play_id,
                        presentation_id,
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
            kernel: None,
        })
    }
}

fn is_installed_kernel_signal_pair(fragment: &PlanFragment) -> bool {
    fragment.placements.len() == 2
        && fragment.connections.len() == 1
        && fragment
            .placements
            .iter()
            .filter(|placement| placement.kind_id.as_str() == PULSE_KIND)
            .count()
            == 1
        && fragment
            .placements
            .iter()
            .filter(|placement| placement.kind_id.as_str() == SHOW_KIND)
            .count()
            == 1
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
        resources: signal_resource_offers("std/timer", "std/presentation", 16),
        capabilities: vec![
            CapabilityOffer {
                capability_id: CapabilityId::from("pulse-1"),
                kind_id: kind_id(PULSE_KIND),
                kind_contract_revision: pulse_contract_revision(),
                execution_profile_id: pulse_execution_profile(),
                implementation_id: ImplementationId::from("std/pulse-v1"),
                artifact_id: ArtifactId::from("conduit-signal/pulse-artifact-v1"),
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
                capability_id: CapabilityId::from("stdout-show-1"),
                kind_id: kind_id(SHOW_KIND),
                kind_contract_revision: show_contract_revision(),
                execution_profile_id: show_execution_profile(),
                implementation_id: ImplementationId::from("std/stdout-show-signal-v1"),
                artifact_id: ArtifactId::from("conduit-signal/show-artifact-v1"),
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

fn write_operator_report<W: Write>(
    out: &mut W,
    advertisement: &HostAdvertisement,
    plan_id: &PlanId,
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
    writeln!(
        out,
        "plan {} source_document={} checked_form={} expanded_form={}",
        plan_id.as_str(),
        fragment.source_document_id.as_str(),
        fragment.checked_form_id.as_str(),
        fragment.expanded_form_id.as_str()
    )
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
    use conduit_core::{
        seal_plan, BootId, ConnectionId, ConnectionProvider, FormIdentity, HostId, OfferGeneration,
        PortDirection, PortId,
    };
    use conduit_form::parse;
    use conduit_signal::signal_profile_catalog;
    use std::time::Duration;

    #[test]
    fn exact_signal_fragment_lowers_to_numeric_kernel_tables() {
        let host = StdHost::new_with_config(StdHostConfig {
            host_id: HostId::from("lowering-host"),
            boot_id: BootId::from("lowering-boot"),
            offer_generation: OfferGeneration(1),
        });
        let form = parse(
            include_str!("../../../examples/signal-demo.form"),
            &signal_profile_catalog(),
        )
        .expect("signal form parses");
        let plan = host.plan_local(&form, None).expect("local plan resolves");
        let fragment = &plan.fragments[0];
        let lowered = conduit_runtime::lowering::lower_plan_fragment(fragment)
            .expect("exact fragment lowers");

        assert_eq!(lowered.identity.plan_id, fragment.plan_id);
        assert_eq!(lowered.identity.fragment_id, fragment.fragment_id);
        assert_eq!(lowered.nodes.len(), 2);
        assert_eq!(lowered.cords.len(), 1);
        assert_eq!(lowered.routes.len(), 1);
        assert_eq!(lowered.host_operations.len(), 2);
        assert_eq!(lowered.resources.len(), 2);
        assert_eq!(lowered.cord_value_slots, 4);
        assert_eq!(lowered.cord_value_bytes, 64);
        assert_eq!(
            lowered.evidence_items,
            fragment.expected_evidence.len() as u16
        );
        assert_eq!(lowered.identity.placements.len(), 2);
        assert_eq!(lowered.identity.connections.len(), 1);
        assert_eq!(lowered.identity.ports.len(), 2);
        assert!(lowered
            .identity
            .ports
            .iter()
            .any(|port| port.direction == PortDirection::Input));
        assert!(lowered
            .identity
            .ports
            .iter()
            .any(|port| port.direction == PortDirection::Output));
        assert_eq!(lowered.evidence.len(), fragment.expected_evidence.len());
        assert!(lowered
            .host_operations
            .iter()
            .any(|operation| operation.binding.maximum_output_bytes == 0));
        assert_eq!(
            lowered.node_specs[1].input_cords[0],
            Some(lowered.cords[0].spec.cord)
        );

        let mut mutated = fragment.clone();
        mutated.fragment_id = conduit_core::FragmentId::from("mutated-after-seal");
        assert!(matches!(
            conduit_runtime::lowering::lower_plan_fragment(&mutated),
            Err(conduit_runtime::lowering::LoweringError::InvalidFragment)
        ));

        let form_identity = FormIdentity {
            source_document_id: fragment.source_document_id.clone(),
            checked_form_id: fragment.checked_form_id.clone(),
            expanded_form_id: fragment.expanded_form_id.clone(),
        };
        let mut concurrent = fragment.clone();
        concurrent.placements[0].host_operations[0].maximum_in_flight = 2;
        let concurrent = seal_plan(form_identity.clone(), vec![concurrent]);
        assert!(matches!(
            conduit_runtime::lowering::lower_plan_fragment(&concurrent.fragments[0]),
            Err(conduit_runtime::lowering::LoweringError::UnsupportedHostOperationConcurrency(_))
        ));

        let mut fan_in = fragment.clone();
        let mut second = fan_in.connections[0].clone();
        second.connection_id = ConnectionId::from("second-cord-to-same-input");
        fan_in.connections.push(second);
        let fan_in = seal_plan(form_identity, vec![fan_in]);
        assert!(matches!(
            conduit_runtime::lowering::lower_plan_fragment(&fan_in.fragments[0]),
            Err(conduit_runtime::lowering::LoweringError::MultipleConnectionsToInput { .. })
        ));

        let form_identity = FormIdentity {
            source_document_id: fragment.source_document_id.clone(),
            checked_form_id: fragment.checked_form_id.clone(),
            expanded_form_id: fragment.expanded_form_id.clone(),
        };
        let mut remote = fragment.clone();
        remote.connections[0].provider = ConnectionProvider::InMemory;
        let remote = seal_plan(form_identity.clone(), vec![remote]);
        assert!(matches!(
            conduit_runtime::lowering::lower_plan_fragment(&remote.fragments[0]),
            Err(conduit_runtime::lowering::LoweringError::UnsupportedRemoteConnection(_))
        ));

        let mut too_wide = fragment.clone();
        let output = too_wide.placements[0].outputs[0].clone();
        for index in 1..=16 {
            let mut extra = output.clone();
            extra.port_id = PortId::from(format!("extra-output-{index}"));
            too_wide.placements[0].outputs.push(extra);
        }
        let too_wide = seal_plan(form_identity, vec![too_wide]);
        assert!(matches!(
            conduit_runtime::lowering::lower_plan_fragment(&too_wide.fragments[0]),
            Err(conduit_runtime::lowering::LoweringError::CapacityOverflow)
        ));
    }

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
        let plan_id = fragment.plan_id.clone();
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
        let kernel = report.kernel.as_ref().expect("signal pair uses kernel");
        assert!(kernel.decisions > 0);
        assert!(kernel.kernel_events > 0);
        assert_ne!(kernel.active_play_id.as_str(), plan_id.as_str());
        assert_eq!(kernel.presentation_ids.len(), 3);
        assert!(kernel
            .presentation_ids
            .windows(2)
            .all(|pair| pair[0] != pair[1]));
        assert_eq!(
            kernel.value_allocation_capacity_before,
            kernel.value_allocation_capacity_after
        );
        assert!(
            report
                .observations
                .iter()
                .filter(|observation| {
                    observation.active_play_id.as_ref() == Some(&kernel.active_play_id)
                        && observation.presentation_id.is_some()
                })
                .count()
                == 3
        );
        assert!(report.observations.iter().any(|observation| matches!(
            observation.kind,
            conduit_core::ObservationKind::PlanTerminal {
                disposition: conduit_core::TerminalDisposition::Completed
            }
        )));
    }
}
