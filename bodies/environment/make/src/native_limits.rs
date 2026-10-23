//! Finite projection ceilings for the resident local sensor workload. These
//! bound construction artifacts; Root still admits complete runtime storage.
use crate::NativeApplianceLimits;
use conduit_environment_runtime::appliance::assigned::{CORDS, NODES, PORTS};
impl NativeApplianceLimits {
    /// The resident sensor profile: fifteen nodes, fourteen local cords and
    /// two native resources (pulse attachment and monotonic timer). No remote
    /// endpoints or resource cords are supported. This is not memory admission.
    pub fn resident_local() -> Self {
        Self {
            image: conduit_embedded_build::EmbeddedImageBounds {
                maximum_nodes: NODES,
                maximum_cords: CORDS,
                maximum_routes: NODES,
                maximum_route_targets: CORDS,
                maximum_host_calls: NODES,
                maximum_resources: 2,
                maximum_sign_expectations: 128,
                maximum_configuration_entries: 16,
                maximum_ports_per_node: PORTS,
                maximum_remote_endpoints: 0,
                maximum_cord_value_slots: CORDS as u16,
                maximum_cord_value_bytes: 32768,
                maximum_sign_items: 128,
                maximum_sign_bytes: 32768,
            },
            computations: conduit_embedded_build::EmbeddedComputationBounds {
                maximum_program_bytes: 32768,
                maximum_total_program_bytes: 65536,
            },
            assignment: conduit_core::AssignedPlanMaxima {
                encoded_bytes: 48 * 1024,
                runtime_state_bytes: 32 * 1024,
                counts: [16, 32, 16, 16, 16, 16, 8, 2, 128, 0, 32, 32, 0],
            },
        }
    }
}
