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
