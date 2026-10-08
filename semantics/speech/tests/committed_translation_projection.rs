#![cfg(feature = "semantic-bindings")]
//! Original learned commitment -> accepted explicit translation -> derived edge.
//! The authored translation is not a translation-provider accuracy benchmark.
use conduit_language::{committed_discourse::*, lexical::*, *};
use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding};
use conduit_speech::{semantic::*, translation_dependency_projection::*};

fn with_field<T: NativeRustBinding>(
    value: T,
    name: &str,
    replacement: conduit_core::StructuredInfoValue,
) -> Result<T, conduit_plot::rust_binding::NativeBindingRefusal> {
    use conduit_core::{StructuredFieldValue, StructuredInfoValue, StructuredInfoValueShape};
    use conduit_plot::rust_binding::NativeBindingRefusal;
    let original = value.into_structured()?;
    let StructuredInfoValueShape::Record(fields) = original.shape() else {
        panic!("record")
    };
    assert!(fields.iter().any(|field| field.name() == name));
    let fields = fields
        .iter()
        .map(|field| {
            StructuredFieldValue::new(
                field.name(),
                if field.name() == name {
                    replacement.clone()
                } else {
                    field.value().clone()
                },
            )
            .unwrap()
        })
        .collect();
    T::from_structured(
        StructuredInfoValue::record(original.value_type().clone(), fields)
            .map_err(NativeBindingRefusal::InvalidValue)?,
    )
}

fn field_type<T: NativeRustBinding + Clone>(
    value: &T,
    name: &str,
) -> conduit_core::StructuredInfoType {
    let original = value.clone().into_structured().unwrap();
    let conduit_core::StructuredInfoValueShape::Record(fields) = original.shape() else {
        panic!("record")
    };
    fields
        .iter()
        .find(|field| field.name() == name)
        .unwrap()
        .value()
        .value_type()
        .clone()
}

fn provenance() -> LinguisticDerivationProvenance {
    LinguisticDerivationProvenance::deterministic_rule(
        "translation/singleton-vocative-projection".into(),
        "test/1".into(),
    )
    .unwrap()
}
fn provider() -> ListeningProvenance {
    ListeningProvenance::new(
        BoundedSequence::new(),
        ListeningProvenanceKind::Derived,
        None,
        None,
        BoundedSequence::new(),
    )
    .unwrap()
}
fn reference(tape: &PreparedLexicalTape, ordinal: usize) -> LanguageTextSegmentRef {
    language_source_occurrence(
        tape.tape().source().material(),
        tape.tape().tokens()[ordinal].span(),
        LanguageTextSegmentKind::Word,
    )
    .unwrap()
}
fn segment(value: &LanguageTextSegmentRef) -> LanguageSegmentRef {
    LanguageSegmentRef::text(
        *value.kind(),
        value.language().clone(),
        value.range().clone(),
        value.revision_id().clone(),
        value.text_id().clone(),
    )
    .unwrap()
}
fn alignment(
    id: &str,
    source: &LanguageTextSegmentRef,
    target: &LanguageTextSegmentRef,
) -> TranslationAlignment {
    TranslationAlignment::new(
        None,
        TranslationCorrespondence::aligned(
            BoundedSequence::try_from_iter([segment(source)]).unwrap(),
            BoundedSequence::try_from_iter([segment(target)]).unwrap(),
        )
        .unwrap(),
        TranslationAlignmentId::new(id.into()).unwrap(),
        provider(),
    )
    .unwrap()
}

#[test]
#[ignore = "requires actual original word-stream commitment receipt"]
fn actual_commit_projects_reordered_vocative_without_target_punctuation() {
    let path = std::env::var("CONDUIT_WORD_STREAM_COMMITTED_ROLES").unwrap();
    assert!(std::fs::metadata(&path).unwrap().len() <= 32 * 1024 * 1024);
    let receipt: serde_json::Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    let bytes =
        |value: &serde_json::Value| -> Vec<u8> { serde_json::from_value(value.clone()).unwrap() };
    let native_tape = LanguageLexicalTape::decode(&bytes(&receipt["lexical_tape_bytes"])).unwrap();
    let mut previous = None;
    for row in receipt["source_revision_history_bytes"].as_array().unwrap() {
        let revision = LanguageTextRevision::decode(&bytes(row)).unwrap();
        previous = Some(
            prepare_lexical_tape(&revision, native_tape.profile(), previous.as_ref()).unwrap(),
        );
    }
    let source_tape = previous.unwrap();
    assert_eq!(source_tape.tape(), &native_tape);
    let original = receipt["commitments"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| {
            LanguageParserCommittedDependencyAdmission::decode(&bytes(
                &row["committed_dependency_admission_bytes"],
            ))
            .unwrap()
        })
        .find(|commit| *commit.admission().fact().query().dependent() == 2)
        .unwrap();
    let source = prepare_committed_vocative_fact(
        "source/addressee".into(),
        &source_tape,
        &original,
        provenance(),
    )
    .unwrap();
    let target_revision = LanguageTextRevision::new(
        LanguageTextFinality::Final,
        LanguageText::new(
            LanguageTextId::new("translation/pt".into()).unwrap(),
            LanguageId::new("language/pt".into()).unwrap(),
            LanguageTextRevisionId::new("r1".into()).unwrap(),
            "Travis olá".into(),
        )
        .unwrap(),
        None,
        provenance(),
        0,
        None,
    )
    .unwrap();
    let profile = LanguageLexicalProfile::new(
        BoundedSequence::new(),
        "translation/pt-words".into(),
        target_revision.material().language().clone(),
        provenance(),
    )
    .unwrap();
    let target = prepare_lexical_tape(&target_revision, &profile, None).unwrap();
    assert_eq!(target.tape().tokens().len(), 2);
    let source_dep = reference(&source_tape, 2);
    let source_gov = reference(&source_tape, 0);
    let target_dep = reference(&target, 0);
    let target_gov = reference(&target, 1);
    let dep_alignment = alignment("translation/addressee", &source_dep, &target_dep);
    let gov_alignment = alignment("translation/governor", &source_gov, &target_gov);
    let candidate = TranslationCandidate::new(
        BoundedSequence::try_from_iter([dep_alignment.clone(), gov_alignment.clone()]).unwrap(),
        TranslationCandidateId::new("translation/candidate".into()).unwrap(),
        provider(),
        source_tape.tape().source().material().clone(),
        target_revision.material().clone(),
    )
    .unwrap();
    let accepted = TranslationAccepted::new(
        provider(),
        candidate.identity().clone(),
        candidate.source().revision().clone(),
        candidate.source().identity().clone(),
        candidate.target().revision().clone(),
        candidate.target().identity().clone(),
    )
    .unwrap();
    let target_analysis =
        LanguageAnalysisRevisionId::new("translation/derived-structure".into()).unwrap();
    let target_ref = |ordinal: usize| {
        LanguageAnalysisTokenRef::new(
            target_analysis.clone(),
            target.tape().tokens()[ordinal].identity().clone(),
        )
        .unwrap()
    };
    let LanguageDependencyHead::Token(governor) = source.fact().fact().basis().governor() else {
        panic!("original committed vocative requires a token governor")
    };
    let dependent_selection = TranslationWordPairSelectionRequest::new(
        accepted.clone(),
        0,
        candidate.clone(),
        source_dep,
        target_dep,
    )
    .unwrap();
    let governor_selection = TranslationWordPairSelectionRequest::new(
        accepted.clone(),
        1,
        candidate.clone(),
        source_gov,
        target_gov,
    )
    .unwrap();
    let dependent_pair = prepare_translation_word_pair(dependent_selection.clone()).unwrap();
    let governor_pair = prepare_translation_word_pair(governor_selection).unwrap();
    let request = TranslationVocativeProjectionRequest::new(
        dependent_pair.pair().clone(),
        governor_pair.pair().clone(),
        source.fact().fact().clone(),
        source_tape.tape().tokens()[2].clone(),
        LanguageAnalysisTokenRef::new(governor.revision().clone(), governor.token().clone())
            .unwrap(),
        source_tape.tape().tokens()[0].clone(),
        target.tape().clone(),
        target_ref(0),
        target_ref(1),
    )
    .unwrap();
    let projected = prepare_committed_translation_vocative(
        &source,
        &request,
        &target,
        &dependent_pair,
        &governor_pair,
        "target/addressee".into(),
        provenance(),
    )
    .unwrap();
    assert_eq!(projected.source().committed(), &original);
    assert_eq!(projected.request(), &request);
    assert_eq!(projected.target().tape(), target.tape());
    assert_eq!(projected.dependency().dependent(), &target_ref(0));
    assert_eq!(
        projected.dependency().governor(),
        &LanguageDependencyHead::token(
            target_analysis,
            target.tape().tokens()[1].identity().clone()
        )
        .unwrap()
    );
    assert_eq!(
        projected.dependency().relation(),
        original.admission().arc().relation()
    );
    assert!(matches!(
        projected.discourse().fact().role(),
        LanguageDiscourseRole::Addressee
    ));
    assert_eq!(
        projected.program().evaluate(projected.input()).unwrap(),
        projected.output()
    );
    assert_eq!(projected.discourse().fact().source(), &target_revision);
    assert!(with_field(
        dependent_selection.clone(),
        "alignment_index",
        conduit_core::StructuredInfoValue::leaf(
            field_type(&dependent_selection, "alignment_index"),
            1_u64.to_le_bytes().to_vec()
        )
        .unwrap()
    )
    .is_err());
    let stale = with_field(
        dependent_selection.accepted().clone(),
        "target_revision",
        LanguageTextRevisionId::new("stale".into())
            .unwrap()
            .into_structured()
            .unwrap(),
    )
    .unwrap();
    assert!(with_field(
        dependent_selection.clone(),
        "accepted",
        stale.into_structured().unwrap()
    )
    .is_err());
    let foreign_source = prepare_committed_vocative_fact(
        "different/source-fact".into(),
        &source_tape,
        &original,
        provenance(),
    )
    .unwrap();
    assert!(matches!(
        prepare_committed_translation_vocative(
            &foreign_source,
            &request,
            &target,
            &dependent_pair,
            &governor_pair,
            "target/addressee".into(),
            provenance()
        ),
        Err(TranslationProjectionRefusal::SourceCustody)
    ));
    // Equal raw selected pairs cannot hide a different complete candidate.
    let foreign_governor_alignment = with_field(
        gov_alignment,
        "identity",
        TranslationAlignmentId::new("foreign/governor-alignment".into())
            .unwrap()
            .into_structured()
            .unwrap(),
    )
    .unwrap();
    let foreign_candidate = with_field(
        candidate.clone(),
        "alignments",
        conduit_core::StructuredInfoValue::sequence(
            field_type(&candidate, "alignments"),
            vec![
                dep_alignment.into_structured().unwrap(),
                foreign_governor_alignment.into_structured().unwrap(),
            ],
        )
        .unwrap(),
    )
    .unwrap();
    let foreign_selection = with_field(
        governor_pair.request().clone(),
        "candidate",
        foreign_candidate.into_structured().unwrap(),
    )
    .unwrap();
    let foreign_pair = prepare_translation_word_pair(foreign_selection).unwrap();
    assert_eq!(foreign_pair.pair(), governor_pair.pair());
    assert!(matches!(
        prepare_committed_translation_vocative(
            &source,
            &request,
            &target,
            &dependent_pair,
            &foreign_pair,
            "target/addressee".into(),
            provenance(),
        ),
        Err(TranslationProjectionRefusal::AlignmentCustody)
    ));
    // Same token identity/span is insufficient: keep its whole committed value.
    let changed_token = with_field(
        request.source_dependent_token().clone(),
        "surface",
        conduit_core::StructuredInfoValue::leaf(
            field_type(request.source_dependent_token(), "surface"),
            b"Jordan".to_vec(),
        )
        .unwrap(),
    )
    .unwrap();
    let changed_request = with_field(
        request.clone(),
        "source_dependent_token",
        changed_token.into_structured().unwrap(),
    )
    .unwrap();
    assert!(matches!(
        prepare_committed_translation_vocative(
            &source,
            &changed_request,
            &target,
            &dependent_pair,
            &governor_pair,
            "target/addressee".into(),
            provenance(),
        ),
        Err(TranslationProjectionRefusal::SourceCustody)
    ));
    let punctuation_token = with_field(
        request.source_dependent_token().clone(),
        "category",
        LinguisticTokenCategory::Punctuation
            .into_structured()
            .unwrap(),
    )
    .unwrap();
    assert!(with_field(
        request.clone(),
        "source_dependent_token",
        punctuation_token.into_structured().unwrap()
    )
    .is_err());
    let dep = request.dependent_alignment();
    let gov = request.governor_alignment();
    let ambiguous = TranslationAlignment::new(
        None,
        TranslationCorrespondence::aligned(
            BoundedSequence::try_from_iter([segment(dep.source()), segment(gov.source())]).unwrap(),
            BoundedSequence::try_from_iter([segment(dep.target()), segment(gov.target())]).unwrap(),
        )
        .unwrap(),
        TranslationAlignmentId::new("ambiguous/group".into()).unwrap(),
        provider(),
    )
    .unwrap();
    let ambiguous_candidate = with_field(
        candidate.clone(),
        "alignments",
        conduit_core::StructuredInfoValue::sequence(
            field_type(&candidate, "alignments"),
            vec![ambiguous.into_structured().unwrap()],
        )
        .unwrap(),
    )
    .unwrap();
    assert!(TranslationWordPairSelectionRequest::new(
        accepted,
        0,
        ambiguous_candidate,
        dep.source().clone(),
        dep.target().clone(),
    )
    .is_err());
    if let Ok(path) = std::env::var("CONDUIT_COMMITTED_TRANSLATION_OUTPUT") {
        std::fs::write(
            path,
            serde_json::to_vec_pretty(&serde_json::json!({
                "schema": "language/actual-commit-singleton-translation-projection@1",
                "source_dependency_commit_bytes": original.clone().encode().unwrap(),
                "source_discourse_bytes": source.fact().fact().clone().encode().unwrap(),
                "source_revision_history_bytes": receipt["source_revision_history_bytes"],
                "dependent_selection_input_bytes": dependent_pair.input(),
                "dependent_selection_output_bytes": dependent_pair.output(),
                "governor_selection_input_bytes": governor_pair.input(),
                "governor_selection_output_bytes": governor_pair.output(),
                "selection_program_bytes": dependent_pair.program().canonical_bytes().unwrap(),
                "projection_request_bytes": projected.input(),
                "projection_program_bytes": projected.program().canonical_bytes().unwrap(),
                "projection_output_bytes": projected.output(),
                "target_dependency_bytes": projected.dependency().clone().encode().unwrap(),
                "target_discourse_bytes": projected.discourse().fact().clone().encode().unwrap(),
                "source_text": source_tape.tape().source().material().text(),
                "target_text": target_revision.material().text(),
                "authored_translation": true,
                "translation_provider_accuracy_claim": false,
                "target_parser_commitment": false,
                "played_commitment": false,
            }))
            .unwrap(),
        )
        .unwrap();
    }
}
