//! One authoring-time constructor compiler for owned and resident storage.
use super::*;
use crate::expression_prepared::fixed_projection;
use crate::expression_prepared::structured::{constant_child, ChildEvaluator};
use crate::{PortableExpressionNode as Node, PortableExpressionOperation as Expression};
use conduit_core::{StructuredInfoType, StructuredInfoTypeShape};
const MAX_DEPTH: u8 = 32;
pub(super) struct ConstructorTape {
    pub(super) code: Vec<Operation>,
    pub(super) constants: Vec<u8>,
    value_count: usize,
}
impl ConstructorTape {
    pub(super) fn compile(
        program: ProgramView<'_>,
        input: &PreparedInput,
        values: &mut impl FnMut(&Node) -> Result<(), Refusal>,
    ) -> Result<Self, Refusal> {
        let mut tape = Self {
            code: Vec::new(),
            constants: Vec::new(),
            value_count: 0,
        };
        tape.lower(program, program.root, input, 0, values)?;
        tape.code.shrink_to_fit();
        tape.constants.shrink_to_fit();
        Ok(tape)
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
        self.constants.extend_from_slice(bytes);
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
        node: &Node,
        values: &mut impl FnMut(&Node) -> Result<(), Refusal>,
    ) -> Result<u16, Refusal> {
        if self.value_count == MAX_VALUES {
            return Err(Refusal::InvalidProgram);
        }
        let value = self.value_count as u16;
        values(node)?;
        self.value_count += 1;
        Ok(value)
    }
    fn lower(
        &mut self,
        program: ProgramView<'_>,
        node: &Node,
        input: &PreparedInput,
        depth: u8,
        values: &mut impl FnMut(&Node) -> Result<(), Refusal>,
    ) -> Result<(), Refusal> {
        if depth == MAX_DEPTH {
            return Err(Refusal::InvalidProgram);
        }
        let view = ProgramView {
            output_type: &node.value_type,
            root: node,
            ..program
        };
        crate::expression_prepared::structured_contract::validate(view)?;
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
                    self.lower(program, value, input, depth + 1, values)?;
                }
            }
            Expression::Tuple(items) => {
                self.bytes(&[2])?;
                self.length(items.len())?;
                for (index, value) in items.iter().enumerate() {
                    self.text(&alloc::format!("item-{index:05}"))?;
                    self.lower(program, value, input, depth + 1, values)?;
                }
            }
            Expression::Collection(items) => {
                self.bytes(&[1])?;
                self.length(items.len())?;
                for value in items {
                    self.lower(program, value, input, depth + 1, values)?;
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
                self.lower(program, payload, input, depth + 1, values)?;
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
                let value = self.value(condition, values)?;
                let branch = self.push(Operation::BranchFalse { value, target: 0 })?;
                self.lower(program, when_true, input, depth + 1, values)?;
                let jump = self.push(Operation::Jump(0))?;
                self.code[branch] = Operation::BranchFalse {
                    value,
                    target: self.code.len() as u16,
                };
                self.lower(program, when_false, input, depth + 1, values)?;
                self.code[jump] = Operation::Jump(self.code.len() as u16);
            }
            _ => {
                let value = self.value(node, values)?;
                self.push(Operation::Value(value))?;
            }
        }
        Ok(())
    }
}
