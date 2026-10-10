//! Allocation-free Thermostat combine Back for one authored transition.

use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    Failure, FailureCode, PortId,
};

use crate::{Command, Refusal, ThermostatState, STATE_BYTES};

pub struct ThermostatBack {
    state: ThermostatState,
    output: [u8; STATE_BYTES],
    output_len: usize,
    staged: bool,
    emitted: bool,
}

impl ThermostatBack {
    pub fn new() -> Self {
        Self {
            state: ThermostatState::default(),
            output: [0; STATE_BYTES],
            output_len: 0,
            staged: false,
            emitted: false,
        }
    }

    pub const fn allocation_capacity(&self) -> usize {
        0
    }
}

impl Default for ThermostatBack {
    fn default() -> Self {
        Self::new()
    }
}

impl<const PORTS: usize> StepBack<PORTS> for ThermostatBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, inputs: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if self.emitted {
            if io.input_closed(PortId(0)) && io.input_closed(PortId(1)) {
                io.consume_closed(PortId(0))
                    .expect("closed Thermostat state");
                io.consume_closed(PortId(1))
                    .expect("closed Thermostat command");
                return StepOutcome::Complete;
            }
            return StepOutcome::Await;
        }
        let (Some(state), Some(command)) = (io.input(PortId(0)), io.input(PortId(1))) else {
            if io.input_closed(PortId(0)) || io.input_closed(PortId(1)) {
                return refuse(Refusal::InvalidState);
            }
            return StepOutcome::Await;
        };
        if !io.output_ready(PortId(0)) {
            return StepOutcome::Await;
        }
        let (Some(state_bytes), Some(command_bytes)) =
            (inputs.input(PortId(0)), inputs.input(PortId(1)))
        else {
            return refuse(Refusal::InvalidState);
        };
        if state.byte_len as usize != state_bytes.len()
            || command.byte_len as usize != command_bytes.len()
        {
            return refuse(Refusal::InvalidState);
        }
        self.state = match ThermostatState::decode(state_bytes) {
            Ok(state) => state,
            Err(error) => return refuse(error),
        };
        let command = match Command::decode(command_bytes) {
            Ok(command) => command,
            Err(error) => return refuse(error),
        };
        self.state = match self.state.apply(command) {
            Ok(state) => state,
            Err(error) => return refuse(error),
        };
        self.output = self.state.encode().expect("validated thermostat state");
        self.output_len = STATE_BYTES;
        io.consume(PortId(0)).expect("present Thermostat state");
        io.consume(PortId(1)).expect("present Thermostat command");
        io.send_prepared(PortId(0), self.output_len as u32)
            .expect("admitted Thermostat state output");
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

fn refuse(error: Refusal) -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: FailureCode::InvalidInput,
        detail: error as u16,
    })
}
