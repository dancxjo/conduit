//! Authoring-time emission of the existing checked scalar execution profile.
use super::*;
use alloc::{collections::BTreeMap, format, string::String};
use sha2::Digest;
pub(crate) mod contracts;

/// Host-side specialization of the existing checked scalar profile into Rust
/// static data. Include this fragment inside a module in the target Seed.
/// The program digest is correlation, not an invitation or execution grant.
/// Unsupported structured/nominal/quantity profiles refuse without fallback.
pub fn generate_static_scalar_expression(
    program: &PortableExpressionProgram,
) -> Result<String, Refusal> {
    let prepared = PreparedPortableExpressionEvaluator::new_scalar_tape(program)?;
    let PreparedInput::Primitive { kind: input, .. } = prepared.input else {
        return Err(Refusal::UnsupportedType(
            "scalar-image/fixed-input@1".into(),
        ));
    };
    let PreparedRoot::ScalarTape {
        tape,
        nominal_type: None,
    } = &prepared.root
    else {
        return Err(Refusal::InvalidProgram);
    };
    if !supported(input) {
        return Err(Refusal::UnsupportedType(
            "scalar-image/fixed-input@1".into(),
        ));
    }
    let output = tape.instructions[tape.result].kind;
    if program
        .input_maximum_bytes
        .is_some_and(|maximum| width(input).is_none_or(|width| (maximum as usize) < width))
    {
        return Err(Refusal::UnsupportedType(
            "scalar-image/input-bound@1".into(),
        ));
    }
    validate(input, output, &tape.instructions, tape.result)?;
    let digest = sha2::Sha256::digest(
        program
            .canonical_bytes()
            .map_err(|_| Refusal::InvalidProgram)?,
    );
    let mut source =
        format!("extern crate alloc;\npub const SOURCE_PROGRAM_SHA256: [u8; 32] = {digest:?};\n");
    let input_contract = contracts::emit(
        &mut source,
        prepared.input_value_contract.as_ref(),
        "INPUT_CONTRACT",
        input,
    )?;
    let output_contract = if input == output
        && prepared.output_value_contract.is_some()
        && prepared.output_value_contract == prepared.input_value_contract
    {
        input_contract.clone()
    } else {
        contracts::emit(
            &mut source,
            prepared.output_value_contract.as_ref(),
            "OUTPUT_CONTRACT",
            output,
        )?
    };
    emit_tape(&mut source, tape);
    source.push_str(&format!("pub fn prepare() -> Result<conduit_plot::StaticScalarExpression, conduit_plot::PortableExpressionEvaluationRefusal> {{ conduit_plot::StaticScalarExpression::new(conduit_plot::rust_binding::semantic_core::PrimitiveInfoKind::{input:?}, conduit_plot::rust_binding::semantic_core::PrimitiveInfoKind::{output:?}, INSTRUCTIONS, {})?.with_value_contracts({input_contract}, {output_contract}) }}\n", tape.result));
    Ok(source)
}

pub(crate) fn emit_tape(source: &mut String, tape: &ScalarTape) {
    let mut literals = BTreeMap::new();
    for instruction in tape.instructions.iter() {
        if let Operation::Literal(bytes) = &instruction.operation {
            if !literals.contains_key(bytes.as_ref()) {
                let index = literals.len();
                source.push_str(&format!("const LITERAL_{index}: &[u8] = &{bytes:?};\n"));
                literals.insert(bytes.to_vec(), index);
            }
        }
    }
    source.push_str("pub static INSTRUCTIONS: &[conduit_plot::ScalarTapeInstruction] = &[\n");
    for instruction in tape.instructions.iter() {
        let operation = match &instruction.operation {
            Operation::Input => "Input".into(),
            Operation::Literal(bytes) => format!(
                "Literal(alloc::borrow::Cow::Borrowed(LITERAL_{}))",
                literals[bytes.as_ref()]
            ),
            Operation::Unary(op, from) => {
                format!("Unary(conduit_plot::UnaryOperator::{op:?}, {from})")
            }
            Operation::Binary(op, _, left, right) => {
                format!("Binary(conduit_plot::BinaryOperator::{op:?}, false, {left}, {right})")
            }
            Operation::BranchFalse(condition, target) => {
                format!("BranchFalse({condition}, {target})")
            }
            Operation::Widen(from) => format!("Widen({from})"),
            Operation::Jump(target) => format!("Jump({target})"),
            Operation::Move(from) => format!("Move({from})"),
        };
        source.push_str(&format!("conduit_plot::ScalarTapeInstruction {{ kind: conduit_plot::rust_binding::semantic_core::PrimitiveInfoKind::{:?}, destination: {}, operation: conduit_plot::ScalarTapeOperation::{operation} }},\n", instruction.kind, instruction.destination));
    }
    source.push_str("];\n");
}
