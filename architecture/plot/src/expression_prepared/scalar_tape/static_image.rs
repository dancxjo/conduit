//! Read-only Make output consumed by the existing scalar evaluator.
use super::*;
pub(super) mod generate;
pub use generate::generate_static_scalar_expression;

/// Fixed-integer/Bool execution over an immutable, resident instruction array.
/// This owns no heap storage. Root still admits its bounded stack and caller
/// output, and binds the compiler-produced artifact to the selected exact Fore.
/// Static lifetime alone proves neither flash placement nor Source authority.
pub struct StaticScalarExpression {
    tape: ScalarTape,
    input: PrimitiveInfoKind,
    output: PrimitiveInfoKind,
    input_contract: Option<conduit_core::BorrowedFixedIntegerContract<'static>>,
    output_contract: Option<conduit_core::BorrowedFixedIntegerContract<'static>>,
}
impl StaticScalarExpression {
    /// Validate storage, types, forward control flow and definite initialization.
    /// The code must be resident trusted Make output, never an incoming Plan's
    /// self-declared implementation. This performs no Source parsing/allocation.
    pub fn new(
        input: PrimitiveInfoKind,
        output: PrimitiveInfoKind,
        instructions: &'static [ScalarTapeInstruction],
        result: usize,
    ) -> Result<Self, Refusal> {
        validate(input, output, instructions, result)?;
        Ok(Self {
            tape: ScalarTape {
                instructions: Cow::Borrowed(instructions),
                result,
                scratch: ScratchProfile::prepare(instructions.len()),
            },
            input,
            output,
            input_contract: None,
            output_contract: None,
        })
    }
    /// Retain exact immutable witnesses; neither Source nor a contract is decoded.
    pub fn with_value_contracts(
        mut self,
        input: Option<&'static conduit_core::FixedIntegerContractSpec<'static>>,
        output: Option<&'static conduit_core::FixedIntegerContractSpec<'static>>,
    ) -> Result<Self, Refusal> {
        let prepare = |spec: &'static conduit_core::FixedIntegerContractSpec<'static>, kind| {
            if conduit_core::primitive_info_kind(spec.value_kind) != Some(kind) {
                return Err(Refusal::InvalidProgram);
            }
            conduit_core::BorrowedFixedIntegerContract::new(spec)
                .map_err(|_| Refusal::InvalidProgram)
        };
        self.input_contract = input.map(|spec| prepare(spec, self.input)).transpose()?;
        self.output_contract = output.map(|spec| prepare(spec, self.output)).transpose()?;
        Ok(self)
    }
    /// Worst-case cooperative private work, including retained predicate checks.
    pub fn computation_work(&self) -> u16 {
        self.instruction_count() as u16
            + self.input_contract.map_or(0, |c| c.validation_work())
            + self.output_contract.map_or(0, |c| c.validation_work())
    }
    pub fn instruction_count(&self) -> usize {
        self.tape.instruction_count()
    }
    pub fn input_bytes(&self) -> usize {
        width(self.input).expect("validated scalar input")
    }
    pub fn scratch_bytes(&self) -> usize {
        self.tape.scratch_bytes()
    }
    pub fn output_bytes(&self) -> usize {
        width(self.output).expect("validated scalar output")
    }
    pub fn evaluate_into(&self, input: &[u8], output: &mut [u8]) -> Result<usize, Refusal> {
        validate_leaf(self.input, input).map_err(|_| Refusal::InvalidInput)?;
        if let Some(contract) = self.input_contract {
            contract
                .validate(input)
                .map_err(Refusal::InputValueConstraint)?;
        }
        let width = self.output_bytes();
        if output.len() < width {
            return Err(Refusal::InvalidProgram);
        }
        let value = self.tape.evaluate(input, Some(self.input))?;
        if value.kind != self.output {
            return Err(Refusal::InvalidProgram);
        }
        validate_leaf(self.output, value.as_slice())?;
        if let Some(contract) = self.output_contract {
            contract
                .validate(value.as_slice())
                .map_err(Refusal::OutputValueConstraint)?;
        }
        output[..width].copy_from_slice(value.as_slice());
        Ok(width)
    }
}
pub(super) fn width(kind: PrimitiveInfoKind) -> Option<usize> {
    Some(match kind {
        PrimitiveInfoKind::Bool | PrimitiveInfoKind::U8 | PrimitiveInfoKind::I8 => 1,
        PrimitiveInfoKind::U16 | PrimitiveInfoKind::I16 => 2,
        PrimitiveInfoKind::U32 | PrimitiveInfoKind::I32 => 4,
        PrimitiveInfoKind::U64 | PrimitiveInfoKind::I64 => 8,
        PrimitiveInfoKind::U128 | PrimitiveInfoKind::I128 => 16,
        _ => return None,
    })
}
pub(super) fn validate_leaf(kind: PrimitiveInfoKind, bytes: &[u8]) -> Result<(), Refusal> {
    if width(kind) != Some(bytes.len()) {
        return Err(Refusal::InvalidProgram);
    }
    if kind == PrimitiveInfoKind::Bool {
        conduit_core::InfoBool::decode(bytes).map_err(|_| Refusal::InvalidProgram)?;
    } else {
        conduit_core::FixedInteger::decode(kind, bytes).map_err(|_| Refusal::InvalidProgram)?;
    }
    Ok(())
}
fn validate(
    input: PrimitiveInfoKind,
    output: PrimitiveInfoKind,
    code: &[Instruction],
    result: usize,
) -> Result<(), Refusal> {
    if !supported(input) {
        return Err(Refusal::InvalidProgram);
    }
    validate_with_inputs(output, code, result, |_, kind| kind == input)
}
pub(super) fn validate_with_inputs(
    output: PrimitiveInfoKind,
    code: &[Instruction],
    result: usize,
    input: impl Fn(usize, PrimitiveInfoKind) -> bool,
) -> Result<(), Refusal> {
    let n = code.len();
    if !supported(output)
        || n == 0
        || n > MAXIMUM_INSTRUCTIONS
        || result >= n
        || code[result].kind != output
    {
        return Err(Refusal::InvalidProgram);
    }
    for (pc, instruction) in code.iter().enumerate() {
        let kind = instruction.kind;
        if !supported(kind)
            || instruction.destination > pc
            || code[instruction.destination].kind != kind
        {
            return Err(Refusal::InvalidProgram);
        }
        let earlier = |index: usize| index < pc;
        let forward = |target: usize| target > pc && target <= n;
        let valid = match &instruction.operation {
            Operation::Input => input(pc, kind),
            Operation::Literal(bytes) => validate_leaf(kind, bytes).is_ok(),
            Operation::Widen(from) => {
                earlier(*from)
                    && crate::expression_semantic_call::is_strict_widening(
                        kind_name(code[*from].kind),
                        kind_name(kind),
                    )
            }
            Operation::Unary(op, from) => {
                earlier(*from)
                    && code[*from].kind == kind
                    && match op {
                        UnaryOperator::Not => kind == PrimitiveInfoKind::Bool,
                        UnaryOperator::Negate => kind != PrimitiveInfoKind::Bool,
                    }
            }
            Operation::Binary(op, proven, left, right) => {
                !proven
                    && earlier(*left)
                    && earlier(*right)
                    && binary_kinds(*op, kind, code[*left].kind, code[*right].kind)
            }
            Operation::BranchFalse(condition, target) => {
                earlier(*condition)
                    && code[*condition].kind == PrimitiveInfoKind::Bool
                    && forward(*target)
            }
            Operation::Jump(target) => forward(*target),
            Operation::Move(from) => earlier(*from) && code[*from].kind == kind,
        };
        if !valid {
            return Err(Refusal::InvalidProgram);
        }
    }
    // Per-slot forward dataflow needs only 129 bytes of temporary state, rather
    // than a 128-by-128 slot matrix. Move consumes its source, as in the executor.
    // Bits: 1 = a path arrives uninitialized, 2 = initialized, 0 = unreachable.
    for slot in 0..n {
        let mut states = [0u8; MAXIMUM_INSTRUCTIONS + 1];
        states[0] = 1;
        for (pc, instruction) in code.iter().enumerate() {
            let mut state = states[pc];
            if state == 0 {
                continue;
            }
            let reads = match instruction.operation {
                Operation::Unary(_, from) | Operation::Widen(from) | Operation::Move(from) => {
                    from == slot
                }
                Operation::Binary(_, _, left, right) => left == slot || right == slot,
                Operation::BranchFalse(condition, _) => condition == slot,
                _ => false,
            };
            if reads && state & 1 != 0 {
                return Err(Refusal::InvalidProgram);
            }
            if matches!(instruction.operation, Operation::Move(from) if from == slot) {
                state = 1;
            }
            if !matches!(
                instruction.operation,
                Operation::Jump(_) | Operation::BranchFalse(..)
            ) && instruction.destination == slot
            {
                state = 2;
            }
            match instruction.operation {
                Operation::Jump(target) => states[target] |= state,
                Operation::BranchFalse(_, target) => {
                    states[target] |= state;
                    states[pc + 1] |= state;
                }
                _ => states[pc + 1] |= state,
            }
        }
        if slot == result && states[n] != 2 {
            return Err(Refusal::InvalidProgram);
        }
    }
    Ok(())
}
fn binary_kinds(
    op: BinaryOperator,
    result: PrimitiveInfoKind,
    left: PrimitiveInfoKind,
    right: PrimitiveInfoKind,
) -> bool {
    use BinaryOperator::*;
    use PrimitiveInfoKind::Bool;
    match op {
        BooleanAnd | BooleanOr => result == Bool && left == Bool && right == Bool,
        Equal | NotEqual => result == Bool && left == right,
        Less | LessOrEqual | Greater | GreaterOrEqual => {
            result == Bool && left == right && left != Bool
        }
        ShiftLeft | ShiftRight => result == left && left != Bool && right != Bool,
        _ => result == left && left == right && left != Bool,
    }
}
