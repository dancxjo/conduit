//! Pre-Play preparation and allocation-free evaluation for primitive expressions.

use crate::{
    BinaryOperator, PortableExpressionEvaluationRefusal as Refusal, PortableExpressionNode,
    PortableExpressionOperation, PortableExpressionProgram, UnaryOperator,
};
use alloc::{boxed::Box, string::String, vec::Vec};
use conduit_core::{PrimitiveInfoKind, StructuredInfoTypeShape};

mod byte_observation;
mod equality;
mod evaluate;
mod quantity_profile;
mod scalar_tape;
use evaluate::evaluate_node;
mod inspection;
mod shared_input;
use shared_input::SharedBytes;
mod member_selection;
mod nominal;
mod sequence_selection;
use member_selection::PreparedMemberSelection;
mod predicate_bounds;
mod prepare;
mod primitive;
pub use predicate_bounds::ExpressionValueContractBounds;
use prepare::prepare_node;
mod angle_reference;
mod inverse_trigonometry;
mod storage_bound;
mod structured;
mod structured_contract;
mod text_material;
mod trigonometry;
use primitive::{
    decode_bool, evaluate_binary, evaluate_unary, kind_name, leaf_info_kind, leaf_kind,
    PrimitiveValue,
};
use structured::PreparedStructuredExpression;

/// A prepared primitive-only evaluator. Construction owns every allocation;
/// `evaluate` uses fixed stack values and one capacity-stable output buffer.
pub struct PreparedPortableExpressionEvaluator {
    root: PreparedRoot,
    input: PreparedInput,
    input_maximum_bytes: Option<u32>,
    input_value_contract: Option<conduit_core::CheckedValueContract>,
    output_value_contract: Option<conduit_core::CheckedValueContract>,
    output: Vec<u8>,
}

#[derive(Clone)]
enum PreparedInput {
    Primitive {
        kind: PrimitiveInfoKind,
        info_kind: String,
        nominal_type: Option<SharedBytes>,
        quantity_profile: conduit_core::QuantityValueProfile,
        maximum_bytes: Option<u32>,
    },
    Structured(SharedBytes, Option<u32>),
}

impl PreparedInput {
    fn quantity_profile(&self) -> conduit_core::QuantityValueProfile {
        match self {
            Self::Primitive {
                quantity_profile, ..
            } => *quantity_profile,
            Self::Structured(..) => conduit_core::QuantityValueProfile::Decimal,
        }
    }
    fn maximum_bytes(&self) -> Option<u32> {
        match self {
            Self::Primitive { maximum_bytes, .. } | Self::Structured(_, maximum_bytes) => {
                *maximum_bytes
            }
        }
    }
}

#[derive(Clone, Copy)]
struct ProgramView<'a> {
    input_maximum_bytes: Option<u32>,
    input_type: &'a conduit_core::StructuredInfoType,
    output_type: &'a conduit_core::StructuredInfoType,
    root: &'a PortableExpressionNode,
}

enum PreparedRoot {
    ScalarTape {
        tape: scalar_tape::ScalarTape,
        nominal_type: Option<Vec<u8>>,
    },
    Primitive {
        node: PreparedNode,
        nominal_type: Option<Vec<u8>>,
    },
    Structured(PreparedStructuredExpression),
}

struct PreparedNode {
    quantity_profile: conduit_core::QuantityValueProfile,
    kind: PrimitiveInfoKind,
    info_kind: String,
    operation: PreparedOperation,
}

enum PreparedOperation {
    InverseTrigonometry {
        function: crate::expression_inverse_trigonometry::Function,
        left: Box<PreparedNode>,
        right: Option<Box<PreparedNode>>,
        output: Vec<u8>,
    },
    AngleReference {
        argument: Box<PreparedNode>,
        output: Vec<u8>,
    },
    Trigonometry {
        function: conduit_core::AngleTrigonometricFunction,
        argument: Box<PreparedNode>,
        output: Vec<u8>,
    },
    QuantityBinary {
        profile: conduit_core::QuantityValueProfile,
        operator: BinaryOperator,
        left: Box<PreparedNode>,
        right: Box<PreparedNode>,
        output: Vec<u8>,
    },
    RationalBinary {
        operator: BinaryOperator,
        left: Box<PreparedNode>,
        right: Box<PreparedNode>,
        output: Vec<u8>,
    },
    Equality(Box<equality::PreparedEquality>),
    Input,
    Literal(Vec<u8>),
    Unary {
        operator: UnaryOperator,
        operand: Box<PreparedNode>,
    },
    Binary {
        operator: BinaryOperator,
        proven: bool,
        left: Box<PreparedNode>,
        right: Box<PreparedNode>,
    },
    Conditional {
        condition: Box<PreparedNode>,
        when_true: Box<PreparedNode>,
        when_false: Box<PreparedNode>,
    },
    Projection(PreparedMemberSelection),
    SequenceSelection(sequence_selection::PreparedSequenceSelection),
    Widen(Box<PreparedNode>),
    TextMaterial(Box<PreparedNode>),
    Inspection(inspection::PreparedInspection),
    Bytes(byte_observation::PreparedByteObservation),
}

use storage_bound::maximum_input_type_bytes;
pub use storage_bound::maximum_prepared_canonical_value_bytes;

impl PortableExpressionProgram {
    /// Conservative canonical transport ceiling, computed before Play from the exact input Type.
    pub fn maximum_prepared_input_bytes(&self) -> Result<u32, Refusal> {
        self.validate_input_bound()
            .map_err(|_| Refusal::InvalidProgram)?;
        let structural = maximum_input_type_bytes(&self.input_type)?;
        Ok(self
            .input_maximum_bytes
            .map_or(structural, |maximum| maximum.min(structural)))
    }

    /// Conservative transport ceiling from the operation and admitted input.
    /// Programs without an input envelope retain their structural Type ceiling.
    pub fn maximum_prepared_output_bytes(&self) -> Result<u32, Refusal> {
        if self.input_type == self.output_type
            && matches!(self.root.operation, PortableExpressionOperation::Input)
        {
            return self.maximum_prepared_input_bytes();
        }
        self.validate_input_bound()
            .map_err(|_| Refusal::InvalidProgram)?;
        u32::try_from(storage_bound::expression(
            &self.root,
            &self.input_type,
            self.input_maximum_bytes,
        )?)
        .map_err(|_| Refusal::InvalidProgram)
    }
}

impl PreparedPortableExpressionEvaluator {
    pub fn new(program: &PortableExpressionProgram) -> Result<Self, Refusal> {
        Self::new_profile(program, conduit_core::QuantityValueProfile::Decimal)
    }

    /// Explicit finite exact-coordinate evaluator profile. Currently restricted
    /// to primitive input/output Types; composite and nominal profiles refuse
    /// before allocating execution state. This constructor is not a Host offer.
    pub fn new_with_exact_quantities(program: &PortableExpressionProgram) -> Result<Self, Refusal> {
        if !matches!(program.input_type.shape(), StructuredInfoTypeShape::Leaf(_))
            || !matches!(
                program.output_type.shape(),
                StructuredInfoTypeShape::Leaf(_)
            )
        {
            return Err(Refusal::UnsupportedType(
                "exact Quantity profile requires primitive ports".into(),
            ));
        }
        quantity_profile::admit_primitive_nodes(&program.root)?;
        Self::new_profile(program, conduit_core::QuantityValueProfile::Exact)
    }

    fn new_profile(
        program: &PortableExpressionProgram,
        quantity_profile: conduit_core::QuantityValueProfile,
    ) -> Result<Self, Refusal> {
        let input = match program.input_type.shape() {
            StructuredInfoTypeShape::Leaf(_) => PreparedInput::Primitive {
                kind: leaf_kind(&program.input_type)?,
                info_kind: leaf_info_kind(&program.input_type)?.into(),
                nominal_type: None,
                quantity_profile,
                maximum_bytes: program.input_maximum_bytes,
            },
            StructuredInfoTypeShape::Nominal { representation, .. }
                if matches!(representation.shape(), StructuredInfoTypeShape::Leaf(_)) =>
            {
                PreparedInput::Primitive {
                    kind: leaf_kind(&program.input_type)?,
                    info_kind: leaf_info_kind(&program.input_type)?.into(),
                    quantity_profile,
                    maximum_bytes: program.input_maximum_bytes,
                    nominal_type: Some(
                        program
                            .input_type
                            .canonical_bytes()
                            .map_err(|_| Refusal::InvalidProgram)?
                            .into(),
                    ),
                }
            }
            _ => PreparedInput::Structured(
                program
                    .input_type
                    .canonical_bytes()
                    .map_err(|_| Refusal::InvalidProgram)?
                    .into(),
                program.input_maximum_bytes,
            ),
        };
        program
            .validate_input_bound()
            .map_err(|_| Refusal::InvalidProgram)?;
        let mut prepared = Self::prepare_with_output_capacity(
            ProgramView {
                input_maximum_bytes: program.input_maximum_bytes,
                input_type: &program.input_type,
                output_type: &program.output_type,
                root: &program.root,
            },
            input,
            quantity_profile::output_capacity(
                &program.root,
                &program.input_type,
                program.input_maximum_bytes,
                quantity_profile,
            )?,
        )?;
        prepared.input_maximum_bytes = program.input_maximum_bytes;
        Ok(prepared)
    }

    fn prepare(program: ProgramView<'_>, input: PreparedInput) -> Result<Self, Refusal> {
        let capacity = quantity_profile::output_capacity(
            program.root,
            program.input_type,
            program.input_maximum_bytes,
            input.quantity_profile(),
        )?;
        Self::prepare_with_output_capacity(program, input, capacity)
    }

    fn prepare_with_output_capacity(
        program: ProgramView<'_>,
        input: PreparedInput,
        capacity: usize,
    ) -> Result<Self, Refusal> {
        let (input_contract, output_contract, body) =
            crate::expression_program::value_contract::boundary(program.root);
        let input_value_contract = input_contract.cloned();
        let output_value_contract = output_contract.cloned();
        let program = ProgramView {
            root: body,
            ..program
        };
        let root = match program.output_type.shape() {
            StructuredInfoTypeShape::Leaf(_) | StructuredInfoTypeShape::Nominal { .. }
                if leaf_kind(program.output_type).is_ok() =>
            {
                let root = prepare_node(program.root, program.input_type, &input)?;
                if root.kind != leaf_kind(program.output_type)? {
                    return Err(Refusal::InvalidProgram);
                }
                PreparedRoot::Primitive {
                    node: root,
                    nominal_type: matches!(
                        program.output_type.shape(),
                        StructuredInfoTypeShape::Nominal { .. }
                    )
                    .then(|| {
                        program
                            .output_type
                            .canonical_bytes()
                            .map_err(|_| Refusal::InvalidProgram)
                    })
                    .transpose()?,
                }
            }
            _ => PreparedRoot::Structured(PreparedStructuredExpression::new(program, &input)?),
        };
        Ok(Self {
            root,
            input,
            input_maximum_bytes: program.input_maximum_bytes,
            input_value_contract,
            output_value_contract,
            output: Vec::with_capacity(capacity),
        })
    }

    pub fn evaluate(&mut self, input: &[u8]) -> Result<&[u8], Refusal> {
        if let Some(contract) = &self.input_value_contract {
            contract
                .validate(input)
                .map_err(Refusal::InputValueConstraint)?;
        }
        if self
            .input_maximum_bytes
            .is_some_and(|maximum| input.len() > maximum as usize)
        {
            return Err(Refusal::InvalidInput);
        }
        let mut primitive_bytes = input;
        let primitive_input = match &self.input {
            PreparedInput::Primitive {
                kind,
                info_kind,
                nominal_type,
                ..
            } => {
                if let Some(expected) = nominal_type {
                    primitive_bytes = nominal::input_payload(input, expected)?;
                }
                quantity_profile::validate_input(
                    info_kind,
                    primitive_bytes,
                    self.input.quantity_profile(),
                )?;
                Some(*kind)
            }
            PreparedInput::Structured(expected, _) => {
                let validated = conduit_core::validate_canonical_structured_value(input)
                    .map_err(|_| Refusal::InvalidInput)?;
                if validated.type_bytes() != expected.as_ref() {
                    return Err(Refusal::InvalidInput);
                }
                None
            }
        };
        self.output.clear();
        match &mut self.root {
            PreparedRoot::Primitive { node, nominal_type } => {
                let value = evaluate_node(node, primitive_bytes, primitive_input)?;
                if let Some(value_type) = nominal_type {
                    nominal::append_output(&mut self.output, value_type, &value)?;
                } else {
                    if value.length > self.output.capacity() {
                        return Err(Refusal::InvalidProgram);
                    }
                    self.output.extend_from_slice(value.as_slice());
                }
            }
            PreparedRoot::ScalarTape { tape, nominal_type } => {
                let value = tape.evaluate(primitive_bytes, primitive_input)?;
                if let Some(value_type) = nominal_type {
                    nominal::append_output(&mut self.output, value_type, &value)?;
                } else {
                    if value.length > self.output.capacity() {
                        return Err(Refusal::InvalidProgram);
                    }
                    self.output.extend_from_slice(value.as_slice());
                }
            }
            PreparedRoot::Structured(root) => root.evaluate(input, &mut self.output)?,
        }
        if let Some(contract) = &self.output_value_contract {
            contract
                .validate(&self.output)
                .map_err(Refusal::OutputValueConstraint)?;
        }
        Ok(&self.output)
    }

    /// Most recent prepared result, borrowed by a kernel Back until its Step
    /// commits. Evaluation/refusal ownership remains with the caller.
    pub fn output_bytes(&self) -> &[u8] {
        &self.output
    }

    pub fn output_capacity(&self) -> usize {
        self.output.capacity()
    }
}
