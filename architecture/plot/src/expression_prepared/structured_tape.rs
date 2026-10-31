//! Linear constructor encoding; scalar/effect-free leaf semantics stay shared.
use super::{append, child, constant_child, ChildEvaluator, PreparedChild};
use crate::expression_prepared::fixed_projection;
use crate::expression_prepared::{PreparedInput, ProgramView, Refusal};
use crate::{PortableExpressionNode as Node, PortableExpressionOperation as Expression};
use alloc::{borrow::Cow, vec::Vec};
use conduit_core::{StructuredInfoType, StructuredInfoTypeShape};

const MAX_OPERATIONS: usize = 512;
const MAX_VALUES: usize = 128;
const MAX_CONSTANT_BYTES: usize = 64 * 1024;
const MAX_DEPTH: u8 = 32;

#[derive(Clone, Copy)]
pub(in crate::expression_prepared) enum Operation {
    Bytes { start: u32, length: u32 },
    InputNode { start: u32, length: u32 },
    Value(u16),
    BranchFalse { value: u16, target: u16 },
    Jump(u16),
}
pub(in crate::expression_prepared) struct StructuredTape {
    pub(in crate::expression_prepared) code: Vec<Operation>,
    pub(in crate::expression_prepared) linked: Option<&'static [u8]>,
    pub(in crate::expression_prepared) constants: Cow<'static, [u8]>,
    pub(in crate::expression_prepared) values: Vec<PreparedChild>,
}
impl StructuredTape {
    pub(in crate::expression_prepared) fn compact_scalars(&mut self) {
        for value in &mut self.values {
            value.evaluator.compact_scalars();
        }
    }

    /// Optimization eligibility is not a new program-acceptance rule. Bounded
    /// compiler refusal leaves ordinary preparation to report its own result.
    pub(in crate::expression_prepared) fn prepare(
        program: ProgramView<'_>,
        input: &PreparedInput,
    ) -> Option<Self> {
        if !matches!(
            program.root.operation,
            Expression::Record(_)
                | Expression::Tuple(_)
                | Expression::Collection(_)
                | Expression::Variant { .. }
                | Expression::Conditional { .. }
        ) {
            return None;
        }
        let mut tape = Self {
            code: Vec::new(),
            linked: None,
            constants: Cow::Owned(Vec::new()),
            values: Vec::new(),
        };
        tape.lower(program, program.root, input, 0).ok()?;
        tape.code.shrink_to_fit();
        tape.constants.to_mut().shrink_to_fit();
        tape.values.shrink_to_fit();
        Some(tape)
    }

    pub(in crate::expression_prepared) fn evaluate(
        &mut self,
        input: &[u8],
        output: &mut Vec<u8>,
    ) -> Result<(), Refusal> {
        let mut pc = 0;
        while pc < self.linked.map_or(self.code.len(), |r| r.len() / 12) {
            match if let Some(r) = self.linked {
                super::super::linked::structured_instruction(r, pc)?
            } else {
                self.code[pc]
            } {
                Operation::Bytes { start, length } => {
                    let bytes = self
                        .constants
                        .get(start as usize..start as usize + length as usize)
                        .ok_or(Refusal::InvalidProgram)?;
                    append(output, bytes)?;
                }
                Operation::InputNode { start, length } => {
                    let bytes = input
                        .get(start as usize..start as usize + length as usize)
                        .ok_or(Refusal::InvalidInput)?;
                    append(output, bytes)?;
                }
                Operation::Value(value) => self
                    .values
                    .get_mut(usize::from(value))
                    .ok_or(Refusal::InvalidProgram)?
                    .append_node(input, output)?,
                Operation::BranchFalse { value, target } => {
                    match self
                        .values
                        .get_mut(usize::from(value))
                        .ok_or(Refusal::InvalidProgram)?
                        .evaluator
                        .evaluate(input)?
                    {
                        [0] => {
                            pc = usize::from(target);
                            continue;
                        }
                        [1] => (),
                        _ => return Err(Refusal::InvalidProgram),
                    }
                }
                Operation::Jump(target) => {
                    pc = usize::from(target);
                    continue;
                }
            }
            pc += 1;
        }
        Ok(())
    }

    fn push(&mut self, operation: Operation) -> Result<usize, Refusal> {
        if self.code.len() == MAX_OPERATIONS {
            return Err(Refusal::InvalidProgram);
        }
        let position = self.code.len();
        self.code.push(operation);
        Ok(position)
    }
    fn bytes(&mut self, bytes: &[u8]) -> Result<(), Refusal> {
        if self.constants.len().saturating_add(bytes.len()) > MAX_CONSTANT_BYTES {
            return Err(Refusal::InvalidProgram);
        }
        let start = self.constants.len() as u32;
        self.constants.to_mut().extend_from_slice(bytes);
        // A jump to the next instruction must execute subsequent common bytes.
        // Extending a previous branch's Bytes instruction would skip them.
        let joins_here = self.code.iter().any(|operation| {
            matches!(operation, Operation::Jump(target) | Operation::BranchFalse { target, .. }
                if usize::from(*target) == self.code.len())
        });
        if !joins_here {
            if let Some(Operation::Bytes {
                start: previous,
                length,
            }) = self.code.last_mut()
            {
                if *previous + *length == start {
                    *length += bytes.len() as u32;
                    return Ok(());
                }
            }
        }
        self.push(Operation::Bytes {
            start,
            length: bytes.len() as u32,
        })?;
        Ok(())
    }
    fn length(&mut self, length: usize) -> Result<(), Refusal> {
        self.bytes(
            &u32::try_from(length)
                .map_err(|_| Refusal::InvalidProgram)?
                .to_le_bytes(),
        )
    }
    fn text(&mut self, text: &str) -> Result<(), Refusal> {
        self.length(text.len())?;
        self.bytes(text.as_bytes())
    }
    fn value(
        &mut self,
        program: ProgramView<'_>,
        node: &Node,
        input: &PreparedInput,
    ) -> Result<u16, Refusal> {
        if self.values.len() == MAX_VALUES {
            return Err(Refusal::InvalidProgram);
        }
        let value = self.values.len() as u16;
        self.values.push(child(program, node, input)?);
        Ok(value)
    }
    fn lower(
        &mut self,
        program: ProgramView<'_>,
        node: &Node,
        input: &PreparedInput,
        depth: u8,
    ) -> Result<(), Refusal> {
        if depth == MAX_DEPTH {
            return Err(Refusal::InvalidProgram);
        }
        let view = ProgramView {
            output_type: &node.value_type,
            root: node,
            ..program
        };
        super::super::structured_contract::validate(view)?;
        if let Some(ChildEvaluator::Constant { encoded }) = constant_child(node) {
            self.bytes(&[0])?;
            self.length(encoded.len())?;
            return self.bytes(&encoded);
        }
        if let PreparedInput::Structured(prefix, _) = input {
            if let Some((start, length)) =
                fixed_projection::prepare(program.input_type, node, prefix.len())
            {
                self.push(Operation::InputNode { start, length })?;
                return Ok(());
            }
        }
        match &node.operation {
            Expression::Record(fields) => {
                self.bytes(&[2])?;
                self.length(fields.len())?;
                let mut ordered: Vec<_> = fields.iter().collect();
                ordered.sort_by(|left, right| left.0.cmp(&right.0));
                for (name, value) in ordered {
                    self.text(name)?;
                    self.lower(program, value, input, depth + 1)?;
                }
            }
            Expression::Tuple(values) => {
                self.bytes(&[2])?;
                self.length(values.len())?;
                for (index, value) in values.iter().enumerate() {
                    self.text(&alloc::format!("item-{index:05}"))?;
                    self.lower(program, value, input, depth + 1)?;
                }
            }
            Expression::Collection(values) => {
                self.bytes(&[1])?;
                self.length(values.len())?;
                for value in values {
                    self.lower(program, value, input, depth + 1)?;
                }
            }
            Expression::Variant { tag, payload } => {
                let StructuredInfoTypeShape::Variant { cases, .. } = node.value_type.shape() else {
                    return Err(Refusal::InvalidProgram);
                };
                if !cases
                    .iter()
                    .any(|case| case.tag() == tag && case.payload_type() == &payload.value_type)
                {
                    return Err(Refusal::InvalidProgram);
                }
                self.bytes(&[3])?;
                self.text(tag)?;
                self.lower(program, payload, input, depth + 1)?;
            }
            Expression::Conditional {
                condition,
                when_true,
                when_false,
            } => {
                if condition.value_type
                    != StructuredInfoType::leaf(conduit_core::kind_id(conduit_core::BOOL_INFO_ID))
                        .map_err(|_| Refusal::InvalidProgram)?
                    || when_true.value_type != node.value_type
                    || when_false.value_type != node.value_type
                {
                    return Err(Refusal::InvalidProgram);
                }
                let value = self.value(program, condition, input)?;
                let branch = self.push(Operation::BranchFalse { value, target: 0 })?;
                self.lower(program, when_true, input, depth + 1)?;
                let jump = self.push(Operation::Jump(0))?;
                self.code[branch] = Operation::BranchFalse {
                    value,
                    target: self.code.len() as u16,
                };
                self.lower(program, when_false, input, depth + 1)?;
                self.code[jump] = Operation::Jump(self.code.len() as u16);
            }
            _ => {
                let value = self.value(program, node, input)?;
                self.push(Operation::Value(value))?;
            }
        }
        Ok(())
    }
}
