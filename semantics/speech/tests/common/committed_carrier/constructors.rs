//! The accepted four-constructor configuration entrance, with exact result Types.
use conduit_core::{ConfigurationEntry, ConfigurationValue, StructuredConfigurationValue};
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_speech::{
    ipa_constructors::{prepare_configuration, IpaConstructor},
    semantic::*,
};
fn entry<T: NativeRustBinding>(key: &str, value: T) -> ConfigurationEntry {
    let profile = T::semantic_type()
        .unwrap()
        .profile()
        .unwrap()
        .value_kind()
        .clone();
    ConfigurationEntry {
        key: key.into(),
        value: ConfigurationValue::Structured(
            StructuredConfigurationValue::new(profile, value.encode().unwrap()).unwrap(),
        ),
    }
}
pub fn universal(
    constructor: IpaConstructor,
    original: &str,
    provenance: SpeechEvidenceProvenance,
) -> Vec<u8> {
    prepare_configuration(
        constructor,
        &[entry(
            "request",
            SpeechIpaUniversalRequest::new(original.into(), provenance).unwrap(),
        )],
    )
    .unwrap()
    .bytes()
    .to_vec()
}
pub fn scoped(
    constructor: IpaConstructor,
    original: &str,
    provenance: SpeechEvidenceProvenance,
    inventory: &SpeechInventory,
    basis: &SpeechIpaInventoryNotationBasis,
    phones: &[SpeechIpaPhoneDefinitionBinding],
    phonemes: &[SpeechIpaPhonemeDefinitionBinding],
) -> Vec<u8> {
    use conduit_plot::rust_binding::BoundedSequence;
    prepare_configuration(
        constructor,
        &[
            entry(
                "request",
                SpeechIpaUniversalRequest::new(original.into(), provenance).unwrap(),
            ),
            entry("inventory", inventory.clone()),
            entry("basis", basis.clone()),
            entry(
                "phone-bindings",
                SpeechIpaPhoneBindings::new(
                    BoundedSequence::try_from_iter(phones.iter().cloned()).unwrap(),
                )
                .unwrap(),
            ),
            entry(
                "phoneme-bindings",
                SpeechIpaPhonemeBindings::new(
                    BoundedSequence::try_from_iter(phonemes.iter().cloned()).unwrap(),
                )
                .unwrap(),
            ),
        ],
    )
    .unwrap()
    .bytes()
    .to_vec()
}
