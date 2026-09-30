use crate::prelude::*;
use crate::{CheckedNativeType, NativeTypeValueContract};
use alloc::collections::BTreeMap;
use conduit_core::{
    CheckedTextPattern, CheckedValueContract, StructuredInfoType, StructuredInfoTypeShape,
    TextPatternState, TextPatternTransition, ValueConstraint,
};
use core::fmt::Write;

use super::generate::{
    primitive_rust_type, rust_pascal_identifier, rust_snake_identifier, rust_type, unit_type,
    RustBindingGenerationError,
};

pub(super) fn emit_value_impl(
    out: &mut String,
    value_type: &CheckedNativeType,
    rust_name: &str,
    constant: &str,
    names: &BTreeMap<String, String>,
) -> Result<(), RustBindingGenerationError> {
    emit_contracts(out, rust_name, &value_type.value_contracts);
    match value_type.value_type.shape() {
        StructuredInfoTypeShape::Nominal { representation, .. } => {
            emit_scalar(out, rust_name, constant, representation, names)?
        }
        StructuredInfoTypeShape::Record { fields, .. } => {
            emit_record_constructor(out, rust_name, fields, names)?;
            super::generate_conversion::emit_record_binding(
                out, rust_name, constant, fields, names,
            )?
        }
        StructuredInfoTypeShape::Variant { cases, .. } => {
            emit_variant_constructors(out, rust_name, cases, names)?;
            super::generate_conversion::emit_variant_binding(
                out, rust_name, constant, cases, names,
            )?
        }
        _ => return Err(RustBindingGenerationError::InvalidSemanticType),
    }
    Ok(())
}

fn emit_contracts(out: &mut String, rust_name: &str, contracts: &[NativeTypeValueContract]) {
    writeln!(out, "impl {rust_name} {{").expect("String writing is infallible");
    if let Some(root) = contracts
        .iter()
        .find(|contract| contract.representation_path.is_empty())
    {
        writeln!(
            out,
            "    pub const MAXIMUM_BYTES: usize = {};",
            root.contract.maximum_bytes
        )
        .expect("String writing is infallible");
    }
    writeln!(
        out,
        "    fn value_contracts() -> Vec<conduit_form::NativeTypeValueContract> {{"
    )
    .expect("String writing is infallible");
    writeln!(out, "        vec![").expect("String writing is infallible");
    for contract in contracts {
        writeln!(
            out,
            "            conduit_form::NativeTypeValueContract {{ representation_path: {:?}.into(), contract: {} }},",
            contract.representation_path,
            contract_literal(&contract.contract)
        )
        .expect("String writing is infallible");
    }
    writeln!(out, "        ]\n    }}\n}}\n").expect("String writing is infallible");
}

fn emit_scalar(
    out: &mut String,
    rust_name: &str,
    constant: &str,
    representation: &StructuredInfoType,
    names: &BTreeMap<String, String>,
) -> Result<(), RustBindingGenerationError> {
    let StructuredInfoTypeShape::Leaf(kind) = representation.shape() else {
        return Err(RustBindingGenerationError::InvalidSemanticType);
    };
    let inner = primitive_rust_type(kind.as_str())?;
    writeln!(out, "impl {rust_name} {{").expect("String writing is infallible");
    writeln!(
        out,
        "    pub fn new(value: {inner}) -> Result<Self, NativeBindingRefusal> {{"
    )
    .expect("String writing is infallible");
    writeln!(out, "        let candidate = Self(value);").expect("String writing is infallible");
    writeln!(
        out,
        "        let structured = candidate.clone().into_structured()?;"
    )
    .expect("String writing is infallible");
    writeln!(out, "        conduit_form::rust_binding::validate_native_contracts(&structured, &Self::value_contracts())?;")
        .expect("String writing is infallible");
    writeln!(out, "        Ok(candidate)\n    }}\n}}\n").expect("String writing is infallible");

    writeln!(out, "impl NativeRustBinding for {rust_name} {{")
        .expect("String writing is infallible");
    writeln!(out, "    fn semantic_type() -> Result<StructuredInfoType, NativeBindingRefusal> {{ Self::semantic_type() }}")
        .expect("String writing is infallible");
    writeln!(
        out,
        "    fn into_structured(self) -> Result<StructuredInfoValue, NativeBindingRefusal> {{"
    )
    .expect("String writing is infallible");
    writeln!(out, "        let semantic = StructuredInfoType::from_canonical_bytes({constant}).map_err(NativeBindingRefusal::InvalidSemanticType)?;")
        .expect("String writing is infallible");
    writeln!(out, "        let representation = conduit_form::rust_binding::nominal_representation_type(&semantic)?;")
        .expect("String writing is infallible");
    writeln!(out, "        let value = conduit_form::rust_binding::primitive_into_structured(representation, &self.0)?;")
        .expect("String writing is infallible");
    writeln!(out, "        StructuredInfoValue::nominal(semantic, value).map_err(NativeBindingRefusal::InvalidValue)\n    }}")
        .expect("String writing is infallible");
    writeln!(out, "    fn from_structured(value: StructuredInfoValue) -> Result<Self, NativeBindingRefusal> {{")
        .expect("String writing is infallible");
    writeln!(out, "        if value.value_type() != &Self::semantic_type()? {{ return Err(NativeBindingRefusal::InvalidValue(conduit_core::StructuredInfoRefusal::WrongType)); }}")
        .expect("String writing is infallible");
    writeln!(out, "        let primitive = conduit_form::rust_binding::primitive_from_structured::<{inner}>(&value)?;")
        .expect("String writing is infallible");
    writeln!(out, "        Self::new(primitive)\n    }}\n}}\n")
        .expect("String writing is infallible");
    let _ = names;
    Ok(())
}

fn emit_record_constructor(
    out: &mut String,
    rust_name: &str,
    fields: &[conduit_core::StructuredFieldType],
    names: &BTreeMap<String, String>,
) -> Result<(), RustBindingGenerationError> {
    writeln!(out, "impl {rust_name} {{").expect("String writing is infallible");
    write!(out, "    pub fn new(").expect("String writing is infallible");
    for (index, field) in fields.iter().enumerate() {
        if index > 0 {
            out.push_str(", ");
        }
        write!(
            out,
            "{}: {}",
            rust_snake_identifier(field.name())?,
            rust_type(field.value_type(), names)?
        )
        .expect("String writing is infallible");
    }
    writeln!(out, ") -> Result<Self, NativeBindingRefusal> {{")
        .expect("String writing is infallible");
    write!(out, "        let candidate = Self {{ ").expect("String writing is infallible");
    for field in fields {
        write!(out, "{}, ", rust_snake_identifier(field.name())?)
            .expect("String writing is infallible");
    }
    writeln!(out, "}};").expect("String writing is infallible");
    writeln!(
        out,
        "        let structured = candidate.clone().into_structured()?;"
    )
    .expect("String writing is infallible");
    writeln!(out, "        conduit_form::rust_binding::validate_native_contracts(&structured, &Self::value_contracts())?;")
        .expect("String writing is infallible");
    writeln!(out, "        Ok(candidate)\n    }}").expect("String writing is infallible");
    for field in fields {
        let name = rust_snake_identifier(field.name())?;
        let value_type = rust_type(field.value_type(), names)?;
        writeln!(
            out,
            "    pub fn {name}(&self) -> &{value_type} {{ &self.{name} }}"
        )
        .expect("String writing is infallible");
    }
    writeln!(out, "}}\n").expect("String writing is infallible");
    Ok(())
}

fn emit_variant_constructors(
    out: &mut String,
    rust_name: &str,
    cases: &[conduit_core::StructuredVariantCase],
    names: &BTreeMap<String, String>,
) -> Result<(), RustBindingGenerationError> {
    writeln!(out, "impl {rust_name} {{").expect("String writing is infallible");
    for case in cases {
        let function = rust_snake_identifier(case.tag())?;
        let variant = rust_pascal_identifier(case.tag())?;
        if unit_type(case.payload_type()) {
            writeln!(out, "    pub fn {function}() -> Self {{ Self::{variant} }}")
                .expect("String writing is infallible");
        } else {
            let payload = format!("{rust_name}{variant}");
            let StructuredInfoTypeShape::Record { fields, .. } = case.payload_type().shape() else {
                return Err(RustBindingGenerationError::InvalidSemanticType);
            };
            write!(out, "    pub fn {function}(").expect("String writing is infallible");
            for (index, field) in fields.iter().enumerate() {
                if index > 0 {
                    out.push_str(", ");
                }
                write!(
                    out,
                    "{}: {}",
                    rust_snake_identifier(field.name())?,
                    rust_type(field.value_type(), names)?
                )
                .expect("String writing is infallible");
            }
            write!(out, ") -> Result<Self, NativeBindingRefusal> {{ let candidate = Self::{variant}({payload} {{ ")
            .expect("String writing is infallible");
            for field in fields {
                write!(out, "{}, ", rust_snake_identifier(field.name())?)
                    .expect("String writing is infallible");
            }
            writeln!(out, "}}); let structured = candidate.clone().into_structured()?; conduit_form::rust_binding::validate_native_contracts(&structured, &Self::value_contracts())?; Ok(candidate) }}")
                .expect("String writing is infallible");
        }
    }
    writeln!(out, "}}\n").expect("String writing is infallible");
    Ok(())
}

fn contract_literal(contract: &CheckedValueContract) -> String {
    format!(
        "conduit_core::CheckedValueContract::new(conduit_core::kind_id({:?}), {}, vec![{}]).expect(\"generated checked contract\")",
        contract.value_kind.as_str(),
        contract.maximum_bytes,
        contract
            .constraints
            .iter()
            .map(constraint_literal)
            .collect::<Vec<_>>()
            .join(", ")
    )
}

fn constraint_literal(constraint: &ValueConstraint) -> String {
    match constraint {
        ValueConstraint::ByteLength { minimum, maximum } => format!(
            "conduit_core::ValueConstraint::ByteLength {{ minimum: {minimum}, maximum: {maximum} }}"
        ),
        ValueConstraint::UnsignedRange { minimum, maximum, minimum_endpoint, maximum_endpoint } => format!(
            "conduit_core::ValueConstraint::UnsignedRange {{ minimum: {minimum}, maximum: {maximum}, minimum_endpoint: {}, maximum_endpoint: {} }}",
            endpoint_literal(*minimum_endpoint), endpoint_literal(*maximum_endpoint)
        ),
        ValueConstraint::SignedRange { minimum, maximum, minimum_endpoint, maximum_endpoint } => format!(
            "conduit_core::ValueConstraint::SignedRange {{ minimum: {minimum}, maximum: {maximum}, minimum_endpoint: {}, maximum_endpoint: {} }}",
            endpoint_literal(*minimum_endpoint), endpoint_literal(*maximum_endpoint)
        ),
        ValueConstraint::FixedIntegerRange { minimum, maximum, minimum_endpoint, maximum_endpoint } => format!(
            "conduit_core::ValueConstraint::FixedIntegerRange {{ minimum: vec!{:?}, maximum: vec!{:?}, minimum_endpoint: {}, maximum_endpoint: {} }}",
            minimum, maximum, endpoint_literal(*minimum_endpoint), endpoint_literal(*maximum_endpoint)
        ),
        ValueConstraint::QuantityRange { minimum, maximum, minimum_endpoint, maximum_endpoint } => format!(
            "conduit_core::ValueConstraint::QuantityRange {{ minimum: conduit_core::Quantity::new({}, conduit_core::QuantityUnit::{:?}), maximum: conduit_core::Quantity::new({}, conduit_core::QuantityUnit::{:?}), minimum_endpoint: {}, maximum_endpoint: {} }}",
            minimum.value(), minimum.unit(), maximum.value(), maximum.unit(), endpoint_literal(*minimum_endpoint), endpoint_literal(*maximum_endpoint)
        ),
        ValueConstraint::CanonicalMembership { members, negated } => {
            let members = members
                .iter()
                .map(|member| format!("vec!{member:?}"))
                .collect::<Vec<_>>()
                .join(", ");
            format!(
                "conduit_core::ValueConstraint::CanonicalMembership {{ members: vec![{members}], negated: {negated} }}"
            )
        }
        ValueConstraint::TextPattern { pattern, anchored_start, anchored_end, negated } => format!(
            "conduit_core::ValueConstraint::TextPattern {{ pattern: {}, anchored_start: {anchored_start}, anchored_end: {anchored_end}, negated: {negated} }}",
            pattern_literal(pattern)
        ),
    }
}

fn endpoint_literal(endpoint: conduit_core::IntervalEndpoint) -> &'static str {
    match endpoint {
        conduit_core::IntervalEndpoint::Inclusive => "conduit_core::IntervalEndpoint::Inclusive",
        conduit_core::IntervalEndpoint::Exclusive => "conduit_core::IntervalEndpoint::Exclusive",
    }
}

fn pattern_literal(pattern: &CheckedTextPattern) -> String {
    format!(
        "conduit_core::CheckedTextPattern {{ states: vec![{}], start_state: {}, maximum_input_characters: {}, maximum_match_steps: {} }}",
        pattern.states.iter().map(state_literal).collect::<Vec<_>>().join(", "),
        pattern.start_state, pattern.maximum_input_characters, pattern.maximum_match_steps
    )
}

fn state_literal(state: &TextPatternState) -> String {
    format!(
        "conduit_core::TextPatternState {{ accepting: {}, transitions: vec![{}] }}",
        state.accepting,
        state
            .transitions
            .iter()
            .map(transition_literal)
            .collect::<Vec<_>>()
            .join(", ")
    )
}

fn transition_literal(transition: &TextPatternTransition) -> String {
    format!(
        "conduit_core::TextPatternTransition {{ first_scalar: {}, last_scalar: {}, target_state: {} }}",
        transition.first_scalar, transition.last_scalar, transition.target_state
    )
}
