//! Declaration custody is preparation evidence, not permission to accept values.
use conduit_plot::*;

fn checked(source: &str, catalog: &StartupCatalog) -> CheckedSyntaxDocument {
    check_syntax_document(&parse_syntax_document(source), catalog).unwrap()
}
const SOURCE: &str = "type CustodyDuration = {\n denominator: U64\n numerator: U64\n where .denominator > 0\n where .numerator > 0\n}\ntype Timing = {\n start: CustodyDuration\n duration: CustodyDuration\n where .start.denominator == .duration.denominator\n where .start.numerator + .duration.numerator <= 65535\n}\n";
#[test]
fn exact_local_nested_declarations_round_trip_without_changing_schema_or_laws() {
    let doc = checked(SOURCE, &StartupCatalog::new());
    for ty in &doc.native_types {
        let witness = doc.native_law_witness(&ty.name).unwrap();
        assert_eq!(witness.checked_type(), ty);
        let verified =
            NativeLawWitness::verify(&witness.material().unwrap(), &ty.value_type).unwrap();
        assert_eq!(verified.checked_type(), ty);
    }
}
#[test]
fn missing_substituted_reordered_and_duplicate_claimed_laws_are_not_self_certifying() {
    let doc = checked(SOURCE, &StartupCatalog::new());
    let witness = doc.native_law_witness("Timing").unwrap();
    let expected = &witness.checked_type().value_type;
    let original = witness.material().unwrap();
    let mut omitted = original.clone();
    omitted.claimed.invariants.clear();
    assert!(matches!(
        NativeLawWitness::verify(&omitted, expected),
        Err(NativeLawCustodyRefusal::Laws)
    ));
    let mut replaced = original.clone();
    replaced.claimed.invariants[0] = doc
        .native_law_witness("CustodyDuration")
        .unwrap()
        .checked_type()
        .invariants[0]
        .clone();
    assert!(NativeLawWitness::verify(&replaced, expected).is_err());
    let mut reordered = original.clone();
    reordered.claimed.invariants.swap(0, 1);
    assert!(NativeLawWitness::verify(&reordered, expected).is_err());
    let mut duplicate = original.clone();
    duplicate
        .claimed
        .invariants
        .push(duplicate.claimed.invariants[0].clone());
    assert!(NativeLawWitness::verify(&duplicate, expected).is_err());
    let mut erased_declaration = original.clone();
    erased_declaration.source =
        SOURCE.replace(" where .start.denominator == .duration.denominator\n", "");
    assert!(matches!(
        NativeLawWitness::verify(&erased_declaration, expected),
        Err(NativeLawCustodyRefusal::Type)
    ));
}
#[test]
fn actual_native_import_alias_keeps_declaration_identity_and_requires_dependency() {
    let owner = checked(SOURCE, &StartupCatalog::new());
    let mut catalog = StartupCatalog::new();
    catalog
        .insert_native_law_witness("owner/Timing", owner.native_law_witness("Timing").unwrap())
        .unwrap();
    let consumer = checked(
        "with owner/Timing as Imported\ntype Holder = {\n value: Imported\n}\n",
        &catalog,
    );
    let witness = consumer.native_law_witness("Holder").unwrap();
    let original = witness.material().unwrap();
    assert!(NativeLawWitness::verify(&original, &witness.checked_type().value_type).is_ok());
    let mut missing = original.clone();
    missing.imports.clear();
    assert!(NativeLawWitness::verify(&missing, &witness.checked_type().value_type).is_err());
    let mut wrong_name = original;
    wrong_name.imports[0].0 = "foreign/Timing".into();
    assert!(NativeLawWitness::verify(&wrong_name, &witness.checked_type().value_type).is_err());
}
#[test]
fn real_generic_substitution_is_replayed_and_wrong_context_does_not_keep_identity() {
    let source = "type Box<T> = {\n value: T\n}\ntype Concrete = Box<U64>\n";
    let doc = checked(source, &StartupCatalog::new());
    let witness = doc.native_law_witness("Concrete").unwrap();
    let mut material = witness.material().unwrap();
    assert!(NativeLawWitness::verify(&material, &witness.checked_type().value_type).is_ok());
    material.source = source.replace("Box<U64>", "Box<U32>");
    assert!(NativeLawWitness::verify(&material, &witness.checked_type().value_type).is_err());
}
#[test]
fn refused_registration_is_transactional_and_legacy_type_does_not_gain_custody() {
    let doc = checked(SOURCE, &StartupCatalog::new());
    let ty = &doc.native_types[0];
    let mut catalog = StartupCatalog::new();
    catalog.insert_checked_native_type("legacy", ty).unwrap();
    assert!(catalog.native_law_witness("legacy").is_none());
    let before = catalog.clone();
    assert!(catalog
        .insert_native_law_witness("legacy", doc.native_law_witness(&ty.name).unwrap())
        .is_err());
    assert_eq!(before, catalog);
    let mut too_large = doc
        .native_law_witness(&ty.name)
        .unwrap()
        .material()
        .unwrap();
    too_large.source = " ".repeat(4 * 1024 * 1024 + 1);
    assert!(matches!(
        NativeLawWitness::verify(&too_large, &ty.value_type),
        Err(NativeLawCustodyRefusal::Capacity)
    ));
}

#[test]
fn retained_cross_field_predicate_still_identifies_invalid_timing_input() {
    use conduit_core::{StructuredFieldValue, StructuredInfoType, StructuredInfoValue};
    use conduit_plot::rust_binding::primitive_into_structured;
    let doc = checked(SOURCE, &StartupCatalog::new());
    let duration = doc
        .native_law_witness("CustodyDuration")
        .unwrap()
        .checked_type();
    let timing = doc.native_law_witness("Timing").unwrap().checked_type();
    let u64_type = StructuredInfoType::leaf(conduit_core::kind_id("value/u64")).unwrap();
    let value = |denominator: u64| {
        StructuredInfoValue::record(
            duration.value_type.clone(),
            vec![
                StructuredFieldValue::new(
                    "denominator",
                    primitive_into_structured(u64_type.clone(), &denominator).unwrap(),
                )
                .unwrap(),
                StructuredFieldValue::new(
                    "numerator",
                    primitive_into_structured(u64_type.clone(), &1u64).unwrap(),
                )
                .unwrap(),
            ],
        )
        .unwrap()
    };
    let valid = StructuredInfoValue::record(
        timing.value_type.clone(),
        vec![
            StructuredFieldValue::new("start", value(30)).unwrap(),
            StructuredFieldValue::new("duration", value(30)).unwrap(),
        ],
    )
    .unwrap();
    let invalid = StructuredInfoValue::record(
        timing.value_type.clone(),
        vec![
            StructuredFieldValue::new("start", value(30)).unwrap(),
            StructuredFieldValue::new("duration", value(60)).unwrap(),
        ],
    )
    .unwrap();
    assert!(
        conduit_plot::rust_binding::validate_native_invariants(&valid, &timing.invariants).is_ok()
    );
    assert!(
        conduit_plot::rust_binding::validate_native_invariants(&invalid, &timing.invariants)
            .is_err()
    );
}

#[test]
fn root_closure_does_not_retain_an_unused_declaration_context_chain() {
    let first = checked("type Leaf0 = {\n value: U64\n}\n", &StartupCatalog::new());
    let mut previous = first.native_law_witness("Leaf0").unwrap().clone();
    for depth in 1..=17 {
        let mut catalog = StartupCatalog::new();
        catalog
            .insert_native_law_witness("owner/previous", &previous)
            .unwrap();
        let name = format!("Leaf{depth}");
        // The imported Type occurs only in another declaration; Leaf's own
        // structural depth stays fixed while its declaration context grows.
        let source = format!("with owner/previous as Prior\ntype Touch = {{\n value: Prior\n}}\ntype {name} = {{\n value: U64\n}}\n");
        let doc = checked(&source, &catalog);
        previous = doc.native_law_witness(&name).unwrap().clone();
        assert!(previous.material().unwrap().imports.is_empty());
    }
}
#[test]
fn collection_counts_shared_dag_expansion_instead_of_only_unique_modules() {
    let first = checked("type Leaf0 = {\n value: U64\n}\n", &StartupCatalog::new());
    let mut previous = first.native_law_witness("Leaf0").unwrap().clone();
    for level in 1..=8 {
        let mut catalog = StartupCatalog::new();
        catalog
            .insert_native_law_witness("owner/left", &previous)
            .unwrap();
        catalog
            .insert_native_law_witness("owner/right", &previous)
            .unwrap();
        let name = format!("Leaf{level}");
        let source = format!("with owner/left as Left\nwith owner/right as Right\ntype {name} = {{\n left: Left\n right: Right\n}}\n");
        let doc = checked(&source, &catalog);
        if level == 8 {
            assert!(doc.native_law_witness(&name).is_none());
        } else {
            previous = doc.native_law_witness(&name).unwrap().clone();
            assert!(previous.material().is_ok());
        }
    }
}

#[test]
fn oversized_decoded_claims_are_refused_before_any_source_replay() {
    let doc = checked(SOURCE, &StartupCatalog::new());
    let witness = doc.native_law_witness("Timing").unwrap();
    let expected = &witness.checked_type().value_type;
    let mut material = witness.material().unwrap();
    material.source = "this deliberately cannot parse as Source".into();
    material.claimed.name = "x".repeat(4 * 1024 * 1024 + 1);
    assert!(matches!(
        NativeLawWitness::verify(&material, expected),
        Err(NativeLawCustodyRefusal::Capacity)
    ));

    let mut material = witness.material().unwrap();
    material.source = "this deliberately cannot parse as Source".into();
    let program = &mut material.claimed.invariants[0];
    for _ in 0..17 {
        program.root = PortableExpressionNode {
            value_type: program.root.value_type.clone(),
            operation: PortableExpressionOperation::Unary {
                operator: UnaryOperator::Not,
                operand: Box::new(program.root.clone()),
            },
        };
    }
    assert!(matches!(
        NativeLawWitness::verify(&material, expected),
        Err(NativeLawCustodyRefusal::Capacity)
    ));
}

#[test]
fn root_custody_ignores_unrelated_legacy_context_but_never_omits_a_required_one() {
    let legacy = checked("type Legacy = {\n value: U64\n}\n", &StartupCatalog::new());
    let mut catalog = StartupCatalog::new();
    catalog
        .insert_checked_native_type("Legacy", &legacy.native_types[0])
        .unwrap();
    let source = "type NeedsLegacy = {\n value: Legacy\n}\ntype Root = {\n value: U64\n where .value <= 7\n}\n";
    let doc = checked(source, &catalog);
    assert!(doc.native_law_witness("NeedsLegacy").is_none());
    let witness = doc.native_law_witness("Root").unwrap();
    let material = witness.material().unwrap();
    assert!(!material.source.contains("NeedsLegacy"));
    assert!(!material.source.contains("Legacy"));
    assert!(NativeLawWitness::verify(&material, &witness.checked_type().value_type).is_ok());
    let expected = doc
        .native_types
        .iter()
        .find(|ty| ty.name == "Root")
        .unwrap();
    assert_eq!(witness.checked_type(), expected);
}

#[test]
fn generic_root_closure_keeps_required_import_template_and_laws_without_unrelated_types() {
    let owner = checked(
        "type Imported = {\n value: U64\n where .value <= 15\n}\n",
        &StartupCatalog::new(),
    );
    let mut catalog = StartupCatalog::new();
    catalog
        .insert_native_law_witness(
            "owner/Imported",
            owner.native_law_witness("Imported").unwrap(),
        )
        .unwrap();
    catalog
        .insert_checked_native_type("Legacy", &owner.native_types[0])
        .unwrap();
    let source = "with owner/Imported as Foreign\ntype Box<T> = {\n value: T\n}\ntype Unrelated = {\n value: Legacy\n}\ntype Root = Box<Foreign>\n";
    let doc = checked(source, &catalog);
    let witness = doc.native_law_witness("Root").unwrap();
    let material = witness.material().unwrap();
    assert!(!material.source.contains("Unrelated"));
    assert!(material.source.contains("Box<T>"));
    assert_eq!(material.imports.len(), 1);
    assert!(NativeLawWitness::verify(&material, &witness.checked_type().value_type).is_ok());
    let mut replaced = material.clone();
    replaced.source = material.source.replace("Box<Foreign>", "Box<U64>");
    assert!(NativeLawWitness::verify(&replaced, &witness.checked_type().value_type).is_err());
    let mut omitted_template = material;
    omitted_template.source = "with owner/Imported as Foreign\ntype Root = Box<Foreign>\n".into();
    assert!(
        NativeLawWitness::verify(&omitted_template, &witness.checked_type().value_type).is_err()
    );
}
