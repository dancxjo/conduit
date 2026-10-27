//! Borrowed canonical fields; self IDs and commitment sets are not fingerprint input.
use crate::*;
#[derive(Clone, Copy)]
pub struct FragmentFingerprintHeader<'a> {
    pub source_document_id: &'a SourceDocumentId,
    pub checked_plot_id: &'a CheckedPlotId,
    pub expanded_plot_id: &'a ExpandedPlotId,
    pub completion_policy: PlanCompletionPolicy,
    pub realization_backs: &'a [PlotBack],
    pub host_id: &'a HostId,
    pub boot_id: &'a BootId,
    pub offer_generation: OfferGeneration,
    pub execution_regions: &'a [ExecutionRegion],
    pub execution_fusions: &'a [PlannedFusion],
}
#[derive(Clone, Copy)]
pub struct FragmentFingerprintTail<'a> {
    pub connections: &'a [PlannedConnection],
    pub fore_ports: &'a [PlannedForePort],
    pub shared_pools: &'a [PlannedSharedPool],
    pub startup_dependencies: &'a [StartupDependency],
    pub startup_order: &'a [PlacementId],
    pub cancellation_policy: CancellationPolicy,
    pub terminal_policy: TerminalPolicy,
    pub expected_terminals: &'a [ExpectedTerminal],
    pub expected_sign: &'a [ExpectedSign],
    pub sign_storage_budget: &'a SignStorageBudget,
    pub states: &'a [PlannedStateBoundary],
}
impl<'a> From<&'a PlanFragment> for FragmentFingerprintHeader<'a> {
    fn from(f: &'a PlanFragment) -> Self {
        Self {
            source_document_id: &f.source_document_id,
            checked_plot_id: &f.checked_plot_id,
            expanded_plot_id: &f.expanded_plot_id,
            completion_policy: f.completion_policy,
            realization_backs: &f.realization_backs,
            host_id: &f.host_id,
            boot_id: &f.boot_id,
            offer_generation: f.offer_generation,
            execution_regions: &f.execution_regions,
            execution_fusions: &f.execution_fusions,
        }
    }
}
impl<'a> From<&'a PlanFragment> for FragmentFingerprintTail<'a> {
    fn from(f: &'a PlanFragment) -> Self {
        Self {
            connections: &f.connections,
            fore_ports: &f.fore_ports,
            shared_pools: &f.shared_pools,
            startup_dependencies: &f.startup_dependencies,
            startup_order: &f.startup_order,
            cancellation_policy: f.cancellation_policy,
            terminal_policy: f.terminal_policy,
            expected_terminals: &f.expected_terminals,
            expected_sign: &f.expected_sign,
            sign_storage_budget: &f.sign_storage_budget,
            states: &f.states,
        }
    }
}

/// Borrowed canonical header; descriptive input only, never checked Plan authority.
#[derive(Clone, Copy)]
pub struct FragmentFingerprintHeaderRef<'a> {
    pub source_document_id: &'a str,
    pub checked_plot_id: &'a str,
    pub expanded_plot_id: &'a str,
    pub completion_policy: PlanCompletionPolicy,
    pub realization_backs: &'a [PlotBack],
    pub host_id: &'a str,
    pub boot_id: &'a str,
    pub offer_generation: OfferGeneration,
    pub execution_regions: &'a [ExecutionRegion],
    pub execution_fusions: &'a [PlannedFusion],
}
impl<'a> From<FragmentFingerprintHeader<'a>> for FragmentFingerprintHeaderRef<'a> {
    fn from(v: FragmentFingerprintHeader<'a>) -> Self {
        Self {
            source_document_id: v.source_document_id.as_str(),
            checked_plot_id: v.checked_plot_id.as_str(),
            expanded_plot_id: v.expanded_plot_id.as_str(),
            completion_policy: v.completion_policy,
            realization_backs: v.realization_backs,
            host_id: v.host_id.as_str(),
            boot_id: v.boot_id.as_str(),
            offer_generation: v.offer_generation,
            execution_regions: v.execution_regions,
            execution_fusions: v.execution_fusions,
        }
    }
}
impl<'a> From<&'a PlanFragment> for FragmentFingerprintHeaderRef<'a> {
    fn from(v: &'a PlanFragment) -> Self {
        FragmentFingerprintHeader::from(v).into()
    }
}
/// Complete tail input; optional nonlocal/state/pool facts remain fully borrowed.
/// Public construction confers neither structural verification nor possession.
#[derive(Clone, Copy)]
pub struct FragmentFingerprintTailRef<'a> {
    pub connections: PlannedConnectionRefs<'a>,
    pub fore_ports: &'a [PlannedForePort],
    pub shared_pools: &'a [PlannedSharedPool],
    pub startup_dependencies: StartupDependencyRefs<'a>,
    pub startup_order: PlacementIdRefs<'a>,
    pub cancellation_policy: CancellationPolicy,
    pub terminal_policy: TerminalPolicy,
    pub expected_terminals: ExpectedTerminalRefs<'a>,
    pub expected_sign: ExpectedSignRefs<'a>,
    pub sign_storage_budget: &'a SignStorageBudget,
    pub states: &'a [PlannedStateBoundary],
}
impl<'a> From<FragmentFingerprintTail<'a>> for FragmentFingerprintTailRef<'a> {
    fn from(v: FragmentFingerprintTail<'a>) -> Self {
        Self {
            connections: PlannedConnectionRefs::Owned(v.connections),
            fore_ports: v.fore_ports,
            shared_pools: v.shared_pools,
            startup_dependencies: StartupDependencyRefs::Owned(v.startup_dependencies),
            startup_order: PlacementIdRefs::Owned(v.startup_order),
            cancellation_policy: v.cancellation_policy,
            terminal_policy: v.terminal_policy,
            expected_terminals: ExpectedTerminalRefs::Owned(v.expected_terminals),
            expected_sign: ExpectedSignRefs::Owned(v.expected_sign),
            sign_storage_budget: v.sign_storage_budget,
            states: v.states,
        }
    }
}
impl<'a> From<&'a PlanFragment> for FragmentFingerprintTailRef<'a> {
    fn from(v: &'a PlanFragment) -> Self {
        FragmentFingerprintTail::from(v).into()
    }
}
