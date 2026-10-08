//! Allocation-free Todo combine Back for one `scan` child invocation.

use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    Failure, FailureCode, PortId,
};

use crate::{
    fixed::{FixedTodoCommand, FixedTodoState},
    TodoRefusal, STATE_MAX_BYTES,
};

pub struct TodoCombineBack {
    state: FixedTodoState,
    output: [u8; STATE_MAX_BYTES],
    output_len: usize,
    staged: bool,
    emitted: bool,
}

impl TodoCombineBack {
    pub fn new() -> Self {
        Self {
            state: FixedTodoState::new(),
            output: [0; STATE_MAX_BYTES],
            output_len: 0,
            staged: false,
            emitted: false,
        }
    }

    pub const fn allocation_capacity(&self) -> usize {
        0
    }
}

impl Default for TodoCombineBack {
    fn default() -> Self {
        Self::new()
    }
}

impl<const PORTS: usize> StepBack<PORTS> for TodoCombineBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, inputs: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if self.emitted {
            if io.input_closed(PortId(0)) && io.input_closed(PortId(1)) {
                io.consume_closed(PortId(0)).expect("closed Todo state");
                io.consume_closed(PortId(1)).expect("closed Todo command");
                return StepOutcome::Complete;
            }
            return StepOutcome::Await;
        }
        let (Some(state), Some(command)) = (io.input(PortId(0)), io.input(PortId(1))) else {
            if io.input_closed(PortId(0)) || io.input_closed(PortId(1)) {
                return refuse(TodoRefusal::InvalidState);
            }
            return StepOutcome::Await;
        };
        if !io.output_ready(PortId(0)) {
            return StepOutcome::Await;
        }
        let (Some(state_bytes), Some(command_bytes)) =
            (inputs.input(PortId(0)), inputs.input(PortId(1)))
        else {
            return refuse(TodoRefusal::InvalidState);
        };
        if state.byte_len as usize != state_bytes.len()
            || command.byte_len as usize != command_bytes.len()
        {
            return refuse(TodoRefusal::InvalidState);
        }
        self.state = match FixedTodoState::decode(state_bytes) {
            Ok(state) => state,
            Err(error) => return refuse(error),
        };
        let command = match FixedTodoCommand::decode(command_bytes) {
            Ok(command) => command,
            Err(error) => return refuse(error),
        };
        if let Err(error) = self.state.apply(&command) {
            return refuse(error);
        }
        self.output_len = self.state.encode_into(&mut self.output);
        io.consume(PortId(0)).expect("present Todo state");
        io.consume(PortId(1)).expect("present Todo command");
        io.send_prepared(PortId(0), self.output_len as u32)
            .expect("admitted Todo state output");
        self.staged = true;
        StepOutcome::Progress
    }

    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        (port == PortId(0) && self.staged).then_some(&self.output[..self.output_len])
    }

    fn step_committed(&mut self) {
        if self.staged {
            self.staged = false;
            self.emitted = true;
        }
    }

    fn cancel(&mut self) {
        self.staged = false;
    }
}

fn refuse(error: TodoRefusal) -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: FailureCode::InvalidInput,
        detail: error.detail(),
    })
}
