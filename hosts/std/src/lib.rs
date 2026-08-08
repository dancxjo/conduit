#[cfg(feature = "legacy-fixture-driver")]
use conduit_core::HostCommand;
#[cfg(feature = "legacy-fixture-driver")]
use conduit_core::ImplementationId;
use conduit_core::{
    ConnectionProvider, HostAdvertisement, HostId, Observation, OfferGeneration, Plan,
    PlanFragment, PlanId,
};
use conduit_form::CheckedForm;
use conduit_planner::{default_placements, parse_placements, plan, PlacementChoices};
#[cfg(feature = "legacy-fixture-driver")]
use conduit_runtime::{HostRuntime, RuntimeOutput};
#[cfg(feature = "legacy-fixture-driver")]
use conduit_signal::signal_registry;
use conduit_signal::{signal_profile_catalog, PULSE_KIND, SHOW_KIND};
use std::fs;
use std::io::Write;
use std::sync::atomic::{AtomicU64, Ordering};
use std::thread;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

mod composition;
pub mod distributed_signal;
pub mod distributed_toggle;
pub use composition::StdHostComposition;
mod installed_std;
#[cfg(test)]
mod installed_std_tests;
pub mod kernel_multivalue;
mod kernel_preparation;
mod kernel_signal;
pub mod pico_usb_source;
pub mod triple_signal;
pub mod usb_cdc;
pub mod websocket;

#[cfg(test)]
mod allocation_probe {
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::cell::Cell;

    pub struct TrackingAllocator;

    thread_local! {
        static TRACKING: Cell<bool> = const { Cell::new(false) };
        static ALLOCATIONS: Cell<usize> = const { Cell::new(0) };
    }

    fn record() {
        let _ = TRACKING.try_with(|tracking| {
            if tracking.get() {
                let _ = ALLOCATIONS.try_with(|allocations| {
                    allocations.set(allocations.get().saturating_add(1));
                });
            }
        });
    }

    unsafe impl GlobalAlloc for TrackingAllocator {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            let pointer = unsafe { System.alloc(layout) };
            if !pointer.is_null() {
                record();
            }
            pointer
        }

        unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
            let pointer = unsafe { System.alloc_zeroed(layout) };
            if !pointer.is_null() {
                record();
            }
            pointer
        }

        unsafe fn dealloc(&self, pointer: *mut u8, layout: Layout) {
            unsafe { System.dealloc(pointer, layout) };
        }

        unsafe fn realloc(&self, pointer: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
            let pointer = unsafe { System.realloc(pointer, layout, new_size) };
            if !pointer.is_null() {
                record();
            }
            pointer
        }
    }

    #[global_allocator]
    static ALLOCATOR: TrackingAllocator = TrackingAllocator;

    pub struct Guard {
        finished: bool,
    }

    impl Guard {
        pub fn finish(mut self) -> usize {
            self.finished = true;
            TRACKING.with(|tracking| tracking.set(false));
            ALLOCATIONS.with(Cell::get)
        }
    }

    impl Drop for Guard {
        fn drop(&mut self) {
            if !self.finished {
                TRACKING.with(|tracking| tracking.set(false));
            }
        }
    }

    pub fn begin() -> Guard {
        ALLOCATIONS.with(|allocations| allocations.set(0));
        TRACKING.with(|tracking| tracking.set(true));
        Guard { finished: false }
    }
}

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
    pub identity: conduit_runtime::lowering::KernelExecutionIdentityMap,
    #[cfg(test)]
    pub post_activation_allocations: usize,
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

pub fn run_kernel_multivalue_path_to<W: Write, T: TimerAdapter>(
    path: &str,
    output: &mut W,
    timer: &mut T,
) -> Result<kernel_multivalue::MultiValueRunReport, String> {
    let source = fs::read_to_string(path).map_err(|error| error.to_string())?;
    let form = conduit_form::parse(&source, &kernel_multivalue::profile_catalog())
        .map_err(|error| error.to_string())?;
    let advertisement = kernel_multivalue::advertisement(
        HostId::from("std-host-1"),
        conduit_core::BootId::from(fresh_boot_id()),
        OfferGeneration(1),
    );
    let plan =
        kernel_multivalue::plan_local(&form, &advertisement).map_err(|error| error.to_string())?;
    let fragment = plan
        .fragments
        .into_iter()
        .find(|fragment| fragment.host_id == advertisement.host_id)
        .ok_or_else(|| "no local multi-value fragment for std host".to_string())?;
    write_operator_report(output, &advertisement, &fragment.plan_id, &fragment)?;
    let mut resources = kernel_preparation::KernelResourceLedger::new(&advertisement)?;
    let reservation = resources.prepare_and_reserve(&advertisement, &fragment)?;
    let mut evidence_sequence = 0;
    let result = kernel_multivalue::execute_fragment(
        &advertisement,
        &fragment,
        0,
        &mut evidence_sequence,
        output,
        timer,
    );
    let release = resources.release(reservation);
    let report = result?;
    release?;
    writeln!(output, "plan {} complete", fragment.plan_id.as_str())
        .map_err(|error| error.to_string())?;
    writeln!(output, "receipts 3 even=(0, 2) latest=(3)").map_err(|error| error.to_string())?;
    writeln!(
        output,
        "kernel active_play={} decisions={} events={} stable_allocations={} pressure_connection={} pressure_items={} pressure_bytes={} input_closed={} terminal_order_exact={}",
        report.active_play_id.as_str(),
        report.decisions,
        report.kernel_events,
        report.value_allocation_capacity_before == report.value_allocation_capacity_after,
        report.pressure_connection_id.as_str(),
        report.pressure_items,
        report.pressure_bytes,
        report.input_closed_events,
        report.terminal_order_exact,
    )
    .map_err(|error| error.to_string())?;
    Ok(report)
}

pub struct StdHost {
    advertisement: HostAdvertisement,
    kernel_resources: kernel_preparation::KernelResourceLedger,
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
        Self::new_with_composition(config, StdHostComposition::reference())
    }

    pub fn new_with_composition(config: StdHostConfig, composition: StdHostComposition) -> Self {
        let advertisement = composition::build_advertisement(config, composition);
        let kernel_resources = kernel_preparation::KernelResourceLedger::new(&advertisement)
            .expect("std kernel resource offers are exact and bounded");
        Self {
            advertisement,
            kernel_resources,
            next_kernel_activation_sequence: 0,
            next_kernel_evidence_sequence: 0,
        }
    }

    pub fn advertisement(&self) -> &HostAdvertisement {
        &self.advertisement
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

    pub fn plan_expanded_local(
        &self,
        form: &conduit_form::ExpandedCanonicalForm,
    ) -> Result<Plan, Box<dyn std::error::Error>> {
        let realm = vec![self.advertisement().clone()];
        let placements = conduit_planner::default_expanded_placements(form, &realm)?;
        Ok(conduit_planner::plan_expanded_canonical(
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

        let installed_standard = installed_std::supports(&fragment);
        if !installed_standard && !is_installed_kernel_signal_profile(&fragment) {
            return Err("fragment does not match the installed std kernel profile".to_string());
        }

        let advertisement = self.advertisement().clone();
        let reservation = self
            .kernel_resources
            .prepare_and_reserve(&advertisement, &fragment)?;
        let result = (|| {
            let activation_sequence = self.next_kernel_activation_sequence;
            self.next_kernel_activation_sequence = activation_sequence
                .checked_add(1)
                .ok_or_else(|| "kernel activation sequence exhausted".to_string())?;
            if installed_standard {
                installed_std::run_fragment(
                    &advertisement,
                    &fragment,
                    activation_sequence,
                    &mut self.next_kernel_evidence_sequence,
                    output,
                    timer,
                )
            } else {
                kernel_signal::run_signal_fragment(
                    &advertisement,
                    &fragment,
                    activation_sequence,
                    &mut self.next_kernel_evidence_sequence,
                    output,
                    timer,
                )
            }
        })();
        let release = self.kernel_resources.release(reservation);
        let report = result?;
        release?;
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
}

/// Explicit compatibility driver for simulation fixtures that have not yet migrated to the
/// kernel protocol. Production std execution never constructs this legacy runtime.
#[cfg(feature = "legacy-fixture-driver")]
pub struct LegacyStdFixtureHost {
    runtime: HostRuntime,
}

#[cfg(feature = "legacy-fixture-driver")]
impl LegacyStdFixtureHost {
    pub fn new_with_config(config: StdHostConfig) -> Self {
        let advertisement =
            composition::build_advertisement(config, StdHostComposition::reference());
        let registry = signal_registry(
            ImplementationId::from("std/pulse-v1"),
            ImplementationId::from("std/stdout-show-signal-v1"),
        )
        .expect("std fixture signal implementations have unique identities");
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

    pub fn replace_link_bindings(&mut self, bindings: Vec<conduit_core::LinkBinding>) {
        self.runtime.replace_link_bindings(bindings);
    }
}

fn is_installed_kernel_signal_profile(fragment: &PlanFragment) -> bool {
    matches!(
        (fragment.placements.len(), fragment.connections.len()),
        (2, 1) | (4, 3)
    ) && fragment
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
            == fragment.placements.len().saturating_sub(1)
        && fragment
            .connections
            .iter()
            .all(|connection| connection.provider == ConnectionProvider::Local)
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

    fn sealed_activation_region(source: &str) -> &str {
        source
            .split_once("SEALED PROFILE ACTIVATION BEGIN")
            .and_then(|(_, remainder)| {
                remainder
                    .split_once("SEALED PROFILE ACTIVATION END")
                    .map(|(activation, _)| activation)
            })
            .expect("sealed activation markers remain paired")
    }

    #[test]
    fn sealed_profiles_do_not_reenter_semantic_or_allocating_preparation() {
        let forbidden = [
            "fragment.",
            "lowered.",
            "kind_id(",
            "provider",
            "registry",
            ".find(",
            ".collect(",
            "Vec::",
            "vec![",
            ".to_vec(",
            ".clone(",
            ".reserve(",
        ];
        for (name, source) in [
            ("signal", include_str!("kernel_signal.rs")),
            ("multi-value", include_str!("kernel_multivalue.rs")),
        ] {
            let activation = sealed_activation_region(source);
            for token in forbidden {
                assert!(
                    !activation.contains(token),
                    "{name} sealed activation reintroduced '{token}'"
                );
            }
        }
    }

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
        for (node, placement) in &lowered.identity.placements {
            assert_eq!(lowered.identity.placement_for_node(*node), Some(placement));
            assert_eq!(lowered.identity.node_for_placement(placement), Some(*node));
        }
        for (cord, connection) in &lowered.identity.connections {
            assert_eq!(
                lowered.identity.connection_for_cord(*cord),
                Some(connection)
            );
            assert_eq!(
                lowered.identity.cord_for_connection(connection),
                Some(*cord)
            );
        }
        for port in &lowered.identity.ports {
            assert_eq!(
                lowered
                    .identity
                    .port_identity(port.node, port.direction, port.port),
                Some(port)
            );
            assert_eq!(
                lowered
                    .identity
                    .port_for_identity(port.node, port.direction, &port.port_id),
                Some(port.port)
            );
        }
        for (node, operation, contract) in &lowered.identity.host_operations {
            assert_eq!(
                lowered.identity.host_operation_contract(*node, *operation),
                Some(contract)
            );
            assert_eq!(
                lowered
                    .identity
                    .host_operation_for_contract(*node, contract),
                Some(*operation)
            );
        }
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
            Err(conduit_runtime::lowering::LoweringError::InvalidFragment)
                | Err(conduit_runtime::lowering::LoweringError::InvalidRemoteConnection(_))
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
        let mut output = Vec::with_capacity(65_536);
        let mut timer = VirtualTimer {
            waits: Vec::with_capacity(2),
        };
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
        assert_eq!(kernel.identity.plan_id, plan_id);
        assert_eq!(kernel.identity.active_play_id, kernel.active_play_id);
        assert_eq!(kernel.identity.lengths(), (5, 3, 4));
        assert_eq!(kernel.post_activation_allocations, 0);
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
        for observation in &report.observations {
            let evidence = kernel
                .identity
                .evidence(&observation.evidence_id)
                .expect("host evidence reverses to its kernel identity row");
            assert_eq!(
                evidence.presentation_id.as_ref(),
                observation.presentation_id.as_ref()
            );
            if let Some(presentation_id) = &observation.presentation_id {
                let presentation = kernel
                    .identity
                    .presentation(presentation_id)
                    .expect("presentation reverses to one kernel request");
                let request = kernel
                    .identity
                    .request(presentation.node, presentation.request)
                    .expect("presentation request reverses to its host-operation contract");
                assert!(kernel
                    .identity
                    .request_for_contract(presentation.node, &request.contract_id)
                    .any(|candidate| candidate == request));
                assert_eq!(
                    kernel
                        .identity
                        .presentation_for_request(presentation.node, presentation.request)
                        .map(|identity| &identity.presentation_id),
                    Some(presentation_id)
                );
                assert_eq!(
                    kernel
                        .identity
                        .evidence_for_presentation(presentation_id)
                        .map(|identity| &identity.evidence_id),
                    Some(&observation.evidence_id)
                );
            }
        }
        assert!(report.observations.iter().any(|observation| matches!(
            observation.kind,
            conduit_core::ObservationKind::PlanTerminal {
                disposition: conduit_core::TerminalDisposition::Completed
            }
        )));
    }

    #[test]
    fn local_three_sink_signal_fanout_uses_only_the_sealed_kernel_profile() {
        let mut host = StdHost::new_with_config(StdHostConfig {
            host_id: HostId::from("std-host-1"),
            boot_id: BootId::from("fanout-boot"),
            offer_generation: OfferGeneration(1),
        });
        let form = parse(
            include_str!("../../../examples/triple-signal.form"),
            &signal_profile_catalog(),
        )
        .expect("triple signal form parses");
        let placements = conduit_planner::parse_placements(include_str!(
            "../../../examples/triple-local.placements"
        ))
        .expect("triple local placements parse");
        let plan = host
            .plan_local(&form, Some(&placements))
            .expect("triple local plan resolves");
        let fragment = plan.fragments[0].clone();
        let mut output = Vec::with_capacity(65_536);
        let mut timer = VirtualTimer {
            waits: Vec::with_capacity(15),
        };
        let report = host
            .run_fragment_to(fragment, &mut output, &mut timer)
            .expect("triple local kernel run completes");

        assert_eq!(timer.waits, vec![Duration::from_millis(250); 15]);
        assert_eq!(report.receipts.len(), 48);
        let kernel = report.kernel.expect("triple local form uses kernel");
        assert_eq!(kernel.identity.lengths(), (63, 48, 49));
        assert_eq!(kernel.post_activation_allocations, 0);
    }

    #[test]
    fn unsupported_production_std_form_fails_closed_without_a_legacy_pump() {
        let mut host = StdHost::new_with_config(StdHostConfig {
            host_id: HostId::from("std-host-1"),
            boot_id: BootId::from("unsupported-form-boot"),
            offer_generation: OfferGeneration(1),
        });
        let form = parse(
            "form 0\n\nwider {\n first: flow/pulse\n second: flow/pulse\n left: presentation/show\n right: presentation/show\n first.count = 1\n second.count = 1\n first > left\n second > right\n}\n",
            &signal_profile_catalog(),
        )
        .expect("unsupported wider form remains semantically valid");
        let plan = host
            .plan_local(&form, None)
            .expect("wider local plan resolves");
        let mut output = Vec::with_capacity(8_192);
        let mut timer = VirtualTimer::default();

        let error = host
            .run_fragment_to(plan.fragments[0].clone(), &mut output, &mut timer)
            .expect_err("production std host must not fall back to the legacy pump");

        assert_eq!(
            error,
            "fragment does not match the installed std kernel profile"
        );
        assert!(timer.waits.is_empty());
    }
}
