//! One constructor compiler; immutable output is assembled on the authoring Host.
use super::*;
use crate::expression_prepared::{PortableExpressionProgram, StaticProjectedGenerationRefusal};
use crate::rust_binding::{
    generate_native_with_work, PreparedNativeBounds, ResidentNativeGenerationRefusal,
};
use alloc::{format, string::String};
use sha2::Digest;
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StaticStructuredGenerationRefusal {
    Expression(Refusal),
    Native(ResidentNativeGenerationRefusal),
    Child(StaticProjectedGenerationRefusal),
    ConstructorLimit {
        resource: &'static str,
        required: usize,
        maximum: usize,
    },
}
impl From<Refusal> for StaticStructuredGenerationRefusal {
    fn from(value: Refusal) -> Self {
        Self::Expression(value)
    }
}
impl From<super::super::compile::CompileRefusal> for StaticStructuredGenerationRefusal {
    fn from(value: super::super::compile::CompileRefusal) -> Self {
        match value {
            super::super::compile::CompileRefusal::Expression(error) => Self::Expression(error),
            super::super::compile::CompileRefusal::Limit {
                resource,
                required,
                maximum,
            } => Self::ConstructorLimit {
                resource,
                required,
                maximum,
            },
        }
    }
}
pub fn generate_static_structured_expression(
    document: &crate::CheckedSyntaxDocument,
    program: &PortableExpressionProgram,
    bounds: PreparedNativeBounds,
    maximum_occurrences: usize,
) -> Result<String, StaticStructuredGenerationRefusal> {
    generate_static_structured_expression_with_work(document, program, bounds, maximum_occurrences)
        .map(|image| image.source)
}
/// Exact to the resident evaluator's conservative cooperative accounting. The
/// report describes code; it neither reserves resources nor initializes a Back.
pub struct GeneratedStaticStructuredExpression {
    pub source: String,
    pub computation_work: u32,
}
pub fn generate_static_structured_expression_with_work(
    document: &crate::CheckedSyntaxDocument,
    program: &PortableExpressionProgram,
    bounds: PreparedNativeBounds,
    maximum_occurrences: usize,
) -> Result<GeneratedStaticStructuredExpression, StaticStructuredGenerationRefusal> {
    use crate::expression_prepared::resident_work::{
        direct_path_work, native_validation_work, structured_work,
    };
    let maximum_input = usize::try_from(program.maximum_prepared_input_bytes()?)
        .map_err(|_| Refusal::InvalidProgram)?;
    let maximum_output = usize::try_from(program.maximum_prepared_output_bytes()?)
        .map_err(|_| Refusal::InvalidProgram)?;
    let mut source = String::new();
    let mut native_work = [0; 2];
    for (index, (name, ty)) in [
        ("input_native", &program.input_type),
        ("output_native", &program.output_type),
    ]
    .into_iter()
    .enumerate()
    {
        // Empty is the built-in unit boundary: it has no Native owner or laws.
        // Retain its canonical Type and ordinary borrowed validator, rather
        // than pretending an authored Native declaration exists for it.
        if matches!(ty.shape(), conduit_core::StructuredInfoTypeShape::Leaf(kind) if kind.as_str() == "value/empty")
        {
            source.push_str(&format!("mod {name} {{ pub const CONTRACT_COUNT:usize=0; pub fn prepare()->Result<conduit_plot::rust_binding::ResidentNativeValidator<0>,conduit_plot::rust_binding::PreparedNativeRefusal>{{ conduit_plot::rust_binding::ResidentNativeValidator::intrinsic_empty({maximum_occurrences}) }} }}\n"));
            native_work[index] =
                native_validation_work(0, 0, core::iter::empty(), core::iter::empty());
            continue;
        }
        let identity = match ty.shape() {
            conduit_core::StructuredInfoTypeShape::Record { schema, .. }
            | conduit_core::StructuredInfoTypeShape::Variant { schema, .. }
            | conduit_core::StructuredInfoTypeShape::Nominal { schema, .. } => schema,
            _ => {
                return Err(
                    Refusal::UnsupportedType("structured-image/native-boundary@1".into()).into(),
                )
            }
        };
        let owner = document
            .retained_native_owner(identity)
            .map_err(|_| Refusal::InvalidProgram)?;
        if &owner.value_type != ty {
            return Err(Refusal::InvalidProgram.into());
        }
        let (native, work) = generate_native_with_work(
            document,
            identity,
            bounds,
            maximum_occurrences,
            if index == 0 {
                maximum_input
            } else {
                maximum_output
            },
        )
        .map_err(StaticStructuredGenerationRefusal::Native)?;
        native_work[index] = work;
        source.push_str(&format!("mod {name} {{\n{native}\n}}\n"));
    }
    let prepared = crate::PreparedPortableExpressionEvaluator::new(program)?;
    if prepared.input_value_contract.is_some() || prepared.output_value_contract.is_some() {
        return Err(Refusal::UnsupportedType("structured-image/boundary-contract@1".into()).into());
    }
    let view = ProgramView {
        input_maximum_bytes: program.input_maximum_bytes,
        input_type: &program.input_type,
        output_type: &program.output_type,
        root: &program.root,
    };
    enum ChildImage {
        Scalar(crate::expression_prepared::scalar_tape::projected::generate::GeneratedProjected),
        InputPath(String, usize),
    }
    let mut children = Vec::new();
    let mut child_refusal = None;
    let tape = ConstructorTape::compile_reported(view, &prepared.input, &mut |node| {
        let child = PortableExpressionProgram {
            input_type: program.input_type.clone(),
            input_maximum_bytes: program.input_maximum_bytes,
            output_type: node.value_type.clone(),
            root: node.clone(),
        };
        if !matches!(
            node.value_type.shape(),
            conduit_core::StructuredInfoTypeShape::Leaf(_)
        ) {
            let selected = crate::PreparedPortableExpressionEvaluator::new(&child)?;
            if let Some(path) = selected.direct_input_path() {
                children.push(ChildImage::InputPath(
                    crate::expression_prepared::resident_projection::render(&path),
                    path.len(),
                ));
                return Ok(());
            }
        }
        match crate::expression_prepared::scalar_tape::projected::generate::generate_with_native(
            document,
            &child,
            bounds,
            maximum_occurrences,
            Some("super::input_native"),
        ) {
            Ok(image) => {
                children.push(ChildImage::Scalar(image));
                Ok(())
            }
            Err(error) => {
                child_refusal = Some(error);
                Err(Refusal::InvalidProgram)
            }
        }
    });
    if let Some(error) = child_refusal {
        return Err(StaticStructuredGenerationRefusal::Child(error));
    }
    let tape = tape?;
    if maximum_input > bounds.canonical_bytes || maximum_output > bounds.canonical_bytes {
        return Err(Refusal::UnsupportedType("structured-image/canonical-bound@1".into()).into());
    }
    let digest = sha2::Sha256::digest(
        program
            .canonical_bytes()
            .map_err(|_| Refusal::InvalidProgram)?,
    );
    source.push_str(&format!(
        "pub const SOURCE_PROGRAM_SHA256: [u8;32] = {digest:?};\n"
    ));
    for (i, image) in children.iter().enumerate() {
        if let ChildImage::Scalar(image) = image {
            source.push_str(&format!("mod child_{i} {{\n{}\n}}\n", image.source));
        }
    }
    source.push_str("static CODE: &[conduit_plot::StructuredTapeInstruction] = &[\n");
    for op in &tape.code {
        source.push_str(&format!(
            "conduit_plot::StructuredTapeInstruction::{op:?},\n"
        ));
    }
    let c = children.len();
    source.push_str(&format!("];\nstatic PROGRAM: conduit_plot::StaticStructuredProgram = conduit_plot::StaticStructuredProgram {{ instructions:CODE,constants:&{:?},maximum_input_bytes:{maximum_input},maximum_output_bytes:{maximum_output} }};\npub const CHILD_COUNT: usize = {c};\npub type Expression = conduit_plot::StaticStructuredExpression<{{input_native::CONTRACT_COUNT}},{{output_native::CONTRACT_COUNT}},{c}>;\npub const RUNTIME_STATE_BYTES: usize=core::mem::size_of::<Expression>();\nstatic CHILDREN:[conduit_plot::StaticStructuredChildProgram;CHILD_COUNT]=[",tape.constants));
    for (i, child) in children.iter().enumerate() {
        match child {
            ChildImage::Scalar(_) => source.push_str(&format!(
                "conduit_plot::StaticStructuredChildProgram::Scalar(&child_{i}::PROGRAM),"
            )),
            ChildImage::InputPath(path, _) => source.push_str(&format!(
                "conduit_plot::StaticStructuredChildProgram::InputPath(&[{path}]),"
            )),
        }
    }
    source.push_str("];\npub fn prepare()->Result<Expression,conduit_plot::StaticStructuredRefusal>{let input=input_native::prepare().map_err(conduit_plot::StaticStructuredRefusal::Input)?;let output=output_native::prepare().map_err(conduit_plot::StaticStructuredRefusal::Output)?;Expression::from_child_programs(input,output,&CHILDREN,&PROGRAM)}\n");
    let computation_work = structured_work(native_work[0], native_work[1], &tape.code, |index| {
        children
            .get(usize::from(index))
            .map_or(u32::MAX, |child| match child {
                ChildImage::Scalar(image) => image.checked_work,
                ChildImage::InputPath(_, depth) => direct_path_work(*depth),
            })
    });
    Ok(GeneratedStaticStructuredExpression {
        source,
        computation_work,
    })
}
