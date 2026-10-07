use conduit_core::{ConfigurationValue, StructuredInfoValue, StructuredInfoValueShape};
use conduit_plot::*;

fn program(source: &str) -> Result<PortableExpressionProgram, CanonicalExpansionDiagnostic> {
    let checked =
        check_syntax_document(&parse_syntax_document(source), &StartupCatalog::new()).unwrap();
    let expanded = expand_canonical_plot_for_authoring(&checked, "make", &ProfileCatalog::new())?;
    let ConfigurationValue::Text(bytes) = &expanded.expanded.gears[0].configuration[0].value else {
        panic!("expression")
    };
    Ok(PortableExpressionProgram::from_canonical_hex(bytes).unwrap())
}
#[test]
fn nominal_exact_collection_preserves_profile_in_prepared_and_reference_execution() {
    let p=program("type Samples = collection U16 = 2\nplot make (\n >> value: U16\n result: Samples >>\n) = ([1,2])\n").unwrap();
    let expected = p.evaluate(&0u16.to_le_bytes()).unwrap();
    let mut prepared = PreparedPortableExpressionEvaluator::new(&p).unwrap();
    let capacity = prepared.output_capacity();
    assert_eq!(prepared.evaluate(&0u16.to_le_bytes()).unwrap(), expected);
    assert_eq!(prepared.output_capacity(), capacity);
    let actual = StructuredInfoValue::from_canonical_bytes(&expected).unwrap();
    assert_eq!(actual.value_type(), &p.output_type);
    let StructuredInfoValueShape::Collection(items) = actual.shape() else {
        panic!("collection")
    };
    assert_eq!(items.len(), 2);
}
#[test]
fn nominal_collection_refuses_wrong_length_and_invalid_refined_element() {
    assert!(program("type Samples = collection U16 = 2\nplot make (\n >> value: U16\n result: Samples >>\n) = ([1])\n").is_err());
    assert!(program("type Positive = U16 in 1..=3\ntype Samples = collection Positive = 2\nplot make (\n >> value: U16\n result: Samples >>\n) = ([0,2])\n").is_err());
}

#[test]
fn fixed_collection_forwarding_retains_exact_native_element_law() {
    let source="type Positive = U16 in 1..=3\ntype Samples = collection Positive = 2\ntype Envelope = {\n samples: Samples\n}\nplot make (\n >> value: Samples\n result: Envelope >>\n) = ({samples: [.1,.0]})\n";
    let p = program(source).unwrap();
    let checked=check_syntax_document(&parse_syntax_document("type Positive = U16 in 1..=3\ntype Samples = collection Positive = 2\nplot seed (\n samples: Samples = [1,3]\n) {\n}\n"),&StartupCatalog::new()).unwrap();
    let CanonicalStartupValue::Structured(seed) = checked.plots[0].startup_parameters[0]
        .default
        .as_ref()
        .unwrap()
    else {
        panic!("seed")
    };
    let bytes = seed.try_concrete().unwrap().canonical_bytes().unwrap();
    let expected = p.evaluate(&bytes).unwrap();
    let mut prepared = PreparedPortableExpressionEvaluator::new(&p).unwrap();
    assert_eq!(prepared.evaluate(&bytes).unwrap(), expected);
}
#[test]
fn collection_law_proof_refuses_unproven_indices_optional_elements_and_foreign_laws() {
    let prefix="type Positive = U16 in 1..=3\ntype Samples = collection Positive = 2\ntype Envelope = {\n samples: Samples\n}\n";
    for (input, expression) in [
        ("Samples", "{samples: [.2,.0]}"),
        ("sequence Positive in 0..=2", "{samples: [.0,.1]}"),
        ("collection U16 = 2", "{samples: [.0,.1]}"),
        ("Samples", "{samples: [.0 + 0,.1]}"),
    ] {
        let source = format!(
            "{prefix}plot make (\n >> value: {input}\n result: Envelope >>\n) = ({expression})\n"
        );
        let refused =
            match check_syntax_document(&parse_syntax_document(&source), &StartupCatalog::new()) {
                Err(_) => true,
                Ok(checked) => {
                    expand_canonical_plot_for_authoring(&checked, "make", &ProfileCatalog::new())
                        .is_err()
                }
            };
        assert!(refused, "{source}");
    }
}

#[test]
fn equal_primitive_ranges_do_not_erase_foreign_element_identity_or_array_invariants() {
    let foreign="type First = U16 in 1..=3\ntype Second = U16 in 1..=3\ntype InputSamples = collection First = 2\ntype OutputSamples = collection Second = 2\ntype Envelope = {\n samples: OutputSamples\n}\nplot make (\n >> value: InputSamples\n result: Envelope >>\n) = ({samples: [.0,.1]})\n";
    let checked =
        check_syntax_document(&parse_syntax_document(foreign), &StartupCatalog::new()).unwrap();
    assert!(expand_canonical_plot_for_authoring(&checked, "make", &ProfileCatalog::new()).is_err());
    let array_law="type Positive = U16 in 1..=3\ntype Samples = collection Positive = 2\ntype InputReceipt = {\n samples: collection U16 = 2\n where .samples.0 >= 1 && .samples.0 <= 3\n}\ntype Envelope = {\n samples: Samples\n}\nplot make (\n >> value: InputReceipt\n result: Envelope >>\n) = ({samples: [.samples.0,.samples.1]})\n";
    let checked =
        check_syntax_document(&parse_syntax_document(array_law), &StartupCatalog::new()).unwrap();
    assert!(expand_canonical_plot_for_authoring(&checked, "make", &ProfileCatalog::new()).is_err());
}
