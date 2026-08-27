//! Fixed production-kernel execution for the complete PREWAKE robotics family.

use alloc::vec::Vec;
use conduit_kernel::scheduler::{CordSpec, FixedScheduler, OperationDriver, SchedulerStatus};
use conduit_kernel::{
    FixedHostOperationBindings, FixedRoutes, FixedSignLog, FixedValueStore, NodeId, ValueStorage,
};
use conduit_plan_lowering::lowering::{FIXED_KERNEL_STORAGE_PORTS_PER_NODE, lower_plan_fragment};

use super::{
    operation::PresentationOperation,
    robotics_operation::RoboticsDiscardOperation,
    robotics_operation::{RoboticsDriveEffect, RoboticsDriveOperation, RoboticsSourceOperation},
    robotics_plan::{
        BATTERY_SINK_KIND, BUMP_SINK_KIND, IMU_SINK_KIND, ODOMETRY_SINK_KIND, PreparedRobotics,
        RANGE_SINK_KIND,
    },
};

const PORTS: usize = FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
const NODES: usize = 12;
const CORDS: usize = 7;
const ROUTES: usize = NODES * PORTS;
const HOST_BINDINGS: usize = NODES * NODES;
const VALUES: usize = 8;
const MAX_VALUE_BYTES: usize = conduit_core::ROBOTICS_ODOMETRY_ENCODED_LEN;
const VALUE_BYTES: usize = VALUES * MAX_VALUE_BYTES;
const SIGNS: usize = 160;

type Kernel = FixedScheduler<
    OperationDriver<PresentationOperation, PORTS>,
    FixedValueStore<VALUES, MAX_VALUE_BYTES>,
    FixedSignLog<SIGNS>,
    NODES,
    CORDS,
    PORTS,
    CORDS,
    ROUTES,
    CORDS,
    HOST_BINDINGS,
    NODES,
>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RoboticsError {
    Catalog,
    Form,
    Placement,
    Plan,
    Lowering,
    Shape,
    Kernel(conduit_kernel::scheduler::SchedulerError),
    Value,
    Configuration,
    Idle(Option<RoboticsDriveEffect>),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RoboticsProof {
    pub plan_id: conduit_core::PlanId,
    pub effect: RoboticsDriveEffect,
    pub node_count: u8,
    pub cord_count: u8,
}

pub fn run_robotics(prepared: &PreparedRobotics) -> Result<RoboticsProof, RoboticsError> {
    let fragment = prepared
        .plan
        .fragments
        .first()
        .ok_or(RoboticsError::Shape)?;
    let lowered = lower_plan_fragment(fragment).map_err(|_| RoboticsError::Lowering)?;
    if lowered.nodes.len() != NODES
        || lowered.cords.len() != CORDS
        || !lowered.remote_endpoints.is_empty()
    {
        return Err(RoboticsError::Shape);
    }
    let (mut kernel, drive) = scheduler(fragment, &lowered)?;
    loop {
        if kernel.next_host_request().is_some() {
            return Err(RoboticsError::Shape);
        }
        match kernel.step().map_err(RoboticsError::Kernel)? {
            SchedulerStatus::Progress { .. } => {}
            SchedulerStatus::Complete => break,
            SchedulerStatus::Idle => {
                return Err(RoboticsError::Idle(
                    kernel.drivers()[usize::from(drive.0)]
                        .operation()
                        .robotics_effect(),
                ));
            }
            SchedulerStatus::Cancelled => {
                return Err(RoboticsError::Kernel(
                    conduit_kernel::scheduler::SchedulerError::Cancelled,
                ));
            }
        }
    }
    let effect = kernel.drivers()[usize::from(drive.0)]
        .operation()
        .robotics_effect()
        .ok_or(RoboticsError::Shape)?;
    Ok(RoboticsProof {
        plan_id: prepared.plan.plan_id.clone(),
        effect,
        node_count: NODES as u8,
        cord_count: CORDS as u8,
    })
}

fn scheduler(
    fragment: &conduit_core::PlanFragment,
    lowered: &conduit_plan_lowering::lowering::LoweredPlanFragment,
) -> Result<(Kernel, NodeId), RoboticsError> {
    let nodes = lowered
        .node_specs
        .as_slice()
        .try_into()
        .map_err(|_| RoboticsError::Shape)?;
    let cords: [CordSpec; CORDS] = lowered
        .cords
        .iter()
        .map(|cord| cord.spec)
        .collect::<Vec<_>>()
        .try_into()
        .map_err(|_| RoboticsError::Shape)?;
    let mut routes = FixedRoutes::<ROUTES, CORDS>::new(PORTS as u16);
    for route in &lowered.routes {
        routes
            .install(
                route.source_node,
                route.source_port,
                route.range,
                &route.targets,
            )
            .map_err(|_| {
                RoboticsError::Kernel(conduit_kernel::scheduler::SchedulerError::InvalidPlan)
            })?;
    }
    routes.seal().map_err(|_| {
        RoboticsError::Kernel(conduit_kernel::scheduler::SchedulerError::InvalidPlan)
    })?;
    let mut bindings = FixedHostOperationBindings::<HOST_BINDINGS>::new(NODES as u16);
    for operation in &lowered.host_operations {
        bindings
            .install(operation.node, operation.binding)
            .map_err(|_| {
                RoboticsError::Kernel(conduit_kernel::scheduler::SchedulerError::InvalidPlan)
            })?;
    }
    bindings.seal().map_err(|_| {
        RoboticsError::Kernel(conduit_kernel::scheduler::SchedulerError::InvalidPlan)
    })?;
    let mut values = FixedValueStore::<VALUES, MAX_VALUE_BYTES>::new(VALUE_BYTES as u32)
        .map_err(|_| RoboticsError::Value)?;
    let mut drive = None;
    let drivers = fragment
        .placements
        .iter()
        .enumerate()
        .map(|(index, placement)| {
            let operation = if matches!(
                placement.kind_id.as_str(),
                BUMP_SINK_KIND
                    | RANGE_SINK_KIND
                    | IMU_SINK_KIND
                    | ODOMETRY_SINK_KIND
                    | BATTERY_SINK_KIND
            ) {
                PresentationOperation::RoboticsDiscard(RoboticsDiscardOperation::new())
            } else if placement.kind_id.as_str()
                == conduit_std_catalog::ROBOTICS_DRIVE_DIFFERENTIAL_KIND
            {
                drive = Some(NodeId(index as u16));
                PresentationOperation::RoboticsDrive(RoboticsDriveOperation::new())
            } else {
                let encoded = conduit_std_catalog::robotics_simulation_values(
                    placement.kind_id.as_str(),
                    &placement.configuration,
                )
                .map_err(|_| RoboticsError::Configuration)?;
                let mut prepared = [None; 2];
                for (slot, canonical) in prepared.iter_mut().zip(encoded.iter()) {
                    if let Some(canonical) = canonical {
                        *slot = Some(values.store(canonical).map_err(|_| RoboticsError::Value)?);
                    }
                }
                PresentationOperation::RoboticsSource(RoboticsSourceOperation {
                    availability: conduit_std_catalog::robotics_simulation_availability(
                        &placement.configuration,
                    )
                    .map_err(|_| RoboticsError::Configuration)?,
                    values: prepared,
                    next: 0,
                    cancelled: false,
                })
            };
            OperationDriver::new(operation).map_err(RoboticsError::Kernel)
        })
        .collect::<Result<Vec<_>, _>>()?
        .try_into()
        .map_err(|_| RoboticsError::Shape)?;
    let signs = FixedSignLog::<SIGNS>::new(
        (SIGNS * core::mem::size_of::<conduit_kernel::KernelEvent>()) as u32,
    )
    .map_err(|_| RoboticsError::Kernel(conduit_kernel::scheduler::SchedulerError::InvalidPlan))?;
    let kernel = FixedScheduler::new_with_host_operations(
        nodes, cords, routes, bindings, drivers, values, signs,
    )
    .map_err(RoboticsError::Kernel)?;
    Ok((kernel, drive.ok_or(RoboticsError::Shape)?))
}
