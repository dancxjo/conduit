use conduit_language::*;
use conduit_plot::rust_binding::NativeRustBinding;
fn revision(
    sequence: u64,
    id: &str,
    text: &str,
    prior: Option<(&str, u64)>,
) -> LanguageTextRevision {
    LanguageTextRevision::new(
        LanguageTextFinality::Final,
        LanguageText::new(
            LanguageTextId::new("source/text".into()).unwrap(),
            LanguageId::new("language/en".into()).unwrap(),
            LanguageTextRevisionId::new(id.into()).unwrap(),
            text.into(),
        )
        .unwrap(),
        prior.map(|(id, sequence)| {
            LanguageTextPriorRevision::new(
                LanguageTextRevisionId::new(id.into()).unwrap(),
                sequence,
            )
            .unwrap()
        }),
        LinguisticDerivationProvenance::deterministic_rule("lineage/fixture".into(), "1".into())
            .unwrap(),
        sequence,
        None,
    )
    .unwrap()
}
#[test]
fn source_admits_exact_unchanged_material_and_whole_revision_advance() {
    let old = revision(0, "r0", "Hello Travis", None);
    let unchanged = prepare_text_revision_lineage(&old, &old).unwrap();
    assert_eq!(unchanged.previous(), unchanged.next());
    let next = revision(1, "r1", "Travis Hello", Some(("r0", 0)));
    validate_text_revision(Some(&old), &next, 0, 4096).unwrap();
    let admitted = prepare_text_revision_lineage(&old, &next).unwrap();
    assert_eq!(admitted.previous(), &old);
    assert_eq!(admitted.next(), &next);
    assert_eq!(
        LanguageTextRevisionLineage::decode(&admitted.clone().encode().unwrap()).unwrap(),
        admitted
    );
}
#[test]
fn source_refuses_missing_foreign_stale_and_reused_revision_metadata() {
    let old = revision(0, "r0", "Hello Travis", None);
    for next in [
        revision(1, "r1", "Travis Hello", None),
        revision(1, "r1", "Travis Hello", Some(("foreign", 0))),
        revision(1, "r1", "Travis Hello", Some(("r0", 1))),
        revision(0, "r1", "Travis Hello", Some(("r0", 0))),
        revision(2, "r1", "Travis Hello", Some(("r0", 0))),
        revision(1, "r0", "Travis Hello", Some(("r0", 0))),
        revision(0, "r0", "same IDs, altered material", None),
    ] {
        assert!(LanguageTextRevisionLineage::new(next, old.clone()).is_err());
    }
}
#[test]
fn source_refuses_foreign_identity_or_language() {
    let old = revision(0, "r0", "Hello Travis", None);
    let next = revision(1, "r1", "Travis Hello", Some(("r0", 0)));
    for material in [
        LanguageText::new(
            LanguageTextId::new("foreign".into()).unwrap(),
            next.material().language().clone(),
            next.material().revision().clone(),
            next.material().text().clone(),
        )
        .unwrap(),
        LanguageText::new(
            next.material().identity().clone(),
            LanguageId::new("language/fr".into()).unwrap(),
            next.material().revision().clone(),
            next.material().text().clone(),
        )
        .unwrap(),
    ] {
        let foreign = LanguageTextRevision::new(
            *next.finality(),
            material,
            next.prior().clone(),
            next.provenance().clone(),
            *next.sequence(),
            *next.stable_prefix(),
        )
        .unwrap();
        assert!(prepare_text_revision_lineage(&old, &foreign).is_err());
    }
}
#[test]
fn source_sequence_is_exact_above_float_precision_and_refuses_overflow() {
    let sequence = (1u64 << 53) + 1;
    let old = revision(sequence, "r0", "Hello Travis", None);
    let next = revision(sequence + 1, "r1", "Travis Hello", Some(("r0", sequence)));
    assert!(prepare_text_revision_lineage(&old, &next).is_ok());
    let overflow = revision(u64::MAX, "r0", "Hello Travis", None);
    let wrapped = revision(0, "r1", "Travis Hello", Some(("r0", u64::MAX)));
    assert!(prepare_text_revision_lineage(&overflow, &wrapped).is_err());
}
#[test]
fn lineage_schema_is_admitted_through_installed_catalog() {
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profiles = conduit_plot::ProfileCatalog::new();
    install_linguistics_catalogs(&mut startup, &mut profiles).unwrap();
    let source = "plot lineage/catalog (\n >> value: LanguageTextRevisionLineage\n result: LanguageTextRevisionLineage >>\n) = (.)";
    conduit_plot::check_syntax_document(&conduit_plot::parse_syntax_document(source), &startup)
        .unwrap();
}
