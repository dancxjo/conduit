//! Execute and certify a fixed operation order, including every intermediate rounding.
use super::*;
use alloc::{boxed::Box, vec::Vec};
pub const BINARY64_ALGORITHM_MAX_STEPS: usize = 32;
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Binary64Operation {
    Constant(Box<ExactRational>),
    /// Explicit finite IEEE input, preserving its signed-zero representation.
    IeeeConstant(IeeeF64),
    Add {
        left: u8,
        right: u8,
    },
    Subtract {
        left: u8,
        right: u8,
    },
    Multiply {
        left: u8,
        right: u8,
    },
    Divide {
        left: u8,
        right: u8,
    },
    Negate {
        input: u8,
    },
}
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct Binary64AlgorithmLimits {
    pub maximum_steps: u8,
    pub maximum_retained_bytes: u32,
}
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Binary64AlgorithmRefusal {
    Projection(Binary64Refusal),
    InvalidProgram,
    StepLimit,
    StorageLimit,
    RoundedDivisionByZero,
}
impl From<Binary64Refusal> for Binary64AlgorithmRefusal {
    fn from(value: Binary64Refusal) -> Self {
        Self::Projection(value)
    }
}
impl From<ExactNumericRefusal> for Binary64AlgorithmRefusal {
    fn from(value: ExactNumericRefusal) -> Self {
        Self::Projection(value.into())
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Binary64Step {
    operation: Binary64Operation,
    semantic_value: ExactRational,
    projection: PreparedBinary64Projection,
}
impl Binary64Step {
    pub fn operation(&self) -> &Binary64Operation {
        &self.operation
    }
    pub const fn semantic_value(&self) -> ExactRational {
        self.semantic_value
    }
    /// Source here is the exact result of this operation on the *previously
    /// rounded* operands, not its unrounded mathematical denotation.
    pub fn projection(&self) -> &PreparedBinary64Projection {
        &self.projection
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedBinary64Algorithm {
    steps: Box<[Binary64Step]>,
    source: ExactRational,
    target: IeeeF64,
    error: Binary64ErrorClaim,
    maximum_error: ExactRational,
    rounding: Binary64Rounding,
    admitted_limb_work: u64,
    spent_limb_work: u64,
    retained_payload_bytes: usize,
}
impl PreparedBinary64Algorithm {
    pub fn prepare(
        program: &[Binary64Operation],
        rounding: Binary64Rounding,
        maximum_error: ExactRational,
        limits: Binary64AlgorithmLimits,
        work: &mut ExactNumericWork,
    ) -> Result<Self, Binary64AlgorithmRefusal> {
        use Binary64AlgorithmRefusal as R;
        if maximum_error.is_negative() {
            return Err(Binary64Refusal::NegativeTolerance.into());
        }
        if program.is_empty() {
            return Err(R::InvalidProgram);
        }
        if program.len() > BINARY64_ALGORITHM_MAX_STEPS
            || program.len() > limits.maximum_steps as usize
        {
            return Err(R::StepLimit);
        }
        let retained_payload_bytes = core::mem::size_of::<Self>()
            + program.len() * core::mem::size_of::<Binary64Step>()
            + program
                .iter()
                .filter(|op| matches!(op, Binary64Operation::Constant(_)))
                .count()
                * core::mem::size_of::<ExactRational>();
        if retained_payload_bytes > limits.maximum_retained_bytes as usize {
            return Err(R::StorageLimit);
        }
        let admitted_limb_work = work.remaining();
        let before = work.spent();
        work.charge(1)?;
        // One finite rounding can lose at most one spacing of the largest
        // finite binade. Final consumer tolerance is checked independently.
        let local_allowance = ExactRational::from_i128(2).checked_power(971, work)?;
        let mut steps = Vec::with_capacity(program.len());
        for operation in program {
            let step = evaluate(operation, rounding, local_allowance, &steps, work)?;
            steps.push(step);
        }
        let final_step = steps.last().ok_or(R::InvalidProgram)?;
        let source = final_step.semantic_value;
        let target = final_step.projection.target;
        let error = error_claim(
            source,
            final_step.projection.represented,
            maximum_error,
            work,
        )?;
        Ok(Self {
            steps: steps.into_boxed_slice(),
            source,
            target,
            error,
            maximum_error,
            rounding,
            admitted_limb_work,
            spent_limb_work: work.spent() - before,
            retained_payload_bytes,
        })
    }
    pub fn steps(&self) -> &[Binary64Step] {
        &self.steps
    }
    pub const fn source(&self) -> ExactRational {
        self.source
    }
    pub const fn target(&self) -> IeeeF64 {
        self.target
    }
    pub const fn error(&self) -> Binary64ErrorClaim {
        self.error
    }
    pub(super) fn error_ref(&self) -> &Binary64ErrorClaim {
        &self.error
    }
    pub const fn maximum_error(&self) -> ExactRational {
        self.maximum_error
    }
    pub const fn rounding(&self) -> Binary64Rounding {
        self.rounding
    }
    pub const fn spent_limb_work(&self) -> u64 {
        self.spent_limb_work
    }
    pub const fn admitted_limb_work(&self) -> u64 {
        self.admitted_limb_work
    }
    pub const fn retained_payload_bytes(&self) -> usize {
        self.retained_payload_bytes
    }
}
fn evaluate(
    operation: &Binary64Operation,
    rounding: Binary64Rounding,
    local_allowance: ExactRational,
    steps: &[Binary64Step],
    work: &mut ExactNumericWork,
) -> Result<Binary64Step, Binary64AlgorithmRefusal> {
    use Binary64Operation as O;
    let get = |index: u8| {
        steps
            .get(index as usize)
            .ok_or(Binary64AlgorithmRefusal::InvalidProgram)
    };
    let (semantic_value, actual_input, sign_for_exact_zero, identity) = match operation {
        O::Constant(value) => (**value, **value, None, None),
        O::IeeeConstant(bits) => {
            let value = ExactRational::from_binary64(*bits, work)?;
            (value, value, None, Some(*bits))
        }
        O::Negate { input } => {
            let input = get(*input)?;
            (
                input.semantic_value.negated(),
                input.projection.represented.negated(),
                Some(input.projection.target.bits() & (1 << 63) == 0),
                None,
            )
        }
        O::Add { left, right }
        | O::Subtract { left, right }
        | O::Multiply { left, right }
        | O::Divide { left, right } => {
            let (left, right) = (get(*left)?, get(*right)?);
            let (a, b) = (left.semantic_value, right.semantic_value);
            let (x, y) = (left.projection.represented, right.projection.represented);
            let (semantic, actual) = match operation {
                O::Add { .. } => (a.checked_add(b, work)?, x.checked_add(y, work)?),
                O::Subtract { .. } => (a.checked_subtract(b, work)?, x.checked_subtract(y, work)?),
                O::Multiply { .. } => (a.checked_multiply(b, work)?, x.checked_multiply(y, work)?),
                O::Divide { .. } => {
                    let semantic = a.checked_divide(b, work)?;
                    if y == ExactRational::ZERO {
                        return Err(Binary64AlgorithmRefusal::RoundedDivisionByZero);
                    }
                    (semantic, x.checked_divide(y, work)?)
                }
                _ => unreachable!(),
            };
            let left_sign = left.projection.target.bits() >> 63 != 0;
            let mut right_sign = right.projection.target.bits() >> 63 != 0;
            let negative_zero = if matches!(operation, O::Multiply { .. } | O::Divide { .. }) {
                left_sign != right_sign
            } else {
                if matches!(operation, O::Subtract { .. }) {
                    right_sign = !right_sign;
                }
                if x == ExactRational::ZERO && y == ExactRational::ZERO && left_sign == right_sign {
                    left_sign
                } else {
                    rounding == Binary64Rounding::TowardNegative
                }
            };
            (semantic, actual, Some(negative_zero), None)
        }
    };
    let mut projection =
        PreparedBinary64Projection::prepare(actual_input, rounding, local_allowance, work)?;
    if let Some(bits) = identity {
        projection.target = bits;
    } else if actual_input == ExactRational::ZERO {
        if let Some(negative) = sign_for_exact_zero {
            projection.target = IeeeF64::from_bits(if negative { 1 << 63 } else { 0 });
        }
    }
    Ok(Binary64Step {
        operation: operation.clone(),
        semantic_value,
        projection,
    })
}
