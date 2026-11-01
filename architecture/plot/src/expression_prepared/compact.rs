//! Optional lowering of admitted intrinsic scalar trees. Rich schemas remain at
//! the outer boundary; structured tape children share that validation and use
//! one existing finite scalar scratch profile per call.
use super::{
    scalar_tape, PreparedInput, PreparedPortableExpressionEvaluator, PreparedRoot, Refusal,
};
use crate::PortableExpressionProgram;
use alloc::vec::Vec;
use conduit_core::PrimitiveInfoKind;

impl PreparedPortableExpressionEvaluator {
    /// Same accepted programs and boundary validation as `new`. Intrinsic fixed
    /// scalar subexpressions become finite tapes; unsupported or nominal/native
    /// law profiles retain the ordinary evaluator. No Type or law is erased.
    pub fn new_compact(program: &PortableExpressionProgram) -> Result<Self, Refusal> {
        let mut prepared = Self::new(program)?;
        if scalar_tape::require_intrinsic_type(&program.input_type).is_ok()
            && scalar_tape::require_intrinsic_type(&program.output_type).is_ok()
            && scalar_tape::require_intrinsic_node_types(&program.root).is_ok()
        {
            prepared.compact_scalars();
        }
        Ok(prepared)
    }
    pub(super) fn compact_scalars(&mut self) {
        match &mut self.root {
            PreparedRoot::Primitive { node, nominal_type } if nominal_type.is_none() => {
                if let Ok(tape) = scalar_tape::ScalarTape::prepare(node) {
                    self.root = PreparedRoot::ScalarTape {
                        tape,
                        nominal_type: None,
                    };
                }
            }
            PreparedRoot::Structured(root) => root.compact_scalars(),
            _ => (),
        }
    }
    pub(super) fn scalar_child_eligible(&self) -> bool {
        matches!(
            self.root,
            PreparedRoot::ScalarTape {
                nominal_type: None,
                ..
            }
        ) && self.input_value_contract.is_none()
            && self.output_value_contract.is_none()
            && matches!(
                self.input,
                PreparedInput::Primitive {
                    nominal_type: None,
                    ..
                } | PreparedInput::Structured(..)
            )
    }
}

pub(super) struct ScalarChild {
    pub(super) tape: scalar_tape::ScalarTape,
    pub(super) output: Vec<u8>,
    pub(super) input_kind: Option<PrimitiveInfoKind>,
}
impl ScalarChild {
    /// The caller has already admitted the child, lowered its whole root and
    /// checked absence of local boundary contracts. Only the outer structured
    /// evaluator invokes this child, after full exact-input validation.
    pub(super) fn take(prepared: PreparedPortableExpressionEvaluator) -> Self {
        let PreparedRoot::ScalarTape {
            tape,
            nominal_type: None,
        } = prepared.root
        else {
            unreachable!("preflighted scalar child")
        };
        Self {
            tape,
            output: prepared.output,
            input_kind: match prepared.input {
                PreparedInput::Primitive { kind, .. } => Some(kind),
                PreparedInput::Structured(..) => None,
            },
        }
    }
    pub(super) fn evaluate(&mut self, input: &[u8]) -> Result<&[u8], Refusal> {
        let value = self.tape.evaluate(input, self.input_kind)?;
        if value.length > self.output.capacity() {
            return Err(Refusal::InvalidProgram);
        }
        self.output.clear();
        self.output.extend_from_slice(value.as_slice());
        Ok(&self.output)
    }
    pub(super) fn output_bytes(&self) -> &[u8] {
        &self.output
    }
}
