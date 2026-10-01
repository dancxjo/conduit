use crate::prelude::*;
use crate::{CheckedNativeType, NativeTypeValueContract};
use alloc::collections::{BTreeMap, BTreeSet};
use conduit_core::{
    CheckedTextPattern, CheckedValueContract, PrimitiveInfoKind, StructuredInfoType,
    StructuredInfoTypeShape, TextPatternState, TextPatternTransition, ValueConstraint,
};
use core::fmt::Write;

use super::generate::{
    copy_type, primitive_rust_type, references_external_type, rust_pascal_identifier,
    rust_snake_identifier, rust_type, unit_type, RustBindingGenerationError,
};

pub(super) struct RecordBindingOptions<'a> {
    pub nominal_copy: bool,
    pub copy: bool,
    pub value_getters: bool,
    pub direct_checked: bool,
    pub constructor_order: Option<&'a [String]>,
    pub constructor_name: &'a str,
    pub boxed_variant_payloads: &'a BTreeSet<String>,
    pub authored_type_name: &'a str,
}

pub(super) fn emit_value_impl(
    out: &mut String,
    value_type: &CheckedNativeType,
    rust_name: &str,
    constant: &str,
    names: &BTreeMap<String, String>,
    owned_identities: &BTreeSet<String>,
    record_options: RecordBindingOptions<'_>,
) -> Result<(), RustBindingGenerationError> {
    emit_contracts(
        out,
        rust_name,
        &value_type.value_contracts,
        &value_type.invariants,
    );
    match value_type.value_type.shape() {
        StructuredInfoTypeShape::Nominal { representation, .. } => emit_nominal(
            out,
            rust_name,
            constant,
            representation,
            names,
            &value_type.value_contracts,
            record_options.nominal_copy,
        )?,
        StructuredInfoTypeShape::Record { fields, .. } => {
            emit_record_constructor(
                out,
                rust_name,
                fields,
                names,
                &record_options,
                &value_type.value_contracts,
                !value_type.invariants.is_empty(),
            )?;
            super::generate_conversion::emit_record_binding(
                out,
                rust_name,
                constant,
                fields,
                names,
                record_options.constructor_order,
                record_options.constructor_name,
            )?
        }
        StructuredInfoTypeShape::Variant { cases, .. } => {
            emit_variant_constructors(
                out,
                rust_name,
                cases,
                names,
                owned_identities,
                &value_type.value_contracts,
                &record_options,
            )?;
            super::generate_conversion::emit_variant_binding(
                out,
                rust_name,
                constant,
                cases,
                names,
                record_options.authored_type_name,
                record_options.boxed_variant_payloads,
            )?
        }
        _ => return Err(RustBindingGenerationError::InvalidSemanticType),
    }
    Ok(())
}

fn emit_contracts(
    out: &mut String,
    rust_name: &str,
    contracts: &[NativeTypeValueContract],
    invariants: &[crate::PortableExpressionProgram],
) {
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
    writeln!(out, "        ]\n    }}").expect("String writing is infallible");
    super::generate_invariant::emit(out, invariants);
    writeln!(out, "}}\n").expect("String writing is infallible");
}

fn emit_nominal(
    out: &mut String,
    rust_name: &str,
    constant: &str,
    representation: &StructuredInfoType,
    names: &BTreeMap<String, String>,
    contracts: &[NativeTypeValueContract],
    copy: bool,
) -> Result<(), RustBindingGenerationError> {
    let inner = rust_type(representation, names)?;
    writeln!(out, "impl {rust_name} {{").expect("String writing is infallible");
    writeln!(
        out,
        "    pub fn new(value: {inner}) -> Result<Self, NativeBindingRefusal> {{"
    )
    .expect("String writing is infallible");
    writeln!(out, "        let candidate = Self(value);").expect("String writing is infallible");
    let directly_checked_integer =
        contracts.iter().all(|contract| {
            contract.representation_path.is_empty()
                && contract.contract.constraints.iter().all(|constraint| {
                    matches!(constraint, ValueConstraint::FixedIntegerRange { .. })
                })
        });
    let direct_kind = match representation.shape() {
        StructuredInfoTypeShape::Leaf(kind) if directly_checked_integer => Some(kind),
        _ => None,
    };
    if let Some(kind) = direct_kind {
        let primitive = primitive_rust_type(kind.as_str())?;
        emit_direct_integer_checks(out, "value", &primitive, contracts)?;
    } else {
        writeln!(
            out,
            "        let structured = candidate{}.into_structured()?;",
            if copy { "" } else { ".clone()" }
        )
        .expect("String writing is infallible");
        writeln!(out, "        conduit_form::rust_binding::validate_native_contracts(&structured, &Self::value_contracts())?;")
            .expect("String writing is infallible");
    }
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
    let encoded = super::generate_conversion::encode_expression(
        representation,
        "self.0",
        "representation.clone()",
        names,
    )?;
    writeln!(out, "        let value = {encoded};").expect("String writing is infallible");
    writeln!(out, "        StructuredInfoValue::nominal(semantic, value).map_err(NativeBindingRefusal::InvalidValue)\n    }}")
        .expect("String writing is infallible");
    writeln!(out, "    fn from_structured(value: StructuredInfoValue) -> Result<Self, NativeBindingRefusal> {{")
        .expect("String writing is infallible");
    writeln!(out, "        if value.value_type() != &Self::semantic_type()? {{ return Err(NativeBindingRefusal::InvalidValue(conduit_core::StructuredInfoRefusal::WrongType)); }}")
        .expect("String writing is infallible");
    writeln!(out, "        let representation = conduit_form::rust_binding::nominal_representation_type(&Self::semantic_type()?)?;")
        .expect("String writing is infallible");
    writeln!(out, "        let _ = &representation;").expect("String writing is infallible");
    let decoded = super::generate_conversion::decode_expression(representation, "value", names)?;
    writeln!(out, "        Self::new({decoded})\n    }}\n}}\n")
        .expect("String writing is infallible");
    Ok(())
}

fn emit_direct_integer_checks(
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

fn emit_record_constructor(
    out: &mut String,
    rust_name: &str,
    fields: &[conduit_core::StructuredFieldType],
    names: &BTreeMap<String, String>,
    options: &RecordBindingOptions<'_>,
    contracts: &[NativeTypeValueContract],
    has_invariants: bool,
) -> Result<(), RustBindingGenerationError> {
    let is_unconstrained = contracts.is_empty() && !has_invariants;
    if has_invariants && options.direct_checked {
        return Err(RustBindingGenerationError::InvalidSemanticType);
    }
    let ordered_fields = if let Some(order) = options.constructor_order {
        if order.len() != fields.len() {
            return Err(RustBindingGenerationError::InvalidSemanticType);
        }
        let mut seen = alloc::collections::BTreeSet::new();
        let mut ordered = Vec::with_capacity(fields.len());
        for name in order {
            if !seen.insert(name) {
                return Err(RustBindingGenerationError::InvalidSemanticType);
            }
            ordered.push(
                fields
                    .iter()
                    .find(|field| field.name() == name)
                    .ok_or(RustBindingGenerationError::InvalidSemanticType)?,
            );
        }
        ordered
    } else {
        fields.iter().collect::<Vec<_>>()
    };
    writeln!(out, "impl {rust_name} {{").expect("String writing is infallible");
    if ordered_fields.len() > 7 {
        writeln!(out, "    #[allow(clippy::too_many_arguments)]")
            .expect("String writing is infallible");
    }
    write!(out, "    pub fn {}(", options.constructor_name).expect("String writing is infallible");
    for (index, field) in ordered_fields.iter().enumerate() {
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
    write!(
        out,
        "        {}Self {{ ",
        if is_unconstrained {
            "Ok("
        } else {
            "let candidate = "
        }
    )
    .expect("String writing is infallible");
    for field in fields {
        write!(out, "{}, ", rust_snake_identifier(field.name())?)
            .expect("String writing is infallible");
    }
    if is_unconstrained {
        writeln!(out, "}})\n    }}").expect("String writing is infallible");
    } else if options.direct_checked {
        writeln!(out, "}};").expect("String writing is infallible");
        emit_direct_record_checks(out, fields, contracts)?;
        writeln!(out, "        Ok(candidate)\n    }}").expect("String writing is infallible");
    } else {
        writeln!(out, "}};").expect("String writing is infallible");
        writeln!(
            out,
            "        let structured = candidate{}.into_structured()?;",
            if options.copy { "" } else { ".clone()" }
        )
        .expect("String writing is infallible");
        writeln!(out, "        conduit_form::rust_binding::validate_native_contracts(&structured, &Self::value_contracts())?;")
            .expect("String writing is infallible");
        writeln!(out, "        conduit_form::rust_binding::validate_native_invariants(&structured, &Self::invariants()?)?;")
            .expect("String writing is infallible");
        writeln!(out, "        Ok(candidate)\n    }}").expect("String writing is infallible");
    }
    for field in fields {
        let name = rust_snake_identifier(field.name())?;
        let value_type = rust_type(field.value_type(), names)?;
        if options.value_getters {
            writeln!(
                out,
                "    pub const fn {name}(self) -> {value_type} {{ self.{name} }}"
            )
            .expect("String writing is infallible");
        } else {
            writeln!(
                out,
                "    pub fn {name}(&self) -> &{value_type} {{ &self.{name} }}"
            )
            .expect("String writing is infallible");
        }
    }
    writeln!(out, "}}\n").expect("String writing is infallible");
    Ok(())
}

fn emit_direct_record_checks(
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
        emit_direct_integer_checks(
            out,
            &field_name,
            &primitive,
            core::slice::from_ref(contract),
        )?;
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

fn emit_variant_constructors(
    out: &mut String,
    rust_name: &str,
    cases: &[conduit_core::StructuredVariantCase],
    names: &BTreeMap<String, String>,
    owned_identities: &BTreeSet<String>,
    contracts: &[NativeTypeValueContract],
    options: &RecordBindingOptions<'_>,
) -> Result<(), RustBindingGenerationError> {
    let copy_payloads = cases.iter().all(|case| {
        !matches!(
            case.payload_type().shape(),
            StructuredInfoTypeShape::Record { .. }
        ) && !references_external_type(case.payload_type(), owned_identities)
            && copy_type(case.payload_type())
    });
    writeln!(out, "impl {rust_name} {{").expect("String writing is infallible");
    for case in cases {
        let function = rust_snake_identifier(case.tag())?;
        let variant = rust_pascal_identifier(case.tag())?;
        if unit_type(case.payload_type()) {
            writeln!(out, "    pub fn {function}() -> Self {{ Self::{variant} }}")
                .expect("String writing is infallible");
        } else {
            let boxed = options.boxed_variant_payloads.contains(&format!(
                "{}.{}",
                options.authored_type_name,
                case.tag()
            ));
            let StructuredInfoTypeShape::Record { fields, .. } = case.payload_type().shape() else {
                let payload_type = rust_type(case.payload_type(), names)?;
                let candidate = if copy_payloads {
                    "candidate"
                } else {
                    "candidate.clone()"
                };
                let payload = if boxed {
                    "Box::new(payload)"
                } else {
                    "payload"
                };
                writeln!(out, "    pub fn {function}(payload: {payload_type}) -> Result<Self, NativeBindingRefusal> {{ let candidate = Self::{variant}({payload}); let structured = {candidate}.into_structured()?; conduit_form::rust_binding::validate_native_contracts(&structured, &Self::value_contracts())?; Ok(candidate) }}")
                    .expect("String writing is infallible");
                continue;
            };
            let payload = format!("{rust_name}{variant}");
            let contract_prefix = format!("|{}.", case.tag());
            let is_unconstrained = contracts.iter().all(|contract| {
                !contract.representation_path.is_empty()
                    && !contract.representation_path.starts_with(&contract_prefix)
            }) && fields.iter().all(|field| {
                matches!(
                    field.value_type().shape(),
                    StructuredInfoTypeShape::Leaf(kind)
                        if conduit_core::primitive_info_kind(kind.as_str())
                            == Some(PrimitiveInfoKind::Bool)
                )
            });
            if fields.len() > 7 {
                writeln!(out, "    #[allow(clippy::too_many_arguments)]")
                    .expect("String writing is infallible");
            }
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
            write!(out, ") -> Result<Self, NativeBindingRefusal> {{ ")
                .expect("String writing is infallible");
            if is_unconstrained {
                write!(
                    out,
                    "Ok(Self::{variant}({}{payload} {{ ",
                    if boxed { "Box::new(" } else { "" }
                )
                .expect("String writing is infallible");
            } else {
                write!(
                    out,
                    "let candidate = Self::{variant}({}{payload} {{ ",
                    if boxed { "Box::new(" } else { "" }
                )
                .expect("String writing is infallible");
            }
            for field in fields {
                write!(out, "{}, ", rust_snake_identifier(field.name())?)
                    .expect("String writing is infallible");
            }
            if is_unconstrained {
                writeln!(out, "}}{})) }}", if boxed { ")" } else { "" })
                    .expect("String writing is infallible");
            } else {
                writeln!(out, "}}{}); let structured = candidate.clone().into_structured()?; conduit_form::rust_binding::validate_native_contracts(&structured, &Self::value_contracts())?; Ok(candidate) }}", if boxed { ")" } else { "" })
                    .expect("String writing is infallible");
            }
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
            "conduit_core::ValueConstraint::UnsignedRange {{ minimum: {minimum:?}, maximum: {maximum:?}, minimum_endpoint: {}, maximum_endpoint: {} }}",
            endpoint_literal(*minimum_endpoint), endpoint_literal(*maximum_endpoint)
        ),
        ValueConstraint::SignedRange { minimum, maximum, minimum_endpoint, maximum_endpoint } => format!(
            "conduit_core::ValueConstraint::SignedRange {{ minimum: {minimum:?}, maximum: {maximum:?}, minimum_endpoint: {}, maximum_endpoint: {} }}",
            endpoint_literal(*minimum_endpoint), endpoint_literal(*maximum_endpoint)
        ),
        ValueConstraint::FixedIntegerRange { minimum, maximum, minimum_endpoint, maximum_endpoint } => format!(
            "conduit_core::ValueConstraint::FixedIntegerRange {{ minimum: {}, maximum: {}, minimum_endpoint: {}, maximum_endpoint: {} }}",
            optional_bytes_literal(minimum), optional_bytes_literal(maximum), endpoint_literal(*minimum_endpoint), endpoint_literal(*maximum_endpoint)
        ),
        ValueConstraint::QuantityRange { minimum, maximum, minimum_endpoint, maximum_endpoint } => format!(
            "conduit_core::ValueConstraint::QuantityRange {{ minimum: {}, maximum: {}, minimum_endpoint: {}, maximum_endpoint: {} }}",
            optional_quantity_literal(minimum), optional_quantity_literal(maximum), endpoint_literal(*minimum_endpoint), endpoint_literal(*maximum_endpoint)
        ),
        ValueConstraint::FloatFinite => "conduit_core::ValueConstraint::FloatFinite".into(),
        ValueConstraint::FloatRange { minimum, maximum, minimum_endpoint, maximum_endpoint } => format!(
            "conduit_core::ValueConstraint::FloatRange {{ minimum: {}, maximum: {}, minimum_endpoint: {}, maximum_endpoint: {} }}",
            optional_bytes_literal(minimum), optional_bytes_literal(maximum), endpoint_literal(*minimum_endpoint), endpoint_literal(*maximum_endpoint)
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

fn optional_bytes_literal(value: &Option<Vec<u8>>) -> String {
    value
        .as_ref()
        .map(|value| format!("Some(vec!{value:?})"))
        .unwrap_or_else(|| "None".into())
}

fn optional_quantity_literal(value: &Option<conduit_core::Quantity>) -> String {
    value
        .as_ref()
        .map(|value| {
            format!(
                "Some(conduit_core::Quantity::new({}, conduit_core::QuantityUnit::{:?}))",
                value.value(),
                value.unit()
            )
        })
        .unwrap_or_else(|| "None".into())
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
