//! Ordinary build-side checking/lowering and complete material export.
use super::super::scalar_tape::{Operation as S, ScalarTape};
use super::super::structured::tape::Operation as T;
use super::super::structured::{ChildEvaluator, ChildOutput, PreparedChild, PreparedShape};
use super::super::*;
use super::wire::*;
use sha2::{Digest, Sha256};
/// Produced only by ordinary complete program checking and compact lowering.
/// Bytes are construction data, never Source or runtime authority.
pub struct MakeExpressionMaterial {
    bytes: Vec<u8>,
}
impl MakeExpressionMaterial {
    pub fn export(program: &PortableExpressionProgram) -> Result<Self, Refusal> {
        let canonical = program
            .canonical_bytes()
            .map_err(|_| Refusal::InvalidProgram)?;
        let checked = PortableExpressionProgram::from_canonical_bytes(&canonical)
            .map_err(|_| Refusal::InvalidProgram)?;
        let input = checked.maximum_prepared_input_bytes()?;
        let output = checked.maximum_prepared_output_bytes()?;
        let prepared = PreparedPortableExpressionEvaluator::new_compact(&checked)?;
        if input > 65536 || output > 65536 {
            return Err(Refusal::InvalidProgram);
        }
        let mut w = Writer(Vec::new(), 0);
        w.0.extend_from_slice(MAGIC);
        w.0.extend_from_slice(&Sha256::digest(&canonical));
        w.n(input);
        w.n(output);
        w.evaluator(&prepared, 0)?;
        if w.0.len() > MAX {
            return Err(Refusal::InvalidProgram);
        };
        Ok(Self { bytes: w.0 })
    }
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
}
struct Writer(Vec<u8>, usize);
impl Writer {
    fn capacity(&mut self, n: usize) -> Result<(), Refusal> {
        Reader::charge(&mut self.1, n)?;
        self.n(n as u32);
        Ok(())
    }
    fn n(&mut self, v: u32) {
        self.0.extend_from_slice(&v.to_le_bytes())
    }
    fn b(&mut self, v: u8) {
        self.0.push(v)
    }
    fn blob(&mut self, v: &[u8]) {
        self.n(v.len() as u32);
        self.0.extend_from_slice(v)
    }
    fn opt(&mut self, v: Option<u32>) {
        self.b(v.is_some() as u8);
        if let Some(v) = v {
            self.n(v)
        }
    }
    fn evaluator(
        &mut self,
        e: &PreparedPortableExpressionEvaluator,
        depth: usize,
    ) -> Result<(), Refusal> {
        if depth > 32 || e.input_value_contract.is_some() || e.output_value_contract.is_some() {
            return Err(Refusal::UnsupportedType("linked-make/contracts@1".into()));
        }
        self.opt(e.input_maximum_bytes);
        self.capacity(e.output.capacity())?;
        match &e.input {
            PreparedInput::Primitive {
                kind,
                info_kind,
                nominal_type: None,
                quantity_profile: conduit_core::QuantityValueProfile::Decimal,
                maximum_bytes,
            } if info_kind.as_ref() == kind_name(*kind) => {
                self.b(0);
                self.b(kind_code(*kind)?);
                self.opt(*maximum_bytes)
            }
            PreparedInput::Structured(schema, bound) => {
                self.b(1);
                self.blob(schema);
                self.opt(*bound)
            }
            _ => return unsupported(),
        }
        match &e.root {
            PreparedRoot::ScalarTape {
                tape,
                nominal_type: None,
            } => {
                self.b(0);
                self.scalar(tape)?
            }
            PreparedRoot::Structured(root) => {
                self.b(1);
                self.structured(root, depth + 1)?
            }
            PreparedRoot::Primitive {
                node,
                nominal_type: None,
            } if matches!(node.operation, PreparedOperation::Literal(_)) => {
                let PreparedOperation::Literal(b) = &node.operation else {
                    unreachable!()
                };
                self.b(2);
                self.b(kind_code(node.kind)?);
                self.blob(b)
            }
            _ => return Err(Refusal::UnsupportedType("linked-make/root@1".into())),
        }
        Ok(())
    }
    fn scalar(&mut self, t: &ScalarTape) -> Result<(), Refusal> {
        if t.linked.is_some() {
            return unsupported();
        }
        let mut records = Vec::new();
        let mut literals = Vec::new();
        for i in &t.instructions {
            let mut args = [0u32; 5];
            args[0] = i.destination as u32;
            let op = match &i.operation {
                S::Input => 0,
                S::Literal(b) => {
                    args[1] = literals.len() as u32;
                    args[2] = b.len() as u32;
                    literals.extend_from_slice(b);
                    1
                }
                S::ProjectedInput { start, length } => {
                    args[1] = *start;
                    args[2] = *length;
                    2
                }
                S::Unary(o, a) => {
                    args[1] = unary_code(*o);
                    args[2] = *a as u32;
                    3
                }
                S::Widen(a) => {
                    args[1] = *a as u32;
                    4
                }
                S::Binary(o, proven, a, b) => {
                    if *proven {
                        return unsupported();
                    }
                    args[1] = binary_code(*o);
                    args[2] = *a as u32;
                    args[3] = *b as u32;
                    5
                }
                S::BranchFalse(a, b) => {
                    args[1] = *a as u32;
                    args[2] = *b as u32;
                    6
                }
                S::Jump(a) => {
                    args[1] = *a as u32;
                    7
                }
                S::Move(a) => {
                    args[1] = *a as u32;
                    8
                }
            };
            records.extend_from_slice(&[kind_code(i.kind)?, op, 0, 0]);
            for a in args {
                records.extend_from_slice(&a.to_le_bytes())
            }
        }
        self.n(t.result as u32);
        self.blob(&records);
        self.blob(&literals);
        Ok(())
    }
    fn structured(
        &mut self,
        s: &PreparedStructuredExpression,
        depth: usize,
    ) -> Result<(), Refusal> {
        self.blob(&s.type_prefix);
        match &s.shape {
            PreparedShape::Tape(t) => {
                self.b(0);
                let mut code = Vec::new();
                for op in &t.code {
                    let (tag, a, b) = match *op {
                        T::Bytes { start, length } => (0, start, length),
                        T::InputNode { start, length } => (1, start, length),
                        T::Value(v) => (2, v as u32, 0),
                        T::BranchFalse { value, target } => (3, value as u32, target as u32),
                        T::Jump(v) => (4, v as u32, 0),
                    };
                    code.extend_from_slice(&[tag, 0, 0, 0]);
                    code.extend_from_slice(&a.to_le_bytes());
                    code.extend_from_slice(&b.to_le_bytes())
                }
                self.blob(&code);
                self.blob(&t.constants);
                self.n(t.values.len() as u32);
                for child in &t.values {
                    self.child(child, depth + 1)?
                }
            }
            PreparedShape::Input => self.b(1),
            PreparedShape::Constant(b) => {
                self.b(2);
                self.blob(b)
            }
            PreparedShape::Selected(s) => {
                self.b(3);
                self.selection(s, depth + 1)?
            }
            _ => return unsupported(),
        }
        Ok(())
    }
    fn child(&mut self, c: &PreparedChild, depth: usize) -> Result<(), Refusal> {
        self.b(match c.output {
            ChildOutput::Leaf => 0,
            ChildOutput::Structured => 1,
        });
        match &c.evaluator {
            ChildEvaluator::Constant { encoded } => {
                self.b(0);
                self.blob(encoded)
            }
            ChildEvaluator::Dynamic(e) => {
                self.b(1);
                self.evaluator(e, depth + 1)?
            }
            ChildEvaluator::Scalar(e) => {
                self.b(2);
                self.capacity(e.output.capacity())?;
                self.b(e.input_kind.map(kind_code).transpose()?.unwrap_or(255));
                self.scalar(&e.tape)?
            }
        }
        Ok(())
    }
    fn selection(
        &mut self,
        s: &member_selection::PreparedMemberSelection,
        depth: usize,
    ) -> Result<(), Refusal> {
        self.b(s.source.is_some() as u8);
        if let Some(e) = &s.source {
            self.evaluator(e, depth + 1)?
        };
        self.n(s.steps.len() as u32);
        for step in &s.steps {
            use member_selection::Member;
            match step {
                Member::Field(n) => {
                    self.b(0);
                    self.blob(n.as_bytes())
                }
                Member::Index(n) => {
                    self.b(1);
                    self.n(*n as u32)
                }
                Member::Variant(n) => {
                    self.b(2);
                    self.blob(n.as_bytes())
                }
            }
        }
        self.blob(&s.output_type);
        self.capacity(s.output.capacity())?;
        self.b(s.primitive as u8);
        self.b(s.fixed.is_some() as u8);
        if let Some((a, b)) = s.fixed {
            self.n(a);
            self.n(b)
        }
        Ok(())
    }
}
