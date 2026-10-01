use crate::prelude::*;
use conduit_core::{
    CheckedValueContract, PrimitiveInfoRefusal, StructuredInfoRefusal, StructuredInfoType,
    StructuredInfoTypeShape, StructuredInfoValue, StructuredInfoValueShape, ValueConstraintRefusal,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NativeBindingRefusal {
    InvalidSemanticType(StructuredInfoRefusal),
    InvalidValue(StructuredInfoRefusal),
    InvalidPrimitive(PrimitiveInfoRefusal),
    ViolatedConstraint {
        representation_path: String,
        refusal: ValueConstraintRefusal,
    },
}

pub fn nominal_representation_type(
    value_type: &StructuredInfoType,
) -> Result<StructuredInfoType, NativeBindingRefusal> {
    let StructuredInfoTypeShape::Nominal { representation, .. } = value_type.shape() else {
        return Err(wrong_type());
    };
    Ok(representation.clone())
}

pub fn record_field_type(
    value_type: &StructuredInfoType,
    name: &str,
) -> Result<StructuredInfoType, NativeBindingRefusal> {
    let StructuredInfoTypeShape::Record { fields, .. } = value_type.shape() else {
        return Err(wrong_type());
    };
    fields
        .iter()
        .find(|field| field.name() == name)
        .map(|field| field.value_type().clone())
        .ok_or_else(wrong_type)
}

pub fn record_field_value(
    value: &StructuredInfoValue,
    name: &str,
) -> Result<StructuredInfoValue, NativeBindingRefusal> {
    let StructuredInfoValueShape::Record(fields) = value.shape() else {
        return Err(wrong_type());
    };
    fields
        .iter()
        .find(|field| field.name() == name)
        .map(|field| field.value().clone())
        .ok_or_else(wrong_type)
}

pub fn sequence_element_type(
    value_type: &StructuredInfoType,
) -> Result<StructuredInfoType, NativeBindingRefusal> {
    let StructuredInfoTypeShape::Sequence { element, .. } = value_type.shape() else {
        return Err(wrong_type());
    };
    Ok(element.clone())
}

pub fn collection_element_type(
    value_type: &StructuredInfoType,
) -> Result<StructuredInfoType, NativeBindingRefusal> {
    let StructuredInfoTypeShape::Collection { element, .. } = value_type.shape() else {
        return Err(wrong_type());
    };
    Ok(element.clone())
}

pub fn variant_payload_type(
    value_type: &StructuredInfoType,
    tag: &str,
) -> Result<StructuredInfoType, NativeBindingRefusal> {
    let StructuredInfoTypeShape::Variant { cases, .. } = value_type.shape() else {
        return Err(wrong_type());
    };
    cases
        .iter()
        .find(|case| case.tag() == tag)
        .map(|case| case.payload_type().clone())
        .ok_or_else(wrong_type)
}

fn wrong_type() -> NativeBindingRefusal {
    NativeBindingRefusal::InvalidValue(StructuredInfoRefusal::WrongType)
}

pub fn validate_native_contracts(
    value: &StructuredInfoValue,
    contracts: &[crate::NativeTypeValueContract],
) -> Result<(), NativeBindingRefusal> {
    for contract in contracts {
        validate_at_path(value, &contract.representation_path, &contract.contract).map_err(
            |refusal| NativeBindingRefusal::ViolatedConstraint {
                representation_path: contract.representation_path.clone(),
                refusal,
            },
        )?;
    }
    Ok(())
}

fn validate_at_path(
    value: &StructuredInfoValue,
    path: &str,
    contract: &CheckedValueContract,
) -> Result<(), ValueConstraintRefusal> {
    if path.is_empty() {
        let StructuredInfoValueShape::Leaf(bytes) = value.shape() else {
            return Err(ValueConstraintRefusal::WrongConstraintKind);
        };
        return contract.validate(bytes);
    }
    if let Some(rest) = path.strip_prefix("[]") {
        let StructuredInfoValueShape::Collection(values) = value.shape() else {
            return Err(ValueConstraintRefusal::WrongConstraintKind);
        };
        for item in values {
            validate_at_path(item, rest, contract)?;
        }
        return Ok(());
    }
    if let Some(rest) = path.strip_prefix("?some") {
        let StructuredInfoValueShape::Variant { tag, payload } = value.shape() else {
            return Err(ValueConstraintRefusal::WrongConstraintKind);
        };
        return if tag == "some" {
            validate_at_path(payload, rest, contract)
        } else {
            Ok(())
        };
    }
    if let Some(rest) = path.strip_prefix('.') {
        let (name, remaining) = split_path_component(rest);
        let StructuredInfoValueShape::Record(fields) = value.shape() else {
            return Err(ValueConstraintRefusal::WrongConstraintKind);
        };
        let field = fields
            .iter()
            .find(|field| field.name() == name)
            .ok_or(ValueConstraintRefusal::WrongConstraintKind)?;
        return validate_at_path(field.value(), remaining, contract);
    }
    if let Some(rest) = path.strip_prefix('|') {
        let (wanted, remaining) = split_path_component(rest);
        let StructuredInfoValueShape::Variant { tag, payload } = value.shape() else {
            return Err(ValueConstraintRefusal::WrongConstraintKind);
        };
        return if tag == wanted {
            validate_at_path(payload, remaining, contract)
        } else {
            Ok(())
        };
    }
    Err(ValueConstraintRefusal::WrongConstraintKind)
}

fn split_path_component(path: &str) -> (&str, &str) {
    let end = path
        .char_indices()
        .find_map(|(index, character)| {
            (index > 0 && matches!(character, '.' | '|' | '[' | '?')).then_some(index)
        })
        .unwrap_or(path.len());
    path.split_at(end)
}

/// Exact conversion boundary implemented by generated Rust bindings.
///
/// Implementations must recover their semantic Type from generated canonical
/// bytes. Rust type names and memory layout therefore cannot become identity.
pub trait NativeRustBinding: Sized {
    fn semantic_type() -> Result<StructuredInfoType, NativeBindingRefusal>;

    fn into_structured(self) -> Result<StructuredInfoValue, NativeBindingRefusal>;

    fn from_structured(value: StructuredInfoValue) -> Result<Self, NativeBindingRefusal>;

    fn encode(self) -> Result<Vec<u8>, NativeBindingRefusal> {
        self.into_structured()?
            .canonical_bytes()
            .map_err(NativeBindingRefusal::InvalidValue)
    }

    fn decode(canonical: &[u8]) -> Result<Self, NativeBindingRefusal> {
        let value = StructuredInfoValue::from_canonical_bytes(canonical)
            .map_err(NativeBindingRefusal::InvalidValue)?;
        if value.value_type() != &Self::semantic_type()? {
            return Err(NativeBindingRefusal::InvalidValue(
                StructuredInfoRefusal::WrongType,
            ));
        }
        Self::from_structured(value)
    }
}
