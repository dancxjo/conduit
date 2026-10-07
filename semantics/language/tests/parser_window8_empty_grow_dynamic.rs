//! Supplied forest fixtures exercise Source availability authority, not accuracy.
use conduit_core::*;
use conduit_language::{parser_window8::lexical::*, parser_window8::*, *};
use conduit_plot::rust_binding::{validate_native_invariants, BoundedSequence, NativeRustBinding};
use conduit_plot::*;
fn record(ty: &CheckedNativeType, fields: Vec<(&str, StructuredInfoValue)>) -> StructuredInfoValue {
    StructuredInfoValue::record(
        ty.value_type.clone(),
        fields
            .into_iter()
            .map(|(n, v)| StructuredFieldValue::new(n, v).unwrap())
            .collect(),
    )
    .unwrap()
}
fn field_type(ty: &CheckedNativeType, name: &str) -> StructuredInfoType {
    let StructuredInfoTypeShape::Record { fields, .. } = ty.value_type.shape() else {
        panic!("record")
    };
    fields
        .iter()
        .find(|field| field.name() == name)
        .unwrap()
        .value_type()
        .clone()
}
fn field(value: &StructuredInfoValue, name: &str) -> StructuredInfoValue {
    let StructuredInfoValueShape::Record(fields) = value.shape() else {
        panic!("record")
    };
    fields
        .iter()
        .find(|field| field.name() == name)
        .unwrap()
        .value()
        .clone()
}
fn scalar(ty: &CheckedNativeType, name: &str, value: u64) -> StructuredInfoValue {
    StructuredInfoValue::leaf(field_type(ty, name), value.to_le_bytes().to_vec()).unwrap()
}
fn checked_program(checked: &CheckedSyntaxDocument, entry: &str) -> PortableExpressionProgram {
    let expanded =
        expand_canonical_plot_for_authoring(checked, entry, &ProfileCatalog::new()).unwrap();
    assert_eq!(expanded.expanded.gears.len(), 1);
    let ConfigurationValue::Text(encoded) = &expanded.expanded.gears[0].configuration[0].value
    else {
        panic!("program")
    };
    PortableExpressionProgram::from_canonical_hex(encoded).unwrap()
}
#[test]
fn source_empty_prefix_grow_preserves_root_only_frontier() {
    let startup = StartupCatalog::new();
    eprintln!("window8 continuation: Source check start");
    // The authored native definitions retain refinement/law metadata for pure
    // construction; imported schema shapes alone cannot authorize construction.
    let source = [
        include_str!("../identity.conduit"),
        include_str!("../types.conduit"),
        include_str!("../text_revision.conduit"),
        include_str!("../revision_lineage.conduit"),
        include_str!("../lexical.conduit"),
        include_str!("../parser.conduit"),
        include_str!("../parser_beam.conduit"),
        include_str!("../parser_scorer.conduit"),
        include_str!("../parser_mask.conduit"),
        include_str!("../parser_window8.conduit"),
        include_str!("../parser_window8_continuation.conduit"),
        include_str!("../parser_window8_empty_grow.conduit"),
    ]
    .join("\n");
    let checked = check_syntax_document(&parse_syntax_document(&source), &startup).unwrap();
    eprintln!("window8 continuation: Source check complete");
    let ty = |name: &str| {
        checked
            .native_types
            .iter()
            .find(|ty| ty.name == name)
            .unwrap()
    };
    let provenance = LinguisticDerivationProvenance::deterministic_rule(
        "window8/empty-fixture".into(),
        "first-token@1".into(),
    )
    .unwrap();
    let profile = LanguageLexicalProfile::new(
        BoundedSequence::try_from_iter([LanguageLexicalEntry::new(
            BoundedSequence::try_from_iter([LanguageLexicalCandidate::new(
                "record".into(),
                BoundedSequence::new(),
                LanguageLexicalPos::Noun,
            )
            .unwrap()])
            .unwrap(),
            "record".into(),
        )
        .unwrap()])
        .unwrap(),
        "window8/empty-profile".into(),
        LanguageId::new("language/en".into()).unwrap(),
        provenance.clone(),
    )
    .unwrap();
    let revision = |sequence, text: &str| {
        LanguageTextRevision::new(
            LanguageTextFinality::Partial,
            LanguageText::new(
                LanguageTextId::new("window8/empty".into()).unwrap(),
                LanguageId::new("language/en".into()).unwrap(),
                LanguageTextRevisionId::new(format!("window8/empty/r{sequence}")).unwrap(),
                text.into(),
            )
            .unwrap(),
            if sequence == 0 {
                None
            } else {
                Some(
                    LanguageTextPriorRevision::new(
                        LanguageTextRevisionId::new("window8/empty/r0".into()).unwrap(),
                        0,
                    )
                    .unwrap(),
                )
            },
            provenance.clone(),
            sequence,
            Some(0),
        )
        .unwrap()
    };
    let previous = revision(0, "rec");
    let next = revision(1, "record ");
    let old_tape =
        conduit_language::lexical::prepare_lexical_tape(&previous, &profile, None).unwrap();
    let new_tape =
        conduit_language::lexical::prepare_lexical_tape(&next, &profile, Some(&old_tape)).unwrap();
    let old_lex = prepare_window8_lexical(&old_tape).unwrap();
    let new_lex = prepare_window8_lexical(&new_tape).unwrap();
    assert_eq!(*old_lex.lexical().token_count(), 0);
    assert_eq!(*new_lex.lexical().token_count(), 1);
    let basis = |source: &LanguageTextRevision, analysis: &str| {
        LanguageParserBasis::new(
            LanguageAnalysisRevisionId::new(analysis.into()).unwrap(),
            source.material().revision().clone(),
            source.material().identity().clone(),
        )
        .unwrap()
    };
    let old_basis = basis(&previous, "window8/empty-analysis0");
    let new_basis = basis(&next, "window8/empty-analysis1");
    let default = LanguageParserRelation::new(
        LanguageUniversalDependencyRelation::Dep,
        LanguageParserSubtype::new("".into()).unwrap(),
    )
    .unwrap();
    let initial = LanguageParserWindow8RawState::new(
        old_basis.clone(),
        0,
        1,
        [9, 9, 9, 9, 9, 9, 9, 9, 8],
        default.clone(),
        default.clone(),
        default.clone(),
        default.clone(),
        default.clone(),
        default.clone(),
        default.clone(),
        default.clone(),
        [8; 9],
        0,
        0,
    )
    .unwrap();
    let wait = record(
        ty("LanguageParserWindow8EmptyPrefix"),
        vec![
            ("state", initial.clone().into_structured().unwrap()),
            (
                "lexical",
                old_lex.lexical().clone().into_structured().unwrap(),
            ),
            ("basis", old_basis.clone().into_structured().unwrap()),
        ],
    );
    validate_native_invariants(&wait, &ty("LanguageParserWindow8EmptyPrefix").invariants).unwrap();
    let grow_ty = ty("LanguageParserWindow8EmptyGrowContext");
    let choices_ty = field_type(grow_ty, "choices");
    let StructuredInfoTypeShape::Collection { element, .. } = choices_ty.shape() else {
        panic!("choices")
    };
    let choices = StructuredInfoValue::collection(
        choices_ty.clone(),
        (0..8)
            .map(|_| {
                StructuredInfoValue::leaf(element.clone(), 0u64.to_le_bytes().to_vec()).unwrap()
            })
            .collect(),
    )
    .unwrap();
    let lineage = prepare_text_revision_lineage(&previous, &next).unwrap();
    let grow =
        |offered_wait: StructuredInfoValue, offered_basis: &LanguageParserBasis, selected| {
            record(
                grow_ty,
                vec![
                    ("previous", offered_wait),
                    ("next", new_lex.lexical().clone().into_structured().unwrap()),
                    ("lineage", lineage.clone().into_structured().unwrap()),
                    ("basis", offered_basis.clone().into_structured().unwrap()),
                    ("choices", choices.clone()),
                    ("selected", scalar(grow_ty, "selected", selected)),
                ],
            )
        };
    let admitted = grow(wait.clone(), &new_basis, 0);
    validate_native_invariants(&admitted, &grow_ty.invariants).unwrap();
    let program = checked_program(&checked, "language-window8-empty-grow");
    let output = StructuredInfoValue::from_canonical_bytes(
        &program
            .evaluate(&admitted.canonical_bytes().unwrap())
            .unwrap(),
    )
    .unwrap();
    let raw = LanguageParserWindow8RawState::from_structured(field(&output, "state")).unwrap();
    let state = prepare_window8_state(&raw).unwrap();
    assert_eq!(*state.state().token_count(), 1);
    assert_eq!(*state.state().unread(), 0);
    assert_eq!(*state.state().depth(), 1);
    assert_eq!(state.state().heads(), initial.heads());
    assert_eq!(state.state().stack(), initial.stack());
    assert_eq!(state.state().basis(), &new_basis);
    assert_eq!(field(&output, "choices"), choices);
    assert!(
        validate_native_invariants(&grow(wait.clone(), &old_basis, 0), &grow_ty.invariants)
            .is_err()
    );
    assert!(
        validate_native_invariants(&grow(wait.clone(), &new_basis, 1), &grow_ty.invariants)
            .is_err()
    );
    // A valid nonempty closed tree is never an empty-prefix continuation.
    let nonempty = initialize_window8(
        &LanguageParserWindow8Begin::new(new_basis.clone(), default.clone(), 1).unwrap(),
    )
    .unwrap();
    let rooted = prepare_window8_step(
        &nonempty,
        &new_basis,
        LanguageParserAction::RightArc,
        &LanguageParserRelation::new(
            LanguageUniversalDependencyRelation::Root,
            LanguageParserSubtype::new("".into()).unwrap(),
        )
        .unwrap(),
    )
    .unwrap();
    assert!(rooted.accepted());
    let reduced = prepare_window8_step(
        rooted.next(),
        &new_basis,
        LanguageParserAction::Reduce,
        &default,
    )
    .unwrap();
    assert!(reduced.accepted());
    let closed = reduced.next();
    assert_eq!(*closed.state().depth(), 1);
    let closed_wait = record(
        ty("LanguageParserWindow8EmptyPrefix"),
        vec![
            ("state", closed.state().clone().into_structured().unwrap()),
            (
                "lexical",
                new_lex.lexical().clone().into_structured().unwrap(),
            ),
            ("basis", new_basis.clone().into_structured().unwrap()),
        ],
    );
    assert!(validate_native_invariants(
        &closed_wait,
        &ty("LanguageParserWindow8EmptyPrefix").invariants
    )
    .is_err());
    assert!(
        validate_native_invariants(&grow(closed_wait, &new_basis, 0), &grow_ty.invariants).is_err()
    );
    let forged_wait = record(
        ty("LanguageParserWindow8EmptyPrefix"),
        vec![
            ("state", closed.state().clone().into_structured().unwrap()),
            (
                "lexical",
                old_lex.lexical().clone().into_structured().unwrap(),
            ),
            ("basis", old_basis.clone().into_structured().unwrap()),
        ],
    );
    assert!(validate_native_invariants(
        &forged_wait,
        &ty("LanguageParserWindow8EmptyPrefix").invariants
    )
    .is_err());
    assert!(
        validate_native_invariants(&grow(forged_wait, &new_basis, 0), &grow_ty.invariants).is_err()
    );
}
