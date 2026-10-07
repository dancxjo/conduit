#![cfg(feature = "semantic-bindings")]
//! Actual learned Partial fact selects supplied phones without dropping its custody.
use conduit_language::{lexical::prepare_lexical_tape, *};
use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding};
use conduit_speech::{
    independent_pronunciation::*, independent_token_role::*, lexical_pronunciation::*, semantic::*,
};

#[test]
#[ignore = "requires immutable actual receipt via CONDUIT_INDEPENDENT_RECEIPT_PATH"]
fn actual_partial_vocative_pronunciation_retains_independent_protection() {
    let bytes = std::fs::read(std::env::var("CONDUIT_INDEPENDENT_RECEIPT_PATH").unwrap()).unwrap();
    let receipt = LanguageParserIndependentProtectedAdmission::decode(&bytes).unwrap();
    let native_tape = receipt.admission().fact().query().beam().lexical().tape();
    let lexical = prepare_lexical_tape(native_tape.source(), native_tape.profile(), None).unwrap();
    let role = prepare_independent_token_role(&lexical, &receipt).unwrap();
    let language = native_tape.source().material().language();
    let policy = |pos| {
        LanguagePronunciationSelectionProfile::new(
            "proof/independent-vocative-pronunciation".into(),
            language.clone(),
            native_tape.profile().provenance().clone(),
            BoundedSequence::try_from_iter([LanguagePronunciationSelectionRule::new(
                pos,
                LanguageDependencyRelation::new(
                    LanguageUniversalDependencyRelation::Vocative,
                    None,
                )
                .unwrap(),
            )
            .unwrap()])
            .unwrap(),
            LanguagePronunciationArcTarget::Dependent,
        )
        .unwrap()
    };
    let selection = prepare_independent_pronunciation_selection(
        &lexical,
        &role,
        &policy(LanguageLexicalPos::ProperNoun),
    )
    .unwrap();
    assert!(std::ptr::eq(selection.role().protected(), &receipt));
    assert_eq!(
        selection.selection().request().basis(),
        receipt.admission().arc()
    );
    assert_eq!(
        selection.selection().request().token(),
        &native_tape.tokens()[2]
    );
    let choice = *role.role().request().choice() as usize;
    assert_eq!(choice, 1, "actual acquired proper-noun choice");
    assert_eq!(
        selection.selection().candidate(),
        &native_tape.tokens()[2].candidates()[choice]
    );
    assert!(matches!(
        prepare_independent_pronunciation_selection(
            &lexical,
            &role,
            &policy(LanguageLexicalPos::Noun)
        ),
        Err(IndependentPronunciationRefusal::Candidate)
    ));
    let phones = BoundedSequence::try_from_iter(
        [
            ("t", SpeechStress::Unstressed),
            ("r", SpeechStress::Unstressed),
            ("ae", SpeechStress::Primary),
            ("v", SpeechStress::Unstressed),
            ("ih", SpeechStress::Unstressed),
            ("s", SpeechStress::Unstressed),
        ]
        .map(|(phone, stress)| {
            SpeechPronunciationPhone::new(PhoneId::new(phone.into()).unwrap(), stress).unwrap()
        }),
    )
    .unwrap();
    let row =
        SpeechPronunciationRow::new(selection.selection().candidate().clone(), phones.clone())
            .unwrap();
    let profile = |rows| {
        SpeechPronunciationProfile::new(
            "proof/independent-vocative-phones".into(),
            language.clone(),
            SpeechEvidenceProvenance::new(
                "reviewed pronunciation fixture".into(),
                SpeechEvidenceSource::Manual,
                None,
            )
            .unwrap(),
            rows,
        )
        .unwrap()
    };
    let pronunciation = prepare_independent_pronunciation(
        &selection,
        &profile(BoundedSequence::try_from_iter([row.clone()]).unwrap()),
    )
    .unwrap();
    assert!(std::ptr::eq(
        pronunciation.selection().role().protected(),
        &receipt
    ));
    assert_eq!(pronunciation.pronunciation().result().phones(), &phones);
    let unlisted_candidate = LanguageLexicalCandidate::new(
        "unlisted-name".into(),
        selection.selection().candidate().morphology().clone(),
        *selection.selection().candidate().pos(),
    )
    .unwrap();
    let unlisted_row = SpeechPronunciationRow::new(unlisted_candidate, phones.clone()).unwrap();
    assert!(matches!(
        prepare_independent_pronunciation(
            &selection,
            &profile(BoundedSequence::try_from_iter([unlisted_row]).unwrap())
        ),
        Err(IndependentPronunciationRefusal::Pronunciation(
            PronunciationRefusal::Unresolved(_)
        ))
    ));
    assert!(matches!(
        prepare_independent_pronunciation(
            &selection,
            &profile(BoundedSequence::try_from_iter([row.clone(), row]).unwrap())
        ),
        Err(IndependentPronunciationRefusal::Pronunciation(
            PronunciationRefusal::Unresolved(_)
        ))
    ));
}
