//! Select exact fixed projection geometry on the authoring Host, not the MCU.
use super::*;
use crate::rust_binding::{
    generate_resident_native_validator, PreparedNativeBounds, ResidentNativeGenerationRefusal,
};
use alloc::{format, string::String};
use sha2::Digest;
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StaticProjectedGenerationRefusal {
    Expression(Refusal),
    Native(ResidentNativeGenerationRefusal),
}
impl From<Refusal> for StaticProjectedGenerationRefusal {
    fn from(value: Refusal) -> Self {
        Self::Expression(value)
    }
}
/// Emit a fixed-scalar expression over a retained checked Native input. Variable
/// geometry and unsupported laws refuse. This reuses the scalar tape compiler,
/// executor and Native predicate emitter; no device semantics occur here.
pub fn generate_static_projected_expression(
    document: &crate::CheckedSyntaxDocument,
    program: &PortableExpressionProgram,
    bounds: PreparedNativeBounds,
    maximum_occurrences: usize,
) -> Result<String, StaticProjectedGenerationRefusal> {
    let unsupported = || Refusal::UnsupportedType("projected-image/fixed-projection@1".into());
    let identity = match program.input_type.shape() {
        StructuredInfoTypeShape::Record { schema, .. }
        | StructuredInfoTypeShape::Variant { schema, .. }
        | StructuredInfoTypeShape::Nominal { schema, .. } => schema,
        _ => return Err(unsupported().into()),
    };
    let owner = document
        .retained_native_owner(identity)
        .map_err(|_| Refusal::InvalidProgram)?;
    if owner.value_type != program.input_type {
        return Err(Refusal::InvalidProgram.into());
    }
    // Scalar outputs are intrinsic; laws on an arbitrary constructed intermediate
    // cannot be recovered from a portable program and must not be erased.
    if !matches!(
        program.output_type.shape(),
        StructuredInfoTypeShape::Leaf(_)
    ) {
        return Err(unsupported().into());
    }
    checked_scalar_nodes(&program.root)?;
    let native =
        generate_resident_native_validator(document, identity, bounds, maximum_occurrences)
            .map_err(StaticProjectedGenerationRefusal::Native)?;
    let prepared = PreparedPortableExpressionEvaluator::new(program)?;
    if prepared.input_value_contract.is_some() {
        return Err(unsupported().into());
    }
    let PreparedRoot::Primitive {
        node,
        nominal_type: None,
    } = &prepared.root
    else {
        return Err(unsupported().into());
    };
    let mut inputs = Vec::new();
    let tape = ScalarTape::prepare_with_inputs(node, &mut |pc, node| {
        let PreparedOperation::Projection(selected) = &node.operation else {
            return Err(unsupported());
        };
        let (start, length) = selected.fixed_scalar_input().ok_or_else(unsupported)?;
        inputs.push(StaticProjectedInput {
            instruction: pc,
            start: start as usize,
            length: length as usize,
        });
        Ok(Operation::Input)
    })?;
    let output = node.kind;
    let digest = sha2::Sha256::digest(
        program
            .canonical_bytes()
            .map_err(|_| Refusal::InvalidProgram)?,
    );
    let mut source = format!("extern crate alloc;\npub const SOURCE_PROGRAM_SHA256: [u8;32] = {digest:?};\nmod native {{\n{native}\n}}\n");
    let output_contract = static_image::generate::contracts::emit(
        &mut source,
        prepared.output_value_contract.as_ref(),
        "OUTPUT_CONTRACT",
        output,
    )?;
    static_image::generate::emit_tape(&mut source, &tape);
    source.push_str("static INPUTS: &[conduit_plot::StaticProjectedInput] = &[\n");
    for input in inputs {
        source.push_str(&format!(
            "conduit_plot::StaticProjectedInput {{ instruction: {}, start: {}, length: {} }},\n",
            input.instruction, input.start, input.length
        ));
    }
    let maximum = usize::try_from(program.maximum_prepared_input_bytes()?)
        .map_err(|_| Refusal::InvalidProgram)?;
    if maximum > bounds.canonical_bytes {
        return Err(Refusal::UnsupportedType("projected-image/input-bound@1".into()).into());
    }
    source.push_str(&format!("];\npub const RUNTIME_STATE_BYTES: usize = core::mem::size_of::<conduit_plot::StaticProjectedExpression<{{native::CONTRACT_COUNT}}>>();\npub fn prepare() -> Result<conduit_plot::StaticProjectedExpression<{{native::CONTRACT_COUNT}}>, conduit_plot::StaticProjectedRefusal> {{ Ok(conduit_plot::StaticProjectedExpression::new(native::prepare()?, {maximum}, conduit_plot::rust_binding::semantic_core::PrimitiveInfoKind::{output:?}, INSTRUCTIONS, {}, INPUTS, {output_contract})?) }}\n", tape.result));
    Ok(source)
}

// Native values are allowed only as direct projections from the checked input.
// A portable literal or constructed intermediate cannot invent a law witness.
fn checked_scalar_nodes(node: &PortableExpressionNode) -> Result<(), Refusal> {
    use PortableExpressionOperation as Op;
    if matches!(node.operation, Op::Projection { .. }) {
        return Ok(());
    }
    if !matches!(node.value_type.shape(), StructuredInfoTypeShape::Leaf(_)) {
        return Err(Refusal::UnsupportedType(
            "projected-image/intermediate-law-witness-unavailable@1".into(),
        ));
    }
    match &node.operation {
        Op::ValueContract { value, .. } => checked_scalar_nodes(value),
        Op::Unary { operand, .. } => checked_scalar_nodes(operand),
        Op::Binary { left, right, .. } => {
            checked_scalar_nodes(left)?;
            checked_scalar_nodes(right)
        }
        Op::Conditional {
            condition,
            when_true,
            when_false,
        } => {
            checked_scalar_nodes(condition)?;
            checked_scalar_nodes(when_true)?;
            checked_scalar_nodes(when_false)
        }
        Op::Literal(_) | Op::CanonicalLiteral(_) | Op::Constant(_) => Ok(()),
        _ => Err(Refusal::UnsupportedType(
            "projected-image/operation@1".into(),
        )),
    }
}
