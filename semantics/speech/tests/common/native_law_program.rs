//! Shared exact program/type inspection for Speech custody tests.
use conduit_core::{ConfigurationValue, StructuredInfoType, StructuredInfoTypeShape as Shape};
use conduit_plot::*;
use std::collections::BTreeSet;

pub fn types(ty: &StructuredInfoType, all: &mut BTreeSet<StructuredInfoType>) {
    all.insert(ty.clone());
    match ty.shape() {
        Shape::Leaf(_) => (),
        Shape::Nominal { representation, .. } => types(representation, all),
        Shape::Collection { element, .. } | Shape::Sequence { element, .. } => types(element, all),
        Shape::Record { fields, .. } => {
            for field in fields {
                types(field.value_type(), all);
            }
        }
        Shape::Variant { cases, .. } => {
            for case in cases {
                types(case.payload_type(), all);
            }
        }
    }
}
pub fn nodes(node: &PortableExpressionNode, all: &mut BTreeSet<StructuredInfoType>) {
    use PortableExpressionOperation as Op;
    types(&node.value_type, all);
    match &node.operation {
        Op::ValueContract { value, .. }
        | Op::Projection { value, .. }
        | Op::Variant { payload: value, .. } => nodes(value, all),
        Op::Unary { operand, .. } => nodes(operand, all),
        Op::Binary { left, right, .. } => {
            nodes(left, all);
            nodes(right, all);
        }
        Op::Conditional {
            condition,
            when_true,
            when_false,
        } => {
            nodes(condition, all);
            nodes(when_true, all);
            nodes(when_false, all);
        }
        Op::Tuple(values)
        | Op::Collection(values)
        | Op::SemanticCall {
            arguments: values, ..
        } => {
            for value in values {
                nodes(value, all);
            }
        }
        Op::Record(fields) => {
            for (_, value) in fields {
                nodes(value, all);
            }
        }
        Op::Input | Op::Literal(_) | Op::CanonicalLiteral(_) | Op::Constant(_) => (),
    }
}
// Unknown authored schemas never receive a naming-based exemption. Rebuild
// only exact public built-in structural identities, retaining all exact members.
pub fn assert_builtin_structure(ty: &StructuredInfoType) {
    match ty.shape() {
        Shape::Leaf(_) | Shape::Collection { .. } | Shape::Sequence { .. } => (),
        Shape::Nominal { schema, .. } => panic!("unowned Nominal {schema:?}"),
        Shape::Variant { cases, .. } => {
            let some = cases
                .iter()
                .find(|case| case.tag() == "some")
                .expect("Core optional some");
            assert_eq!(
                *ty,
                conduit_core::optional_info_type(some.payload_type().clone()).unwrap()
            );
        }
        Shape::Record { fields, .. } => {
            let tuple = conduit_core::tuple_info_type(
                fields
                    .iter()
                    .map(|field| field.value_type().clone())
                    .collect(),
            )
            .unwrap();
            if &tuple == ty {
                return;
            }
            let shape = CheckedExpressionType::Record(
                fields
                    .iter()
                    .map(|field| {
                        (
                            field.name().to_owned(),
                            CheckedExpressionType::Semantic(
                                field.value_type().profile().unwrap().value_kind().clone(),
                            ),
                        )
                    })
                    .collect(),
            )
            .structured_info_type()
            .unwrap();
            let Shape::Record { schema, .. } = shape.shape() else {
                unreachable!()
            };
            assert_eq!(
                *ty,
                StructuredInfoType::record(schema.clone(), fields.to_vec()).unwrap()
            );
        }
    }
}
pub fn program(document: &CheckedSyntaxDocument, name: &str) -> String {
    let expanded =
        expand_canonical_plot_for_authoring(document, name, &ProfileCatalog::new()).unwrap();
    let [gear] = expanded.expanded.gears.as_slice() else {
        panic!("one expression")
    };
    let [entry] = gear.configuration.as_slice() else {
        panic!("one program")
    };
    let ConfigurationValue::Text(encoded) = &entry.value else {
        panic!("canonical program")
    };
    encoded.clone()
}
