use conduit_core::{port_id, CheckedValueContract, FrontValueLocation, ValueConstraint};
use conduit_plot::{
    check_syntax_document, parse_syntax_document, rust_binding::NativeRustBinding, StartupCatalog,
    MAXIMUM_TEXT_PATTERN_SOURCE_BYTES, MAXIMUM_TEXT_PATTERN_SOURCE_DEPTH,
};
use conduit_text::{PortablePatternSpecification, PortablePatternSpecificationRefusal};

fn bare_constraint(literal: &str, maximum: u32) -> ValueConstraint {
    let source = format!("plot pattern (\n >> value: Text <= {maximum}B ~ {literal}\n) {{\n}}\n");
    let checked =
        check_syntax_document(&parse_syntax_document(&source), &StartupCatalog::new()).unwrap();
    checked.plots[0]
        .runtime_front
        .value_contract(&FrontValueLocation::Input(port_id("value")))
        .unwrap()
        .constraints[0]
        .clone()
}

#[test]
fn explicit_specifications_reuse_bare_pattern_constraints_exactly() {
    for (source, insensitive, start, end, bare) in [
        ("[A-Z]+", true, true, true, "/^[A-Z]+$/i"),
        ("a[/]b", false, false, false, "/a[/]b/"),
        (r"a\.", false, false, false, r"/a\./"),
        (r"path\\", false, false, true, r"/path\\$/"),
        ("t͡ʃ", false, false, false, "/t͡ʃ/"),
    ] {
        let specification =
            PortablePatternSpecification::new(source.into(), insensitive, start, end).unwrap();
        assert_eq!(specification.source(), source);
        assert_eq!(
            specification.checked_constraint(32, false).unwrap(),
            bare_constraint(bare, 32)
        );
        let bytes = specification.clone().encode().unwrap();
        assert_eq!(
            PortablePatternSpecification::decode(&bytes).unwrap(),
            specification
        );
        assert!(PortablePatternSpecification::decode(&bytes[..bytes.len() - 1]).is_err());
    }
}

#[test]
fn input_bound_is_owned_by_the_checked_consumer() {
    let specification =
        PortablePatternSpecification::new("[A-Z]+".into(), true, true, true).unwrap();
    let constraint = specification.checked_constraint(8, false).unwrap();
    let contract =
        CheckedValueContract::new(conduit_core::kind_id("value/text"), 8, vec![constraint])
            .unwrap();
    assert!(contract.validate(b"AbCd").is_ok());
    assert!(contract.validate(b"letters!").is_err());
    assert!(contract.validate(b"ABCDEFGHI").is_err());
    assert!(matches!(
        specification.checked_constraint(u32::MAX, false),
        Err(PortablePatternSpecificationRefusal::Definition(_))
    ));
    let negated = specification.checked_constraint(8, true).unwrap();
    let contract =
        CheckedValueContract::new(conduit_core::kind_id("value/text"), 8, vec![negated]).unwrap();
    assert!(contract.validate(b"AbCd").is_err());
    assert!(contract.validate(b"123").is_ok());
}

#[test]
fn malformed_and_over_budget_specifications_refuse_before_matching() {
    for source in ["", r"a\/b", "[Z-A]", "[", "(a", r"\d"] {
        assert!(
            matches!(
                PortablePatternSpecification::new(source.into(), false, false, false),
                Err(PortablePatternSpecificationRefusal::Source(_))
            ),
            "{source:?}"
        );
    }
    assert!(PortablePatternSpecification::new(
        "a".repeat(MAXIMUM_TEXT_PATTERN_SOURCE_BYTES + 1),
        false,
        false,
        false
    )
    .is_err());
    let source = "(".repeat(MAXIMUM_TEXT_PATTERN_SOURCE_DEPTH + 1)
        + "a"
        + &")".repeat(MAXIMUM_TEXT_PATTERN_SOURCE_DEPTH + 1);
    assert!(PortablePatternSpecification::new(source, false, false, false).is_err());
    // A structurally decoded candidate is not permission to bypass domain admission.
    let candidate =
        PortablePatternSpecification::new_native(false, false, false, r"a\/b".into()).unwrap();
    let received = PortablePatternSpecification::decode(&candidate.encode().unwrap()).unwrap();
    assert!(matches!(
        received.checked_constraint(32, false),
        Err(PortablePatternSpecificationRefusal::Source(_))
    ));
}

#[test]
fn foreign_same_layout_type_cannot_be_decoded_as_the_pattern_specification() {
    let source = include_str!("../pattern-types.conduit").replace(
        "PortablePatternSpecification",
        "ForeignPatternSpecification",
    );
    let checked =
        check_syntax_document(&parse_syntax_document(&source), &StartupCatalog::new()).unwrap();
    let expected = PortablePatternSpecification::semantic_type().unwrap();
    let foreign_type = checked.native_types[0].value_type.clone();
    assert_ne!(foreign_type, expected);
    let original = PortablePatternSpecification::new("a".into(), false, false, false)
        .unwrap()
        .into_structured()
        .unwrap();
    let conduit_core::StructuredInfoValueShape::Record(fields) = original.shape() else {
        panic!("pattern specification is an ordinary record")
    };
    let foreign = conduit_core::StructuredInfoValue::record(foreign_type, fields.to_vec()).unwrap();
    assert!(PortablePatternSpecification::decode(&foreign.canonical_bytes().unwrap()).is_err());
}

#[test]
fn installed_type_is_available_to_ordinary_source_without_notation() {
    let mut startup = StartupCatalog::new();
    conduit_text::install_portable_pattern_type(&mut startup).unwrap();
    let source = "with pattern/portable/specification as Pattern\nplot pattern (\n >> specification: Pattern\n) {\n}\n";
    let syntax = parse_syntax_document(source);
    let checked = check_syntax_document(&syntax, &startup).unwrap();
    let expected = PortablePatternSpecification::semantic_type().unwrap();
    assert_eq!(
        checked.plots[0].runtime_front.inputs()[0].value_kind,
        expected.profile().unwrap().value_kind().clone()
    );
}
