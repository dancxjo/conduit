//! Retain checked selectors and finite observations on the authoring Host.
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
/// Emit fixed-scalar results over a retained checked Native input, using fixed
/// offsets or typed paths and finite sequence/variant observations. Unsupported
/// laws and non-input selections refuse; scalar execution and predicates stay
/// shared with the existing compiler and loop. No device semantics occur here.
pub fn generate_static_projected_expression(
    document: &crate::CheckedSyntaxDocument,
    program: &PortableExpressionProgram,
    bounds: PreparedNativeBounds,
    maximum_occurrences: usize,
) -> Result<String, StaticProjectedGenerationRefusal> {
    generate_with_native(document, program, bounds, maximum_occurrences, None)
        .map(|image| image.source)
}
pub(in crate::expression_prepared) struct GeneratedProjected {
    pub source: String,
    pub checked_work: u32,
    pub has_output_contract: bool,
}
pub(in crate::expression_prepared) fn generate_with_native(
    document: &crate::CheckedSyntaxDocument,
    program: &PortableExpressionProgram,
    bounds: PreparedNativeBounds,
    maximum_occurrences: usize,
    shared_native: Option<&str>,
) -> Result<GeneratedProjected, StaticProjectedGenerationRefusal> {
    use crate::expression_prepared::resident_work::{projected_work, ProjectedInputWork as W};
    let unsupported = || Refusal::UnsupportedType("projected-image/input-observation@1".into());
    let native = if matches!(program.input_type.shape(), StructuredInfoTypeShape::Leaf(kind) if kind.as_str() == "value/empty")
    {
        format!("pub const CONTRACT_COUNT:usize=0; pub fn prepare()->Result<conduit_plot::rust_binding::ResidentNativeValidator<0>,conduit_plot::rust_binding::PreparedNativeRefusal>{{conduit_plot::rust_binding::ResidentNativeValidator::intrinsic_empty({maximum_occurrences})}}")
    } else {
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
        generate_resident_native_validator(document, identity, bounds, maximum_occurrences)
            .map_err(StaticProjectedGenerationRefusal::Native)?
    };
    // Scalar outputs are intrinsic; laws on an arbitrary constructed intermediate
    // cannot be recovered from a portable program and must not be erased.
    if !matches!(
        program.output_type.shape(),
        StructuredInfoTypeShape::Leaf(_)
    ) {
        return Err(unsupported().into());
    }
    checked_scalar_nodes(&program.root)?;
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
        use crate::expression_prepared::resident_projection::render;
        let prefix = "conduit_plot::StaticProjectedSource";
        let (source, work) = match &node.operation {
            PreparedOperation::Projection(selected) => {
                if let Some((start, length)) = selected.fixed_scalar_input() {
                    (
                        format!("{prefix}::Fixed {{start:{start},length:{length}}}"),
                        W::Fixed,
                    )
                } else {
                    let path = selected.direct_input_path().ok_or_else(unsupported)?;
                    (
                        format!("{prefix}::Path(&[{}])", render(&path)),
                        W::Path(path.len()),
                    )
                }
            }
            PreparedOperation::Inspection(value) => {
                use crate::expression_prepared::inspection::ResidentObservation as O;
                let (path, observation) = value.resident_source().ok_or_else(unsupported)?;
                match observation {
                    O::VariantIs(tag) if node.kind == PrimitiveInfoKind::Bool => (
                        format!(
                            "{prefix}::VariantIs {{path:&[{}],tag:{tag:?}}}",
                            render(&path)
                        ),
                        W::VariantIs(path.len()),
                    ),
                    O::Length if node.kind == PrimitiveInfoKind::U64 => (
                        format!("{prefix}::Length(&[{}])", render(&path)),
                        W::Length(path.len()),
                    ),
                    _ => return Err(unsupported()),
                }
            }
            _ => return Err(unsupported()),
        };
        inputs.push((pc, source, work));
        Ok(Operation::Input)
    })?;
    let output = node.kind;
    let native_module = shared_native.map_or_else(
        || format!("mod native {{\n{native}\n}}\n"),
        |_| String::new(),
    );
    let mut source = format!("extern crate alloc;\n{native_module}");
    // Nested children belong to the enclosing program's exact canonical hash.
    // Do not repeat unused standalone receipts for every operand in that image.
    if shared_native.is_none() {
        let digest = sha2::Sha256::digest(
            program
                .canonical_bytes()
                .map_err(|_| Refusal::InvalidProgram)?,
        );
        source.push_str(&format!(
            "pub const SOURCE_PROGRAM_SHA256: [u8;32] = {digest:?};\n"
        ));
    }
    let (output_contract, output_work) = static_image::generate::contracts::emit_with_work(
        &mut source,
        prepared.output_value_contract.as_ref(),
        "OUTPUT_CONTRACT",
        output,
    )?;
    static_image::generate::emit_tape(&mut source, &tape);
    source.push_str("static INPUTS: &[conduit_plot::StaticProjectedInput] = &[\n");
    let work = projected_work(
        tape.instruction_count(),
        inputs.iter().map(|(_, _, work)| *work),
        output_work,
    );
    for (instruction, source_value, _) in inputs {
        source.push_str(&format!("conduit_plot::StaticProjectedInput {{instruction:{instruction},source:{source_value}}},\n"));
    }
    let maximum = usize::try_from(program.maximum_prepared_input_bytes()?)
        .map_err(|_| Refusal::InvalidProgram)?;
    if maximum > bounds.canonical_bytes {
        return Err(Refusal::UnsupportedType("projected-image/input-bound@1".into()).into());
    }
    let visibility = if shared_native.is_some() {
        "pub(super) "
    } else {
        ""
    };
    source.push_str(&format!("];\n{visibility}static PROGRAM:conduit_plot::StaticProjectedProgram=conduit_plot::StaticProjectedProgram{{maximum_input_bytes:{maximum},output:conduit_plot::rust_binding::semantic_core::PrimitiveInfoKind::{output:?},instructions:INSTRUCTIONS,result:{},inputs:INPUTS,output_contract:{output_contract}}};\n", tape.result));
    if shared_native.is_none() {
        source.push_str("pub const RUNTIME_STATE_BYTES:usize=core::mem::size_of::<conduit_plot::StaticProjectedExpression<{native::CONTRACT_COUNT}>>();\n");
        source.push_str("pub fn prepare() -> Result<conduit_plot::StaticProjectedExpression<{native::CONTRACT_COUNT}>, conduit_plot::StaticProjectedRefusal> { Ok(conduit_plot::StaticProjectedExpression::new(native::prepare()?, &PROGRAM)?) }\n");
    }
    Ok(GeneratedProjected {
        source,
        checked_work: work,
        has_output_contract: prepared.output_value_contract.is_some(),
    })
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
        Op::SemanticCall { kind, arguments }
            if crate::expression_semantic_call::integer_widening_target(kind).is_some() =>
        {
            let [argument] = arguments.as_slice() else {
                return Err(Refusal::InvalidProgram);
            };
            checked_scalar_nodes(argument)
        }
        Op::SemanticCall { kind, .. }
            if matches!(kind.as_str(), "variant/is" | "sequence/length") =>
        {
            Ok(())
        }
        _ => Err(Refusal::UnsupportedType(
            "projected-image/operation@1".into(),
        )),
    }
}
