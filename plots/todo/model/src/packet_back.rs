//! Allocation-prepared packing Back shared by std and browser realizations.

use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    Failure, FailureCode, PortId,
};

use crate::PreparedTodoPacket;

pub struct TodoPacketBack {
    packet: PreparedTodoPacket,
    emitted: bool,
    staged: bool,
}

impl TodoPacketBack {
    pub fn new() -> Self {
        Self {
            packet: PreparedTodoPacket::new(),
            emitted: false,
            staged: false,
        }
    }

    pub fn allocation_capacity(&self) -> usize {
        self.packet.allocation_capacity()
    }
}

impl Default for TodoPacketBack {
    fn default() -> Self {
        Self::new()
    }
}

impl<const PORTS: usize> StepBack<PORTS> for TodoPacketBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, inputs: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if self.emitted {
            if io.input_closed(PortId(0)) && io.input_closed(PortId(1)) {
                io.consume_closed(PortId(0))
                    .expect("closed Todo state input");
                io.consume_closed(PortId(1))
                    .expect("closed Todo command input");
                return StepOutcome::Complete;
            }
            return StepOutcome::Await;
        }
        let (Some(state), Some(command)) = (io.input(PortId(0)), io.input(PortId(1))) else {
            if io.input_closed(PortId(0)) || io.input_closed(PortId(1)) {
                return StepOutcome::Fail(Failure {
                    code: FailureCode::InvalidInput,
                    detail: 1,
                });
            }
            return StepOutcome::Await;
        };
        if !io.output_ready(PortId(0)) {
            return StepOutcome::Await;
        }
        let (Some(state_bytes), Some(command_bytes)) =
            (inputs.input(PortId(0)), inputs.input(PortId(1)))
        else {
            return StepOutcome::Fail(Failure {
                code: FailureCode::InvalidInput,
                detail: 2,
            });
        };
        if state.byte_len as usize != state_bytes.len()
            || command.byte_len as usize != command_bytes.len()
            || self.packet.pack(state_bytes, command_bytes).is_err()
        {
            return StepOutcome::Fail(Failure {
                code: FailureCode::InvalidInput,
                detail: 3,
            });
        }
        io.consume(PortId(0)).expect("exact Todo state input");
        io.consume(PortId(1)).expect("exact Todo command input");
        io.send_prepared(PortId(0), self.packet.bytes().len() as u32)
            .expect("admitted Todo transition output");
        self.staged = true;
        StepOutcome::Progress
    }

    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        (port == PortId(0) && self.staged).then_some(self.packet.bytes())
    }

    fn step_committed(&mut self) {
        if self.staged {
            self.emitted = true;
            self.staged = false;
        }
    }

    fn cancel(&mut self) {
        self.staged = false;
    }
}
