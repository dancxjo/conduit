//! Finite fixed-integer/boolean tape; lowering happens before evaluation.
use super::*;
const MAXIMUM_INSTRUCTIONS: usize = 128;
pub(super) struct ScalarTape {
    instructions: Vec<Instruction>,
    result: usize,
    scratch: ScratchProfile,
}

// Bound before Play. Four monomorphized loops limit both scratch initialization
// and code growth; the largest profile retains the original 128-slot ceiling.
#[derive(Clone, Copy)]
enum ScratchProfile {
    Tiny,
    Small,
    Medium,
    Maximum,
}
impl ScratchProfile {
    fn prepare(instructions: usize) -> Self {
        match instructions {
            0..=4 => Self::Tiny,
            5..=8 => Self::Small,
            9..=32 => Self::Medium,
            _ => Self::Maximum,
        }
    }
    fn slots(self) -> usize {
        match self {
            Self::Tiny => 4,
            Self::Small => 8,
            Self::Medium => 32,
            Self::Maximum => MAXIMUM_INSTRUCTIONS,
        }
    }
}
struct Instruction {
    kind: PrimitiveInfoKind,
    destination: usize,
    operation: Operation,
}
enum Operation {
    Input,
    Literal(Vec<u8>),
    Unary(UnaryOperator, usize),
    Binary(BinaryOperator, bool, usize, usize),
    BranchFalse(usize, usize),
    Jump(usize),
    Move(usize),
}
fn supported(kind: PrimitiveInfoKind) -> bool {
    matches!(
        kind,
        PrimitiveInfoKind::Bool
            | PrimitiveInfoKind::U8
            | PrimitiveInfoKind::U16
            | PrimitiveInfoKind::U32
            | PrimitiveInfoKind::U64
            | PrimitiveInfoKind::U128
            | PrimitiveInfoKind::I8
            | PrimitiveInfoKind::I16
            | PrimitiveInfoKind::I32
            | PrimitiveInfoKind::I64
            | PrimitiveInfoKind::I128
    )
}
impl ScalarTape {
    pub(super) fn prepare(root: &PreparedNode) -> Result<Self, Refusal> {
        let mut tape = Self {
            instructions: Vec::with_capacity(MAXIMUM_INSTRUCTIONS),
            result: 0,
            scratch: ScratchProfile::Maximum,
        };
        tape.result = tape.lower(root)?;
        tape.scratch = ScratchProfile::prepare(tape.instructions.len());
        Ok(tape)
    }
    fn push(&mut self, kind: PrimitiveInfoKind, operation: Operation) -> Result<usize, Refusal> {
        if self.instructions.len() == MAXIMUM_INSTRUCTIONS {
            return Err(Refusal::UnsupportedType(
                "scalar-tape/instruction-bound@1".into(),
            ));
        }
        let slot = self.instructions.len();
        self.instructions.push(Instruction {
            kind,
            destination: slot,
            operation,
        });
        Ok(slot)
    }
    fn lower(&mut self, node: &PreparedNode) -> Result<usize, Refusal> {
        if !supported(node.kind) {
            return Err(Refusal::UnsupportedType(
                "scalar-tape/fixed-integer-bool@1".into(),
            ));
        }
        let operation = match &node.operation {
            PreparedOperation::Input => Operation::Input,
            PreparedOperation::Literal(bytes) if bytes.len() <= 16 => {
                Operation::Literal(bytes.clone())
            }
            PreparedOperation::Unary { operator, operand } => {
                Operation::Unary(*operator, self.lower(operand)?)
            }
            PreparedOperation::Binary {
                operator,
                proven,
                left,
                right,
            } => Operation::Binary(*operator, *proven, self.lower(left)?, self.lower(right)?),
            PreparedOperation::Conditional {
                condition,
                when_true,
                when_false,
            } => {
                let condition = self.lower(condition)?;
                let result = self.push(node.kind, Operation::BranchFalse(condition, 0))?;
                let yes = self.lower(when_true)?;
                let yes_move = self.push(node.kind, Operation::Move(yes))?;
                self.instructions[yes_move].destination = result;
                let jump = self.push(node.kind, Operation::Jump(0))?;
                let no_start = self.instructions.len();
                self.instructions[result].operation = Operation::BranchFalse(condition, no_start);
                let no = self.lower(when_false)?;
                let no_move = self.push(node.kind, Operation::Move(no))?;
                self.instructions[no_move].destination = result;
                self.instructions[jump].operation = Operation::Jump(self.instructions.len());
                return Ok(result);
            }
            _ => return Err(Refusal::UnsupportedType("scalar-tape/operation@1".into())),
        };
        self.push(node.kind, operation)
    }
    pub(super) fn instruction_count(&self) -> usize {
        self.instructions.len()
    }
    pub(super) fn scratch_bytes(&self) -> usize {
        core::mem::size_of::<Option<PrimitiveValue<'static>>>() * self.scratch.slots()
    }
    pub(super) fn evaluate<'a>(
        &'a self,
        input: &'a [u8],
        input_kind: Option<PrimitiveInfoKind>,
    ) -> Result<PrimitiveValue<'a>, Refusal> {
        match self.scratch {
            ScratchProfile::Tiny => self.evaluate_slots::<4>(input, input_kind),
            ScratchProfile::Small => self.evaluate_slots::<8>(input, input_kind),
            ScratchProfile::Medium => self.evaluate_slots::<32>(input, input_kind),
            ScratchProfile::Maximum => {
                self.evaluate_slots::<MAXIMUM_INSTRUCTIONS>(input, input_kind)
            }
        }
    }
    // Keep each fixed array in its own stack frame. Inlining a maximum-size
    // arm into the caller would defeat the small-profile stack reservation.
    #[inline(never)]
    fn evaluate_slots<'a, const SLOTS: usize>(
        &'a self,
        input: &'a [u8],
        input_kind: Option<PrimitiveInfoKind>,
    ) -> Result<PrimitiveValue<'a>, Refusal> {
        let mut slots: [Option<PrimitiveValue<'a>>; SLOTS] = core::array::from_fn(|_| None);
        let mut pc = 0;
        while pc < self.instructions.len() {
            let instruction = &self.instructions[pc];
            let get = |index: usize| {
                slots
                    .get(index)
                    .and_then(Option::as_ref)
                    .ok_or(Refusal::InvalidProgram)
            };
            let value = match &instruction.operation {
                Operation::Input => {
                    if input_kind != Some(instruction.kind) {
                        return Err(Refusal::InvalidProgram);
                    }
                    PrimitiveValue::borrowed(instruction.kind, input)?
                }
                Operation::Literal(bytes) => PrimitiveValue::borrowed(instruction.kind, bytes)?,
                Operation::Unary(op, index) => evaluate_unary(*op, instruction.kind, get(*index)?)?,
                Operation::Binary(op, proven, left, right) => {
                    evaluate_binary(*op, *proven, instruction.kind, get(*left)?, get(*right)?)?
                }
                Operation::BranchFalse(condition, target) => {
                    if !decode_bool(get(*condition)?)? {
                        pc = *target;
                    } else {
                        pc += 1;
                    }
                    continue;
                }
                Operation::Jump(target) => {
                    pc = *target;
                    continue;
                }
                Operation::Move(from) => slots
                    .get_mut(*from)
                    .and_then(Option::take)
                    .ok_or(Refusal::InvalidProgram)?,
            };
            slots[instruction.destination] = Some(value);
            pc += 1;
        }
        slots[self.result].take().ok_or(Refusal::InvalidProgram)
    }
}

impl PreparedPortableExpressionEvaluator {
    /// Explicit fixed-integer/boolean specialization; unsupported profiles refuse.
    pub fn new_scalar_tape(program: &PortableExpressionProgram) -> Result<Self, Refusal> {
        let mut prepared = Self::new(program)?;
        let PreparedRoot::Primitive { node, nominal_type } = &prepared.root else {
            return Err(Refusal::UnsupportedType("scalar-tape/structured@1".into()));
        };
        let tape = ScalarTape::prepare(node)?;
        prepared.root = PreparedRoot::ScalarTape {
            tape,
            nominal_type: nominal_type.clone(),
        };
        Ok(prepared)
    }
    pub fn scalar_tape_limits(&self) -> Option<(usize, usize)> {
        match &self.root {
            PreparedRoot::ScalarTape { tape, .. } => {
                Some((tape.instruction_count(), tape.scratch_bytes()))
            }
            _ => None,
        }
    }

    /// Retained tape heap payload, excluding allocator metadata and the evaluator
    /// object. Literal capacities and the admitted instruction capacity are
    /// charged even when a lazy branch does not execute them.
    pub fn scalar_tape_storage_bytes(&self) -> Option<usize> {
        let PreparedRoot::ScalarTape { tape, .. } = &self.root else {
            return None;
        };
        Some(
            tape.instructions.capacity() * core::mem::size_of::<Instruction>()
                + tape
                    .instructions
                    .iter()
                    .map(|instruction| match &instruction.operation {
                        Operation::Literal(bytes) => bytes.capacity(),
                        _ => 0,
                    })
                    .sum::<usize>(),
        )
    }
}
