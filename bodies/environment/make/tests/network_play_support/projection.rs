//! Real Make output and ordinary allocation-free assigned-kernel preparation.
use conduit_core as c;
use conduit_environment_make::{PreparedNativeNetworkSetup, PreparedNetworkSetup};
use conduit_environment_runtime::assigned::{AssignedKernelTables, ResidentLayout};
use conduit_kernel::NodeId;
use conduit_plan_lowering::selected_assigned_operation::SelectedAssignedOperationPlan;
const P: usize = conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
pub struct Projection {
    pub native: PreparedNativeNetworkSetup,
    maxima: c::AssignedPlanMaxima,
    calls: Vec<c::AssignedIdentity>,
    resources: Vec<u16>,
    counts: [u8; c::ASSIGNED_PLAN_COUNT_KINDS],
}
impl Projection {
    pub fn prepare(checked: PreparedNetworkSetup) -> Self {
        let maxima = c::AssignedPlanMaxima {
            encoded_bytes: 8192,
            runtime_state_bytes: 32768,
            counts: [32; c::ASSIGNED_PLAN_COUNT_KINDS],
        };
        let native = checked
            .prepare_native(
                conduit_embedded_build::EmbeddedImageBounds {
                    maximum_nodes: 3,
                    maximum_cords: 2,
                    maximum_routes: 3 * P,
                    maximum_route_targets: 2,
                    maximum_host_calls: 3,
                    maximum_resources: 2,
                    maximum_sign_expectations: 64,
                    maximum_configuration_entries: 0,
                    maximum_ports_per_node: P,
                    maximum_remote_endpoints: 0,
                    maximum_cord_value_slots: 8,
                    maximum_cord_value_bytes: 8 * 167,
                    maximum_sign_items: 128,
                    maximum_sign_bytes: 32768,
                },
                maxima,
            )
            .unwrap();
        let calls = native
            .image()
            .host_calls
            .iter()
            .map(|call| c::AssignedIdentity::from_text(&call.contract_id))
            .collect();
        let resources = native
            .image()
            .resources
            .iter()
            .map(|resource| resource.resource)
            .collect();
        let mut result = Self {
            native,
            maxima,
            calls,
            resources,
            counts: [0; c::ASSIGNED_PLAN_COUNT_KINDS],
        };
        result.counts = c::decode_assigned_plan_with_computations(
            result.native.assignment(),
            maxima,
            result.requirements(),
            Self::computations(),
        )
        .unwrap()
        .counts;
        result
    }
    fn requirements(&self) -> c::AssignedPlanRequirements<'_> {
        c::AssignedPlanRequirements {
            host: c::AssignedIdentity::from_text(&self.native.image().host_id),
            boot: c::AssignedIdentity::from_text(&self.native.image().boot_id),
            host_calls: &self.calls,
            resources: &self.resources,
            remote_bindings: &[],
        }
    }
    fn computations() -> c::AssignedComputationProfile<'static> {
        c::AssignedComputationProfile {
            maximum_text_bytes: 0,
            maximum_total_text_bytes: 0,
            requirements: &[],
        }
    }
    pub fn selected<'a>(
        &'a self,
        active: &'a c::ActivePlayIdentity,
        node: NodeId,
    ) -> SelectedAssignedOperationPlan<'a> {
        SelectedAssignedOperationPlan {
            bytes: self.native.assignment(),
            maxima: self.maxima,
            requirements: self.requirements(),
            computations: Self::computations(),
            fragment_id: &self.native.checked().plan().fragments[0].fragment_id,
            active,
            node,
        }
    }
    pub fn tables(&self) -> AssignedKernelTables<3, 2, P, { 3 * P }, 2, 3> {
        let layout = ResidentLayout {
            records: self.native.resident_records(),
            counts: self.counts,
        };
        let prepare = |required, layout: &ResidentLayout<'_>| {
            AssignedKernelTables::<3, 2, P, { 3 * P }, 2, 3>::prepare(
                self.native.assignment(),
                self.maxima,
                required,
                Self::computations(),
                layout,
            )
        };
        let tables = prepare(self.requirements(), &layout).unwrap();
        let mut foreign = self.requirements();
        foreign.boot = c::AssignedIdentity::from_text("boot/foreign");
        assert!(prepare(foreign, &layout).is_err());
        assert!(prepare(
            self.requirements(),
            &ResidentLayout {
                records: &[],
                counts: self.counts
            }
        )
        .is_err());
        tables
    }
}
