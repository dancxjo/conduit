//! Finite fixed-integer/boolean tape; lowering happens before evaluation.
use super::*;
const MAXIMUM_INSTRUCTIONS: usize = 128;
pub(super) struct ScalarTape {
    pub(super) instructions: Vec<Instruction>,
    pub(super) linked: Option<(&'static [u8], &'static [u8])>,
    pub(super) result: usize,
    pub(super) scratch: ScratchProfile,
}

// Bound before Play. Four small wrappers retain their fixed scratch profiles;
// one shared interpreter avoids duplicating the loop for each array length.
#[derive(Clone, Copy)]
pub(super) enum ScratchProfile {
    Tiny,
    Small,
    Medium,
    Maximum,
}
impl ScratchProfile {
    pub(super) fn prepare(instructions: usize) -> Self {
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
pub(super) struct Instruction<B = Vec<u8>> {
    pub(super) kind: PrimitiveInfoKind,
    pub(super) destination: usize,
    pub(super) operation: Operation<B>,
}
pub(super) enum Operation<B = Vec<u8>> {
    Input,
    Literal(B),
    ProjectedInput { start: u32, length: u32 },
    Unary(UnaryOperator, usize),
    Widen(usize),
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
            instructions: Vec::new(),
            linked: None,
            result: 0,
            scratch: ScratchProfile::Maximum,
        };
        tape.result = tape.lower(root)?;
        tape.scratch = ScratchProfile::prepare(tape.instructions.len());
        tape.instructions.shrink_to_fit();
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
            PreparedOperation::Projection(selection) => {
                let (start, length) = selection.fixed_leaf().ok_or_else(|| {
                    Refusal::UnsupportedType("scalar-tape/variable-projection@1".into())
                })?;
                Operation::ProjectedInput { start, length }
            }
            PreparedOperation::Literal(bytes) if bytes.len() <= 16 => {
                Operation::Literal(bytes.clone())
            }
            PreparedOperation::Widen(operand) => Operation::Widen(self.lower(operand)?),
            PreparedOperation::Unary { operator, operand } => {
                Operation::Unary(*operator, self.lower(operand)?)
            }
            PreparedOperation::Binary {
                operator,
                left,
                right,
                ..
            } => {
                // The portable hint alone carries no retained range witness.
                // Bind checked arithmetic, including for valid proven programs.
                Operation::Binary(*operator, false, self.lower(left)?, self.lower(right)?)
            }
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
        self.linked
            .map_or(self.instructions.len(), |(r, _)| r.len() / 24)
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
        self.evaluate_in_slots(input, input_kind, &mut slots)
    }
    // The caller retains the exact admitted array and owns its initialization
    // and cleanup. Only the instruction loop is shared; no scratch is allocated
    // here, and a returned value borrows the tape/input rather than this slice.
    #[inline(never)]
    fn evaluate_in_slots<'a>(
        &'a self,
        input: &'a [u8],
        input_kind: Option<PrimitiveInfoKind>,
        slots: &mut [Option<PrimitiveValue<'a>>],
    ) -> Result<PrimitiveValue<'a>, Refusal> {
        let mut pc = 0;
        while pc < self.instruction_count() {
            let instruction = self.instruction(pc)?;
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
                Operation::ProjectedInput { start, length } => {
                    let node = input
                        .get(*start as usize..*start as usize + *length as usize)
                        .ok_or(Refusal::InvalidInput)?;
                    let [0, a, b, c, d, bytes @ ..] = node else {
                        return Err(Refusal::InvalidInput);
                    };
                    if bytes.len() != u32::from_le_bytes([*a, *b, *c, *d]) as usize {
                        return Err(Refusal::InvalidInput);
                    }
                    PrimitiveValue::borrowed(instruction.kind, bytes)?
                }
                Operation::Unary(op, index) => evaluate_unary(*op, instruction.kind, get(*index)?)?,
                Operation::Widen(index) => {
                    primitive::evaluate_widen(instruction.kind, get(*index)?)?
                }
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
        require_intrinsic_type(&program.input_type)?;
        require_intrinsic_type(&program.output_type)?;
        require_intrinsic_node_types(&program.root)?;
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

// A nominal schema commits to native laws, but this portable format does not
// retain their validation programs. Unwrapping its representation would erase
// those laws, including assumptions used by proven arithmetic. Such profiles
// need a retained validation witness before this Back can admit them.
pub(super) fn require_intrinsic_type(
    value_type: &conduit_core::StructuredInfoType,
) -> Result<(), Refusal> {
    match value_type.shape() {
        StructuredInfoTypeShape::Leaf(_) => Ok(()),
        StructuredInfoTypeShape::Nominal { .. } => Err(Refusal::UnsupportedType(
            "scalar-tape/native-law-witness-unavailable@1".into(),
        )),
        StructuredInfoTypeShape::Record { fields, .. } => fields
            .iter()
            .try_for_each(|field| require_intrinsic_type(field.value_type())),
        StructuredInfoTypeShape::Variant { cases, .. } => cases
            .iter()
            .try_for_each(|case| require_intrinsic_type(case.payload_type())),
        StructuredInfoTypeShape::Collection { element, .. }
        | StructuredInfoTypeShape::Sequence { element, .. } => require_intrinsic_type(element),
    }
}

pub(super) fn require_intrinsic_node_types(node: &PortableExpressionNode) -> Result<(), Refusal> {
    require_intrinsic_type(&node.value_type)?;
    match &node.operation {
        PortableExpressionOperation::ValueContract { value: operand, .. }
        | PortableExpressionOperation::Unary { operand, .. } => {
            require_intrinsic_node_types(operand)
        }
        PortableExpressionOperation::Binary { left, right, .. } => {
            require_intrinsic_node_types(left)?;
            require_intrinsic_node_types(right)
        }
        PortableExpressionOperation::Conditional {
            condition,
            when_true,
            when_false,
        } => {
            require_intrinsic_node_types(condition)?;
            require_intrinsic_node_types(when_true)?;
            require_intrinsic_node_types(when_false)
        }
        PortableExpressionOperation::Projection { value, .. }
        | PortableExpressionOperation::Variant { payload: value, .. } => {
            require_intrinsic_node_types(value)
        }
        PortableExpressionOperation::Tuple(values)
        | PortableExpressionOperation::Collection(values)
        | PortableExpressionOperation::SemanticCall {
            arguments: values, ..
        } => values.iter().try_for_each(require_intrinsic_node_types),
        PortableExpressionOperation::Record(fields) => fields
            .iter()
            .try_for_each(|(_, value)| require_intrinsic_node_types(value)),
        _ => Ok(()),
    }
}

impl ScalarTape {
    fn instruction(&self, pc: usize) -> Result<Instruction<&[u8]>, Refusal> {
        if let Some((records, literals)) = self.linked {
            return super::linked::scalar_instruction(records, literals, pc);
        }
        let i = self.instructions.get(pc).ok_or(Refusal::InvalidProgram)?;
        let operation = match &i.operation {
            Operation::Input => Operation::Input,
            Operation::Literal(b) => Operation::Literal(b.as_slice()),
            Operation::ProjectedInput { start, length } => Operation::ProjectedInput {
                start: *start,
                length: *length,
            },
            Operation::Unary(a, b) => Operation::Unary(*a, *b),
            Operation::Widen(a) => Operation::Widen(*a),
            Operation::Binary(a, b, c, d) => Operation::Binary(*a, *b, *c, *d),
            Operation::BranchFalse(a, b) => Operation::BranchFalse(*a, *b),
            Operation::Jump(a) => Operation::Jump(*a),
            Operation::Move(a) => Operation::Move(*a),
        };
        Ok(Instruction {
            kind: i.kind,
            destination: i.destination,
            operation,
        })
    }
}
