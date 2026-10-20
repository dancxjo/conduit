//! Guarded payload staging. Only the actual admitted alternative is read.
use super::*;

pub(super) fn expression(
    ty: &StructuredInfoType,
    path: String,
    types: &[CheckedNativeType],
    names: &BTreeMap<String, String>,
    layout: &mut Layout,
    depth: usize,
    guard: &str,
) -> R<String> {
    let StructuredInfoTypeShape::Variant { schema, cases } = ty.shape() else {
        return Err(PreparedStorageGenerationError::Unsupported(
            "not variant".into(),
        ));
    };
    let optional = schema.as_str() == "conduit.conduitese.optional.v1";
    let native = if optional {
        let some = cases
            .iter()
            .find(|c| c.tag() == "some")
            .ok_or_else(|| PreparedStorageGenerationError::Unsupported("optional schema".into()))?;
        if conduit_core::optional_info_type(some.payload_type().clone())
            .ok()
            .as_ref()
            != Some(ty)
        {
            return Err(PreparedStorageGenerationError::Unsupported(
                "optional schema".into(),
            ));
        }
        None
    } else {
        Some(dependency(schema.as_str(), ty, types, layout)?)
    };
    if cases.is_empty() || cases.len() > 256 {
        return Err(PreparedStorageGenerationError::Bound);
    }
    layout.branched = true;
    let index = layout.slots.len();
    layout.slots.push(Slot {
        path: path.clone(),
        kind: SlotKind::Tag(cases.iter().map(|c| c.tag().into()).collect()),
        rust: "u16".into(),
        guard: guard.into(),
    });
    let mut alternatives = Vec::new();
    for (ordinal, case) in cases.iter().enumerate() {
        if native.is_some_and(|n| layout.boxed.contains(&format!("{}.{}", n.name, case.tag()))) {
            return Err(PreparedStorageGenerationError::Unsupported(
                "boxed payload".into(),
            ));
        }
        let selected = format!("self.f{index} == Some({ordinal})");
        let guard = if guard.is_empty() {
            selected
        } else {
            format!("({guard}) && ({selected})")
        };
        let payload_path = format!("{path}.representation().map_err(|_| E::Canonical)?.variant_payload({:?}).map_err(|_| E::Canonical)?.ok_or(E::Canonical)?",case.tag());
        let variant = if optional {
            if case.tag() == "some" {
                "Some".into()
            } else {
                "None".into()
            }
        } else {
            format!(
                "{}::{}",
                names[schema.as_str()],
                rust_pascal_identifier(case.tag())?
            )
        };
        let value = if unit_type(case.payload_type()) {
            if optional && case.tag() == "some" {
                "Some(())".into()
            } else {
                variant
            }
        } else {
            let payload = if let StructuredInfoTypeShape::Record {
                schema: child,
                fields,
            } = case.payload_type().shape()
            {
                // Ordinary bindings name record payload structs after the enum case.
                // Only a complete checked Native record dependency is supported.
                dependency(child.as_str(), case.payload_type(), types, layout)?;
                let fields=fields.iter().map(|f|Ok(format!("{}: {}",rust_snake_identifier(f.name())?,super::expression(f.value_type(),format!("{payload_path}.representation().map_err(|_| E::Canonical)?.record_field({:?}).map_err(|_| E::Canonical)?.ok_or(E::Canonical)?",f.name()),None,types,names,layout,depth+1,&guard)?))).collect::<R<Vec<_>>>()?;
                let constructor = if optional {
                    names[child.as_str()].clone()
                } else {
                    format!(
                        "{}{}",
                        names[schema.as_str()],
                        rust_pascal_identifier(case.tag())?
                    )
                };
                format!("{constructor} {{ {} }}", fields.join(","))
            } else {
                super::expression(
                    case.payload_type(),
                    payload_path,
                    None,
                    types,
                    names,
                    layout,
                    depth + 1,
                    &guard,
                )?
            };
            format!("{variant}({payload})")
        };
        alternatives.push(format!("{ordinal} => {value}"));
    }
    Ok(format!(
        "match self.f{index}.ok_or(E::State)? {{ {}, _=>return Err(E::Canonical) }}",
        alternatives.join(",")
    ))
}
