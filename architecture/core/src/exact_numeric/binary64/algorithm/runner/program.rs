//! Reserve one fixed program without evaluating a fabricated runtime input.
use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Binary64ProgramRunnerLimits {
    pub algorithm: Binary64AlgorithmLimits,
    pub maximum_retained_payload_bytes: u32,
    pub maximum_execution_work: u64,
}

/// Preparation validates structure and reserves storage, not a numerical result.
/// A certificate becomes available only after the existing evaluator succeeds.
pub struct PreparedBinary64ProgramRunner {
    inner: PreparedBinary64AlgorithmRunner,
    has_success: bool,
}
impl PreparedBinary64ProgramRunner {
    pub fn new(
        program: &[Binary64Operation],
        input_steps: &[u8],
        rounding: Binary64Rounding,
        maximum_error: ExactRational,
        limits: Binary64ProgramRunnerLimits,
        work: &mut ExactNumericWork,
    ) -> Result<Self, Binary64AlgorithmRefusal> {
        if maximum_error.is_negative() {
            return Err(Binary64Refusal::NegativeTolerance.into());
        }
        let retained = retained_program_bytes(program, limits.algorithm)?;
        for (index, operation) in program.iter().enumerate() {
            work.charge(1)?;
            let prior = |step: u8| usize::from(step) < index;
            let valid = match operation {
                Binary64Operation::Add { left, right }
                | Binary64Operation::Subtract { left, right }
                | Binary64Operation::Multiply { left, right }
                | Binary64Operation::Divide { left, right } => prior(*left) && prior(*right),
                Binary64Operation::Negate { input } => prior(*input),
                _ => true,
            };
            if !valid {
                return Err(Binary64AlgorithmRefusal::InvalidProgram);
            }
        }
        let overhead =
            core::mem::size_of::<Self>() - core::mem::size_of::<PreparedBinary64AlgorithmRunner>();
        let required = retained
            .checked_mul(2)
            .and_then(|n| n.checked_add(core::mem::size_of::<Self>()))
            .ok_or(Binary64AlgorithmRefusal::StorageLimit)?;
        if required > limits.maximum_retained_payload_bytes as usize {
            return Err(Binary64AlgorithmRefusal::StorageLimit);
        }
        let inner_limit = limits
            .maximum_retained_payload_bytes
            .checked_sub(overhead as u32)
            .ok_or(Binary64AlgorithmRefusal::StorageLimit)?;
        // These private slots are storage, not an executed algorithm. The wrapper
        // never exposes them through a certificate, even after a failed first run.
        // evaluate() overwrites every step before committing a successful run.
        let zero = PreparedBinary64Projection::prepare(
            ExactRational::ZERO,
            rounding,
            ExactRational::ZERO,
            work,
        )?;
        let slots = program
            .iter()
            .map(|operation| Binary64Step {
                operation: operation.clone(),
                semantic_value: ExactRational::ZERO,
                projection: zero,
            })
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let storage = PreparedBinary64Algorithm {
            steps: slots,
            source: ExactRational::ZERO,
            target: zero.target(),
            error: Binary64ErrorClaim::ExactDistance(ExactRational::ZERO),
            maximum_error,
            rounding,
            admitted_limb_work: limits.maximum_execution_work,
            spent_limb_work: 0,
            retained_payload_bytes: retained,
            limits: limits.algorithm,
        };
        Ok(Self {
            inner: PreparedBinary64AlgorithmRunner::new(&storage, input_steps, inner_limit)?,
            has_success: false,
        })
    }

    pub fn retained_payload_bytes(&self) -> usize {
        self.inner.retained_payload_bytes() + core::mem::size_of::<Self>()
            - core::mem::size_of::<PreparedBinary64AlgorithmRunner>()
    }

    pub const fn maximum_execution_work(&self) -> u64 {
        self.inner.maximum_limb_work
    }

    /// None before the first successful execution. A prior successful record
    /// never authorizes emitting it for a later input whose execution failed.
    pub fn last_success(&self) -> Option<&PreparedBinary64Algorithm> {
        self.has_success.then(|| self.inner.last_success())
    }

    pub fn execute(
        &mut self,
        inputs: &[ExactRational],
        work: &mut ExactNumericWork,
    ) -> Result<&PreparedBinary64Algorithm, Binary64AlgorithmRefusal> {
        let result = self.inner.execute(inputs, work)?;
        self.has_success = true;
        Ok(result)
    }

    pub fn execute_inputs<'a>(
        &mut self,
        inputs: &'a [Binary64AlgorithmInput<'a>],
        work: &mut ExactNumericWork,
    ) -> Result<&PreparedBinary64Algorithm, Binary64AlgorithmRefusal> {
        let result = self.inner.execute_inputs(inputs, work)?;
        self.has_success = true;
        Ok(result)
    }
}
