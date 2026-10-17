use conduit_language::{lexical::*, *};
use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding};
fn provenance() -> LinguisticDerivationProvenance {
    LinguisticDerivationProvenance::deterministic_rule("fixture/lexical".into(), "profile/1".into())
        .unwrap()
}
fn revision(
    text: &str,
    sequence: u64,
    previous: Option<&LanguageTextRevision>,
    stable: Option<u32>,
    finality: LanguageTextFinality,
) -> LanguageTextRevision {
    LanguageTextRevision::new(
        finality,
        LanguageText::new(
            LanguageTextId::new("text".into()).unwrap(),
            LanguageId::new("language/en".into()).unwrap(),
            LanguageTextRevisionId::new(format!("r/{sequence}")).unwrap(),
            text.into(),
        )
        .unwrap(),
        previous.map(|old| {
            LanguageTextPriorRevision::new(old.material().revision().clone(), *old.sequence())
                .unwrap()
        }),
        provenance(),
        sequence,
        stable,
    )
    .unwrap()
}
fn profile() -> LanguageLexicalProfile {
    let candidate = |lemma: &str, pos| {
        LanguageLexicalCandidate::new(lemma.into(), BoundedSequence::new(), pos).unwrap()
    };
    let entries = BoundedSequence::try_from_iter([
        LanguageLexicalEntry::new(
            BoundedSequence::try_from_iter([candidate("answer", LanguageLexicalPos::Noun)])
                .unwrap(),
            "answer".into(),
        )
        .unwrap(),
        LanguageLexicalEntry::new(
            BoundedSequence::try_from_iter([
                candidate("record", LanguageLexicalPos::Noun),
                candidate("record", LanguageLexicalPos::Verb),
            ])
            .unwrap(),
            "record".into(),
        )
        .unwrap(),
    ])
    .unwrap();
    LanguageLexicalProfile::new(
        entries,
        "lexical/fixture".into(),
        LanguageId::new("language/en".into()).unwrap(),
        provenance(),
    )
    .unwrap()
}

fn derive(prefix: LanguageParserAvailableLexical) -> LanguageParserAvailability {
    let program = conduit_plot::PortableExpressionProgram::from_canonical_hex(include_str!(
        concat!(env!("OUT_DIR"), "/parser_availability_program.hex")
    ))
    .unwrap();
    let output = program.evaluate(&prefix.encode().unwrap()).unwrap();
    let raw = LanguageParserRawAvailability::decode(&output).unwrap();
    LanguageParserAvailability::new(*raw.final_input(), raw.lexical().clone(), *raw.waiting())
        .unwrap()
}
#[test]
fn source_availability_keeps_incomplete_occurrences_out_and_finality_distinct() {
    let material = revision("answer re", 0, None, None, LanguageTextFinality::Partial);
    let prepared = prepare_lexical_tape(&material, &profile(), None).unwrap();
    let zero = LanguageParserAvailableLexical::new(prepared.tape().clone(), 0).unwrap();
    let prefix = LanguageParserAvailableLexical::new(prepared.tape().clone(), 1).unwrap();
    assert!(LanguageParserAvailableLexical::new(prepared.tape().clone(), 2).is_err());
    let waiting = derive(zero);
    assert!(*waiting.waiting());
    assert!(!*waiting.final_input());
    let available = derive(prefix.clone());
    assert!(!*available.waiting());
    assert!(!*available.final_input());
    assert!(LanguageParserAvailability::new(true, prefix, false).is_err());
    let final_material = revision("answer record", 0, None, None, LanguageTextFinality::Final);
    let final_tape = prepare_lexical_tape(&final_material, &profile(), None).unwrap();
    assert!(
        !*derive(LanguageParserAvailableLexical::new(final_tape.tape().clone(), 1).unwrap())
            .final_input()
    );
    assert!(
        *derive(LanguageParserAvailableLexical::new(final_tape.tape().clone(), 2).unwrap())
            .final_input()
    );
}
#[test]
fn source_availability_rejects_foreign_occurrence_revision_and_excess_extent() {
    let material = revision("answer record", 0, None, None, LanguageTextFinality::Final);
    let prepared = prepare_lexical_tape(&material, &profile(), None).unwrap();
    assert!(LanguageParserAvailableLexical::new(prepared.tape().clone(), 3).is_err());
    assert!(LanguageParserAvailableLexical::new(prepared.tape().clone(), 5).is_err());
    let next = revision(
        "answer record",
        1,
        Some(&material),
        None,
        LanguageTextFinality::Final,
    );
    let foreign_tape = LanguageLexicalTape::new(
        prepared.tape().profile().clone(),
        next,
        prepared.tape().tokens().clone(),
    )
    .unwrap();
    assert!(LanguageParserAvailableLexical::new(foreign_tape, 1).is_err());
    let too_long = revision(
        "answer record answer record answer",
        0,
        None,
        None,
        LanguageTextFinality::Final,
    );
    let too_long = prepare_lexical_tape(&too_long, &profile(), None).unwrap();
    assert!(LanguageParserAvailableLexical::new(too_long.tape().clone(), 4).is_err());
}
