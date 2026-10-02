use crate::prelude::*;
use alloc::collections::BTreeMap;
use conduit_core::{StructuredInfoType, StructuredInfoTypeShape};
use core::fmt::Write;

use super::generate::{
    data_reference_content_kind, primitive_rust_type, rust_pascal_identifier,
    rust_snake_identifier, rust_type, unit_type, RustBindingGenerationError,
};

pub(super) fn emit_record_binding(
    out: &mut String,
    rust_name: &str,
    constant: &str,
    fields: &[conduit_core::StructuredFieldType],
    names: &BTreeMap<String, String>,
    constructor_order: Option<&[String]>,
    constructor_name: &str,
) -> Result<(), RustBindingGenerationError> {
    trait_header(out, rust_name, constant);
    writeln!(out, "        let mut fields = Vec::new();").expect("String writing is infallible");
    for field in fields {
        let rust_field = rust_snake_identifier(field.name())?;
        writeln!(out, "        let field_type = conduit_plot::rust_binding::record_field_type(&semantic, {:?})?;", field.name())
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
    writeln!(out, "        Self::{constructor_name}(").expect("String writing is infallible");
    let constructor_fields = constructor_order
        .map(|order| {
            order
                .iter()
                .map(|name| {
                    fields
                        .iter()
                        .find(|field| field.name() == name)
                        .ok_or(RustBindingGenerationError::InvalidSemanticType)
                })
                .collect::<Result<Vec<_>, _>>()
        })
        .transpose()?
        .unwrap_or_else(|| fields.iter().collect());
    for field in constructor_fields {
        let raw = format!(
            "conduit_plot::rust_binding::record_field_value(&value, {:?})?",
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
    authored_type_name: &str,
    boxed_variant_payloads: &alloc::collections::BTreeSet<String>,
) -> Result<(), RustBindingGenerationError> {
    trait_header(out, rust_name, constant);
    writeln!(out, "        match self {{").expect("String writing is infallible");
    for case in cases {
        let variant = rust_pascal_identifier(case.tag())?;
        if unit_type(case.payload_type()) {
            writeln!(out, "            Self::{variant} => {{")
                .expect("String writing is infallible");
            writeln!(out, "                let payload_type = conduit_plot::rust_binding::variant_payload_type(&semantic, {:?})?;", case.tag())
                .expect("String writing is infallible");
            writeln!(out, "                let payload = conduit_plot::rust_binding::primitive_into_structured(payload_type, &())?;")
                .expect("String writing is infallible");
            writeln!(out, "                StructuredInfoValue::variant(semantic, {:?}, payload).map_err(NativeBindingRefusal::InvalidValue)\n            }}", case.tag())
                .expect("String writing is infallible");
        } else {
            writeln!(out, "            Self::{variant}(payload) => {{")
                .expect("String writing is infallible");
            writeln!(out, "                let payload_type = conduit_plot::rust_binding::variant_payload_type(&semantic, {:?})?;", case.tag())
                .expect("String writing is infallible");
            if !matches!(
                case.payload_type().shape(),
                StructuredInfoTypeShape::Record { .. }
            ) {
                let encoded =
                    encode_expression(case.payload_type(), "payload", "payload_type", names)?;
                writeln!(out, "                let _ = &payload_type;")
                    .expect("String writing is infallible");
                writeln!(out, "                let payload = {encoded};")
                    .expect("String writing is infallible");
                writeln!(out, "                StructuredInfoValue::variant(semantic, {:?}, payload).map_err(NativeBindingRefusal::InvalidValue)\n            }}", case.tag())
                    .expect("String writing is infallible");
                continue;
            }
            writeln!(out, "                let mut fields = Vec::new();")
                .expect("String writing is infallible");
            let StructuredInfoTypeShape::Record { fields, .. } = case.payload_type().shape() else {
                return Err(RustBindingGenerationError::InvalidSemanticType);
            };
            for field in fields {
                let rust_field = rust_snake_identifier(field.name())?;
                writeln!(out, "                let field_type = conduit_plot::rust_binding::record_field_type(&payload_type, {:?})?;", field.name())
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
    let payload_pattern = if cases.iter().all(|case| unit_type(case.payload_type())) {
        "payload: _payload"
    } else {
        "payload"
    };
    writeln!(out, "        let StructuredInfoValueShape::Variant {{ tag, {payload_pattern} }} = value.shape() else {{ return Err(NativeBindingRefusal::InvalidValue(conduit_core::StructuredInfoRefusal::WrongType)); }};")
        .expect("String writing is infallible");
    writeln!(out, "        match tag {{").expect("String writing is infallible");
    for case in cases {
        let variant = rust_pascal_identifier(case.tag())?;
        let boxed =
            boxed_variant_payloads.contains(&format!("{authored_type_name}.{}", case.tag()));
        if unit_type(case.payload_type()) {
            writeln!(out, "            {:?} => Ok(Self::{variant}),", case.tag())
                .expect("String writing is infallible");
        } else {
            if !matches!(
                case.payload_type().shape(),
                StructuredInfoTypeShape::Record { .. }
            ) {
                let decoded = decode_expression(case.payload_type(), "payload.clone()", names)?;
                writeln!(
                    out,
                    "            {:?} => Ok(Self::{variant}({}{decoded}{})),",
                    case.tag(),
                    if boxed { "Box::new(" } else { "" },
                    if boxed { ")" } else { "" },
                )
                .expect("String writing is infallible");
                continue;
            }
            let payload_name = format!("{rust_name}{variant}");
            let StructuredInfoTypeShape::Record { fields, .. } = case.payload_type().shape() else {
                return Err(RustBindingGenerationError::InvalidSemanticType);
            };
            writeln!(
                out,
                "            {:?} => Ok(Self::{variant}({}{payload_name} {{",
                case.tag(),
                if boxed { "Box::new(" } else { "" },
            )
            .expect("String writing is infallible");
            for field in fields {
                let name = rust_snake_identifier(field.name())?;
                let raw = format!(
                    "conduit_plot::rust_binding::record_field_value(payload, {:?})?",
                    field.name()
                );
                let decoded = decode_expression(field.value_type(), &raw, names)?;
                writeln!(out, "                {name}: {decoded},")
                    .expect("String writing is infallible");
            }
            writeln!(out, "            }}{})),", if boxed { ")" } else { "" })
                .expect("String writing is infallible");
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

pub(super) fn encode_expression(
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
        StructuredInfoTypeShape::Leaf(kind) => {
            if let Some(content_kind) = data_reference_content_kind(kind.as_str()) {
                return Ok(format!(
                    "{{ {value}.validate_for(&conduit_core::KindId::from({content_kind:?})).map_err(|_| NativeBindingRefusal::InvalidValue(conduit_core::StructuredInfoRefusal::WrongType))?; let encoded = {value}.encode().map_err(|_| NativeBindingRefusal::InvalidValue(conduit_core::StructuredInfoRefusal::WrongType))?; StructuredInfoValue::leaf({expected}, encoded).map_err(NativeBindingRefusal::InvalidValue)? }}"
                ));
            }
            if kind.as_str() == conduit_core::RESOURCE_REFERENCE_INFO_ID {
                return Ok(format!(
                    "{{ {value}.validate().map_err(|_| NativeBindingRefusal::InvalidValue(conduit_core::StructuredInfoRefusal::WrongType))?; let encoded = {value}.encode().map_err(|_| NativeBindingRefusal::InvalidValue(conduit_core::StructuredInfoRefusal::WrongType))?; StructuredInfoValue::leaf({expected}, encoded).map_err(NativeBindingRefusal::InvalidValue)? }}"
                ));
            }
            Ok(format!(
                "conduit_plot::rust_binding::primitive_into_structured({expected}, &{value})?"
            ))
        }
        StructuredInfoTypeShape::Sequence { element, .. } => {
            let inner = encode_expression(element, "item", "element_type.clone()", names)?;
            Ok(format!(
                "{{ let sequence_type = {expected}; let element_type = conduit_plot::rust_binding::sequence_element_type(&sequence_type)?; let _ = &element_type; let mut values = Vec::new(); for item in {value} {{ values.push({inner}); }} StructuredInfoValue::sequence(sequence_type, values).map_err(NativeBindingRefusal::InvalidValue)? }}"
            ))
        }
        StructuredInfoTypeShape::Collection { element, .. } => {
            let inner = encode_expression(element, "item", "element_type.clone()", names)?;
            Ok(format!(
                "{{ let collection_type = {expected}; let element_type = conduit_plot::rust_binding::collection_element_type(&collection_type)?; let _ = &element_type; let mut values = Vec::new(); for item in {value} {{ values.push({inner}); }} StructuredInfoValue::collection(collection_type, values).map_err(NativeBindingRefusal::InvalidValue)? }}"
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
                "match {value} {{ Some(item) => {{ let payload_type = conduit_plot::rust_binding::variant_payload_type(&{expected}, \"some\")?; let _ = &payload_type; let payload = {inner}; StructuredInfoValue::variant({expected}, \"some\", payload).map_err(NativeBindingRefusal::InvalidValue)? }}, None => {{ let payload_type = conduit_plot::rust_binding::variant_payload_type(&{expected}, \"none\")?; let payload = conduit_plot::rust_binding::primitive_into_structured(payload_type, &())?; StructuredInfoValue::variant({expected}, \"none\", payload).map_err(NativeBindingRefusal::InvalidValue)? }} }}"
            ))
        }
        _ => Err(RustBindingGenerationError::InvalidSemanticType),
    }
}

pub(super) fn decode_expression(
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
        StructuredInfoTypeShape::Leaf(kind) => {
            if let Some(content_kind) = data_reference_content_kind(kind.as_str()) {
                return Ok(format!(
                    "{{ let reference_value = {value}; let StructuredInfoValueShape::Leaf(encoded) = reference_value.shape() else {{ return Err(NativeBindingRefusal::InvalidValue(conduit_core::StructuredInfoRefusal::WrongType)); }}; conduit_data::DataReference::decode_for(&conduit_core::KindId::from({content_kind:?}), encoded).map_err(|_| NativeBindingRefusal::InvalidValue(conduit_core::StructuredInfoRefusal::WrongType))? }}"
                ));
            }
            if kind.as_str() == conduit_core::RESOURCE_REFERENCE_INFO_ID {
                return Ok(format!(
                    "{{ let reference_value = {value}; let StructuredInfoValueShape::Leaf(encoded) = reference_value.shape() else {{ return Err(NativeBindingRefusal::InvalidValue(conduit_core::StructuredInfoRefusal::WrongType)); }}; conduit_core::BoundedResourceRef::decode(encoded).map_err(|_| NativeBindingRefusal::InvalidValue(conduit_core::StructuredInfoRefusal::WrongType))? }}"
                ));
            }
            Ok(format!(
                "conduit_plot::rust_binding::primitive_from_structured::<{}>(&{value})?",
                primitive_rust_type(kind.as_str())?
            ))
        }
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
        StructuredInfoTypeShape::Collection { element, length } => {
            let decoded = decode_expression(element, "item.clone()", names)?;
            Ok(format!(
                "{{ let collection_value = {value}; let StructuredInfoValueShape::Collection(items) = collection_value.shape() else {{ return Err(NativeBindingRefusal::InvalidValue(conduit_core::StructuredInfoRefusal::WrongType)); }}; let mut decoded_values = Vec::new(); for item in items {{ decoded_values.push({decoded}); }} let result: [{}; {length}] = decoded_values.try_into().map_err(|_| NativeBindingRefusal::InvalidValue(conduit_core::StructuredInfoRefusal::WrongCollectionLength))?; result }}",
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
