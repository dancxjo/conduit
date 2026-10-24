//! Bound untrusted decoded graphs before parsing, checking or cloning a child.
use super::*;
use crate::{PortableExpressionNode as Node, PortableExpressionOperation as Op};

pub(super) fn check(material: &NativeLawMaterial) -> Result<(), NativeLawCustodyRefusal> {
    visit(material, 0, &mut Budget::default())
}
fn visit(
    material: &NativeLawMaterial,
    depth: usize,
    budget: &mut Budget,
) -> Result<(), NativeLawCustodyRefusal> {
    budget.admit(&material.source, depth)?;
    if material.imports.len() > MAXIMUM_MODULES || material.declaration.len() > 4096 {
        return Err(NativeLawCustodyRefusal::Capacity);
    }
    budget.charge(material.declaration.len())?;
    budget.claims(&material.claimed)?;
    let mut previous: Option<&str> = None;
    for (name, dependency) in &material.imports {
        if name.len() > 4096 {
            return Err(NativeLawCustodyRefusal::Capacity);
        }
        if previous.is_some_and(|p| p >= name.as_str()) {
            return Err(NativeLawCustodyRefusal::DuplicateImport);
        }
        previous = Some(name);
        budget.charge(name.len())?;
        visit(dependency, depth + 1, budget)?;
    }
    Ok(())
}

// Public decoded program nodes can be assembled without going through the
// canonical decoder. Bound their tree before its recursive size encoder runs.
pub(super) fn program(
    value: &crate::PortableExpressionProgram,
    budget: &mut Budget,
) -> Result<(), NativeLawCustodyRefusal> {
    let mut nodes = 0;
    node(&value.root, 0, &mut nodes, budget)
}
fn node(
    value: &Node,
    depth: usize,
    nodes: &mut usize,
    budget: &mut Budget,
) -> Result<(), NativeLawCustodyRefusal> {
    *nodes = nodes
        .checked_add(1)
        .ok_or(NativeLawCustodyRefusal::Capacity)?;
    if depth > conduit_core::MAXIMUM_STRUCTURED_INFO_DEPTH
        || *nodes > conduit_core::MAXIMUM_STRUCTURED_INFO_NODES
    {
        return Err(NativeLawCustodyRefusal::Capacity);
    }
    // Core's opaque Type constructors already enforce its 262144-byte ceiling.
    // No public streaming Type-size API exists; this bounded scratch allocation
    // is unavoidable here. It never creates a complete program or law vector.
    budget.charge(
        value
            .value_type
            .canonical_bytes()
            .map_err(|_| NativeLawCustodyRefusal::Type)?
            .len(),
    )?;
    match &value.operation {
        Op::Variant { tag, .. } => budget.charge(tag.len())?,
        Op::Projection {
            member: crate::PortableExpressionProjection::Field(name),
            ..
        } => budget.charge(name.len())?,
        Op::SemanticCall { kind, .. } => budget.charge(kind.len())?,
        Op::Record(fields) => {
            if fields.len() > conduit_core::MAXIMUM_STRUCTURED_INFO_NODES {
                return Err(NativeLawCustodyRefusal::Capacity);
            }
            for (name, _) in fields {
                budget.charge(name.len())?;
            }
        }
        _ => (),
    }
    let mut child = |value: &Node| node(value, depth + 1, nodes, budget);
    match &value.operation {
        Op::Input => Ok(()),
        Op::Literal(text) => budget.charge(text.len()),
        Op::CanonicalLiteral(bytes) => budget.charge(bytes.len()),
        Op::Constant(value) => budget.charge(
            value
                .canonical_bytes()
                .map_err(|_| NativeLawCustodyRefusal::Laws)?
                .len(),
        ),
        Op::Unary { operand, .. }
        | Op::Variant {
            payload: operand, ..
        } => child(operand),
        Op::Projection { value, .. } => child(value),
        Op::Binary { left, right, .. } => {
            child(left)?;
            child(right)
        }
        Op::Conditional {
            condition,
            when_true,
            when_false,
        } => {
            child(condition)?;
            child(when_true)?;
            child(when_false)
        }
        Op::Tuple(values)
        | Op::Collection(values)
        | Op::SemanticCall {
            arguments: values, ..
        } => {
            if values.len() > conduit_core::MAXIMUM_STRUCTURED_INFO_NODES {
                return Err(NativeLawCustodyRefusal::Capacity);
            }
            for value in values {
                child(value)?;
            }
            Ok(())
        }
        Op::Record(fields) => {
            if fields.len() > conduit_core::MAXIMUM_STRUCTURED_INFO_NODES {
                return Err(NativeLawCustodyRefusal::Capacity);
            }
            for (_, value) in fields {
                child(value)?;
            }
            Ok(())
        }
        Op::ValueContract {
            input,
            output,
            value,
        } => {
            // Finish the child borrow before separately charging contracts.
            child(value)?;
            for contract in [input, output].into_iter().flatten() {
                budget.contract(contract)?;
            }
            Ok(())
        }
    }
}
