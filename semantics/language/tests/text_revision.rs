use conduit_language::*;
use conduit_plot::rust_binding::NativeRustBinding;

fn revision(
    sequence: u64,
    previous: Option<&LanguageTextRevision>,
    content: &str,
    stable: Option<u32>,
    finality: LanguageTextFinality,
) -> LanguageTextRevision {
    LanguageTextRevision::new(
        finality,
        LanguageText::new(
            LanguageTextId::new("stream/1".into()).unwrap(),
            LanguageId::new("language/fixture".into()).unwrap(),
            LanguageTextRevisionId::new(format!("source/{sequence}")).unwrap(),
            content.into(),
        )
        .unwrap(),
        previous.map(|value| {
            LanguageTextPriorRevision::new(value.material().revision().clone(), *value.sequence())
                .unwrap()
        }),
        LinguisticDerivationProvenance::deterministic_rule(
            "revision-test".into(),
            "profile/1".into(),
        )
        .unwrap(),
        sequence,
        stable,
    )
    .unwrap()
}

#[test]
fn revisions_grow_and_revise_before_finality_on_scalar_basis() {
    let first = revision(0, None, "猫は", Some(1), LanguageTextFinality::Partial);
    let second = revision(
        1,
        Some(&first),
        "猫が寝る",
        Some(2),
        LanguageTextFinality::Partial,
    );
    let last = revision(
        2,
        Some(&second),
        "猫が寝る。",
        Some(5),
        LanguageTextFinality::Final,
    );
    validate_text_revision(None, &first, 0, 8).unwrap();
    validate_text_revision(Some(&first), &second, 1, 8).unwrap();
    validate_text_revision(Some(&second), &last, 2, 8).unwrap();
    assert_eq!(
        LanguageTextRevision::decode(&last.clone().encode().unwrap()).unwrap(),
        last
    );
    let after = revision(
        3,
        Some(&last),
        "猫が寝る。次",
        Some(5),
        LanguageTextFinality::Partial,
    );
    validate_text_revision(Some(&last), &after, 2, 8).unwrap();
    // Finality describes a completed snapshot, not permanent closure: an
    // authored/typed source can be edited under the same exact revision law.
}

#[test]
fn finality_and_stability_do_not_commit_or_bypass_finite_tail() {
    let final_text = revision(0, None, "Hello, Travis.", None, LanguageTextFinality::Final);
    validate_text_revision(None, &final_text, 0, 32).unwrap();
    assert_eq!(
        validate_text_revision(None, &final_text, 1, 32),
        Err(TextRevisionRefusal::CommittedPrefixRange)
    );
    assert_eq!(
        validate_text_revision(None, &final_text, 0, 4),
        Err(TextRevisionRefusal::RevisableFrontier)
    );
    let outside = revision(0, None, "猫", Some(2), LanguageTextFinality::Partial);
    assert_eq!(
        validate_text_revision(None, &outside, 0, 8),
        Err(TextRevisionRefusal::StablePrefixRange)
    );
}

#[test]
fn refused_or_unpublished_candidates_preserve_previous_material() {
    let first = revision(0, None, "Hello, Tr", Some(7), LanguageTextFinality::Partial);
    let snapshot = first.clone().encode().unwrap();
    let changed = revision(
        1,
        Some(&first),
        "Hallo, Travis",
        Some(7),
        LanguageTextFinality::Partial,
    );
    assert_eq!(
        validate_text_revision(Some(&first), &changed, 7, 16),
        Err(TextRevisionRefusal::CommittedPrefixChanged)
    );
    assert_eq!(
        validate_text_revision(Some(&first), &changed, 0, 16),
        Err(TextRevisionRefusal::StablePrefixChanged)
    );
    let regressed = revision(
        1,
        Some(&first),
        "Hello, Travis",
        Some(6),
        LanguageTextFinality::Partial,
    );
    assert_eq!(
        validate_text_revision(Some(&first), &regressed, 0, 16),
        Err(TextRevisionRefusal::StablePrefixRegressed)
    );
    let admitted = revision(
        1,
        Some(&first),
        "Hello, Travis",
        Some(7),
        LanguageTextFinality::Partial,
    );
    validate_text_revision(Some(&first), &admitted, 7, 16).unwrap();
    // Preparation is pure: downstream pressure/cancellation may discard this
    // candidate; the source epoch has not been published or committed here.
    drop(admitted);
    assert_eq!(first.clone().encode().unwrap(), snapshot);
}

#[test]
fn exact_prior_and_sequence_are_required() {
    let first = revision(0, None, "Hello", None, LanguageTextFinality::Partial);
    let skipped = revision(
        2,
        Some(&first),
        "Hello there",
        None,
        LanguageTextFinality::Partial,
    );
    assert_eq!(
        validate_text_revision(Some(&first), &skipped, 0, 32),
        Err(TextRevisionRefusal::Sequence)
    );
    let missing = revision(1, None, "Hello there", None, LanguageTextFinality::Partial);
    assert_eq!(
        validate_text_revision(Some(&first), &missing, 0, 32),
        Err(TextRevisionRefusal::PriorRevision)
    );
    let older = revision(
        1,
        Some(&first),
        "Hello there",
        None,
        LanguageTextFinality::Partial,
    );
    let wrong = revision(
        2,
        Some(&first),
        "Hello there!",
        None,
        LanguageTextFinality::Partial,
    );
    assert_eq!(
        validate_text_revision(Some(&older), &wrong, 0, 32),
        Err(TextRevisionRefusal::PriorRevision)
    );
}
