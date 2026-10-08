#![cfg(feature = "semantic-bindings")]
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_speech::{ipa_phone::phone_from_ipa, semantic::*};
fn provenance() -> SpeechEvidenceProvenance {
    SpeechEvidenceProvenance::new(
        "universal phonetic notation".into(),
        SpeechEvidenceSource::Manual,
        None,
    )
    .unwrap()
}
#[test]
fn universal_phone_needs_no_inventory_and_retains_multiscalar_spelling() {
    for spelling in ["t͡ʃ", "d͡ʒ", "pʰ", "tʰ", "kʰ", "n̩", "ã", "ã", "ɾ"] {
        let admitted = phone_from_ipa(spelling.into(), provenance()).unwrap();
        let value = admitted.notation();
        assert_eq!(value.spelling().get(), spelling);
        assert_eq!(
            SpeechPhoneNotation::decode(&value.clone().encode().unwrap()).unwrap(),
            *value
        );
    }
}
#[test]
fn single_phone_refuses_sequences_suprasegmentals_and_provider_codes() {
    for spelling in ["", "tʃ", "ˈt", "tː", "tʰæp", "ax", "p_aspirated", "̃", "n̩ʰ"] {
        assert!(
            phone_from_ipa(spelling.into(), provenance()).is_err(),
            "{spelling}"
        );
    }
}
