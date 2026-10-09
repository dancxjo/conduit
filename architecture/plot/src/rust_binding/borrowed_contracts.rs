//! Contract traversal over an immutable, fully validated canonical value.
use super::NativeBindingRefusal;
use crate::NativeTypeValueContract;
use conduit_core::{
    CheckedValueContract, ValidatedCanonicalStructuredValue, ValueConstraintRefusal,
};

/// Preserves the owned entrance's contract and representation-path order.
/// The caller must first compare the complete expected root Type bytes.
pub fn validate_borrowed_native_contracts(
    value: ValidatedCanonicalStructuredValue<'_>,
    contracts: &[NativeTypeValueContract],
) -> Result<(), NativeBindingRefusal> {
    validate_borrowed_native_contracts_with_record_access(value, contracts, None)
}

pub(super) fn validate_borrowed_native_contracts_with_record_access(
    value: ValidatedCanonicalStructuredValue<'_>,
    contracts: &[NativeTypeValueContract],
    access: Option<&conduit_core::PreparedCanonicalRecordAccess<'static>>,
) -> Result<(), NativeBindingRefusal> {
    for contract in contracts {
        validate_at_path(
            value,
            &contract.representation_path,
            &contract.contract,
            access,
        )
        .map_err(|refusal| NativeBindingRefusal::ViolatedConstraint {
            representation_path: contract.representation_path.clone(),
            refusal,
        })?;
    }
    Ok(())
}

fn validate_at_path(
    mut value: ValidatedCanonicalStructuredValue<'_>,
    path: &str,
    contract: &CheckedValueContract,
    access: Option<&conduit_core::PreparedCanonicalRecordAccess<'static>>,
) -> Result<(), ValueConstraintRefusal> {
    let wrong = || ValueConstraintRefusal::WrongConstraintKind;
    // Owned values expose the nominal representation shape directly.
    while let Ok(representation) = value.nominal_representation() {
        value = representation;
    }
    if path.is_empty() {
        let (_, bytes) = value.primitive().map_err(|_| wrong())?;
        return contract.validate(bytes);
    }
    if let Some(rest) = path.strip_prefix("[]") {
        for item in value.collection_elements().map_err(|_| wrong())? {
            let item = item.map_err(|_| wrong())?;
            validate_at_path(item, rest, contract, None)?;
        }
        return Ok(());
    }
    if let Some(rest) = path.strip_prefix("?some") {
        let tag = value.variant_tag().map_err(|_| wrong())?;
        return if tag == "some" {
            let payload = value
                .variant_payload("some")
                .map_err(|_| wrong())?
                .ok_or_else(wrong)?;
            validate_at_path(payload, rest, contract, None)
        } else {
            Ok(())
        };
    }
    if let Some(rest) = path.strip_prefix('.') {
        let (name, remaining) = split_component(rest);
        let field = match access {
            Some(access) if value.type_bytes().first() == Some(&2) => access.field(value, name),
            _ => value.record_field(name),
        }
        .map_err(|_| wrong())?
        .ok_or_else(wrong)?;
        return validate_at_path(field, remaining, contract, None);
    }
    if let Some(rest) = path.strip_prefix('|') {
        let (wanted, remaining) = split_component(rest);
        let tag = value.variant_tag().map_err(|_| wrong())?;
        return if tag == wanted {
            let payload = value
                .variant_payload(wanted)
                .map_err(|_| wrong())?
                .ok_or_else(wrong)?;
            validate_at_path(payload, remaining, contract, None)
        } else {
            Ok(())
        };
    }
    Err(wrong())
}

fn split_component(path: &str) -> (&str, &str) {
    let end = path
        .char_indices()
        .find_map(|(index, character)| {
            (index > 0 && matches!(character, '.' | '|' | '[' | '?')).then_some(index)
        })
        .unwrap_or(path.len());
    path.split_at(end)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::{string::ToString, vec};
    use conduit_core::{
        kind_id, validate_canonical_structured_value, StructuredFieldValue, StructuredInfoValue,
        ValueConstraint,
    };

    #[test]
    fn all_path_shapes_and_first_refusals_match_owned_reference() {
        let checked = crate::check_syntax_document(
            &crate::parse_syntax_document("type Byte = U8\ntype Envelope = {\n    values: sequence Byte <= 3\n}\ntype Maybe = Envelope?\ntype Wrapper = {\n    choice: Maybe\n}"),
            &crate::StartupCatalog::new(),
        ).unwrap();
        let ty = |name: &str| {
            checked
                .native_types
                .iter()
                .find(|ty| ty.name == name)
                .unwrap()
                .value_type
                .clone()
        };
        let byte = |number| {
            StructuredInfoValue::nominal(
                ty("Byte"),
                StructuredInfoValue::leaf(
                    conduit_core::StructuredInfoType::leaf(kind_id("value/u8")).unwrap(),
                    vec![number],
                )
                .unwrap(),
            )
            .unwrap()
        };
        let envelope_type = ty("Envelope");
        let sequence_type = super::super::record_field_type(&envelope_type, "values").unwrap();
        let envelope = StructuredInfoValue::record(
            envelope_type,
            vec![StructuredFieldValue::new(
                "values",
                StructuredInfoValue::sequence(sequence_type, vec![byte(3), byte(7)]).unwrap(),
            )
            .unwrap()],
        )
        .unwrap();
        let maybe_type = ty("Maybe");
        let representation = super::super::nominal_representation_type(&maybe_type).unwrap();
        let maybe = StructuredInfoValue::nominal(
            maybe_type,
            StructuredInfoValue::variant(representation, "some", envelope).unwrap(),
        )
        .unwrap();
        let value = StructuredInfoValue::record(
            ty("Wrapper"),
            vec![StructuredFieldValue::new("choice", maybe).unwrap()],
        )
        .unwrap();
        let encoded = value.canonical_bytes().unwrap();
        let borrowed = validate_canonical_structured_value(&encoded).unwrap();
        let recipe_type: &'static [u8] =
            alloc::boxed::Box::leak(ty("Wrapper").canonical_bytes().unwrap().into_boxed_slice());
        let bound =
            conduit_core::PreparedCanonicalRecordAccess::storage_bound(recipe_type).unwrap();
        let recipe =
            conduit_core::PreparedCanonicalRecordAccess::prepare(recipe_type, bound).unwrap();
        for path in [
            ".choice?some.values[]",
            ".choice|some.values[]",
            ".choice|none.bad",
            ".absent",
            ".choice?some.values",
            ".choice[]",
            "bad",
        ] {
            for members in [vec![vec![3], vec![7]], vec![vec![3]]] {
                let contracts = vec![NativeTypeValueContract {
                    representation_path: path.to_string(),
                    contract: CheckedValueContract::new(
                        kind_id("value/u8"),
                        1,
                        vec![ValueConstraint::CanonicalMembership {
                            members,
                            negated: false,
                        }],
                    )
                    .unwrap(),
                }];
                assert_eq!(
                    validate_borrowed_native_contracts_with_record_access(
                        borrowed,
                        &contracts,
                        Some(&recipe)
                    ),
                    validate_borrowed_native_contracts(borrowed, &contracts),
                    "prepared {path}"
                );
                assert_eq!(
                    validate_borrowed_native_contracts(borrowed, &contracts),
                    super::super::validate_native_contracts(&value, &contracts),
                    "{path}"
                );
            }
        }
    }
}
