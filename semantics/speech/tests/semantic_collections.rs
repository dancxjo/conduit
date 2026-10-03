#![cfg(feature = "semantic-bindings")]
use conduit_core::IeeeF32;
use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding};
use conduit_speech::{admission::*, semantic::*};

fn bundle(entries: &[(&str, FeatureSpecification)]) -> SpeechFeatureBundle {
    SpeechFeatureBundle::new(
        BoundedSequence::try_from_iter(entries.iter().map(|(identity, specification)| {
            SpeechFeature::new(
                SpeechFeatureId::new((*identity).into()).unwrap(),
                specification.clone(),
            )
            .unwrap()
        }))
        .unwrap(),
    )
    .unwrap()
}

#[test]
fn feature_maps_refuse_duplicate_keys_without_merging_uncertainty() {
    let distinct = bundle(&[
        ("é", FeatureSpecification::unknown()),
        ("e\u{301}", FeatureSpecification::unknown()),
    ]);
    assert_eq!(validate_feature_bundle(&distinct), Ok(()));
    assert_eq!(validate_feature_bundle(&bundle(&[])), Ok(()));
    let duplicate = bundle(&[
        ("voice", FeatureSpecification::unknown()),
        ("voice", FeatureSpecification::unspecified()),
    ]);
    assert_eq!(
        validate_feature_bundle(&duplicate),
        Err(LocalSemanticRefusal::DuplicateFeatureIdentity)
    );
    let original = duplicate.clone().into_structured().unwrap();
    let decoded = SpeechFeatureBundle::from_structured(original.clone()).unwrap();
    assert_eq!(
        validate_feature_bundle(&decoded),
        Err(LocalSemanticRefusal::DuplicateFeatureIdentity)
    );
    assert_eq!(decoded.into_structured().unwrap(), original);
    let keys: Vec<_> = (0..16).map(|index| format!("feature/{index}")).collect();
    let entries: Vec<_> = keys
        .iter()
        .map(|key| (key.as_str(), FeatureSpecification::unspecified()))
        .collect();
    assert_eq!(validate_feature_bundle(&bundle(&entries)), Ok(()));
    let mut entries = entries;
    entries[15].0 = entries[0].0;
    assert_eq!(
        validate_feature_bundle(&bundle(&entries)),
        Err(LocalSemanticRefusal::DuplicateFeatureIdentity)
    );
}

fn candidate(
    text: &str,
    split: u32,
    stable: &str,
    unstable: &str,
    words: Option<&str>,
    count: u32,
) -> AsrTranscriptCandidate {
    AsrTranscriptCandidate::new(
        ListeningSegmentId::new("recognition/1".into()).unwrap(),
        Some(IeeeF32::from_value(-0.0)),
        split,
        stable.into(),
        count,
        words.map(str::to_owned),
        text.into(),
        unstable.into(),
    )
    .unwrap()
}

#[test]
fn stability_snapshots_preserve_utf8_bytes_and_pinned_word_boundaries() {
    for value in [
        candidate("", 0, "", "", None, 0),
        candidate("été 世界", 6, "été ", "世界", Some("été"), 1),
        candidate("hello world", 9, "hello wor", "ld", Some("hello"), 1),
        candidate("hé\u{2003}x", 7, "hé\u{2003}x", "", Some("hé"), 1),
        candidate(" hello world", 7, " hello ", "world", Some(" hello"), 1),
        candidate("hello", 5, "hello", "", None, 0),
    ] {
        assert_eq!(validate_transcript_candidate(&value), Ok(()));
        let original = value.into_structured().unwrap();
        let decoded = AsrTranscriptCandidate::from_structured(original.clone()).unwrap();
        assert_eq!(validate_transcript_candidate(&decoded), Ok(()));
        assert_eq!(decoded.confidence().unwrap().bits(), (-0.0_f32).to_bits());
        assert_eq!(decoded.into_structured().unwrap(), original);
    }
}

#[test]
fn malformed_stability_metadata_refuses_after_decode_without_clamping() {
    for (value, refusal) in [
        (
            candidate("été", 6, "été", "", None, 0),
            LocalSemanticRefusal::StablePrefixOutOfRange,
        ),
        (
            candidate("été", 4, "ét", "é", None, 0),
            LocalSemanticRefusal::StablePrefixNotUtf8Boundary,
        ),
        (
            candidate("hello world", 6, "HELLO ", "world", Some("hello"), 1),
            LocalSemanticRefusal::InconsistentTranscriptSplit,
        ),
        (
            candidate("hello world", 6, "hello ", "word", Some("hello"), 1),
            LocalSemanticRefusal::InconsistentTranscriptSplit,
        ),
        (
            candidate("hello world", 6, "hello ", "world", None, 0),
            LocalSemanticRefusal::InconsistentStableWords,
        ),
        (
            candidate("hello world", 6, "hello ", "world", Some("hello"), 2),
            LocalSemanticRefusal::InconsistentStableWords,
        ),
    ] {
        let original = value.into_structured().unwrap();
        let decoded = AsrTranscriptCandidate::from_structured(original.clone()).unwrap();
        assert_eq!(validate_transcript_candidate(&decoded), Err(refusal));
        assert_eq!(decoded.into_structured().unwrap(), original);
    }
}
