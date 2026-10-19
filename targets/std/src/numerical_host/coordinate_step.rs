//! One exact input-consuming conversion, with atomic result/report publication.
use super::coordinate_encoding::CoordinateEncoding;
use conduit_core::{projection::*, *};
use conduit_kernel::{
    scheduler::{SchedulerError, StepBack, StepInputBytes, StepIo, StepOutcome},
    CanonicalValue, Failure, FailureCode, PortId,
};
pub const COORDINATE_WORK: u64 = 200_000_000;
pub const COORDINATE_TOTAL_WORK: u64 =
    COORDINATE_WORK + 32 * MATHEMATICAL_VALUE_VALIDATION_WORK + 8 * 131072;
pub struct QuantityCoordinateBack {
    state: Box<State>,
    staged: bool,
    complete: bool,
}
struct State {
    target: Unit,
    converter: PreparedQuantityCoordinateConversion,
    output: Vec<u8>,
    output_len: usize,
    evidence: Vec<u8>,
    encoding: CoordinateEncoding,
}
fn fail(detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: match detail {
            3 => FailureCode::WorkBudgetExhausted,
            5 | 7 => FailureCode::StorageExhausted,
            6 => FailureCode::InvalidLifecycle,
            _ => FailureCode::InvalidInput,
        },
        detail,
    })
}
fn io_failure(why: SchedulerError) -> StepOutcome {
    fail(if why == SchedulerError::StepFuelExceeded {
        3
    } else {
        7
    })
}
mod refusal;
impl QuantityCoordinateBack {
    pub fn new(target: Unit) -> Result<Self, String> {
        Ok(Self {
            state: Box::new(State {
                target,
                converter: PreparedQuantityCoordinateConversion::new(500000, COORDINATE_WORK)
                    .map_err(|why| format!("coordinates/converter-preparation@1: {why:?}"))?,
                output: vec![0; MATHEMATICAL_QUANTITY_MAX_ENCODED_BYTES],
                output_len: 0,
                evidence: vec![0; QUANTITY_COORDINATE_CONVERSION_EVIDENCE_LIMITS.total_bytes],
                encoding: CoordinateEncoding::new()
                    .map_err(|why| format!("coordinates/encoding-preparation@1: {why:?}"))?,
            }),
            staged: false,
            complete: false,
        })
    }
    pub fn retained_capacity_bytes(&self) -> usize {
        core::mem::size_of::<State>() + self.state.converter.retained_payload_bytes()
            - core::mem::size_of::<PreparedQuantityCoordinateConversion>()
            + self.state.output.capacity()
            + self.state.evidence.capacity()
            + self.state.encoding.retained_bytes()
    }
    fn evaluate(&mut self, bytes: &[u8]) -> Result<(), u16> {
        let state = &mut self.state;
        let mut work = ExactNumericWork::new(COORDINATE_WORK);
        let receipt = state
            .converter
            .execute(bytes, state.target, &mut state.output, &mut work)
            .map_err(refusal::conversion)?;
        let output = &state.output[..receipt.encoded_bytes()];
        // Validation and canonical report passes have separately admitted work.
        let target = QuantityValueProfile::Mathematical
            .decode(
                QUANTITY_INFO_ID,
                output,
                &mut ExactNumericWork::new(MATHEMATICAL_VALUE_VALIDATION_WORK),
            )
            .map_err(|why| refusal::conversion(QuantityCoordinateConversionRefusal::Input(why)))?;
        let ids = QuantityCoordinateContentIds::new(bytes, output, state.target);
        let domain = QuantityCoordinateConversionDomain::new();
        let policy = QuantityCoordinateConversionPolicy {
            target_unit: state.target,
        };
        let facts = domain.facts(&receipt);
        let report = ProjectionReport::new(
            &domain,
            &policy,
            ProjectionInput {
                basis: ids.basis(),
                source: &receipt,
                target: Some(&target),
                facts: &facts,
                native: &[],
                scores: &[],
                admitted: &[],
                attempts: &[],
                selected_attempt: None,
                mechanism: ProjectionMechanism::Completed,
                diagnostics: &[],
            },
        )
        .map_err(|_| 5u16)?;
        report
            .require_bound_policy(policy.identity(), policy.configuration_digest().unwrap())
            .map_err(|_| 5u16)?;
        let n = encode_projection_evidence(
            &report,
            &QuantityCoordinateConversionEvidenceCodec::new(policy),
            QUANTITY_COORDINATE_CONVERSION_EVIDENCE_LIMITS,
            &mut state.evidence,
        )
        .map_err(|_| 5u16)?;
        state
            .encoding
            .encode(&state.evidence[..n])
            .map_err(|_| 5u16)?;
        state.output_len = output.len();
        Ok(())
    }
}
// Ordinals match the sealed contract: input value; outputs evidence, result.
impl<const PORTS: usize> StepBack<PORTS> for QuantityCoordinateBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, bytes: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if self.complete {
            return StepOutcome::Complete;
        }
        if self.staged {
            return fail(6);
        }
        let Some(value) = io.input(PortId(0)) else {
            if !io.input_closed(PortId(0)) {
                return StepOutcome::Await;
            }
            if let Err(why) = io.consume_closed(PortId(0)) {
                return io_failure(why);
            }
            self.complete = true;
            return StepOutcome::Complete;
        };
        if !io.output_ready(PortId(0)) || !io.output_ready(PortId(1)) {
            return StepOutcome::Await;
        }
        if value.byte_len as usize > MATHEMATICAL_QUANTITY_MAX_ENCODED_BYTES {
            return fail(1);
        }
        let Some(input) = bytes.input(PortId(0)) else {
            return fail(1);
        };
        if let Err(why) = io.consume_fuel(1) {
            return io_failure(why);
        }
        if let Err(why) = self.evaluate(input) {
            if why == 2 {
                if let Err(why) = io.consume(PortId(0)) {
                    return io_failure(why);
                }
                // Content correlation only; the kernel supplies actual causal
                // execution custody. No producer/Plan/Sign identity is invented.
                let mut binding = [0; 32 + UNIT_ENCODED_LEN];
                binding[..32].copy_from_slice(&semantic_digest(QUANTITY_INFO_ID, input));
                binding[32..].copy_from_slice(&self.state.target.encode());
                let terminal = TerminalInfo::new(
                    TerminalCategory::RefusalOrAdmission,
                    semantic_digest("physical/coordinate-incompatible@1", &binding),
                );
                self.complete = true;
                return StepOutcome::Abnormal {
                    port: PortId(0),
                    terminal: CanonicalValue::new(&terminal.encode())
                        .expect("bounded terminal info"),
                };
            }
            return fail(why);
        }
        if let Err(why) = io.consume(PortId(0)) {
            return io_failure(why);
        }
        if let Err(why) = io.send_prepared(PortId(0), self.state.encoding.encoded().len() as u32) {
            return io_failure(why);
        }
        if let Err(why) = io.send_prepared(PortId(1), self.state.output_len as u32) {
            return io_failure(why);
        }
        self.staged = true;
        StepOutcome::Progress
    }
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        if !self.staged {
            return None;
        }
        match port {
            PortId(0) => Some(self.state.encoding.encoded()),
            PortId(1) => Some(&self.state.output[..self.state.output_len]),
            _ => None,
        }
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
#[cfg(test)]
mod tests;
