//! Preallocate all alternatives; step and assembly visit only the selected one.
use super::*;
pub(super) fn build(
    layout: &mut Layout<'_>,
    ty: &StructuredInfoType,
    path: &mut Vec<NativePathStep>,
    node: &mut Node,
) -> R<()> {
    let Shape::Variant { schema, cases } = ty.shape() else {
        return Err(Error::Bound);
    };
    let optional = schema.as_str() == "conduit.conduitese.optional.v1";
    if !optional {
        layout.dependency(ty, schema.as_str())?;
    }
    node.fields = "tag:Option<usize>,".into();
    node.init = "tag:None,".into();
    node.step =
        "if self.tag.is_none(){self.tag=Some(match value.variant_tag().map_err(|_|E::Canonical)? {"
            .into();
    for (i, case) in cases.iter().enumerate() {
        write!(node.step, "{:?}=>{i},", case.tag()).unwrap();
    }
    node.step
        .push_str("_=>return Err(E::Canonical)});return Ok(match self.tag {");
    for (i, case) in cases.iter().enumerate() {
        write!(node.step, "Some({i})=>{},", unit_type(case.payload_type())).unwrap();
    }
    node.step
        .push_str("_=>return Err(E::State)})} match self.tag.ok_or(E::State)? {");
    node.take = "match self.tag.expect(\"completed private variant\"){".into();
    // Storage charges every alternative, whereas the work ceiling takes only
    // the largest possible selected alternative plus the actual tag step.
    node.cost = "let mut branch_steps=0usize;".into();
    for (i, case) in cases.iter().enumerate() {
        let variant = if optional {
            if case.tag() == "some" {
                "Some".into()
            } else {
                "None".into()
            }
        } else {
            format!("{}::{}", node.rust, rust_pascal_identifier(case.tag())?)
        };
        let value = if unit_type(case.payload_type()) {
            write!(node.step, "{i}=>Ok(true),").unwrap();
            if optional && case.tag() == "some" {
                "Some(())".into()
            } else {
                variant
            }
        } else {
            if !optional {
                let n = layout
                    .types
                    .iter()
                    .find(|n| n.identity == *schema)
                    .ok_or(Error::Bound)?;
                if layout.options.boxed_variant_payloads.contains(&format!(
                    "{}.{}",
                    n.name,
                    case.tag()
                )) {
                    return Err(Error::Unsupported(format!(
                        "boxed payload at {path:?}/{:?}",
                        NativePathStep::Variant(case.tag().into())
                    )));
                }
            }
            let constructor =
                if !optional && matches!(case.payload_type().shape(), Shape::Record { .. }) {
                    Some(format!(
                        "{}{}",
                        node.rust,
                        rust_pascal_identifier(case.tag())?
                    ))
                } else {
                    None
                };
            path.push(NativePathStep::Variant(case.tag().into()));
            let child = layout.walk(case.payload_type(), path, constructor)?;
            path.pop();
            write!(node.fields, "f{i}:{child},").unwrap();
            write!(node.init, "f{i}:{child}::new(caps)?,").unwrap();
            write!(node.step,"{i}=>self.f{i}.step(value.variant_payload({:?}).map_err(|_|E::Canonical)?.ok_or(E::Canonical)?),",case.tag()).unwrap();
            write!(node.cost,"let child={child}::cost(caps)?;branch_steps=branch_steps.max(child.maximum_steps);cost=cost.checked_add(child)?;").unwrap();
            format!("{variant}(self.f{i}.take())")
        };
        write!(node.take, "{i}=>{value},").unwrap();
    }
    node.step.push_str("_=>Err(E::State)}");
    node.take
        .push_str("_=>unreachable!(\"private admitted tag\")}");
    node.cost
        .push_str("cost.maximum_steps=branch_steps.checked_add(1)?;");
    Ok(())
}
