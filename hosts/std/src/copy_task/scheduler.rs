use super::operation::{CopyOperation, CopyResultSink, CopyTaskOperation};
use conduit_core::PlanFragment;
use conduit_kernel::scheduler::{FixedScheduler, OperationDriver};
use conduit_kernel::{
    FixedHostOperationBindings, FixedRoutes, HostedSignLog, HostedValueStore, ValueStorage,
};
use conduit_runtime::lowering::{lower_plan_fragment, MAXIMUM_KERNEL_PORTS_PER_NODE};

const MAX_SIGN_ITEMS: u16 = 20_000;
const PORTS: usize = MAXIMUM_KERNEL_PORTS_PER_NODE;

pub(super) type CopyScheduler = FixedScheduler<
    OperationDriver<CopyTaskOperation, PORTS>,
    HostedValueStore,
    HostedSignLog,
    2,
    1,
    PORTS,
    1,
    { 2 * PORTS },
    1,
    4,
    2,
>;

pub(super) fn prepare_copy_scheduler(
    fragment: &PlanFragment,
    expected_bytes: u64,
) -> Result<(CopyScheduler, Vec<u8>), String> {
    let lowered =
        lower_plan_fragment(fragment).map_err(|error| format!("lower copy: {error:?}"))?;
    if lowered.nodes.len() != 2 || lowered.cords.len() != 1 || lowered.host_operations.len() != 2 {
        return Err("lowered copy shape is not two nodes, one Cord, two host operations".into());
    }
    let maximum = conduit_core::MAXIMUM_STRUCTURED_CANONICAL_BYTES as u32;
    let mut values = HostedValueStore::new(3, maximum, maximum * 2 + 1)
        .map_err(|error| format!("prepare copy values: {error:?}"))?;
    let command = values
        .store(&[0])
        .map_err(|error| format!("store copy command: {error:?}"))?;
    let success = conduit_std_catalog::copy_success_value(expected_bytes)?;
    let success_encoded = success
        .canonical_bytes()
        .map_err(|error| format!("encode admitted copy result: {error:?}"))?;
    let mut drivers = Vec::with_capacity(2);
    for node in &lowered.nodes {
        let placement = &fragment.placements[usize::from(node.node.0)];
        let operation = if placement.kind_id.as_str() == conduit_std_catalog::COPY_FILE_KIND {
            CopyTaskOperation::Copy(CopyOperation::new(command))
        } else if placement.implementation_id.as_str()
            == conduit_std_catalog::COPY_RESULT_PRESENTATION_IMPLEMENTATION
        {
            CopyTaskOperation::Sink(CopyResultSink::new())
        } else {
            return Err("copy Plan selected an unsupported operation".into());
        };
        drivers.push(
            OperationDriver::new(operation)
                .map_err(|error| format!("prepare copy operation: {error:?}"))?,
        );
    }
    let drivers = drivers
        .try_into()
        .map_err(|_| "copy driver table is incomplete")?;
    let mut routes = FixedRoutes::<{ 2 * PORTS }, 1>::new(PORTS as u16);
    for route in &lowered.routes {
        routes
            .install(
                route.source_node,
                route.source_port,
                route.range,
                &route.targets,
            )
            .map_err(|error| format!("install copy route: {error:?}"))?;
    }
    routes
        .seal()
        .map_err(|error| format!("seal copy routes: {error:?}"))?;
    let mut bindings = FixedHostOperationBindings::<4>::new(2);
    for operation in &lowered.host_operations {
        bindings
            .install(operation.node, operation.binding)
            .map_err(|error| format!("install copy host operation: {error:?}"))?;
    }
    bindings
        .seal()
        .map_err(|error| format!("seal copy host operation: {error:?}"))?;
    let sign_bytes = u32::from(MAX_SIGN_ITEMS)
        .checked_mul(core::mem::size_of::<conduit_kernel::KernelEvent>() as u32)
        .ok_or_else(|| "copy sign byte budget overflow".to_string())?;
    let sign = HostedSignLog::new(MAX_SIGN_ITEMS, sign_bytes)
        .map_err(|error| format!("prepare copy sign: {error:?}"))?;
    let scheduler = CopyScheduler::new_with_active_counts_and_host_operations(
        2,
        1,
        lowered
            .node_specs
            .as_slice()
            .try_into()
            .map_err(|_| "copy node specs")?,
        [lowered.cords[0].spec],
        routes,
        bindings,
        drivers,
        values,
        sign,
    )
    .map_err(|error| format!("prepare copy scheduler: {error:?}"))?;
    Ok((scheduler, success_encoded))
}
