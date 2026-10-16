//! One Native refinement traversal for owned and borrowed canonical values.
use super::value::NativeBindingRefusal;
use crate::NativeTypeValueContract;
use conduit_core::{
    CheckedValueContract, StructuredInfoValue, StructuredInfoValueShape,
    ValidatedCanonicalStructuredValue, ValueConstraintRefusal,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BorrowedNativeContractRefusal<'a> {
    pub representation_path: &'a str,
    pub refusal: ValueConstraintRefusal,
}

pub(super) fn validate_owned(
    value: &StructuredInfoValue,
    contracts: &[NativeTypeValueContract],
) -> Result<(), NativeBindingRefusal> {
    for contract in contracts {
        validate_at_path(
            Value::Owned(value),
            &contract.representation_path,
            &contract.contract,
        )
        .map_err(|refusal| NativeBindingRefusal::ViolatedConstraint {
            representation_path: contract.representation_path.clone(),
            refusal,
        })?;
    }
    Ok(())
}

/// The caller must first establish the exact expected Native semantic Type.
/// Representation traversal does not admit a foreign nominal or grant access.
pub fn validate_native_contracts_borrowed<'a>(
    value: ValidatedCanonicalStructuredValue<'_>,
    contracts: &'a [NativeTypeValueContract],
) -> Result<(), BorrowedNativeContractRefusal<'a>> {
    for contract in contracts {
        validate_at_path(
            Value::Borrowed(value),
            &contract.representation_path,
            &contract.contract,
        )
        .map_err(|refusal| BorrowedNativeContractRefusal {
            representation_path: &contract.representation_path,
            refusal,
        })?;
    }
    Ok(())
}

#[derive(Clone, Copy)]
enum Value<'a> {
    Owned(&'a StructuredInfoValue),
    Borrowed(ValidatedCanonicalStructuredValue<'a>),
}
impl<'a> Value<'a> {
    fn bytes(self, contract: &CheckedValueContract) -> Result<&'a [u8], ValueConstraintRefusal> {
        match self {
            Self::Owned(value) => match value.shape() {
                StructuredInfoValueShape::Leaf(bytes) => Ok(bytes),
                _ => Err(ValueConstraintRefusal::WrongConstraintKind),
            },
            Self::Borrowed(value) => value
                .primitive_representation_bytes(contract.value_kind.as_str())
                .map_err(|_| ValueConstraintRefusal::WrongConstraintKind),
        }
    }
    fn count(self) -> Result<usize, ValueConstraintRefusal> {
        match self {
            Self::Owned(value) => match value.shape() {
                StructuredInfoValueShape::Collection(values) => Ok(values.len()),
                _ => Err(ValueConstraintRefusal::WrongConstraintKind),
            },
            Self::Borrowed(value) => value
                .collection_length()
                .map(|v| v as usize)
                .map_err(|_| ValueConstraintRefusal::WrongConstraintKind),
        }
    }
    fn index(self, index: usize) -> Result<Self, ValueConstraintRefusal> {
        match self {
            Self::Owned(value) => match value.shape() {
                StructuredInfoValueShape::Collection(values) => values
                    .get(index)
                    .map(Self::Owned)
                    .ok_or(ValueConstraintRefusal::WrongConstraintKind),
                _ => Err(ValueConstraintRefusal::WrongConstraintKind),
            },
            Self::Borrowed(value) => value
                .collection_index(
                    u16::try_from(index)
                        .map_err(|_| ValueConstraintRefusal::WrongConstraintKind)?,
                )
                .map_err(|_| ValueConstraintRefusal::WrongConstraintKind)?
                .map(Self::Borrowed)
                .ok_or(ValueConstraintRefusal::WrongConstraintKind),
        }
    }
    fn field(self, name: &str) -> Result<Self, ValueConstraintRefusal> {
        match self {
            Self::Owned(value) => match value.shape() {
                StructuredInfoValueShape::Record(fields) => fields
                    .iter()
                    .find(|f| f.name() == name)
                    .map(|f| Self::Owned(f.value()))
                    .ok_or(ValueConstraintRefusal::WrongConstraintKind),
                _ => Err(ValueConstraintRefusal::WrongConstraintKind),
            },
            Self::Borrowed(value) => value
                .representation()
                .and_then(|v| v.record_field(name))
                .map_err(|_| ValueConstraintRefusal::WrongConstraintKind)?
                .map(Self::Borrowed)
                .ok_or(ValueConstraintRefusal::WrongConstraintKind),
        }
    }
    fn variant(self, wanted: &str) -> Result<Option<Self>, ValueConstraintRefusal> {
        match self {
            Self::Owned(value) => match value.shape() {
                StructuredInfoValueShape::Variant { tag, payload } => {
                    Ok((tag == wanted).then_some(Self::Owned(payload)))
                }
                _ => Err(ValueConstraintRefusal::WrongConstraintKind),
            },
            Self::Borrowed(value) => value
                .representation()
                .and_then(|v| v.variant_payload(wanted))
                .map(|v| v.map(Self::Borrowed))
                .map_err(|_| ValueConstraintRefusal::WrongConstraintKind),
        }
    }
}
fn validate_at_path(
    value: Value<'_>,
    path: &str,
    contract: &CheckedValueContract,
) -> Result<(), ValueConstraintRefusal> {
    if path.is_empty() {
        return contract.validate(value.bytes(contract)?);
    }
    if let Some(rest) = path.strip_prefix("[]") {
        for index in 0..value.count()? {
            validate_at_path(value.index(index)?, rest, contract)?;
        }
        return Ok(());
    }
    if let Some(rest) = path.strip_prefix("?some") {
        if let Some(payload) = value.variant("some")? {
            validate_at_path(payload, rest, contract)?;
        }
        return Ok(());
    }
    if let Some(rest) = path.strip_prefix('.') {
        let (name, remaining) = split_path_component(rest);
        return validate_at_path(value.field(name)?, remaining, contract);
    }
    if let Some(rest) = path.strip_prefix('|') {
        let (wanted, remaining) = split_path_component(rest);
        if let Some(payload) = value.variant(wanted)? {
            validate_at_path(payload, remaining, contract)?;
        }
        return Ok(());
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
