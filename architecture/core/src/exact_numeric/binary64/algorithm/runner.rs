//! Reuse the existing arithmetic evaluator with preallocated transactional state.
use super::*;
mod program;
pub use program::{Binary64ProgramRunnerLimits, PreparedBinary64ProgramRunner};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Binary64AlgorithmInput<'a> {
    Exact(&'a ExactRational),
    Projected(&'a Binary64ProjectedInput),
}
enum InputValues<'a> {
    Exact(&'a [ExactRational]),
    Mixed(&'a [Binary64AlgorithmInput<'a>]),
}
impl InputValues<'_> {
    fn len(&self) -> usize {
        match self {
            Self::Exact(v) => v.len(),
            Self::Mixed(v) => v.len(),
        }
    }
    fn at(&self, index: usize) -> Option<Binary64AlgorithmInput<'_>> {
        match self {
            Self::Exact(v) => v.get(index).map(Binary64AlgorithmInput::Exact),
            Self::Mixed(v) => v.get(index).copied(),
        }
    }
}

/// Every input binds one declared Constant or ProjectedConstant step; other operations, rounding,
/// tolerance and bounds remain those of the prepared template. This runner does
/// not establish a Source/Type binding for its input slots; that is the caller's
/// admission obligation. It does not reinterpret explicit IEEE constants.
#[derive(Debug)]
pub struct PreparedBinary64AlgorithmRunner {
    committed: PreparedBinary64Algorithm,
    scratch: PreparedBinary64Algorithm,
    input_for_step: [Option<u8>; BINARY64_ALGORITHM_MAX_STEPS],
    input_count: usize,
    maximum_limb_work: u64,
    retained_payload_bytes: usize,
}
impl PreparedBinary64AlgorithmRunner {
    /// Allocates both state buffers before execution. The conservative payload
    /// bound includes two complete certificates and this runner's container;
    /// it is not an allocator/RSS or whole-process memory claim.
    pub fn new(
        template: &PreparedBinary64Algorithm,
        input_steps: &[u8],
        maximum_retained_payload_bytes: u32,
    ) -> Result<Self, Binary64AlgorithmRefusal> {
        use Binary64AlgorithmRefusal as R;
        let retained_payload_bytes = template
            .retained_payload_bytes
            .checked_mul(2)
            .and_then(|n| n.checked_add(core::mem::size_of::<Self>()))
            .ok_or(R::StorageLimit)?;
        if retained_payload_bytes > maximum_retained_payload_bytes as usize {
            return Err(R::StorageLimit);
        }
        if input_steps.len() > BINARY64_ALGORITHM_MAX_STEPS {
            return Err(R::StepLimit);
        }
        let mut input_for_step = [None; BINARY64_ALGORITHM_MAX_STEPS];
        for (input, &step) in input_steps.iter().enumerate() {
            let index = usize::from(step);
            if !matches!(
                template.steps.get(index).map(|s| &s.operation),
                Some(Binary64Operation::Constant(_) | Binary64Operation::ProjectedConstant(_))
            ) || input_for_step[index].is_some()
            {
                return Err(R::InvalidProgram);
            }
            input_for_step[index] = Some(input as u8);
        }
        Ok(Self {
            committed: template.clone(),
            scratch: template.clone(),
            input_for_step,
            input_count: input_steps.len(),
            maximum_limb_work: template.admitted_limb_work,
            retained_payload_bytes,
        })
    }
    pub const fn retained_payload_bytes(&self) -> usize {
        self.retained_payload_bytes
    }
    /// The initial template or last successful execution. An error from execute
    /// never authorizes emitting this earlier record for the failed input.
    pub fn last_success(&self) -> &PreparedBinary64Algorithm {
        &self.committed
    }
    /// Reevaluate the fixed operation order without allocation. Work spent by
    /// failed computations is charged too. A failed run cannot partially replace
    /// the committed certificate, and a later run overwrites all scratch steps.
    pub fn execute(
        &mut self,
        inputs: &[ExactRational],
        work: &mut ExactNumericWork,
    ) -> Result<&PreparedBinary64Algorithm, Binary64AlgorithmRefusal> {
        self.execute_values(InputValues::Exact(inputs), work)
    }
    /// Preserve incoming mathematical source and represented value separately.
    /// Exact and projected slots are distinct; neither can be silently substituted
    /// for the other. Each projected slot retains its template error ceiling.
    pub fn execute_inputs<'a>(
        &mut self,
        inputs: &'a [Binary64AlgorithmInput<'a>],
        work: &mut ExactNumericWork,
    ) -> Result<&PreparedBinary64Algorithm, Binary64AlgorithmRefusal> {
        self.execute_values(InputValues::Mixed(inputs), work)
    }
    fn execute_values(
        &mut self,
        inputs: InputValues<'_>,
        work: &mut ExactNumericWork,
    ) -> Result<&PreparedBinary64Algorithm, Binary64AlgorithmRefusal> {
        if inputs.len() != self.input_count {
            return Err(Binary64AlgorithmRefusal::InvalidProgram);
        }
        let mut local = ExactNumericWork::new(work.remaining().min(self.maximum_limb_work));
        let result = self.evaluate(inputs, &mut local);
        work.charge(local.spent())?;
        result?;
        core::mem::swap(&mut self.committed, &mut self.scratch);
        Ok(&self.committed)
    }
    fn evaluate(
        &mut self,
        inputs: InputValues<'_>,
        work: &mut ExactNumericWork,
    ) -> Result<(), Binary64AlgorithmRefusal> {
        let admitted = work.remaining();
        let before = work.spent();
        work.charge(1)?;
        let local_allowance = ExactRational::from_i128(2).checked_power(971, work)?;
        let scratch = &mut self.scratch;
        for index in 0..scratch.steps.len() {
            let (prior, tail) = scratch.steps.split_at_mut(index);
            let step = &mut tail[0];
            if let Some(input) = self.input_for_step[index] {
                match (&mut step.operation, inputs.at(usize::from(input))) {
                    (
                        Binary64Operation::Constant(value),
                        Some(Binary64AlgorithmInput::Exact(input)),
                    ) => **value = *input,
                    (
                        Binary64Operation::ProjectedConstant(value),
                        Some(Binary64AlgorithmInput::Projected(input)),
                    ) => value.stage(input),
                    _ => return Err(Binary64AlgorithmRefusal::InvalidProgram),
                }
            }
            let (semantic, projection) = evaluate_value(
                &step.operation,
                scratch.rounding,
                local_allowance,
                prior,
                work,
            )?;
            step.semantic_value = semantic;
            step.projection = projection;
        }
        let final_step = scratch
            .steps
            .last()
            .ok_or(Binary64AlgorithmRefusal::InvalidProgram)?;
        let error = error_claim(
            final_step.semantic_value,
            final_step.projection.represented,
            scratch.maximum_error,
            work,
        )?;
        scratch.source = final_step.semantic_value;
        scratch.target = final_step.projection.target;
        scratch.error = error;
        scratch.admitted_limb_work = admitted;
        scratch.spent_limb_work = work.spent() - before;
        Ok(())
    }
}
