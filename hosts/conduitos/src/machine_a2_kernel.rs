//! One finite, pre-admitted cooperative lane for the AArch64 A2 proof.
//!
//! The architecture supplies only an exact timer completion fact. The
//! production kernel remains the sole owner of operation lifecycle and
//! terminal progress.

use conduit_kernel::{
    BoundedValueRef, CordId, FixedHostOperationBindings, FixedRoutes, FixedSignLog,
    FixedValueStore, HostOperationBinding, HostOperationDisposition, HostOperationId,
    HostOperationOutcome, KernelEvent, NodeId, Operation, OperationAction, OperationInput, PortId,
    RequestId, RouteRange, RouteTarget, SignSink, ValueStorage,
    scheduler::{
        CordCapacity, CordSpec, FixedScheduler, HostOperationRequest, NodeSpec, OperationDriver,
        SchedulerError, SchedulerStatus,
    },
};

use crate::machine::KernelInterest;

pub const TIMER_NODE: NodeId = NodeId(0);
pub const TIMER_REQUEST: RequestId = RequestId(1);
pub const TIMER_OPERATION: HostOperationId = HostOperationId(0);
#[cfg(target_arch = "aarch64")]
pub const LANE_ID: &str = "lane/aarch64/cooperative/0";
#[cfg(target_arch = "x86")]
pub const LANE_ID: &str = "lane/ia32/cooperative/0";
const PORTS: usize = 1;
const SIGN_CAPACITY: usize = 16;

#[derive(Clone, Copy)]
struct WaitOperation {
    duration: BoundedValueRef,
    completed: bool,
}

impl Operation for WaitOperation {
    fn start(&mut self) -> OperationAction {
        OperationAction::RequestHostOperation {
            request: TIMER_REQUEST,
            operation: TIMER_OPERATION,
            input: self.duration,
        }
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::HostOperationCompleted { request, outcome }
                if request == TIMER_REQUEST
                    && outcome.disposition == HostOperationDisposition::Completed
                    && outcome.output.is_none()
                    && outcome.failure.is_none() =>
            {
                self.completed = true;
                OperationAction::Complete
            }
            _ => OperationAction::Fail(conduit_kernel::Failure {
                code: conduit_kernel::FailureCode::HostOperationFailed,
                detail: 0xa2,
            }),
        }
    }

    fn cancel(&mut self) {}
}

type Scheduler = FixedScheduler<
    OperationDriver<WaitOperation, PORTS>,
    FixedValueStore<1, 8>,
    FixedSignLog<SIGN_CAPACITY>,
    1,
    1,
    PORTS,
    1,
    1,
    1,
    1,
    1,
>;

pub struct AdmittedLane {
    scheduler: Scheduler,
}

impl AdmittedLane {
    pub fn new() -> Result<Self, SchedulerError> {
        let mut values = FixedValueStore::<1, 8>::new(8)?;
        let duration = values.store(&1_u64.to_le_bytes())?;
        let mut routes = FixedRoutes::<1, 1>::new(PORTS as u16);
        routes.install(
            TIMER_NODE,
            PortId(0),
            RouteRange { start: 0, len: 1 },
            &[RouteTarget {
                cord: CordId(0),
                sink: conduit_kernel::CordEndpoint::local(TIMER_NODE, PortId(0)),
            }],
        )?;
        routes.seal()?;
        let mut bindings = FixedHostOperationBindings::<1>::new(1);
        bindings.install(
            TIMER_NODE,
            HostOperationBinding {
                operation: TIMER_OPERATION,
                maximum_input_bytes: 8,
                maximum_output_bytes: 0,
            },
        )?;
        bindings.seal()?;
        let driver = OperationDriver::new(WaitOperation {
            duration: BoundedValueRef::new(duration, 8)?,
            completed: false,
        })?;
        let sign_bytes = (SIGN_CAPACITY * core::mem::size_of::<KernelEvent>()) as u32;
        Ok(Self {
            scheduler: FixedScheduler::new_with_host_operations(
                [NodeSpec {
                    input_cords: [Some(CordId(0)); PORTS],
                    maximum_step_work: 1,
                }],
                [CordSpec::local(
                    CordId(0),
                    (TIMER_NODE, PortId(0)),
                    (TIMER_NODE, PortId(0)),
                    CordCapacity {
                        slot_start: 0,
                        item_capacity: 1,
                        byte_capacity: 8,
                    },
                )],
                routes,
                bindings,
                [driver],
                values,
                FixedSignLog::new(sign_bytes)?,
            )?,
        })
    }

    pub fn step(&mut self) -> Result<SchedulerStatus, SchedulerError> {
        self.scheduler.step()
    }

    pub fn take_timer_interest(&mut self) -> Result<KernelInterest, SchedulerError> {
        let request = self
            .scheduler
            .next_host_request()
            .ok_or(SchedulerError::InvalidHostOperationAccess)?;
        Self::validate_timer_request(request)
    }

    fn validate_timer_request(
        request: HostOperationRequest,
    ) -> Result<KernelInterest, SchedulerError> {
        if request.node != TIMER_NODE
            || request.request != TIMER_REQUEST
            || request.operation != TIMER_OPERATION
        {
            return Err(SchedulerError::InvalidHostOperationAccess);
        }
        Ok(KernelInterest {
            node: request.node,
            request: request.request,
            input: request.input,
        })
    }

    pub fn complete_timer(&mut self, interest: KernelInterest) -> Result<(), SchedulerError> {
        if interest.node != TIMER_NODE || interest.request != TIMER_REQUEST {
            return Err(SchedulerError::InvalidHostOperationAccess);
        }
        self.scheduler.complete_host_operation(
            interest.node,
            interest.request,
            HostOperationOutcome {
                disposition: HostOperationDisposition::Completed,
                output: None,
                failure: None,
            },
        )
    }

    pub fn decisions(&self) -> u32 {
        self.scheduler.decisions()
    }
    pub fn signs(&self) -> u16 {
        self.scheduler.signs().len()
    }
    pub fn pending(&self) -> usize {
        self.scheduler.pending_host_operation_count()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_timer_fact_is_required_for_terminal_kernel_progress() {
        let mut lane = AdmittedLane::new().unwrap();
        assert!(matches!(
            lane.step().unwrap(),
            SchedulerStatus::Progress { .. }
        ));
        let interest = lane.take_timer_interest().unwrap();
        let stale = KernelInterest {
            request: RequestId(9),
            ..interest
        };
        assert_eq!(
            lane.complete_timer(stale),
            Err(SchedulerError::InvalidHostOperationAccess)
        );
        lane.complete_timer(interest).unwrap();
        assert!(matches!(lane.step().unwrap(), SchedulerStatus::Complete));
        assert_eq!(lane.pending(), 0);
    }
}
