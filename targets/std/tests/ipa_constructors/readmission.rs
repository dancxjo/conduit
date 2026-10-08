//! Re-admit delivered Native values against independently decoded planned scope.
use conduit_core::{ConfigurationEntry, ConfigurationValue};
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_speech::{
    ipa_constructors::IpaConstructor, ipa_inventory::PreparedIpaInventory, semantic::*,
};

fn argument<T: NativeRustBinding>(configuration: &[ConfigurationEntry], name: &str) -> T {
    let entry = configuration
        .iter()
        .find(|entry| entry.key == name)
        .unwrap();
    let ConfigurationValue::Structured(value) = &entry.value else {
        panic!("typed argument")
    };
    T::decode(value.canonical_value()).unwrap()
}

pub(super) fn check(
    constructor: IpaConstructor,
    configuration: &[ConfigurationEntry],
    bytes: &[u8],
) {
    let request: SpeechIpaUniversalRequest = argument(configuration, "request");
    match constructor {
        IpaConstructor::Phone => {
            let delivered = SpeechPhoneNotation::decode(bytes).unwrap();
            let admitted = conduit_speech::ipa_phone::phone_from_ipa(
                request.original().clone(),
                request.provenance().clone(),
            )
            .unwrap();
            assert_eq!(&delivered, admitted.notation());
        }
        IpaConstructor::Phonetic => {
            let delivered = SpeechPhoneticTranscription::decode(bytes).unwrap();
            let admitted = conduit_speech::ipa_phonetic::phonetic_from_ipa(
                request.original().clone(),
                request.provenance().clone(),
            )
            .unwrap();
            assert_eq!(&delivered, admitted.transcription());
        }
        IpaConstructor::Phoneme | IpaConstructor::Phonemic => {
            let inventory: SpeechInventory = argument(configuration, "inventory");
            let basis: SpeechIpaInventoryNotationBasis = argument(configuration, "basis");
            let phones: SpeechIpaPhoneBindings = argument(configuration, "phone-bindings");
            let phonemes: SpeechIpaPhonemeBindings = argument(configuration, "phoneme-bindings");
            let scope = PreparedIpaInventory::prepare(
                &inventory,
                basis.profile(),
                basis.variety(),
                basis.revision(),
                phones.values().as_slice(),
                phonemes.values().as_slice(),
            )
            .unwrap();
            if constructor == IpaConstructor::Phoneme {
                let delivered = SpeechPhonemeNotation::decode(bytes).unwrap();
                let admitted = scope
                    .phoneme_from_ipa(request.original().clone(), request.provenance().clone())
                    .unwrap();
                assert_eq!(delivered.definition(), admitted.definition());
                assert_eq!(delivered.basis(), admitted.basis());
                assert_eq!(delivered.notation(), admitted.notation().transcription());
                assert!(phonemes.values().as_slice().contains(delivered.binding()));
            } else {
                let delivered = SpeechPhonemicTranscription::decode(bytes).unwrap();
                let admitted = scope
                    .phonemic_from_ipa(request.original().clone(), request.provenance().clone())
                    .unwrap();
                admitted.require_inventory(&inventory).unwrap();
                assert_eq!(&delivered, admitted.transcription());
            }
        }
    }
}
