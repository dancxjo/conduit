use conduit_kernel::{
    scheduler::{StepInputBytes, StepIo, StepOperation, StepOutcome},
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, PortId, RequestId,
};

use crate::network_image::PORTS;

pub(crate) struct JoinBack {
    input_port: PortId,
    output_port: PortId,
    operation: conduit_kernel::HostCallId,
    pending: Option<RequestId>,
    completed: bool,
}

impl StepOperation<PORTS> for JoinBack {
    fn step(
        &mut self,
        io: &mut StepIo<PORTS>,
        _input_bytes: &StepInputBytes<'_, PORTS>,
    ) -> StepOutcome {
        if self.pending.is_none() {
            if let Some(value) = io.input(self.input_port) {
                let request = RequestId(0);
                if io.consume(self.input_port).is_err()
                    || io
                        .request_host_call(
                            request,
                            self.operation,
                            BoundedValueRef::new(value, conduit_net::MAXIMUM_JOIN_INPUT_BYTES)
                                .expect("planned join input is exactly bounded"),
                        )
                        .is_err()
                {
                    return fail(1);
                }
                self.pending = Some(request);
                return StepOutcome::Progress;
            }
            if io.input_closed(self.input_port) && self.completed {
                if io.consume_closed(self.input_port).is_err() {
                    return fail(1);
                }
                return StepOutcome::Complete;
            }
            return StepOutcome::Await;
        }

        let Some((request, outcome)) = io.host_completion() else {
            return StepOutcome::Await;
        };
        let Some(output) = outcome.output else {
            return fail(1);
        };
        if self.pending != Some(request)
            || outcome.disposition != HostCallDisposition::Completed
            || outcome.failure.is_some()
        {
            return fail(1);
        }
        if !io.output_ready(self.output_port) {
            return StepOutcome::Await;
        }
        if io.consume_host_completion().is_err() || io.send(self.output_port, output.value).is_err() {
            return fail(1);
        }
        self.pending = None;
        self.completed = true;
        StepOutcome::Progress
    }
}

pub(crate) struct AttachmentSignBack {
    input_port: PortId,
    operation: conduit_kernel::HostCallId,
    pending: Option<RequestId>,
    completed: bool,
}

impl StepOperation<PORTS> for AttachmentSignBack {
    fn step(
        &mut self,
        io: &mut StepIo<PORTS>,
        _input_bytes: &StepInputBytes<'_, PORTS>,
    ) -> StepOutcome {
        if self.pending.is_none() {
            if let Some(value) = io.input(self.input_port) {
                let request = RequestId(0);
                if io.consume(self.input_port).is_err()
                    || io
                        .request_host_call(
                            request,
                            self.operation,
                            BoundedValueRef::new(value, conduit_net::MAXIMUM_JOIN_OUTPUT_BYTES)
                                .expect("planned attachment Info is exactly bounded"),
                        )
                        .is_err()
                {
                    return fail(2);
                }
                self.pending = Some(request);
                return StepOutcome::Progress;
            }
            if io.input_closed(self.input_port) && self.completed {
                if io.consume_closed(self.input_port).is_err() {
                    return fail(2);
                }
                return StepOutcome::Complete;
            }
            return StepOutcome::Await;
        }

        let Some((request, outcome)) = io.host_completion() else {
            return StepOutcome::Await;
        };
        if self.pending != Some(request)
            || outcome.disposition != HostCallDisposition::Completed
            || outcome.output.is_some()
            || outcome.failure.is_some()
            || io.consume_host_completion().is_err()
        {
            return fail(2);
        }
        self.pending = None;
        self.completed = true;
        StepOutcome::Progress
    }
}

pub enum NetworkBack {
    Join(JoinBack),
    AttachmentSign(AttachmentSignBack),
}

impl NetworkBack {
    pub fn join(
        input_port: PortId,
        output_port: PortId,
        operation: conduit_kernel::HostCallId,
    ) -> Self {
        Self::Join(JoinBack {
            input_port,
            output_port,
            operation,
            pending: None,
            completed: false,
        })
    }

    pub fn attachment_sign(input_port: PortId, operation: conduit_kernel::HostCallId) -> Self {
        Self::AttachmentSign(AttachmentSignBack {
            input_port,
            operation,
            pending: None,
            completed: false,
        })
    }
}

impl StepOperation<PORTS> for NetworkBack {
    fn step(
        &mut self,
        io: &mut StepIo<PORTS>,
        input_bytes: &StepInputBytes<'_, PORTS>,
    ) -> StepOutcome {
        match self {
            Self::Join(back) => back.step(io, input_bytes),
            Self::AttachmentSign(back) => back.step(io, input_bytes),
        }
    }
}

fn fail(detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: FailureCode::InvalidLifecycle,
        detail,
    })
}
