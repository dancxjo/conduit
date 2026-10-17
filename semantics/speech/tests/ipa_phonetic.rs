#![cfg(feature = "semantic-bindings")]
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_speech::{ipa_notation::IpaNotationRefusal, ipa_phonetic::*, semantic::*};

fn provenance() -> SpeechEvidenceProvenance {
    SpeechEvidenceProvenance::new(
        "universal phonetic fixture".into(),
        SpeechEvidenceSource::Manual,
        None,
    )
    .unwrap()
}

#[test]
fn universal_transcription_preserves_typed_phones_events_and_source_coordinates() {
    for source in ["foˈnetika", "ˈt͡ʃpʰn̩ããː.d͡ʒ"] {
        let admitted = phonetic_from_ipa(source.into(), provenance()).unwrap();
        let value = admitted.transcription();
        assert_eq!(value.original(), source);
        assert_eq!(value.grammar_revision(), PHONETIC_GRAMMAR_REVISION);
        let decoded =
            SpeechPhoneticTranscription::decode(&value.clone().encode().unwrap()).unwrap();
        assert_eq!(&decoded, value);
        let mut cursor = 0;
        for occurrence in value.units().as_slice() {
            assert_eq!(*occurrence.start() as usize, cursor);
            cursor = *occurrence.end() as usize;
            assert_eq!(
                &source[*occurrence.start() as usize..cursor],
                occurrence.source_spelling().get()
            );
            assert_eq!(
                *occurrence.scalar_start() as usize,
                source[..*occurrence.start() as usize].chars().count()
            );
            assert_eq!(
                *occurrence.scalar_end() as usize,
                source[..cursor].chars().count()
            );
            if let SpeechPhoneticEvent::Phone(phone) = occurrence.value() {
                assert_eq!(phone.phone().spelling(), occurrence.source_spelling());
            }
        }
        assert_eq!(cursor, source.len());
    }
    let admitted = phonetic_from_ipa("ˈt͡ʃpʰn̩ããː.d͡ʒ".into(), provenance()).unwrap();
    assert_eq!(
        admitted
            .transcription()
            .units()
            .as_slice()
            .iter()
            .filter(|unit| matches!(unit.value(), SpeechPhoneticEvent::Phone(_)))
            .count(),
        6
    );
}

#[test]
fn transcription_does_not_convert_ascii_codes_or_invent_equivalence() {
    let admitted = phonetic_from_ipa("ih".into(), provenance()).unwrap();
    assert_eq!(admitted.transcription().units().len(), 2); // IPA [i h], never provider ih -> [ɪ].
    for spelling in ["ã", "ã"] {
        let admitted = phonetic_from_ipa(spelling.into(), provenance()).unwrap();
        let SpeechPhoneticEvent::Phone(phone) = admitted.transcription().units()[0].value() else {
            panic!("phone")
        };
        assert_eq!(phone.phone().spelling().get(), spelling);
    }
}

#[test]
fn unsupported_marks_delimiters_order_and_capacity_refuse_at_the_exact_body_span() {
    for (source, invalid) in [
        ("p̃", "̃"),
        ("t͡p", "͡"),
        ("ˈˌa", "ˌ"),
        ("aːː", "ː"),
        ("a.", "."),
        ("[a]", "["),
        ("/a/", "/"),
    ] {
        let error = match phonetic_from_ipa(source.into(), provenance()) {
            Ok(_) => panic!("{source}"),
            Err(error) => error,
        };
        let start = source.rfind(invalid).unwrap();
        // A leading display delimiter is the first invalid scalar.
        let start = if source.starts_with(invalid) {
            0
        } else {
            start
        };
        assert_eq!(
            (error.span.byte_start, error.span.byte_end),
            (start, start + invalid.len()),
            "{source}"
        );
    }
    let error = match phonetic_from_ipa("p".repeat(257), provenance()) {
        Ok(_) => panic!("bound"),
        Err(error) => error,
    };
    assert!(matches!(error.refusal, IpaNotationRefusal::Capacity));
    assert_eq!((error.span.byte_start, error.span.byte_end), (256, 257));
}

#[test]
fn a_variant_cannot_smuggle_an_unchecked_phone_past_its_native_spelling_law() {
    use conduit_core::{StructuredFieldValue, StructuredInfoValue};
    let raw_phone = StructuredInfoValue::record(
        SpeechPhoneNotation::semantic_type().unwrap(),
        vec![
            StructuredFieldValue::new("provenance", provenance().into_structured().unwrap())
                .unwrap(),
            StructuredFieldValue::new(
                "spelling",
                SpeechIpaSpelling::new("p_aspirated".into())
                    .unwrap()
                    .into_structured()
                    .unwrap(),
            )
            .unwrap(),
        ],
    )
    .unwrap();
    let payload = StructuredInfoValue::record(
        SpeechPhoneticPhone::semantic_type().unwrap(),
        vec![StructuredFieldValue::new("phone", raw_phone).unwrap()],
    )
    .unwrap();
    let candidate = StructuredInfoValue::variant(
        SpeechPhoneticEvent::semantic_type().unwrap(),
        "phone",
        payload,
    )
    .unwrap();
    assert!(SpeechPhoneticEvent::from_structured(candidate).is_err());
}
