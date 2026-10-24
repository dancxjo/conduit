//! Request selection only: metadata never issues compiler law custody.
use alloc::{collections::BTreeSet, format, string::String, vec::Vec};
use conduit_core::{StructuredInfoType, StructuredInfoTypeShape as Shape};
use conduit_plot::{PortableExpressionNode, PortableExpressionOperation as Op};

/// Select Audio requests by exact checked Types reachable from the requested
/// Speech owners and their checked invariant nodes. Actual complete Source replay
/// still validates every dependency; a missed Source-only dependency refuses.
pub(super) fn audio_roots_for(names: &[&str]) -> Result<Vec<String>, String> {
    let speech = super::checked_types()?;
    let audio = conduit_audio::authoring::checked_types()
        .map_err(|why| format!("Audio owner metadata: {why:?}"))?;
    let mut pending = BTreeSet::new();
    for name in names {
        let owner = speech
            .iter()
            .find(|ty| ty.name == *name)
            .ok_or_else(|| format!("Speech requested root '{name}' is not owned"))?;
        pending.insert(owner.name.as_str());
    }
    let mut visited = BTreeSet::new();
    let mut types = BTreeSet::new();
    let mut selected = BTreeSet::new();
    while let Some(name) = pending.pop_first() {
        if !visited.insert(name) {
            continue;
        }
        if visited.len() > 4096 {
            return Err("Speech owner dependency capacity".into());
        }
        let owner = speech
            .iter()
            .find(|ty| ty.name == name)
            .expect("pending owned Speech Type");
        collect_type(&owner.value_type, &mut types)?;
        for invariant in &owner.invariants {
            collect_type(&invariant.input_type, &mut types)?;
            collect_type(&invariant.output_type, &mut types)?;
            collect_node(&invariant.root, &mut types)?;
        }
        for ty in &types {
            if let Some(dependency) = speech.iter().find(|owner| owner.value_type == *ty) {
                if !visited.contains(dependency.name.as_str()) {
                    pending.insert(dependency.name.as_str());
                }
            }
            if let Some(dependency) = audio.iter().find(|owner| owner.value_type == *ty) {
                selected.insert(dependency.name.clone());
            }
        }
    }
    Ok(selected.into_iter().collect())
}
fn collect_type(
    ty: &StructuredInfoType,
    types: &mut BTreeSet<StructuredInfoType>,
) -> Result<(), String> {
    if types.contains(ty) {
        return Ok(());
    }
    if types.len() >= conduit_core::MAXIMUM_STRUCTURED_INFO_NODES {
        return Err("Speech dependency Type-node capacity".into());
    }
    types.insert(ty.clone());
    match ty.shape() {
        Shape::Leaf(_) => (),
        Shape::Nominal { representation, .. } => collect_type(representation, types)?,
        Shape::Collection { element, .. } | Shape::Sequence { element, .. } => {
            collect_type(element, types)?
        }
        Shape::Record { fields, .. } => {
            for field in fields {
                collect_type(field.value_type(), types)?;
            }
        }
        Shape::Variant { cases, .. } => {
            for case in cases {
                collect_type(case.payload_type(), types)?;
            }
        }
    }
    Ok(())
}
fn collect_node(
    node: &PortableExpressionNode,
    types: &mut BTreeSet<StructuredInfoType>,
) -> Result<(), String> {
    collect_type(&node.value_type, types)?;
    match &node.operation {
        Op::ValueContract { value, .. }
        | Op::Projection { value, .. }
        | Op::Variant { payload: value, .. } => collect_node(value, types)?,
        Op::Unary { operand, .. } => collect_node(operand, types)?,
        Op::Binary { left, right, .. } => {
            collect_node(left, types)?;
            collect_node(right, types)?;
        }
        Op::Conditional {
            condition,
            when_true,
            when_false,
        } => {
            collect_node(condition, types)?;
            collect_node(when_true, types)?;
            collect_node(when_false, types)?;
        }
        Op::Tuple(values)
        | Op::Collection(values)
        | Op::SemanticCall {
            arguments: values, ..
        } => {
            for value in values {
                collect_node(value, types)?;
            }
        }
        Op::Record(fields) => {
            for (_, value) in fields {
                collect_node(value, types)?;
            }
        }
        Op::Input | Op::Literal(_) | Op::CanonicalLiteral(_) | Op::Constant(_) => (),
    }
    Ok(())
}
