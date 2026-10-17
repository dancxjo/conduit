//! Admit retained Native laws for every selected context value and child.
use crate::prelude::*;
use crate::StartupCatalog;
use conduit_core::{
    StructuredInfoTypeShape as T, StructuredInfoValue, StructuredInfoValueShape as V,
};

pub(super) fn validate(
    value: &StructuredInfoValue,
    catalog: &StartupCatalog,
) -> Result<(), String> {
    let mut remaining = conduit_core::MAXIMUM_STRUCTURED_INFO_NODES;
    visit(value, catalog, 1, &mut remaining)
}
fn visit(
    value: &StructuredInfoValue,
    catalog: &StartupCatalog,
    depth: usize,
    remaining: &mut usize,
) -> Result<(), String> {
    if depth > conduit_core::MAXIMUM_STRUCTURED_INFO_DEPTH || *remaining == 0 {
        return Err("Source context Native admission exceeds its finite value bound".into());
    }
    *remaining -= 1;
    if let Some((contracts, invariants)) = catalog.native_laws_for(value.value_type()) {
        crate::rust_binding::validate_native_contracts(value, contracts)
            .and_then(|()| crate::rust_binding::validate_native_invariants(value, invariants))
            .map_err(|error| {
                format!("Source context violates its retained Native laws: {error:?}")
            })?;
    } else if matches!(value.value_type().shape(), T::Nominal { schema, .. } | T::Record { schema, .. } | T::Variant { schema, .. } if schema.as_str().starts_with("type/"))
    {
        return Err("Source context Native Type lacks retained admission laws".into());
    }
    match value.shape() {
        V::Leaf(_) => {}
        V::Collection(items) => {
            for item in items {
                visit(item, catalog, depth + 1, remaining)?;
            }
        }
        V::Record(fields) => {
            for field in fields {
                visit(field.value(), catalog, depth + 1, remaining)?;
            }
        }
        V::Variant { payload, .. } => visit(payload, catalog, depth + 1, remaining)?,
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::*;
    use conduit_core::{StructuredFieldValue, StructuredInfoType, StructuredInfoTypeShape};
    #[test]
    fn nested_native_invariants_and_missing_laws_refuse_before_projection() {
        let source = "type Interval = {\n start: U32\n end: U32\n where .start <= .end\n}\n";
        let checked =
            check_syntax_document(&parse_syntax_document(source), &StartupCatalog::new()).unwrap();
        let interval = &checked.native_types[0];
        let mut catalog = StartupCatalog::new();
        catalog
            .insert_checked_native_type("Interval", interval)
            .unwrap();
        let make = |start: u32, end: u32| {
            let StructuredInfoTypeShape::Record { fields, .. } = interval.value_type.shape() else {
                panic!()
            };
            let values = fields
                .iter()
                .map(|field| {
                    let number = if field.name() == "start" { start } else { end };
                    StructuredFieldValue::new(
                        field.name(),
                        crate::rust_binding::primitive_into_structured(
                            field.value_type().clone(),
                            &number,
                        )
                        .unwrap(),
                    )
                    .unwrap()
                })
                .collect();
            let record = StructuredInfoValue::record(interval.value_type.clone(), values).unwrap();
            StructuredInfoValue::collection(
                StructuredInfoType::collection(interval.value_type.clone(), Some(1)).unwrap(),
                vec![record],
            )
            .unwrap()
        };
        validate(&make(4, 4), &catalog).unwrap();
        assert!(validate(&make(5, 4), &catalog)
            .unwrap_err()
            .contains("retained Native laws"));
        assert!(validate(&make(4, 4), &StartupCatalog::new())
            .unwrap_err()
            .contains("lacks retained admission laws"));
    }
}
