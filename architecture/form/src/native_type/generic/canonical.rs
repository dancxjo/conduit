use crate::prelude::*;
use crate::{TypeExpressionSyntax, ValueRefinement};
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
            arguments
                .iter()
                .map(self::expression)
                .collect::<Vec<_>>()
                .join(","),
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
        } => alloc::format!("collection({length},{})", self::expression(element)),
        TypeExpressionSyntax::Sequence {
            element,
            minimum_items,
            maximum_items,
            ..
        } => alloc::format!(
            "sequence({minimum_items},{maximum_items},{})",
            self::expression(element)
        ),
    }
}

pub(super) fn application_key(name: &str, arguments: &[TypeExpressionSyntax]) -> String {
    alloc::format!(
        "{name}<{}>",
        arguments
            .iter()
            .map(expression)
            .collect::<Vec<_>>()
            .join(",")
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
