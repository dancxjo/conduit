use crate::prelude::*;
use alloc::collections::BTreeMap;
use conduit_core::{StructuredInfoType, StructuredInfoTypeShape};
use core::fmt::Write;

use super::generate::{
    primitive_rust_type, rust_pascal_identifier, rust_snake_identifier, rust_type, unit_type,
    RustBindingGenerationError,
};

pub(super) fn emit_record_binding(
    out: &mut String,
    rust_name: &str,
    constant: &str,
    fields: &[conduit_core::StructuredFieldType],
    names: &BTreeMap<String, String>,
) -> Result<(), RustBindingGenerationError> {
    trait_header(out, rust_name, constant);
    writeln!(out, "        let mut fields = Vec::new();").expect("String writing is infallible");
    for field in fields {
        let rust_field = rust_snake_identifier(field.name())?;
        writeln!(out, "        let field_type = conduit_form::rust_binding::record_field_type(&semantic, {:?})?;", field.name())
            .expect("String writing is infallible");
        writeln!(out, "        let _ = &field_type;").expect("String writing is infallible");
        let encoded = encode_expression(
            field.value_type(),
            &format!("self.{rust_field}"),
            "field_type",
            names,
        )?;
        writeln!(out, "        fields.push(StructuredFieldValue::new({:?}, {encoded}).map_err(NativeBindingRefusal::InvalidValue)?);", field.name())
            .expect("String writing is infallible");
    }
    writeln!(out, "        StructuredInfoValue::record(semantic, fields).map_err(NativeBindingRefusal::InvalidValue)\n    }}")
        .expect("String writing is infallible");
    writeln!(out, "    fn from_structured(value: StructuredInfoValue) -> Result<Self, NativeBindingRefusal> {{")
        .expect("String writing is infallible");
    exact_type_guard(out);
    writeln!(out, "        Self::new(").expect("String writing is infallible");
    for field in fields {
        let raw = format!(
            "conduit_form::rust_binding::record_field_value(&value, {:?})?",
            field.name()
        );
        let decoded = decode_expression(field.value_type(), &raw, names)?;
        writeln!(out, "            {decoded},").expect("String writing is infallible");
    }
    writeln!(out, "        )\n    }}\n}}\n").expect("String writing is infallible");
    Ok(())
}

pub(super) fn emit_variant_binding(
    out: &mut String,
    rust_name: &str,
    constant: &str,
    cases: &[conduit_core::StructuredVariantCase],
    names: &BTreeMap<String, String>,
) -> Result<(), RustBindingGenerationError> {
    trait_header(out, rust_name, constant);
    writeln!(out, "        match self {{").expect("String writing is infallible");
    for case in cases {
        let variant = rust_pascal_identifier(case.tag())?;
        if unit_type(case.payload_type()) {
            writeln!(out, "            Self::{variant} => {{")
                .expect("String writing is infallible");
            writeln!(out, "                let payload_type = conduit_form::rust_binding::variant_payload_type(&semantic, {:?})?;", case.tag())
                .expect("String writing is infallible");
            writeln!(out, "                let payload = conduit_form::rust_binding::primitive_into_structured(payload_type, &())?;")
                .expect("String writing is infallible");
            writeln!(out, "                StructuredInfoValue::variant(semantic, {:?}, payload).map_err(NativeBindingRefusal::InvalidValue)\n            }}", case.tag())
                .expect("String writing is infallible");
        } else {
            writeln!(out, "            Self::{variant}(payload) => {{")
                .expect("String writing is infallible");
            writeln!(out, "                let payload_type = conduit_form::rust_binding::variant_payload_type(&semantic, {:?})?;", case.tag())
                .expect("String writing is infallible");
            writeln!(out, "                let mut fields = Vec::new();")
                .expect("String writing is infallible");
            let StructuredInfoTypeShape::Record { fields, .. } = case.payload_type().shape() else {
                return Err(RustBindingGenerationError::InvalidSemanticType);
            };
            for field in fields {
                let rust_field = rust_snake_identifier(field.name())?;
                writeln!(out, "                let field_type = conduit_form::rust_binding::record_field_type(&payload_type, {:?})?;", field.name())
                    .expect("String writing is infallible");
                writeln!(out, "                let _ = &field_type;")
                    .expect("String writing is infallible");
                let encoded = encode_expression(
                    field.value_type(),
                    &format!("payload.{rust_field}"),
                    "field_type",
                    names,
                )?;
                writeln!(out, "                fields.push(StructuredFieldValue::new({:?}, {encoded}).map_err(NativeBindingRefusal::InvalidValue)?);", field.name())
                    .expect("String writing is infallible");
            }
            writeln!(out, "                let payload = StructuredInfoValue::record(payload_type, fields).map_err(NativeBindingRefusal::InvalidValue)?;")
                .expect("String writing is infallible");
            writeln!(out, "                StructuredInfoValue::variant(semantic, {:?}, payload).map_err(NativeBindingRefusal::InvalidValue)\n            }}", case.tag())
                .expect("String writing is infallible");
        }
    }
    writeln!(out, "        }}\n    }}").expect("String writing is infallible");
    writeln!(out, "    fn from_structured(value: StructuredInfoValue) -> Result<Self, NativeBindingRefusal> {{")
        .expect("String writing is infallible");
    exact_type_guard(out);
    let payload = if cases.iter().all(|case| unit_type(case.payload_type())) {
        "_payload"
    } else {
        "payload"
    };
    writeln!(out, "        let StructuredInfoValueShape::Variant {{ tag, payload: {payload} }} = value.shape() else {{ return Err(NativeBindingRefusal::InvalidValue(conduit_core::StructuredInfoRefusal::WrongType)); }};")
        .expect("String writing is infallible");
    writeln!(out, "        match tag {{").expect("String writing is infallible");
    for case in cases {
        let variant = rust_pascal_identifier(case.tag())?;
        if unit_type(case.payload_type()) {
            writeln!(out, "            {:?} => Ok(Self::{variant}),", case.tag())
                .expect("String writing is infallible");
        } else {
            let payload_name = format!("{rust_name}{variant}");
            let StructuredInfoTypeShape::Record { fields, .. } = case.payload_type().shape() else {
                return Err(RustBindingGenerationError::InvalidSemanticType);
            };
            writeln!(
                out,
                "            {:?} => Ok(Self::{variant}({payload_name} {{",
                case.tag()
            )
            .expect("String writing is infallible");
            for field in fields {
                let name = rust_snake_identifier(field.name())?;
                let raw = format!(
                    "conduit_form::rust_binding::record_field_value(payload, {:?})?",
                    field.name()
                );
                let decoded = decode_expression(field.value_type(), &raw, names)?;
                writeln!(out, "                {name}: {decoded},")
                    .expect("String writing is infallible");
            }
            writeln!(out, "            }})),").expect("String writing is infallible");
        }
    }
    writeln!(out, "            _ => Err(NativeBindingRefusal::InvalidValue(conduit_core::StructuredInfoRefusal::UnknownVariantTag)),\n        }}\n    }}\n}}\n")
        .expect("String writing is infallible");
    Ok(())
}

fn trait_header(out: &mut String, rust_name: &str, constant: &str) {
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
}

fn exact_type_guard(out: &mut String) {
    writeln!(out, "        if value.value_type() != &Self::semantic_type()? {{ return Err(NativeBindingRefusal::InvalidValue(conduit_core::StructuredInfoRefusal::WrongType)); }}")
        .expect("String writing is infallible");
}

fn encode_expression(
    value_type: &StructuredInfoType,
    value: &str,
    expected: &str,
    names: &BTreeMap<String, String>,
) -> Result<String, RustBindingGenerationError> {
    match value_type.shape() {
        StructuredInfoTypeShape::Nominal { schema, .. }
        | StructuredInfoTypeShape::Record { schema, .. }
        | StructuredInfoTypeShape::Variant { schema, .. }
            if names.contains_key(schema.as_str()) =>
        {
            Ok(format!("NativeRustBinding::into_structured({value})?"))
        }
        StructuredInfoTypeShape::Leaf(_) => Ok(format!(
            "conduit_form::rust_binding::primitive_into_structured({expected}, &{value})?"
        )),
        StructuredInfoTypeShape::Sequence { element, .. } => {
            let inner = encode_expression(element, "item", "element_type.clone()", names)?;
            Ok(format!(
                "{{ let element_type = conduit_form::rust_binding::sequence_element_type(&{expected})?; let _ = &element_type; let mut values = Vec::new(); for item in {value} {{ values.push({inner}); }} StructuredInfoValue::sequence({expected}, values).map_err(NativeBindingRefusal::InvalidValue)? }}"
            ))
        }
        StructuredInfoTypeShape::Variant { schema, cases }
            if schema.as_str() == "conduit.conduitese.optional.v1" =>
        {
            let some = cases
                .iter()
                .find(|case| case.tag() == "some")
                .ok_or(RustBindingGenerationError::InvalidSemanticType)?;
            let inner = encode_expression(some.payload_type(), "item", "payload_type", names)?;
            Ok(format!(
                "match {value} {{ Some(item) => {{ let payload_type = conduit_form::rust_binding::variant_payload_type(&{expected}, \"some\")?; let _ = &payload_type; let payload = {inner}; StructuredInfoValue::variant({expected}, \"some\", payload).map_err(NativeBindingRefusal::InvalidValue)? }}, None => {{ let payload_type = conduit_form::rust_binding::variant_payload_type(&{expected}, \"none\")?; let payload = conduit_form::rust_binding::primitive_into_structured(payload_type, &())?; StructuredInfoValue::variant({expected}, \"none\", payload).map_err(NativeBindingRefusal::InvalidValue)? }} }}"
            ))
        }
        _ => Err(RustBindingGenerationError::InvalidSemanticType),
    }
}

fn decode_expression(
    value_type: &StructuredInfoType,
    value: &str,
    names: &BTreeMap<String, String>,
) -> Result<String, RustBindingGenerationError> {
    match value_type.shape() {
        StructuredInfoTypeShape::Nominal { schema, .. }
        | StructuredInfoTypeShape::Record { schema, .. }
        | StructuredInfoTypeShape::Variant { schema, .. }
            if names.contains_key(schema.as_str()) =>
        {
            Ok(format!(
                "{}::from_structured({value})?",
                names[schema.as_str()]
            ))
        }
        StructuredInfoTypeShape::Leaf(kind) => Ok(format!(
            "conduit_form::rust_binding::primitive_from_structured::<{}>(&{value})?",
            primitive_rust_type(kind.as_str())?
        )),
        StructuredInfoTypeShape::Sequence {
            element,
            maximum_items,
            ..
        } => {
            let decoded = decode_expression(element, "item.clone()", names)?;
            Ok(format!(
                "{{ let sequence_value = {value}; let StructuredInfoValueShape::Collection(items) = sequence_value.shape() else {{ return Err(NativeBindingRefusal::InvalidValue(conduit_core::StructuredInfoRefusal::WrongType)); }}; let mut result = BoundedSequence::<{}, {maximum_items}>::new(); for item in items {{ result.push({decoded}).map_err(|_| NativeBindingRefusal::InvalidValue(conduit_core::StructuredInfoRefusal::WrongCollectionLength))?; }} result }}",
                rust_type(element, names)?
            ))
        }
        StructuredInfoTypeShape::Variant { schema, cases }
            if schema.as_str() == "conduit.conduitese.optional.v1" =>
        {
            let some = cases
                .iter()
                .find(|case| case.tag() == "some")
                .ok_or(RustBindingGenerationError::InvalidSemanticType)?;
            let decoded = decode_expression(some.payload_type(), "payload.clone()", names)?;
            Ok(format!(
                "{{ let optional_value = {value}; let StructuredInfoValueShape::Variant {{ tag, payload }} = optional_value.shape() else {{ return Err(NativeBindingRefusal::InvalidValue(conduit_core::StructuredInfoRefusal::WrongType)); }}; match tag {{ \"none\" => None, \"some\" => Some({decoded}), _ => return Err(NativeBindingRefusal::InvalidValue(conduit_core::StructuredInfoRefusal::UnknownVariantTag)) }} }}"
            ))
        }
        _ => Err(RustBindingGenerationError::InvalidSemanticType),
    }
}
