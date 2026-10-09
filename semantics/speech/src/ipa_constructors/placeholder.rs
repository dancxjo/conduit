//! Finite schema placeholders; required constructor startup has no default.
use crate::semantic::*;
use conduit_core::{ConfigurationValue, StructuredConfigurationValue};
use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding};

pub(super) fn configuration(name: &str) -> ConfigurationValue {
    let provenance = SpeechEvidenceProvenance::new(
        "schema placeholder".into(),
        SpeechEvidenceSource::Manual,
        None,
    )
    .expect("finite placeholder");
    match name {
        "request" => configured(
            SpeechIpaUniversalRequest::new("t".into(), provenance).expect("finite request"),
        ),
        "phone-bindings" => {
            configured(SpeechIpaPhoneBindings::new(BoundedSequence::new()).expect("empty bindings"))
        }
        "phoneme-bindings" => configured(
            SpeechIpaPhonemeBindings::new(BoundedSequence::new()).expect("empty bindings"),
        ),
        "inventory" => configured(
            SpeechInventory::new(
                SpeechInventoryId::new("schema/placeholder".into()).expect("finite identity"),
                conduit_language::LanguageId::new("schema/placeholder".into())
                    .expect("finite identity"),
                BoundedSequence::new(),
                BoundedSequence::new(),
            )
            .expect("empty definitions"),
        ),
        "basis" => {
            let language = conduit_language::LanguageId::new("schema/placeholder".into())
                .expect("finite identity");
            let variety = conduit_language::LanguageVariety::new(
                conduit_language::VarietyId::new("schema/placeholder".into())
                    .expect("finite identity"),
                language.clone(),
            )
            .expect("finite variety");
            let id = SpeechInventoryId::new("schema/placeholder".into()).expect("finite identity");
            let revision =
                SpeechSegmentRevisionId::new("schema/placeholder".into()).expect("finite identity");
            let unit = SpeechIpaUnitDefinition::new(
                SpeechIpaUnitId::new("schema/t".into()).expect("finite identity"),
                SpeechIpaUnitKind::Segment,
                provenance.clone(),
                SpeechIpaSpelling::new("t".into()).expect("one phone"),
            )
            .expect("finite unit");
            let profile = SpeechIpaNotationProfile::new(
                BoundedSequence::new(),
                SpeechIpaNotationProfileId::new("schema/placeholder".into())
                    .expect("finite identity"),
                id.clone(),
                provenance,
                revision.clone(),
                BoundedSequence::try_from_iter([unit]).expect("one unit"),
                variety.clone(),
            )
            .expect("finite profile");
            configured(
                SpeechIpaInventoryNotationBasis::new(id, language, profile, revision, variety)
                    .expect("exact basis"),
            )
        }
        _ => unreachable!("reviewed constructor field"),
    }
}
fn configured<T: NativeRustBinding>(value: T) -> ConfigurationValue {
    ConfigurationValue::Structured(
        StructuredConfigurationValue::new(
            T::semantic_type()
                .expect("checked placeholder")
                .profile()
                .expect("finite placeholder")
                .value_kind()
                .clone(),
            value.encode().expect("finite placeholder encoding"),
        )
        .expect("finite typed placeholder"),
    )
}
