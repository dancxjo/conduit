//! Stateful semantic selection and terminal drive operation for the capstone.

use conduit_kernel::{
    scheduler::{StepInputBytes, StepIo, StepOperation, StepOutcome},
    BoundedValueRef, HostCallDisposition, HostCallId, PortId, RequestId,
};

const OPERATION: HostCallId = HostCallId(0);
pub(super) const DRIVE_REQUEST: RequestId = RequestId(1);
const SCALAR_BYTES: u32 = conduit_core::SCALAR_ENCODED_LEN as u32;

#[derive(Clone, Copy)]
pub(super) struct CurrentSelector {
    pub(super) selector: Option<bool>,
    pub(super) candidates: [Option<[u8; conduit_core::SCALAR_ENCODED_LEN]>; 2],
    pub(super) closed: [bool; 3],
}

impl StepOperation<3> for CurrentSelector {
    fn step(&mut self, io: &mut StepIo<3>, bytes: &StepInputBytes<'_, 3>) -> StepOutcome {
        for index in 0..3 {
            let port = PortId(index as u16);
            if let Some(canonical) = bytes.input(port) {
                if self.closed[index] {
                    return invalid(8);
                }
                match index {
                    0 => {
                        let Ok(value) = conduit_core::InfoBool::decode(canonical) else {
                            return invalid(6);
                        };
                        self.selector = Some(value.get());
                    }
                    1 | 2 => {
                        if conduit_core::Scalar::decode(canonical).is_err() {
                            return invalid(7);
                        }
                        self.candidates[index - 1] = Some(
                            canonical
                                .try_into()
                                .expect("decoded Scalar has the exact canonical length"),
                        );
                    }
                    _ => unreachable!(),
                }
                io.consume(port).expect("present selector input");
                if let Some(selector) = self.selector {
                    if self.candidates.iter().all(Option::is_some) {
                        if !io.output_ready(PortId(0)) {
                            return StepOutcome::Await;
                        }
                        let value = conduit_kernel::CanonicalValue::new(
                            &self.candidates[usize::from(selector)]
                                .expect("both candidates are present"),
                        )
                        .expect("Scalar fits the derived-value bound");
                        io.send_canonical(PortId(0), value)
                            .expect("ready selector Cord");
                        self.selector = None;
                        self.candidates = [None; 2];
                    }
                }
                return StepOutcome::Progress;
            }
        }
        for index in 0..3 {
            let port = PortId(index as u16);
            if !self.closed[index] && io.input_closed(port) {
                io.consume_closed(port).expect("observed selector closure");
                self.closed[index] = true;
                return if self.closed.into_iter().all(|closed| closed) {
                    StepOutcome::Complete
                } else {
                    StepOutcome::Progress
                };
            }
        }
        StepOutcome::Await
    }

    fn cancel(&mut self) {
        self.selector = None;
        self.candidates = [None; 2];
    }
}

#[derive(Clone, Copy)]
pub(super) struct DriveSink {
    pub(super) angular_is_zero: bool,
    pub(super) closed: [bool; 2],
    pub(super) pending: bool,
    pub(super) completed: bool,
}

impl StepOperation<3> for DriveSink {
    fn step(&mut self, io: &mut StepIo<3>, bytes: &StepInputBytes<'_, 3>) -> StepOutcome {
        if let Some((request, outcome)) = io.host_completion() {
            if request != DRIVE_REQUEST || !self.pending || outcome.output.is_some() {
                return invalid(9);
            }
            io.consume_host_completion()
                .expect("observed capstone drive completion");
            self.pending = false;
            match (outcome.disposition, outcome.failure) {
                (HostCallDisposition::Completed, None) => self.completed = true,
                (HostCallDisposition::Failed, Some(failure)) => return StepOutcome::Fail(failure),
                _ => return invalid(9),
            }
            return if self.closed.into_iter().all(|closed| closed) {
                StepOutcome::Complete
            } else {
                StepOutcome::Progress
            };
        }
        if !self.pending && !self.completed {
            if let (Some(linear_bytes), Some(angular_bytes)) =
                (bytes.input(PortId(0)), bytes.input(PortId(1)))
            {
                if conduit_core::Scalar::decode(linear_bytes).is_err() {
                    return invalid(10);
                }
                let Ok(angular) = conduit_core::Scalar::decode(angular_bytes) else {
                    return invalid(10);
                };
                if angular.raw_microunits() != 0 {
                    return invalid(11);
                }
                let linear = io.consume(PortId(0)).expect("present linear input");
                io.consume(PortId(1)).expect("present angular input");
                self.angular_is_zero = true;
                io.request_host_call(
                    DRIVE_REQUEST,
                    OPERATION,
                    BoundedValueRef::new(linear, 2 * SCALAR_BYTES)
                        .expect("linear input is within the drive request bound"),
                )
                .expect("planned capstone drive Host Call");
                self.pending = true;
                return StepOutcome::Progress;
            }
        }
        for index in 0..2 {
            let port = PortId(index as u16);
            if !self.closed[index] && io.input_closed(port) {
                io.consume_closed(port).expect("observed drive closure");
                self.closed[index] = true;
                return if self.completed && self.closed.into_iter().all(|closed| closed) {
                    StepOutcome::Complete
                } else {
                    StepOutcome::Progress
                };
            }
        }
        StepOutcome::Await
    }

    fn cancel(&mut self) {
        self.pending = false;
        self.completed = false;
    }
}

const fn invalid(detail: u16) -> StepOutcome {
    StepOutcome::Fail(conduit_kernel::Failure {
        code: conduit_kernel::FailureCode::InvalidLifecycle,
        detail,
    })
}
