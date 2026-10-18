//! One fixed operation over two Value inputs, through the existing kernel.
use super::{ieee_encoding::PreparedIeeeEncoding, ieee_native};
use conduit_core::*;
use conduit_kernel::{
    scheduler::{SchedulerError, StepBack, StepInputBytes, StepIo, StepOutcome},
    Failure, FailureCode, PortId,
};
use conduit_semantic_catalog::IeeeBinary64Configuration;

pub struct IeeeBinary64Back {
    configuration: IeeeBinary64Configuration,
    encoding: PreparedIeeeEncoding,
    staged: bool,
    complete: bool,
}
fn fail(code: FailureCode, detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure { code, detail })
}
fn io_failure(why: SchedulerError) -> StepOutcome {
    fail(
        if why == SchedulerError::StepFuelExceeded {
            FailureCode::WorkBudgetExhausted
        } else {
            FailureCode::StorageExhausted
        },
        1,
    )
}
impl IeeeBinary64Back {
    pub fn new(configuration: IeeeBinary64Configuration) -> Result<Self, String> {
        let quiet = matches!(configuration, IeeeBinary64Configuration::Quiet { .. });
        if !quiet && !ieee_native::available() {
            return Err("ieee/native-target-unavailable@1".into());
        }
        Ok(Self {
            configuration,
            encoding: PreparedIeeeEncoding::new(quiet)
                .map_err(|e| format!("IEEE output preparation: {e:?}"))?,
            staged: false,
            complete: false,
        })
    }
    pub fn retained_capacity_bytes(&self) -> usize {
        self.encoding.prepared_heap_bytes()
    }
    pub fn output_bytes(&self) -> usize {
        self.encoding.result_bytes().len()
    }
}
impl<const PORTS: usize> StepBack<PORTS> for IeeeBinary64Back {
    fn step(&mut self, io: &mut StepIo<PORTS>, bytes: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if self.complete {
            return StepOutcome::Complete;
        }
        if self.staged {
            return fail(FailureCode::InvalidLifecycle, 2);
        }
        let left = io.input(PortId(0));
        let right = io.input(PortId(1));
        if left.is_none() || right.is_none() {
            if (left.is_none() && io.input_closed(PortId(0)))
                || (right.is_none() && io.input_closed(PortId(1)))
            {
                return fail(FailureCode::InvalidInput, 3);
            }
            return StepOutcome::Await;
        }
        if !io.output_ready(PortId(0)) {
            return StepOutcome::Await;
        }
        let decode = |port| -> Option<IeeeF64> {
            if io.input(port)?.byte_len != 8 {
                return None;
            }
            Some(IeeeF64::from_bits(u64::from_le_bytes(
                bytes.input(port)?.try_into().ok()?,
            )))
        };
        let (Some(left), Some(right)) = (decode(PortId(0)), decode(PortId(1))) else {
            return fail(FailureCode::InvalidInput, 4);
        };
        // A single fixed instruction/classification and two fixed-shape records.
        // No source-sized loop, arbitrary exact arithmetic or runtime fallback.
        if let Err(why) = io.consume_fuel(1) {
            return io_failure(why);
        }
        let encoded = match self.configuration {
            IeeeBinary64Configuration::Arithmetic {
                operation,
                rounding,
            } => match ieee_native::execute(operation, left, right, rounding) {
                Ok(result) => self.encoding.arithmetic(result),
                Err(_) => return fail(FailureCode::InvalidLifecycle, 5),
            },
            IeeeBinary64Configuration::Quiet { predicate } => {
                let Ok(result) = compare_ieee_binary64_quiet(
                    predicate,
                    left,
                    right,
                    &mut ExactNumericWork::new(1),
                ) else {
                    return fail(FailureCode::WorkBudgetExhausted, 6);
                };
                self.encoding.comparison(result.value, result.flags)
            }
        };
        let Ok(output) = encoded else {
            return fail(FailureCode::InvalidLifecycle, 7);
        };
        let len = output.len() as u32;
        for port in [PortId(0), PortId(1)] {
            if let Err(why) = io.consume(port) {
                return io_failure(why);
            }
        }
        if let Err(why) = io.send_prepared(PortId(0), len) {
            return io_failure(why);
        }
        self.staged = true;
        StepOutcome::Progress
    }
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        (self.staged && port == PortId(0)).then_some(self.encoding.result_bytes())
    }
    fn step_committed(&mut self) {
        if self.staged {
            self.complete = true;
            self.staged = false;
        }
    }
    fn cancel(&mut self) {
        self.complete = true;
        self.staged = false;
    }
}
