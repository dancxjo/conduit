#![cfg(feature = "semantic-bindings")]
use conduit_core::{ConfigurationValue, StructuredInfoType, StructuredInfoTypeShape as Shape};
use conduit_plot::*;
use std::collections::BTreeSet;

fn types(ty: &StructuredInfoType, all: &mut BTreeSet<StructuredInfoType>) {
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
fn nodes(node: &PortableExpressionNode, all: &mut BTreeSet<StructuredInfoType>) {
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
#[test]
fn all_four_g2_program_native_owners_have_complete_replayable_custody() {
    let speech = conduit_speech::authoring::checked_document_with_law_custody().unwrap();
    let audio = conduit_audio::authoring::checked_document_with_law_custody().unwrap();
    let expected = conduit_speech::authoring::checked_types().unwrap();
    assert_eq!(speech.native_types, expected);
    let mut owners = expected;
    owners.extend(conduit_audio::authoring::checked_types().unwrap());
    let mut found = BTreeSet::new();
    for name in ["advance", "phases", "curve", "energy"] {
        let expanded = expand_canonical_plot_for_authoring(
            &speech,
            &format!("speech/phone-gesture-{name}"),
            &ProfileCatalog::new(),
        )
        .unwrap();
        let [gear] = expanded.expanded.gears.as_slice() else {
            panic!("one expression")
        };
        let [entry] = gear.configuration.as_slice() else {
            panic!("one program")
        };
        let ConfigurationValue::Text(encoded) = &entry.value else {
            panic!("canonical program")
        };
        let program = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
        let mut all = BTreeSet::new();
        types(&program.input_type, &mut all);
        types(&program.output_type, &mut all);
        nodes(&program.root, &mut all);
        for ty in all {
            if let Some(expected) = owners.iter().find(|owner| owner.value_type == ty) {
                if !found.insert(expected.name.clone()) {
                    continue;
                }
                let witness = speech
                    .native_law_witness(&expected.name)
                    .or_else(|| audio.native_law_witness(&expected.name))
                    .unwrap_or_else(|| panic!("missing owner {}", expected.name));
                assert_eq!(witness.checked_type(), expected);
                let anchors = witness.family_anchors().unwrap();
                let replay = NativeLawWitness::verify_with_context(
                    &witness.material().unwrap(),
                    &expected.value_type,
                    &NativeLawContext { families: &anchors },
                )
                .unwrap();
                assert_eq!(replay.checked_type(), expected);
            } else if let Shape::Nominal { schema, .. }
            | Shape::Record { schema, .. }
            | Shape::Variant { schema, .. } = ty.shape()
            {
                assert!(
                    !schema.as_str().starts_with("type/"),
                    "unowned Native {}",
                    schema.as_str()
                );
            }
        }
    }
    assert!(found.contains("SpeechPhoneGestureTiming"));
    assert!(found.contains("AudioQuantityTrajectory"));
}
#[test]
fn requested_roots_are_transactional_and_cannot_upgrade_forged_metadata() {
    let mut catalog = StartupCatalog::new();
    let before = catalog.clone();
    assert!(conduit_speech::authoring::install_law_custody_for(
        &mut catalog,
        &["SpeechPhoneGestureTiming", "Absent"]
    )
    .is_err());
    assert_eq!(catalog, before);
    let doc = conduit_speech::authoring::checked_document_with_law_custody().unwrap();
    let mut forged = doc
        .native_law_witness("SpeechPhoneGestureTiming")
        .unwrap()
        .checked_type()
        .clone();
    forged.invariants.clear();
    catalog
        .insert_checked_native_type("SpeechPhoneGestureTiming", &forged)
        .unwrap();
    let before = catalog.clone();
    assert!(conduit_speech::authoring::install_law_custody_for(
        &mut catalog,
        &["SpeechPhoneGestureTiming"]
    )
    .is_err());
    assert_eq!(catalog, before);
}
