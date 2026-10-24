#![cfg(target_has_atomic = "ptr")]
#[path = "common/resource_lexicon.rs"]
mod fixture;
use conduit_language::{resource_lexicon_snapshot::*, *};
use fixture::{pinned_owner, source};

#[test]
#[ignore = "requires explicit existing private pinned TRAIN path; no corpus/resource files written"]
fn actual_pinned_resource_rows_survive_source_binding_without_selected_pos() {
    use conduit_plot::rust_binding::NativeRustBinding;
    let owner = pinned_owner();
    let program = PreparedResourceSnapshotProgram::prepare().unwrap();
    let original = source("answer answer", LanguageTextFinality::Final);
    let before = original.clone().encode().unwrap();
    let snapshot = PreparedResourceLexicalSnapshot::prepare(&original, &owner, &program)
        .unwrap_or_else(|e| panic!("snapshot: {e:?}"));
    assert!(std::ptr::eq(snapshot.lexicon(), &owner));
    assert_eq!(snapshot.lexical().tape().source(), &original);
    assert_eq!(snapshot.lexical().tape().profile().entries().len(), 1);
    assert_eq!(snapshot.bindings().len(), 2);
    // The pinned TRAIN row retains both NOUN and VERB; "a" is excluded
    // because its eight observed POS classes exceed this profile's bound.
    let expected = owner.lookup("answer").unwrap().unwrap();
    assert_eq!(expected.pos_codes().len(), 2);
    for (ordinal, binding) in snapshot.bindings().iter().enumerate() {
        assert_eq!(binding.ordinal(), ordinal);
        assert!(std::ptr::eq(binding.row().owner(), &owner));
        assert_eq!(binding.row().pos_codes(), expected.pos_codes());
        assert_eq!(binding.row().row_canonical(), expected.row_canonical());
        assert_eq!(
            binding.query().token().candidates(),
            binding.query().row().candidates()
        );
        assert_eq!(
            binding.query().token().candidates().len(),
            expected.pos_codes().len()
        );
        assert_eq!(binding.output_canonical(), [1]);
        assert_eq!(
            binding.input_canonical(),
            binding.query().clone().encode().unwrap()
        );
    }
    assert_eq!(original.encode().unwrap(), before);
    assert!(matches!(
        PreparedResourceLexicalSnapshot::prepare(
            &source("zzopaquelexiconabsent9", LanguageTextFinality::Final),
            &owner,
            &program
        ),
        Err(ResourceSnapshotRefusal::Absent { ordinal: 0 })
    ));
    assert!(matches!(
        PreparedResourceLexicalSnapshot::prepare(
            &source(
                "answer answer answer answer answer",
                LanguageTextFinality::Final
            ),
            &owner,
            &program
        ),
        Err(ResourceSnapshotRefusal::Bound)
    ));
    let initial = source("answer", LanguageTextFinality::Final);
    let next = LanguageTextRevision::new(
        LanguageTextFinality::Final,
        initial.material().clone(),
        Some(
            LanguageTextPriorRevision::new(LanguageTextRevisionId::new("0".into()).unwrap(), 0)
                .unwrap(),
        ),
        initial.provenance().clone(),
        1,
        Some(1),
    )
    .unwrap();
    assert!(matches!(
        PreparedResourceLexicalSnapshot::prepare(&next, &owner, &program),
        Err(ResourceSnapshotRefusal::PriorRequiresRebase)
    ));
    let partial = PreparedResourceLexicalSnapshot::prepare(
        &source("answer", LanguageTextFinality::Partial),
        &owner,
        &program,
    )
    .unwrap();
    assert!(partial.bindings().is_empty());
    assert!(partial.lexical().tape().tokens()[0].candidates().is_empty());
}
