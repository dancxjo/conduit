//! One bounded child Step evaluates the actual checked game/map programs.
use conduit_core::{StructuredCanonicalSelection, StructuredSelector};
use conduit_face::{
    ParsecMapTransitionResult, PreparedParsecMapTransition, PARSEC_MAP_FRAME_BYTES,
};
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    Failure, FailureCode, PortId,
};

pub(super) struct ParsecCombineBack {
    evaluator: PreparedParsecMapTransition,
    selector: StructuredSelector,
    input_type: Vec<u8>,
    state_type: Vec<u8>,
    state: Vec<u8>,
    output: [u8; PARSEC_MAP_FRAME_BYTES],
    output_len: usize,
    staged: bool,
    emitted: bool,
    cancelled: bool,
}
impl ParsecCombineBack {
    pub(super) fn prepare(evaluator: PreparedParsecMapTransition) -> Result<Self, String> {
        let selector = StructuredSelector::field(
            ParsecMapTransitionResult::semantic_type().map_err(|e| format!("{e:?}"))?,
            "state",
        )
        .map_err(|e| format!("Parsec state selector: {e:?}"))?;
        Ok(Self {
            input_type: selector
                .input_type()
                .canonical_bytes()
                .map_err(|e| format!("{e:?}"))?,
            state_type: selector
                .output_type()
                .canonical_bytes()
                .map_err(|e| format!("{e:?}"))?,
            selector,
            evaluator,
            state: Vec::with_capacity(PARSEC_MAP_FRAME_BYTES),
            output: [0; PARSEC_MAP_FRAME_BYTES],
            output_len: 0,
            staged: false,
            emitted: false,
            cancelled: false,
        })
    }
}
fn fail() -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: FailureCode::InvalidInput,
        detail: 1,
    })
}
impl<const PORTS: usize> StepBack<PORTS> for ParsecCombineBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, inputs: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if self.cancelled {
            return StepOutcome::Fail(Failure {
                code: FailureCode::Cancelled,
                detail: 0,
            });
        }
        if self.emitted {
            if io.input_closed(PortId(0)) && io.input_closed(PortId(1)) {
                io.consume_closed(PortId(0))
                    .expect("closed Parsec accumulator");
                io.consume_closed(PortId(1)).expect("closed Parsec command");
                return StepOutcome::Complete;
            }
            return StepOutcome::Await;
        }
        let (Some(accumulator), Some(command)) = (io.input(PortId(0)), io.input(PortId(1))) else {
            return if io.input_closed(PortId(0)) || io.input_closed(PortId(1)) {
                fail()
            } else {
                StepOutcome::Await
            };
        };
        if !io.output_ready(PortId(0)) {
            return StepOutcome::Await;
        }
        let (Some(state), Some(action)) = (inputs.input(PortId(0)), inputs.input(PortId(1))) else {
            return fail();
        };
        if accumulator.byte_len as usize != state.len()
            || command.byte_len as usize != action.len()
            || state.len() > PARSEC_MAP_FRAME_BYTES
            || action.len() > PARSEC_MAP_FRAME_BYTES
        {
            return fail();
        }
        if !matches!(
            self.selector.select_canonical_into(
                state,
                &self.input_type,
                &self.state_type,
                &mut self.state
            ),
            Ok(StructuredCanonicalSelection::Matched)
        ) {
            return fail();
        }
        let result = match self.evaluator.evaluate(&self.state, action) {
            Ok(result) => result,
            Err(_) => return fail(),
        };
        let bytes = result.result_bytes();
        self.output[..bytes.len()].copy_from_slice(bytes);
        self.output_len = bytes.len();
        io.consume(PortId(0)).expect("present Parsec accumulator");
        io.consume(PortId(1)).expect("present Parsec command");
        io.send_prepared(PortId(0), self.output_len as u32)
            .expect("admitted Parsec output");
        self.staged = true;
        StepOutcome::Progress
    }
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        (port == PortId(0) && self.staged && !self.cancelled)
            .then_some(&self.output[..self.output_len])
    }
    fn step_committed(&mut self) {
        if self.staged {
            self.staged = false;
            self.emitted = true;
        }
    }
    fn cancel(&mut self) {
        self.cancelled = true;
        self.staged = false;
        self.output.fill(0);
        self.output_len = 0;
        self.state.clear();
    }
}
