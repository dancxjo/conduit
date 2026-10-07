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
fn source_wait_grow_preserves_open_state_and_exact_stable_occurrences() {
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
        "window8/continuation-fixture".into(),
        "open-prefix@1".into(),
    )
    .unwrap();
    let candidate = LanguageLexicalCandidate::new(
        "record".into(),
        BoundedSequence::new(),
        LanguageLexicalPos::Noun,
    )
    .unwrap();
    let entry = LanguageLexicalEntry::new(
        BoundedSequence::try_from_iter([candidate]).unwrap(),
        "record".into(),
    )
    .unwrap();
    let profile = LanguageLexicalProfile::new(
        BoundedSequence::try_from_iter([entry]).unwrap(),
        "window8/continuation-profile".into(),
        LanguageId::new("language/en".into()).unwrap(),
        provenance.clone(),
    )
    .unwrap();
    let revision = |sequence, finality, text: &str| {
        LanguageTextRevision::new(
            finality,
            LanguageText::new(
                LanguageTextId::new("window8/continuation".into()).unwrap(),
                LanguageId::new("language/en".into()).unwrap(),
                LanguageTextRevisionId::new(format!("window8/continuation/r{sequence}")).unwrap(),
                text.into(),
            )
            .unwrap(),
            if sequence == 0 {
                None
            } else {
                Some(
                    LanguageTextPriorRevision::new(
                        LanguageTextRevisionId::new(format!(
                            "window8/continuation/r{}",
                            sequence - 1
                        ))
                        .unwrap(),
                        sequence - 1,
                    )
                    .unwrap(),
                )
            },
            provenance.clone(),
            sequence,
            Some(6),
        )
        .unwrap()
    };
    let previous = revision(0, LanguageTextFinality::Partial, "record ");
    let next = revision(1, LanguageTextFinality::Partial, "record record ");
    let old_tape =
        conduit_language::lexical::prepare_lexical_tape(&previous, &profile, None).unwrap();
    let new_tape =
        conduit_language::lexical::prepare_lexical_tape(&next, &profile, Some(&old_tape)).unwrap();
    let old_lex = prepare_window8_lexical(&old_tape).unwrap();
    let new_lex = prepare_window8_lexical(&new_tape).unwrap();
    let basis = |source: &LanguageTextRevision, analysis: &str| {
        LanguageParserBasis::new(
            LanguageAnalysisRevisionId::new(analysis.into()).unwrap(),
            source.material().revision().clone(),
            source.material().identity().clone(),
        )
        .unwrap()
    };
    let old_basis = basis(&previous, "window8/continuation-analysis0");
    let new_basis = basis(&next, "window8/continuation-analysis1");
    let default = LanguageParserRelation::new(
        LanguageUniversalDependencyRelation::Dep,
        LanguageParserSubtype::new("".into()).unwrap(),
    )
    .unwrap();
    let initial = initialize_window8(
        &LanguageParserWindow8Begin::new(old_basis.clone(), default.clone(), 1).unwrap(),
    )
    .unwrap();
    let shifted =
        prepare_window8_step(&initial, &old_basis, LanguageParserAction::Shift, &default).unwrap();
    assert!(shifted.accepted());
    let state = shifted.next();
    assert_eq!(state.state().heads()[0], 9);
    let continuation = |proof: &PreparedWindow8State,
                        lexical: &LanguageParserWindow8Lexical,
                        offered_basis: &LanguageParserBasis| {
        record(
            ty("LanguageParserWindow8Continuation"),
            vec![
                ("state", proof.proof().clone().into_structured().unwrap()),
                ("lexical", lexical.clone().into_structured().unwrap()),
                ("basis", offered_basis.clone().into_structured().unwrap()),
            ],
        )
    };
    let context = continuation(state, old_lex.lexical(), &old_basis);
    validate_native_invariants(
        &context,
        &ty("LanguageParserWindow8Continuation").invariants,
    )
    .unwrap();
    let wait = record(
        ty("LanguageParserWindow8Wait"),
        vec![("context", context.clone())],
    );
    validate_native_invariants(&wait, &ty("LanguageParserWindow8Wait").invariants).unwrap();
    let waiting_program = checked_program(&checked, "language-window8-wait");
    let output = StructuredInfoValue::from_canonical_bytes(
        &waiting_program
            .evaluate(&context.canonical_bytes().unwrap())
            .unwrap(),
    )
    .unwrap();
    assert_eq!(field(&output, "context"), context);
    let not_exhausted = record(
        ty("LanguageParserWindow8Wait"),
        vec![(
            "context",
            continuation(&initial, old_lex.lexical(), &old_basis),
        )],
    );
    assert!(validate_native_invariants(
        &not_exhausted,
        &ty("LanguageParserWindow8Wait").invariants
    )
    .is_err());
    let final_tape = conduit_language::lexical::prepare_lexical_tape(
        &revision(0, LanguageTextFinality::Final, "record "),
        &profile,
        None,
    )
    .unwrap();
    let final_lex = prepare_window8_lexical(&final_tape).unwrap();
    let final_wait = record(
        ty("LanguageParserWindow8Wait"),
        vec![(
            "context",
            continuation(state, final_lex.lexical(), &old_basis),
        )],
    );
    assert!(
        validate_native_invariants(&final_wait, &ty("LanguageParserWindow8Wait").invariants)
            .is_err()
    );
    let grow_ty = ty("LanguageParserWindow8GrowContext");
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
    let grow = |wait: StructuredInfoValue,
                next_lexical: &LanguageParserWindow8Lexical,
                offered_basis: &LanguageParserBasis| {
        record(
            grow_ty,
            vec![
                ("previous", wait),
                ("next", next_lexical.clone().into_structured().unwrap()),
                ("lineage", lineage.clone().into_structured().unwrap()),
                ("basis", offered_basis.clone().into_structured().unwrap()),
                ("choices", choices.clone()),
                ("selected", scalar(grow_ty, "selected", 1)),
            ],
        )
    };
    let admitted = grow(wait.clone(), new_lex.lexical(), &new_basis);
    validate_native_invariants(&admitted, &grow_ty.invariants).unwrap();
    eprintln!("window8 continuation: expand grow");
    let program = checked_program(&checked, "language-window8-grow");
    let output = StructuredInfoValue::from_canonical_bytes(
        &program
            .evaluate(&admitted.canonical_bytes().unwrap())
            .unwrap(),
    )
    .unwrap();
    let next_state =
        LanguageParserWindow8RawState::from_structured(field(&output, "state")).unwrap();
    let full_next = prepare_window8_state(&next_state).unwrap();
    assert_eq!(*full_next.state().token_count(), 2);
    assert_eq!(full_next.state().heads(), state.state().heads());
    assert_eq!(full_next.state().stack(), state.state().stack());
    assert_eq!(*full_next.state().unread(), 1);
    assert_eq!(full_next.state().basis(), &new_basis);
    assert_eq!(field(&output, "choices"), choices);
    assert_eq!(
        new_lex.lexical().tape().tokens()[0].prior_occurrence(),
        &Some(old_lex.lexical().tape().tokens()[0].identity().clone())
    );
    assert!(validate_native_invariants(
        &grow(wait.clone(), new_lex.lexical(), &old_basis),
        &grow_ty.invariants
    )
    .is_err());
    // Deliberately manufactured well-shaped input bypasses private lexical
    // preparation, and must still refuse the missing exact stable identity.
    let tokens =
        BoundedSequence::try_from_iter(new_lex.lexical().tape().tokens().iter().map(|token| {
            LanguageLexicalToken::new(
                token.candidates().clone(),
                *token.category(),
                *token.completeness(),
                token.identity().clone(),
                None,
                token.span().clone(),
                token.surface().clone(),
            )
            .unwrap()
        }))
        .unwrap();
    let unlinked = LanguageParserWindow8Lexical::new(
        LanguageLexicalTape::new(profile.clone(), next.clone(), tokens).unwrap(),
        2,
    )
    .unwrap();
    assert!(validate_native_invariants(
        &grow(wait.clone(), &unlinked, &new_basis),
        &grow_ty.invariants
    )
    .is_err());
    let changed_candidate = LanguageLexicalCandidate::new(
        "record".into(),
        BoundedSequence::new(),
        LanguageLexicalPos::Verb,
    )
    .unwrap();
    let changed_profile = LanguageLexicalProfile::new(
        BoundedSequence::try_from_iter([LanguageLexicalEntry::new(
            BoundedSequence::try_from_iter([changed_candidate]).unwrap(),
            "record".into(),
        )
        .unwrap()])
        .unwrap(),
        profile.identity().clone(),
        profile.language().clone(),
        profile.provenance().clone(),
    )
    .unwrap();
    let foreign_profile = LanguageParserWindow8Lexical::new(
        LanguageLexicalTape::new(
            changed_profile,
            next.clone(),
            new_lex.lexical().tape().tokens().clone(),
        )
        .unwrap(),
        2,
    )
    .unwrap();
    assert!(validate_native_invariants(
        &grow(wait.clone(), &foreign_profile, &new_basis),
        &grow_ty.invariants
    )
    .is_err());
    let foreign_previous = LanguageTextRevision::new(
        *previous.finality(),
        previous.material().clone(),
        None,
        LinguisticDerivationProvenance::deterministic_rule(
            "foreign/continuation".into(),
            "other".into(),
        )
        .unwrap(),
        0,
        Some(6),
    )
    .unwrap();
    let wrong_lineage = prepare_text_revision_lineage(&foreign_previous, &next).unwrap();
    let wrong_lineage_context = record(
        grow_ty,
        vec![
            ("previous", wait.clone()),
            ("next", new_lex.lexical().clone().into_structured().unwrap()),
            ("lineage", wrong_lineage.into_structured().unwrap()),
            ("basis", new_basis.clone().into_structured().unwrap()),
            ("choices", choices.clone()),
            ("selected", scalar(grow_ty, "selected", 1)),
        ],
    );
    assert!(validate_native_invariants(&wrong_lineage_context, &grow_ty.invariants).is_err());
    let root = LanguageParserRelation::new(
        LanguageUniversalDependencyRelation::Root,
        LanguageParserSubtype::new("".into()).unwrap(),
    )
    .unwrap();
    let closed =
        prepare_window8_step(&initial, &old_basis, LanguageParserAction::RightArc, &root).unwrap();
    let closed_wait = record(
        ty("LanguageParserWindow8Wait"),
        vec![(
            "context",
            continuation(closed.next(), old_lex.lexical(), &old_basis),
        )],
    );
    validate_native_invariants(&closed_wait, &ty("LanguageParserWindow8Wait").invariants).unwrap();
    assert!(validate_native_invariants(
        &grow(closed_wait, new_lex.lexical(), &new_basis),
        &grow_ty.invariants
    )
    .is_err());
    let s = closed.next().state();
    let committed = prepare_window8_state(
        &LanguageParserWindow8RawState::new(
            s.basis().clone(),
            1,
            *s.depth(),
            *s.heads(),
            s.relation0().clone(),
            s.relation1().clone(),
            s.relation2().clone(),
            s.relation3().clone(),
            s.relation4().clone(),
            s.relation5().clone(),
            s.relation6().clone(),
            s.relation7().clone(),
            *s.stack(),
            *s.token_count(),
            *s.unread(),
        )
        .unwrap(),
    )
    .unwrap();
    let committed_wait = record(
        ty("LanguageParserWindow8Wait"),
        vec![(
            "context",
            continuation(&committed, old_lex.lexical(), &old_basis),
        )],
    );
    validate_native_invariants(&committed_wait, &ty("LanguageParserWindow8Wait").invariants)
        .unwrap();
    assert!(validate_native_invariants(
        &grow(committed_wait, new_lex.lexical(), &new_basis),
        &grow_ty.invariants
    )
    .is_err());
}
