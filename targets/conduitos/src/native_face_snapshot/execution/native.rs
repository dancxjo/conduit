//! Root retains reviewed snapshot Fore identities; guest constructs the scheduler.
use super::*;
use conduitos_runtime_core::{admission::Admission as Numeric, snapshot::Admission};
pub(super) fn prepare(
    lowered: &LoweredPlanFragment,
    input: &LoweredForePort,
    output: &LoweredForePort,
) -> Result<crate::protected_runtime::snapshot::Snapshot, FaceSnapshotRefusal> {
    let mut numeric = Numeric::new(1, 2);
    numeric.nodes[0] = lowered.node_specs[0];
    numeric.cords[0] = lowered.cords[0].spec;
    numeric.cords[1] = lowered.cords[1].spec;
    for route in &lowered.routes {
        numeric
            .install_route(
                route.source_node,
                route.source_port,
                route.range,
                &route.targets,
            )
            .map_err(|_| FaceSnapshotRefusal::Shape)?;
    }
    crate::protected_runtime::snapshot::Snapshot::new(Admission {
        numeric,
        input_endpoint: input.endpoint,
        input_cord: input.cord,
        output_endpoint: output.endpoint,
        output_cord: output.cord,
    })
    .map_err(|_| FaceSnapshotRefusal::Kernel)
}
