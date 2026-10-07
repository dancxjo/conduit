//! Exact Source/Native receipt reuse prototype; no production decoder change.
extern crate alloc;
#[path = "../src/parser_window8_program_bank.rs"]
#[allow(dead_code)]
mod bank;
#[path = "common/window8_admission_memo.rs"]
mod memo;
use conduit_language::{
    parser_window8::{self, lexical, *},
    *,
};
use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding};
use std::cell::Cell;
fn scope(profile: &LanguageLexicalProfile) -> memo::Scope {
    macro_rules! programs {($($name:literal),* $(,)?)=>{vec![$(include_bytes!(concat!(env!("OUT_DIR"),"/",$name,".hex")).to_vec()),*]};}
    memo::Scope::new(
        programs!(
            "window8_walk_initialize",
            "window8_walk_follow",
            "window8_root_count"
        ),
        vec![
            include_bytes!("../parser_window8.conduit").to_vec(),
            include_bytes!("../parser.conduit").to_vec(),
            include_bytes!("../types.conduit").to_vec(),
            include_bytes!("../identity.conduit").to_vec(),
            include_bytes!("../src/parser_window8_program_bank.rs").to_vec(),
        ],
        profile.clone().encode().unwrap(),
    )
    .unwrap()
}
fn basis(label: &str) -> LanguageParserBasis {
    LanguageParserBasis::new(
        LanguageAnalysisRevisionId::new(label.into()).unwrap(),
        LanguageTextRevisionId::new("memo/source/r0".into()).unwrap(),
        LanguageTextId::new("memo/source".into()).unwrap(),
    )
    .unwrap()
}
fn relation(root: bool) -> LanguageParserRelation {
    LanguageParserRelation::new(
        if root {
            LanguageUniversalDependencyRelation::Root
        } else {
            LanguageUniversalDependencyRelation::Dep
        },
        LanguageParserSubtype::new("".into()).unwrap(),
    )
    .unwrap()
}
#[test]
fn exact_source_memo_hits_refuses_and_eviction_preserves_external_custody() {
    let source = bank::Window8ProgramBank::prepare().unwrap();
    let initial = source
        .initialize(
            &LanguageParserWindow8Begin::new(basis("memo/analysis"), relation(false), 2).unwrap(),
        )
        .unwrap();
    let provenance = LinguisticDerivationProvenance::deterministic_rule(
        "memo/profile".into(),
        "finite-record@1".into(),
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
        "memo/profile".into(),
        LanguageId::new("language/en".into()).unwrap(),
        provenance.clone(),
    )
    .unwrap();
    let material = LanguageTextRevision::new(
        LanguageTextFinality::Final,
        LanguageText::new(
            initial.state().basis().text().clone(),
            profile.language().clone(),
            initial.state().basis().source_revision().clone(),
            "record record ".into(),
        )
        .unwrap(),
        None,
        provenance.clone(),
        0,
        None,
    )
    .unwrap();
    let lexical =
        conduit_language::lexical::prepare_lexical_tape(&material, &profile, None).unwrap();
    let carrier = lexical.tape().clone().encode().unwrap();
    let changed_profile = LanguageLexicalProfile::new(
        profile.entries().clone(),
        profile.identity().clone(),
        profile.language().clone(),
        LinguisticDerivationProvenance::deterministic_rule(
            "memo/alternate-profile".into(),
            "finite-record@2".into(),
        )
        .unwrap(),
    )
    .unwrap();
    let changed_lexical =
        conduit_language::lexical::prepare_lexical_tape(&material, &changed_profile, None).unwrap();
    let changed_carrier = changed_lexical.tape().clone().encode().unwrap();
    let calls = Cell::new(0);
    let mut memo = memo::Memo::new(scope(&profile), 2, |raw: &LanguageParserWindow8RawState| {
        calls.set(calls.get() + 1);
        source
            .admit_state(raw)
            .map(|v| v.proof().clone())
            .map_err(|e| format!("{e:?}"))
    })
    .unwrap();
    let expected = scope(&profile);
    assert_eq!(memo.scope(), &expected);
    let started = std::time::Instant::now();
    let owned = memo.admit(initial.state(), carrier.clone()).unwrap();
    let first_elapsed = started.elapsed();
    assert_eq!(calls.get(), 1);
    let started = std::time::Instant::now();
    let hit = memo.admit(initial.state(), carrier.clone()).unwrap();
    let hit_elapsed = started.elapsed();
    assert_eq!(calls.get(), 1, "hit schedules zero new Source walks/laws");
    assert_eq!(
        hit.clone().encode().unwrap(),
        owned.clone().encode().unwrap()
    );
    memo.admit(initial.state(), changed_carrier.clone())
        .unwrap();
    assert_eq!(calls.get(), 2);
    let changed = source
        .initialize(
            &LanguageParserWindow8Begin::new(basis("memo/changed-analysis"), relation(false), 2)
                .unwrap(),
        )
        .unwrap();
    memo.admit(changed.state(), carrier.clone()).unwrap();
    assert_eq!(calls.get(), 3);
    assert_eq!(memo.len(), 2);
    assert_eq!(
        owned,
        initial.proof().clone(),
        "eviction does not destroy returned custody"
    );
    memo.admit(initial.state(), carrier.clone()).unwrap();
    assert_eq!(calls.get(), 4, "evicted body is re-admitted");
    let rooted = parser_window8::prepare_window8_step(
        &parser_window8::prepare_window8_state(initial.state()).unwrap(),
        &basis("memo/analysis"),
        LanguageParserAction::RightArc,
        &relation(true),
    )
    .unwrap();
    assert!(rooted.accepted());
    memo.admit(rooted.next().state(), carrier.clone()).unwrap();
    assert_eq!(calls.get(), 5, "changed head/full state misses");
    let prior = rooted.next().state();
    let cyclic = LanguageParserWindow8RawState::new(
        prior.basis().clone(),
        0,
        3,
        [1, 0, 9, 9, 9, 9, 9, 9, 8],
        relation(false),
        relation(false),
        relation(false),
        relation(false),
        relation(false),
        relation(false),
        relation(false),
        relation(false),
        [8, 0, 1, 8, 8, 8, 8, 8, 8],
        2,
        2,
    )
    .unwrap();
    let retained = memo.len();
    assert!(memo.admit(&cyclic, carrier.clone()).is_err());
    assert!(memo.admit(&cyclic, carrier.clone()).is_err());
    assert_eq!(calls.get(), 7, "refused inputs never become hits");
    assert_eq!(memo.len(), retained);
    // A new exact Source/dependency profile requires a distinct immutable owner.
    let changed_scope = scope(&changed_profile);
    assert_ne!(changed_scope, expected);
    let mut foreign = memo::Memo::new(changed_scope, 1, |raw: &LanguageParserWindow8RawState| {
        calls.set(calls.get() + 1);
        source
            .admit_state(raw)
            .map(|v| v.proof().clone())
            .map_err(|e| format!("{e:?}"))
    })
    .unwrap();
    foreign.admit(initial.state(), carrier.clone()).unwrap();
    assert_eq!(
        calls.get(),
        8,
        "different immutable profile owner cannot reuse entries"
    );
    eprintln!("exact memo fixture first={first_elapsed:?}, hit={hit_elapsed:?}; no whole-decoder performance claim");
    assert!(memo::Memo::new(
        scope(&profile),
        0,
        |_: &LanguageParserWindow8RawState| -> Result<LanguageParserWindow8StateProof, String> {
            Err("unreachable".into())
        }
    )
    .is_err());
    assert!(memo::Memo::new(
        scope(&profile),
        1025,
        |_: &LanguageParserWindow8RawState| -> Result<LanguageParserWindow8StateProof, String> {
            Err("unreachable".into())
        }
    )
    .is_err());
}
