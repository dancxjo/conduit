//! Allocation-free generated checks for directly represented primitive fields.

use super::generate::{primitive_rust_type, rust_snake_identifier, RustBindingGenerationError};
use crate::prelude::*;
use crate::NativeTypeValueContract;
use conduit_core::{StructuredInfoTypeShape, ValueConstraint};
use core::fmt::Write;

pub(super) fn emit_direct_integer_checks(
    out: &mut String,
    value: &str,
    primitive: &str,
    contracts: &[NativeTypeValueContract],
) -> Result<(), RustBindingGenerationError> {
    for contract in contracts {
        for constraint in &contract.contract.constraints {
            let ValueConstraint::FixedIntegerRange {
                minimum,
                maximum,
                minimum_endpoint,
                maximum_endpoint,
            } = constraint
            else {
                return Err(RustBindingGenerationError::InvalidSemanticType);
            };
            if let (Some(minimum), Some(maximum)) = (minimum, maximum) {
                if matches!(minimum_endpoint, conduit_core::IntervalEndpoint::Inclusive)
                    && matches!(maximum_endpoint, conduit_core::IntervalEndpoint::Inclusive)
                {
                    let minimum = fixed_integer_literal(minimum, primitive)?;
                    let maximum = fixed_integer_literal(maximum, primitive)?;
                    writeln!(
                        out,
                        "        if !({minimum}..={maximum}).contains(&{value}) {{ return Err(NativeBindingRefusal::ViolatedConstraint {{ representation_path: {:?}.into(), refusal: conduit_core::ValueConstraintRefusal::FixedIntegerRange }}); }}",
                        contract.representation_path,
                    )
                    .expect("String writing is infallible");
                    continue;
                }
            }
            let mut predicates = Vec::new();
            if let Some(minimum) = minimum {
                if !matches!(minimum_endpoint, conduit_core::IntervalEndpoint::Inclusive)
                    || !fixed_integer_is_type_minimum(minimum, primitive)?
                {
                    let minimum = fixed_integer_literal(minimum, primitive)?;
                    let operator = match minimum_endpoint {
                        conduit_core::IntervalEndpoint::Inclusive => ">=",
                        conduit_core::IntervalEndpoint::Exclusive => ">",
                    };
                    predicates.push(format!("{value} {operator} {minimum}"));
                }
            }
            if let Some(maximum) = maximum {
                if !matches!(maximum_endpoint, conduit_core::IntervalEndpoint::Inclusive)
                    || !fixed_integer_is_type_maximum(maximum, primitive)?
                {
                    let maximum = fixed_integer_literal(maximum, primitive)?;
                    let operator = match maximum_endpoint {
                        conduit_core::IntervalEndpoint::Inclusive => "<=",
                        conduit_core::IntervalEndpoint::Exclusive => "<",
                    };
                    predicates.push(format!("{value} {operator} {maximum}"));
                }
            }
            if !predicates.is_empty() {
                writeln!(
                    out,
                    "        if !({}) {{ return Err(NativeBindingRefusal::ViolatedConstraint {{ representation_path: {:?}.into(), refusal: conduit_core::ValueConstraintRefusal::FixedIntegerRange }}); }}",
                    predicates.join(" && "),
                    contract.representation_path,
                )
                .expect("String writing is infallible");
            }
        }
    }
    Ok(())
}

pub(super) fn emit_direct_record_checks(
    out: &mut String,
    fields: &[conduit_core::StructuredFieldType],
    contracts: &[NativeTypeValueContract],
) -> Result<(), RustBindingGenerationError> {
    for contract in contracts {
        let Some(path) = contract.representation_path.strip_prefix('.') else {
            continue;
        };
        if path.contains(['.', '|', '[', '?']) {
            continue;
        }
        let field = fields
            .iter()
            .find(|field| field.name() == path)
            .ok_or(RustBindingGenerationError::InvalidSemanticType)?;
        let StructuredInfoTypeShape::Leaf(kind) = field.value_type().shape() else {
            continue;
        };
        let field_name = rust_snake_identifier(field.name())?;
        let primitive = primitive_rust_type(kind.as_str())?;
        if kind.as_str() == conduit_core::TEXT_INFO_ID {
            emit_direct_text_checks(out, &format!("candidate.{field_name}"), contract)?;
        } else {
            emit_direct_integer_checks(
                out,
                &field_name,
                &primitive,
                core::slice::from_ref(contract),
            )?;
        }
    }
    Ok(())
}

fn fixed_integer_is_type_minimum(
    bytes: &[u8],
    primitive: &str,
) -> Result<bool, RustBindingGenerationError> {
    macro_rules! compare {
        ($type:ty) => {{
            Ok(<$type>::from_le_bytes(
                bytes
                    .try_into()
                    .map_err(|_| RustBindingGenerationError::InvalidSemanticType)?,
            ) == <$type>::MIN)
        }};
    }
    match primitive {
        "u8" => compare!(u8),
        "u16" => compare!(u16),
        "u32" => compare!(u32),
        "u64" => compare!(u64),
        "i8" => compare!(i8),
        "i16" => compare!(i16),
        "i32" => compare!(i32),
        "i64" => compare!(i64),
        _ => Err(RustBindingGenerationError::InvalidSemanticType),
    }
}

fn fixed_integer_is_type_maximum(
    bytes: &[u8],
    primitive: &str,
) -> Result<bool, RustBindingGenerationError> {
    macro_rules! compare {
        ($type:ty) => {{
            Ok(<$type>::from_le_bytes(
                bytes
                    .try_into()
                    .map_err(|_| RustBindingGenerationError::InvalidSemanticType)?,
            ) == <$type>::MAX)
        }};
    }
    match primitive {
        "u8" => compare!(u8),
        "u16" => compare!(u16),
        "u32" => compare!(u32),
        "u64" => compare!(u64),
        "i8" => compare!(i8),
        "i16" => compare!(i16),
        "i32" => compare!(i32),
        "i64" => compare!(i64),
        _ => Err(RustBindingGenerationError::InvalidSemanticType),
    }
}

fn fixed_integer_literal(
    bytes: &[u8],
    primitive: &str,
) -> Result<String, RustBindingGenerationError> {
    macro_rules! decode {
        ($type:ty) => {{
            let value = <$type>::from_le_bytes(
                bytes
                    .try_into()
                    .map_err(|_| RustBindingGenerationError::InvalidSemanticType)?,
            );
            Ok(format!("{value}{primitive}"))
        }};
    }
    match primitive {
        "u8" => decode!(u8),
        "u16" => decode!(u16),
        "u32" => decode!(u32),
        "u64" => decode!(u64),
        "i8" => decode!(i8),
        "i16" => decode!(i16),
        "i32" => decode!(i32),
        "i64" => decode!(i64),
        _ => Err(RustBindingGenerationError::InvalidSemanticType),
    }
}

fn emit_direct_text_checks(
    out: &mut String,
    value: &str,
    contract: &NativeTypeValueContract,
) -> Result<(), RustBindingGenerationError> {
    let path = &contract.representation_path;
    let maximum = contract.contract.maximum_bytes;
    writeln!(out, "        if {value}.len() > {maximum}usize {{ return Err(NativeBindingRefusal::ViolatedConstraint {{ representation_path: {path:?}.into(), refusal: conduit_core::ValueConstraintRefusal::Oversize {{ actual: u32::try_from({value}.len()).unwrap_or(u32::MAX), maximum: {maximum} }} }}); }}")
        .expect("String writing is infallible");
    for constraint in &contract.contract.constraints {
        let (predicate, refusal) = match constraint {
            ValueConstraint::ByteLength { minimum, maximum } => (
                format!("!({minimum}usize..={maximum}usize).contains(&{value}.len())"),
                "ByteLength",
            ),
            ValueConstraint::CanonicalMembership { members, negated } => {
                let literals = members
                    .iter()
                    .map(|member| {
                        core::str::from_utf8(member)
                            .map(|member| format!("{member:?}"))
                            .map_err(|_| RustBindingGenerationError::InvalidSemanticType)
                    })
                    .collect::<Result<Vec<_>, _>>()?
                    .join(", ");
                (
                    format!(
                        "{}[{literals}].contains(&{value}.as_str())",
                        if *negated { "" } else { "!" }
                    ),
                    "Membership",
                )
            }
            // Reject unsupported constraints rather than silently skipping laws.
            _ => return Err(RustBindingGenerationError::InvalidSemanticType),
        };
        writeln!(out, "        if {predicate} {{ return Err(NativeBindingRefusal::ViolatedConstraint {{ representation_path: {path:?}.into(), refusal: conduit_core::ValueConstraintRefusal::{refusal} }}); }}")
            .expect("String writing is infallible");
    }
    Ok(())
}
