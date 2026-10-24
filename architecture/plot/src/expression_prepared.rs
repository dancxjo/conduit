//! Pre-Play preparation and allocation-free evaluation for primitive expressions.

use crate::{
    BinaryOperator, PortableExpressionEvaluationRefusal as Refusal, PortableExpressionNode,
    PortableExpressionOperation, PortableExpressionProgram, UnaryOperator,
};
use alloc::{borrow::Cow, boxed::Box, vec::Vec};
use conduit_core::{PrimitiveInfoKind, StructuredInfoTypeShape};

mod arithmetic_policy;
mod execution;
mod law_guard;
use arithmetic_policy::ArithmeticPolicy;
pub use law_guard::{
    native_law_failure_detail, PreparedExpressionLawBounds, PreparedExpressionLawCapacityReport,
};
mod byte_observation;
mod equality;
mod evaluate;
mod native_u64;
mod quantity_profile;
mod scalar_tape;
use evaluate::evaluate_node;
pub use native_u64::PreparedNativeU64Expression;
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

fn prepare_info_kind(
    value_type: &conduit_core::StructuredInfoType,
) -> Result<Cow<'static, str>, Refusal> {
    let name = leaf_info_kind(value_type)?;
    let standard = kind_name(leaf_kind(value_type)?);
    Ok(if name == standard {
        Cow::Borrowed(standard)
    } else {
        // Dynamic profiles and other exact names keep their original identity.
        Cow::Owned(name.into())
    })
}

/// A prepared primitive-only evaluator. Construction owns every allocation;
/// `evaluate` uses fixed stack values and one capacity-stable output buffer.
pub struct PreparedPortableExpressionEvaluator {
    root: PreparedRoot,
    input: PreparedInput,
    input_maximum_bytes: Option<u32>,
    input_value_contract: Option<conduit_core::CheckedValueContract>,
    output_value_contract: Option<conduit_core::CheckedValueContract>,
    output: Vec<u8>,
    input_law: Option<Box<law_guard::NodeGuard>>,
    output_law: Option<Box<law_guard::NodeGuard>>,
}

#[derive(Clone)]
enum PreparedInput {
    Primitive {
        kind: PrimitiveInfoKind,
        info_kind: Cow<'static, str>,
        nominal_type: Option<SharedBytes>,
        quantity_profile: conduit_core::QuantityValueProfile,
        maximum_bytes: Option<u32>,
        arithmetic_policy: ArithmeticPolicy,
        laws: Option<law_guard::Context>,
    },
    Structured(
        SharedBytes,
        Option<u32>,
        ArithmeticPolicy,
        Option<law_guard::Context>,
    ),
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
    fn arithmetic_policy(&self) -> ArithmeticPolicy {
        match self {
            Self::Primitive {
                arithmetic_policy, ..
            }
            | Self::Structured(_, _, arithmetic_policy, _) => *arithmetic_policy,
        }
    }
    fn maximum_bytes(&self) -> Option<u32> {
        match self {
            Self::Primitive { maximum_bytes, .. } | Self::Structured(_, maximum_bytes, _, _) => {
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
    info_kind: Cow<'static, str>,
    operation: PreparedOperation,
    law: Option<Box<law_guard::NodeGuard>>,
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
        arithmetic_policy: ArithmeticPolicy,
        left: Box<PreparedNode>,
        right: Box<PreparedNode>,
    },
    Conditional {
        condition: Box<PreparedNode>,
        when_true: Box<PreparedNode>,
        when_false: Box<PreparedNode>,
    },
    // Reserve selector objects only for nodes that use them, during preparation.
    // Keeping selector buffers inline inflated arithmetic nodes as well.
    Projection(Box<PreparedMemberSelection>),
    SequenceSelection(Box<sequence_selection::PreparedSequenceSelection>),
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
        Self::new_profile(
            program,
            conduit_core::QuantityValueProfile::Decimal,
            ArithmeticPolicy::LegacyProven,
            None,
        )
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
        Self::new_profile(
            program,
            conduit_core::QuantityValueProfile::Exact,
            ArithmeticPolicy::LegacyProven,
            None,
        )
    }

    fn new_profile(
        program: &PortableExpressionProgram,
        quantity_profile: conduit_core::QuantityValueProfile,
        arithmetic_policy: ArithmeticPolicy,
        laws: Option<law_guard::Context>,
    ) -> Result<Self, Refusal> {
        let input = match program.input_type.shape() {
            StructuredInfoTypeShape::Leaf(_) => PreparedInput::Primitive {
                kind: leaf_kind(&program.input_type)?,
                info_kind: prepare_info_kind(&program.input_type)?,
                nominal_type: None,
                quantity_profile,
                maximum_bytes: program.input_maximum_bytes,
                arithmetic_policy,
                laws: laws.clone(),
            },
            StructuredInfoTypeShape::Nominal { representation, .. }
                if matches!(representation.shape(), StructuredInfoTypeShape::Leaf(_)) =>
            {
                PreparedInput::Primitive {
                    kind: leaf_kind(&program.input_type)?,
                    info_kind: prepare_info_kind(&program.input_type)?,
                    quantity_profile,
                    maximum_bytes: program.input_maximum_bytes,
                    arithmetic_policy,
                    laws: laws.clone(),
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
                arithmetic_policy,
                laws.clone(),
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
        prepared.input_law = law_guard::input_guard(program, laws.as_ref())?;
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
        let output_law = law_guard::output_guard(program, &input, capacity)?;
        Ok(Self {
            root,
            input,
            input_maximum_bytes: program.input_maximum_bytes,
            input_value_contract,
            output_value_contract,
            output: Vec::with_capacity(capacity),
            input_law: None,
            output_law,
        })
    }

    // Immutable after preparation. Structured children compare against this
    // admitted prefix without retaining another identical schema byte buffer.
    fn output_type_prefix(&self) -> Option<&[u8]> {
        match &self.root {
            PreparedRoot::Primitive { nominal_type, .. }
            | PreparedRoot::ScalarTape { nominal_type, .. } => nominal_type.as_deref(),
            PreparedRoot::Structured(root) => Some(root.type_prefix()),
        }
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
