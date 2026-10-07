#![cfg(feature = "semantic-bindings")]
//! Exact acquired independent fact is retained through Source token-role preparation.
use conduit_language::*;
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_speech::{independent_token_role::*, semantic::*};

#[test]
#[ignore = "requires immutable actual receipt via CONDUIT_INDEPENDENT_RECEIPT_PATH"]
fn actual_partial_vocative_role_borrows_whole_protected_receipt() {
    let path = std::env::var("CONDUIT_INDEPENDENT_RECEIPT_PATH").unwrap();
    let bytes = std::fs::read(path).unwrap();
    let receipt = LanguageParserIndependentProtectedAdmission::decode(&bytes).unwrap();
    let native_tape = receipt.admission().fact().query().beam().lexical().tape();
    let tape = conduit_language::lexical::prepare_lexical_tape(
        native_tape.source(),
        native_tape.profile(),
        None,
    )
    .unwrap();
    assert_eq!(tape.tape(), native_tape);
    let role = prepare_independent_token_role(&tape, &receipt).unwrap();
    assert!(std::ptr::eq(role.protected(), &receipt));
    assert_eq!(*role.role().result().role(), SpeechTextTokenRole::Spoken);
    assert_eq!(role.role().request().basis(), receipt.admission().arc());
    assert_eq!(role.role().request().source(), native_tape.source());
    assert_eq!(
        role.role().request().token(),
        &native_tape.tokens().as_slice()[2]
    );
    assert_eq!(
        *native_tape.source().finality(),
        LanguageTextFinality::Partial
    );
    let foreign_profile = LanguageLexicalProfile::new(
        native_tape.profile().entries().clone(),
        "foreign-profile".into(),
        native_tape.profile().language().clone(),
        native_tape.profile().provenance().clone(),
    )
    .unwrap();
    let foreign = conduit_language::lexical::prepare_lexical_tape(
        native_tape.source(),
        &foreign_profile,
        None,
    )
    .unwrap();
    assert!(matches!(
        prepare_independent_token_role(&foreign, &receipt),
        Err(IndependentTokenRoleRefusal::LexicalTape)
    ));
}
