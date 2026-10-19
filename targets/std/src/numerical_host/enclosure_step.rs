//! One bounded input-consuming enclosure on the ordinary kernel Step boundary.
use super::enclosure_encoding::EnclosureEncoding;
use conduit_core::{projection::*, *};
use conduit_kernel::{
    scheduler::{SchedulerError, StepBack, StepInputBytes, StepIo, StepOutcome},
    Failure, FailureCode, PortId,
};
pub const ENCLOSURE_WORK: u64 = 100_000_000;
// Composer leaf checks, child views and final record validation have separate
// existing meters. This conservative ceiling covers all five mathematical fields.
pub const ENCLOSURE_TOTAL_WORK: u64 = ENCLOSURE_WORK + 32 * MATHEMATICAL_VALUE_VALIDATION_WORK;
pub struct MathematicalEnclosureBack {
    state: Box<State>,
    staged: bool,
    complete: bool,
}
struct State {
    evaluator: PreparedMathematicalEnclosureEvaluator,
    encoding: EnclosureEncoding,
}
fn fail(detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: if detail == 3 {
            FailureCode::WorkBudgetExhausted
        } else if detail == 6 {
            FailureCode::InvalidLifecycle
        } else if detail == 5 || detail == 7 {
            FailureCode::StorageExhausted
        } else {
            FailureCode::InvalidInput
        },
        detail,
    })
}
fn io_failure(why: SchedulerError) -> StepOutcome {
    if why == SchedulerError::StepFuelExceeded {
        fail(3)
    } else {
        fail(7)
    }
}
fn refusal(why: MathematicalEnclosureRefusal) -> u16 {
    match why {
        MathematicalEnclosureRefusal::Numeric(ExactNumericRefusal::WorkLimit) => 3,
        MathematicalEnclosureRefusal::ToleranceExceeded => 2,
        MathematicalEnclosureRefusal::UnsupportedTrigonometricEnclosure
        | MathematicalEnclosureRefusal::ExactConstantEnclosureUnsupported
        | MathematicalEnclosureRefusal::ConstantTermLimit
        | MathematicalEnclosureRefusal::Root(_) => 4,
        _ => 1,
    }
}
impl MathematicalEnclosureBack {
    pub fn new(requirements: MathematicalEnclosureRequirements) -> Result<Self, String> {
        Ok(Self {
            state: Box::new(State {
                evaluator: PreparedMathematicalEnclosureEvaluator::new(
                    requirements,
                    ENCLOSURE_WORK,
                    MATHEMATICAL_VALUE_MAX_ENCODED_BYTES,
                )
                .map_err(|why| format!("numeric/enclose-preparation@1: {why:?}"))?,
                encoding: EnclosureEncoding::new()
                    .map_err(|why| format!("numeric/enclose-output-preparation@1: {why:?}"))?,
            }),
            staged: false,
            complete: false,
        })
    }
    pub fn retained_capacity_bytes(&self) -> usize {
        core::mem::size_of::<State>()
            + self.state.evaluator.scratch_payload_bytes()
            + self.state.encoding.prepared_heap_bytes()
    }
    fn evaluate(&mut self, input: &[u8]) -> Result<(), u16> {
        let result = self
            .state
            .evaluator
            .execute(input, &mut ExactNumericWork::new(ENCLOSURE_WORK))
            .map_err(refusal)?;
        let domain = BorrowedMathematicalEnclosureDomain::new();
        let policy = BorrowedMathematicalEnclosurePolicy {
            requirements: result.requirements(),
        };
        let target = result.target();
        let facts = result.facts();
        let ids = Ids::new(input, target, result.width(), result.requirements());
        let report = ProjectionReport::new(
            &domain,
            &policy,
            ProjectionInput {
                basis: ProjectionBasis {
                    report: text(id(&ids.report)),
                    source: text(id(&ids.source)),
                    target: Some(text(id(&ids.target))),
                    projector: domain.target_contract(),
                    requested_route: text("math/enclose"),
                    boundary: None,
                },
                source: &result,
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
        .map_err(|_| 5_u16)?;
        if !policy.permits(&result, &target, &facts) {
            return Err(2);
        }
        report
            .require_policy(policy.identity())
            .map_err(|_| 2_u16)?;
        self.state.encoding.encode(input, &result).map_err(|_| 5)
    }
}
impl<const PORTS: usize> StepBack<PORTS> for MathematicalEnclosureBack {
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
        if !io.output_ready(PortId(0)) {
            return StepOutcome::Await;
        }
        if value.byte_len as usize > MATHEMATICAL_VALUE_MAX_ENCODED_BYTES {
            return fail(1);
        }
        let Some(input) = bytes.input(PortId(0)) else {
            return fail(1);
        };
        if io.consume_fuel(1).is_err() {
            return fail(3);
        }
        if let Err(why) = self.evaluate(input) {
            return fail(why);
        }
        if let Err(why) = io.consume(PortId(0)) {
            return io_failure(why);
        }
        if let Err(why) = io.send_prepared(PortId(0), self.state.encoding.encoded().len() as u32) {
            return io_failure(why);
        }
        self.staged = true;
        StepOutcome::Progress
    }
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        (self.staged && port == PortId(0)).then_some(self.state.encoding.encoded())
    }
    fn step_committed(&mut self) {
        if self.staged {
            self.staged = false;
            self.complete = true;
        }
    }
    fn cancel(&mut self) {
        self.staged = false;
        self.complete = true;
    }
}
struct Ids {
    source: [u8; 64],
    target: [u8; 64],
    report: [u8; 64],
}
#[cfg(test)]
#[path = "enclosure_step_tests.rs"]
mod tests;
fn text(s: &str) -> ProjectionText<'_> {
    ProjectionText::new(s).unwrap()
}
fn id(bytes: &[u8]) -> &str {
    core::str::from_utf8(bytes).unwrap()
}
fn hex(digest: [u8; 32]) -> [u8; 64] {
    let mut result = [0; 64];
    for (i, byte) in digest.iter().enumerate() {
        result[2 * i] = b"0123456789abcdef"[(byte >> 4) as usize];
        result[2 * i + 1] = b"0123456789abcdef"[(byte & 15) as usize];
    }
    result
}
impl Ids {
    fn new(
        input: &[u8],
        target: MathematicalInterval,
        width: ExactRational,
        req: MathematicalEnclosureRequirements,
    ) -> Self {
        let source = semantic_digest(MATHEMATICAL_VALUE_ID, input);
        let mut values = [0; 182];
        values[..32].copy_from_slice(&target.lower().semantic_digest());
        values[32..64].copy_from_slice(&target.upper().semantic_digest());
        values[64..96].copy_from_slice(&width.semantic_digest());
        values[96..128].copy_from_slice(&req.maximum_width.semantic_digest());
        values[128..160].copy_from_slice(&req.maximum_leaf_width.semantic_digest());
        values[160..162].copy_from_slice(&req.maximum_constant_terms.to_le_bytes());
        values[162..166].copy_from_slice(&req.maximum_root_refinements.to_le_bytes());
        values[166..170].copy_from_slice(&req.maximum_scratch_payload_bytes.to_le_bytes());
        values[170..174].copy_from_slice(&req.maximum_retained_payload_bytes.to_le_bytes());
        values[174..].copy_from_slice(&ENCLOSURE_WORK.to_le_bytes());
        let target = semantic_digest("numeric/exact-interval-expression@1", &values[..96]);
        let policy = semantic_digest("numeric/enclose-policy-parameters@1", &values[96..]);
        let mut pair = [0; 96];
        pair[..32].copy_from_slice(&source);
        pair[32..64].copy_from_slice(&target);
        pair[64..].copy_from_slice(&policy);
        Self {
            source: hex(source),
            target: hex(target),
            report: hex(semantic_digest("numeric/enclose-report@1", &pair)),
        }
    }
}
