//! Reviewed quoted-IPA Kinds. Parsing and membership run during preparation.
mod contract;
mod notation;
mod placeholder;
mod prepare;
pub use notation::{install_notation, NOTATION_EXPORT_PATH};
mod source;
use crate::semantic::*;
use alloc::vec::Vec;
use conduit_core::StructuredInfoType;
pub use contract::{contract, install, offer};
pub use prepare::{
    prepare_configuration, IpaConstructorDiagnostic, IpaConstructorLocation, IpaConstructorRefusal,
    PreparedIpaValue,
};
pub use source::{validate_source, IpaSourceDiagnostic};

pub const REVISION: &str = "conduit.speech/ipa-quoted@1";
pub const PROFILE: &str = "conduit-native/checked-ipa@1";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IpaConstructor {
    Phone,
    Phonetic,
    Phoneme,
    Phonemic,
}
impl IpaConstructor {
    pub const ALL: [Self; 4] = [Self::Phone, Self::Phonetic, Self::Phoneme, Self::Phonemic];
    pub fn from_kind(kind: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|constructor| constructor.kind() == kind)
    }
    pub const fn kind(self) -> &'static str {
        match self {
            Self::Phone => "speech/phone-from-ipa",
            Self::Phonetic => "speech/phonetic-from-ipa",
            Self::Phoneme => "speech/phoneme-from-ipa",
            Self::Phonemic => "speech/phonemic-from-ipa",
        }
    }
    pub const fn implementation(self) -> &'static str {
        match self {
            Self::Phone => "conduit-native/ipa-phone@1",
            Self::Phonetic => "conduit-native/ipa-phonetic@1",
            Self::Phoneme => "conduit-native/ipa-phoneme@1",
            Self::Phonemic => "conduit-native/ipa-phonemic@1",
        }
    }
    pub const fn request_name(self) -> &'static str {
        "SpeechIpaUniversalRequest"
    }
    pub fn request_type(self) -> StructuredInfoType {
        SpeechIpaUniversalRequest::semantic_type().expect("checked finite request Type")
    }
    /// Rich inventories retain their own admitted depth; wrapping them in
    /// another request would exceed the existing Core nesting limit.
    pub fn parameters(self) -> Vec<(&'static str, &'static str, StructuredInfoType)> {
        let mut parameters = alloc::vec![("request", self.request_name(), self.request_type())];
        if matches!(self, Self::Phoneme | Self::Phonemic) {
            parameters.extend([
                (
                    "inventory",
                    "SpeechInventory",
                    SpeechInventory::semantic_type().expect("checked inventory"),
                ),
                (
                    "basis",
                    "SpeechIpaInventoryNotationBasis",
                    SpeechIpaInventoryNotationBasis::semantic_type().expect("checked basis"),
                ),
                (
                    "phone-bindings",
                    "SpeechIpaPhoneBindings",
                    SpeechIpaPhoneBindings::semantic_type().expect("checked bindings"),
                ),
                (
                    "phoneme-bindings",
                    "SpeechIpaPhonemeBindings",
                    SpeechIpaPhonemeBindings::semantic_type().expect("checked bindings"),
                ),
            ]);
        }
        parameters
    }
    pub fn output_type(self) -> StructuredInfoType {
        match self {
            Self::Phone => SpeechPhoneNotation::semantic_type(),
            Self::Phonetic => SpeechPhoneticTranscription::semantic_type(),
            Self::Phoneme => SpeechPhonemeNotation::semantic_type(),
            Self::Phonemic => SpeechPhonemicTranscription::semantic_type(),
        }
        .expect("checked finite output Type")
    }
}
