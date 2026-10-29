//! Independently authenticated linked material to per-instance mutable state.
use super::super::structured::tape::{Operation as T, StructuredTape};
use super::super::structured::{ChildEvaluator, ChildOutput, PreparedChild, PreparedShape};
use super::super::*;
use super::wire::*;
impl PreparedPortableExpressionEvaluator {
    /// Initialize mutable state from immutable, independently authenticated Make
    /// output. The expected digest must bind the actual checked Source/config,
    /// not an identity claimed by received material.
    /// # Safety
    /// Root must establish complete checked Make origin and exact binding of ALL
    /// material, including lowering/profile/bounds, independently of input. Like
    /// IndexedSourceImage::from_make_image, checks here are not Source checking.
    /// Never call for network/user-supplied bytes merely because a digest matches.
    pub unsafe fn from_linked_make(
        bytes: &'static [u8],
        expected_program_digest: [u8; 32],
    ) -> Result<Self, Refusal> {
        Self::import_linked(bytes, expected_program_digest, None)
    }
    /// The installed-Make contract of `from_linked_make` applies. Expected
    /// bounds must come from the exact checked configuration, not received data.
    /// # Safety
    /// Complete independently authenticated Make origin and Source binding are
    /// required exactly as for `from_linked_make`.
    pub unsafe fn from_linked_make_with_bounds(
        bytes: &'static [u8],
        expected_program_digest: [u8; 32],
        expected_bounds: (u32, u32),
    ) -> Result<Self, Refusal> {
        Self::import_linked(bytes, expected_program_digest, Some(expected_bounds))
    }
    fn import_linked(
        bytes: &'static [u8],
        expected_program_digest: [u8; 32],
        expected_bounds: Option<(u32, u32)>,
    ) -> Result<Self, Refusal> {
        if bytes.len() > MAX {
            return Err(Refusal::InvalidProgram);
        }
        let mut r = Reader {
            bytes,
            input_limit: 65536,
        };
        if r.take(MAGIC.len())? != MAGIC || r.take(32)? != expected_program_digest {
            return Err(Refusal::InvalidProgram);
        }
        let input = r.n()?;
        r.input_limit = input;
        let output = r.n()?;
        if input > 65536 || output > 65536 || expected_bounds.is_some_and(|b| b != (input, output))
        {
            return Err(Refusal::InvalidProgram);
        }
        let saved = r.bytes;
        let mut budget = 0;
        let capacity = r.scan_evaluator(0, &mut budget)?;
        if !r.bytes.is_empty() || capacity != output as usize {
            return Err(Refusal::InvalidProgram);
        }
        r.bytes = saved;
        let evaluator = r.evaluator(0)?;
        if !r.bytes.is_empty() || evaluator.output.capacity() != output as usize {
            return Err(Refusal::InvalidProgram);
        };
        Ok(evaluator)
    }
}
impl Reader {
    fn capacity(&mut self) -> Result<Vec<u8>, Refusal> {
        let n = self.n()? as usize;
        if n > 65536 {
            return Err(Refusal::InvalidProgram);
        }
        Ok(Vec::with_capacity(n))
    }
    fn evaluator(&mut self, depth: usize) -> Result<PreparedPortableExpressionEvaluator, Refusal> {
        if depth > 32 {
            return Err(Refusal::InvalidProgram);
        }
        let input_maximum_bytes = self.opt()?;
        let output = self.capacity()?;
        let input = match self.b()? {
            0 => {
                let kind = kind(self.b()?)?;
                PreparedInput::Primitive {
                    kind,
                    info_kind: Cow::Borrowed(kind_name(kind)),
                    nominal_type: None,
                    quantity_profile: conduit_core::QuantityValueProfile::Decimal,
                    maximum_bytes: self.opt()?,
                }
            }
            1 => PreparedInput::Structured(SharedBytes::Static(self.blob()?), self.opt()?),
            _ => return Err(Refusal::InvalidProgram),
        };
        let root = match self.b()? {
            0 => PreparedRoot::ScalarTape {
                tape: self.scalar()?,
                nominal_type: None,
            },
            1 => PreparedRoot::Structured(self.structured(depth + 1)?),
            2 => {
                let kind = kind(self.b()?)?;
                let bytes = self.blob()?;
                PreparedRoot::LinkedLiteral { kind, bytes }
            }
            _ => return Err(Refusal::InvalidProgram),
        };
        Ok(PreparedPortableExpressionEvaluator {
            root,
            input,
            input_maximum_bytes,
            input_value_contract: None,
            output_value_contract: None,
            output,
        })
    }
    fn structured(&mut self, depth: usize) -> Result<PreparedStructuredExpression, Refusal> {
        if depth > 32 {
            return Err(Refusal::InvalidProgram);
        }
        let type_prefix = Cow::Borrowed(self.blob()?);
        let shape = match self.b()? {
            0 => {
                let code = self.blob()?;
                let constants = self.blob()?;
                let n = self.n()? as usize;
                if code.len() % 12 != 0
                    || code.len() / 12 > 512
                    || n > 128
                    || constants.len() > 65536
                {
                    return Err(Refusal::InvalidProgram);
                }
                let mut values = Vec::with_capacity(n);
                for _ in 0..n {
                    values.push(self.child(depth + 1)?)
                }
                for pc in 0..code.len() / 12 {
                    match structured_instruction(code, pc)? {
                        T::Bytes { start, length } => {
                            constants
                                .get(
                                    start as usize
                                        ..(start as usize)
                                            .checked_add(length as usize)
                                            .ok_or(Refusal::InvalidProgram)?,
                                )
                                .ok_or(Refusal::InvalidProgram)?;
                        }
                        T::InputNode { start, length } => {
                            if start
                                .checked_add(length)
                                .is_none_or(|end| end > self.input_limit)
                            {
                                return Err(Refusal::InvalidProgram);
                            };
                        }
                        T::Value(v) => {
                            if v as usize >= n {
                                return Err(Refusal::InvalidProgram);
                            }
                        }
                        T::BranchFalse { value, target } => {
                            if value as usize >= n
                                || target as usize <= pc
                                || target as usize > code.len() / 12
                            {
                                return Err(Refusal::InvalidProgram);
                            }
                        }
                        T::Jump(target) => {
                            if target as usize <= pc || target as usize > code.len() / 12 {
                                return Err(Refusal::InvalidProgram);
                            }
                        }
                    }
                }
                PreparedShape::Tape(Box::new(StructuredTape {
                    code: Vec::new(),
                    linked: Some(code),
                    constants: Cow::Borrowed(constants),
                    values,
                }))
            }
            1 => PreparedShape::Input,
            2 => PreparedShape::Constant(Cow::Borrowed(self.blob()?)),
            3 => PreparedShape::Selected(Box::new(self.selection(depth + 1)?)),
            _ => return Err(Refusal::InvalidProgram),
        };
        Ok(PreparedStructuredExpression { type_prefix, shape })
    }
    fn child(&mut self, depth: usize) -> Result<PreparedChild, Refusal> {
        let output = match self.b()? {
            0 => ChildOutput::Leaf,
            1 => ChildOutput::Structured,
            _ => return Err(Refusal::InvalidProgram),
        };
        let evaluator = match self.b()? {
            0 => ChildEvaluator::Constant {
                encoded: Cow::Borrowed(self.blob()?),
            },
            1 => ChildEvaluator::Dynamic(Box::new(self.evaluator(depth + 1)?)),
            2 => {
                let output = self.capacity()?;
                let k = self.b()?;
                let input_kind = if k == 255 { None } else { Some(kind(k)?) };
                ChildEvaluator::Scalar(Box::new(compact::ScalarChild {
                    tape: self.scalar()?,
                    output,
                    input_kind,
                }))
            }
            _ => return Err(Refusal::InvalidProgram),
        };
        Ok(PreparedChild { output, evaluator })
    }
    fn selection(
        &mut self,
        depth: usize,
    ) -> Result<member_selection::PreparedMemberSelection, Refusal> {
        let source = if self.flag()? {
            Some(Box::new(self.evaluator(depth + 1)?))
        } else {
            None
        };
        let n = self.n()? as usize;
        if n > 128 {
            return Err(Refusal::InvalidProgram);
        }
        let start = self.bytes;
        for _ in 0..n {
            next_member(&mut self.bytes)?;
        }
        let consumed = start.len() - self.bytes.len();
        let linked_steps = Some((n, &start[..consumed]));
        let steps = Vec::new();
        let output_type = Cow::Borrowed(self.blob()?);
        let output = self.capacity()?;
        let primitive = self.flag()?;
        let fixed = if self.flag()? {
            let a = self.n()?;
            let b = self.n()?;
            if a.checked_add(b).is_none_or(|end| end > self.input_limit) {
                return Err(Refusal::InvalidProgram);
            };
            Some((a, b))
        } else {
            None
        };
        Ok(member_selection::PreparedMemberSelection {
            source,
            steps,
            linked_steps,
            output_type,
            output,
            primitive,
            fixed,
        })
    }
}
