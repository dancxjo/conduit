use crate::prelude::*;
use crate::{NativeTypeArgumentSyntax, TypeExpressionSyntax, ValueRefinement};
use sha2::{Digest, Sha256};

pub(super) fn expression(expression: &TypeExpressionSyntax) -> String {
    match expression {
        TypeExpressionSyntax::Reference {
            value_type,
            arguments,
            maximum_bytes,
            refinements,
            ..
        } => alloc::format!(
            "ref:{}<{}>:{}:{}",
            value_type.text,
            arguments.iter().map(argument).collect::<Vec<_>>().join(","),
            maximum_bytes.map_or_else(|| "-".into(), |value| value.to_string()),
            refinements_identity(refinements)
        ),
        TypeExpressionSyntax::Optional { value, .. } => {
            alloc::format!("optional({})", self::expression(value))
        }
        TypeExpressionSyntax::DataReference { value, .. } => {
            alloc::format!("data({})", self::expression(value))
        }
        TypeExpressionSyntax::Collection {
            element, length, ..
        } => alloc::format!(
            "collection({},{})",
            super::integer::canonical(length),
            self::expression(element)
        ),
        TypeExpressionSyntax::Sequence {
            element,
            minimum_items,
            maximum_items,
            ..
        } => alloc::format!(
            "sequence({},{},{})",
            super::integer::canonical(minimum_items),
            super::integer::canonical(maximum_items),
            self::expression(element)
        ),
    }
}

pub(super) fn argument(value: &NativeTypeArgumentSyntax) -> String {
    match value {
        NativeTypeArgumentSyntax::Type(value) => expression(value),
        NativeTypeArgumentSyntax::Value(value) => {
            alloc::format!("info:{}", super::integer::canonical(value))
        }
    }
}

pub(super) fn application_key(name: &str, arguments: &[NativeTypeArgumentSyntax]) -> String {
    alloc::format!(
        "{name}<{}>",
        arguments.iter().map(argument).collect::<Vec<_>>().join(",")
    )
}

pub(super) fn instantiated_name(template: &str, key: &str) -> String {
    let digest = Sha256::digest(key.as_bytes());
    let mut suffix = String::with_capacity(16);
    for byte in &digest[..8] {
        use core::fmt::Write;
        write!(&mut suffix, "{byte:02x}").expect("writing to String cannot fail");
    }
    alloc::format!("{template}Instantiation{suffix}")
}

fn refinements_identity(refinements: &[ValueRefinement]) -> String {
    refinements
        .iter()
        .map(|refinement| match refinement {
            ValueRefinement::Finite { .. } => "finite".into(),
            ValueRefinement::TextPattern {
                source,
                case_insensitive,
                anchored_start,
                anchored_end,
                negated,
                ..
            } => alloc::format!(
                "pattern:{:?}:{case_insensitive}:{anchored_start}:{anchored_end}:{negated}",
                source.text
            ),
            ValueRefinement::Range {
                minimum,
                maximum,
                minimum_endpoint,
                maximum_endpoint,
                ..
            } => alloc::format!(
                "range:{:?}:{minimum_endpoint:?}:{:?}:{maximum_endpoint:?}",
                minimum.as_ref().map(|value| &value.text),
                maximum.as_ref().map(|value| &value.text)
            ),
            ValueRefinement::Membership {
                members, negated, ..
            } => alloc::format!(
                "members:{negated}:{}",
                members
                    .iter()
                    .map(|member| alloc::format!("{:?}", member.text))
                    .collect::<Vec<_>>()
                    .join(",")
            ),
        })
        .collect::<Vec<_>>()
        .join(";")
}

/// Value-family identity retains the originating declaration's semantic version.
/// Existing Type-only families keep their established identity contract.
pub(super) fn family_key(
    template: &crate::TypeSyntax,
    arguments: &[NativeTypeArgumentSyntax],
) -> String {
    let application = application_key(&template.name.text, arguments);
    if template
        .parameters
        .iter()
        .all(|parameter| parameter.value_type.is_none())
    {
        return application;
    }
    let mut meaning = String::from("conduit.native-info-family@1\0");
    for parameter in &template.parameters {
        meaning.push_str(&alloc::format!(
            "{:?}:{};",
            parameter.name.text,
            parameter
                .value_type
                .as_ref()
                .map_or_else(|| "Type".into(), |value| expression(value))
        ));
    }
    meaning.push_str(&definition(&template.definition));
    let mut laws = template
        .invariants
        .iter()
        .map(|law| crate::syntax_identity::canonical_expression(&law.syntax))
        .collect::<Vec<_>>();
    laws.sort();
    for law in laws {
        meaning.push_str(&alloc::format!("law:{law:?}"));
    }
    let digest = Sha256::digest(meaning.as_bytes());
    let mut encoded = String::new();
    for byte in digest {
        use core::fmt::Write;
        write!(&mut encoded, "{byte:02x}").expect("writing to String cannot fail");
    }
    alloc::format!("{application};family={encoded}")
}

fn definition(value: &crate::TypeDefinitionSyntax) -> String {
    match value {
        crate::TypeDefinitionSyntax::Scalar(value) => {
            alloc::format!("scalar:{}", expression(value))
        }
        crate::TypeDefinitionSyntax::Record(fields) => fields_identity(fields),
        crate::TypeDefinitionSyntax::Variant(cases) => cases
            .iter()
            .map(|case| {
                let payload = match &case.payload {
                    crate::TypeVariantPayloadSyntax::Unit => "unit".into(),
                    crate::TypeVariantPayloadSyntax::Type(value) => expression(value),
                    crate::TypeVariantPayloadSyntax::Record(fields) => fields_identity(fields),
                };
                alloc::format!("case:{:?}:{payload:?};", case.tag.text)
            })
            .collect::<Vec<_>>()
            .join(""),
    }
}

fn fields_identity(fields: &[crate::TypeFieldSyntax]) -> String {
    fields
        .iter()
        .map(|field| {
            alloc::format!(
                "field:{:?}:{:?};",
                field.name.text,
                expression(&field.value_type)
            )
        })
        .collect::<Vec<_>>()
        .join("")
}
