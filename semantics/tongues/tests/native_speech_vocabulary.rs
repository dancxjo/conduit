use conduit_form::rust_binding::NativeRustBinding;
use conduit_tongues::{
    no_speech_result, recognized_result, CommittedUserMessage, RecognitionTextRefusal,
    SpeakableSegment, SpeechCommitReason, SpeechCommitRefusal, SpeechRecognitionAttempt,
    SpeechRecognitionAudioDigest, SpeechRecognitionDisposition, SpeechRecognitionRefusal,
    SpeechRecognitionResult, SpeechRecognitionValueError, StreamingRecognitionRefusal,
};

fn round_trip<T>(value: T)
where
    T: NativeRustBinding + Clone + core::fmt::Debug + PartialEq,
{
    let structured = value.clone().into_structured().unwrap();
    assert_eq!(T::from_structured(structured).unwrap(), value);
}

#[test]
fn speech_commit_and_recognition_vocabularies_are_native() {
    round_trip(
        SpeakableSegment::new(
            "stream/1".into(),
            3,
            "Hello there.".into(),
            SpeechCommitReason::TonguesBoundary,
        )
        .unwrap(),
    );
    round_trip(
        CommittedUserMessage::new(
            "tongues/1/turn".into(),
            "user".into(),
            "committed-external-speech".into(),
            "Hello there.".into(),
        )
        .unwrap(),
    );
    for value in [
        SpeechCommitReason::TonguesBoundary,
        SpeechCommitReason::FinalFlush,
    ] {
        round_trip(value);
    }
    for value in [
        SpeechCommitRefusal::EmptyStreamIdentity,
        SpeechCommitRefusal::EmptyDelta,
        SpeechCommitRefusal::PendingBoundExceeded,
        SpeechCommitRefusal::SegmentBoundExceeded,
        SpeechCommitRefusal::SegmentQueueFull,
        SpeechCommitRefusal::AlreadyClosed,
    ] {
        round_trip(value);
    }
    for value in [
        SpeechRecognitionDisposition::Recognized,
        SpeechRecognitionDisposition::NoSpeech,
    ] {
        round_trip(value);
    }
    for value in [
        SpeechRecognitionRefusal::EmptyFixtures,
        SpeechRecognitionRefusal::TooManyFixtures,
        SpeechRecognitionRefusal::EmptyTranscript,
        SpeechRecognitionRefusal::TranscriptTooLarge,
        SpeechRecognitionRefusal::DuplicateAudio,
        SpeechRecognitionRefusal::AudioTooLarge,
        SpeechRecognitionRefusal::InvalidPcm,
        SpeechRecognitionRefusal::UnsupportedPcmProfile,
    ] {
        round_trip(value);
    }
}

#[test]
fn speech_value_and_stream_refusals_are_native() {
    for value in [
        SpeechRecognitionValueError::BoundExceeded,
        SpeechRecognitionValueError::Malformed,
        SpeechRecognitionValueError::NonCanonical,
        SpeechRecognitionValueError::InvalidValue,
    ] {
        round_trip(value);
    }
    for value in [
        RecognitionTextRefusal::InvalidResult,
        RecognitionTextRefusal::NotRecognized,
    ] {
        round_trip(value);
    }
    for value in [
        StreamingRecognitionRefusal::BoundExceeded,
        StreamingRecognitionRefusal::InvalidEvent,
    ] {
        round_trip(value);
    }
}

#[test]
fn established_json_representations_remain_exact_boundary_adapters() {
    assert_eq!(
        serde_json::to_string(&SpeechCommitReason::TonguesBoundary).unwrap(),
        "\"tongues-boundary\""
    );
    assert_eq!(
        serde_json::from_str::<SpeechCommitReason>("\"final-flush\"").unwrap(),
        SpeechCommitReason::FinalFlush
    );
    assert_eq!(
        serde_json::to_string(&SpeechRecognitionDisposition::NoSpeech).unwrap(),
        "\"NoSpeech\""
    );
    assert_eq!(
        serde_json::from_str::<SpeechRecognitionDisposition>("\"Recognized\"").unwrap(),
        SpeechRecognitionDisposition::Recognized
    );
}

#[test]
fn recognition_results_and_attempts_are_native_payload_rich_types() {
    let recognized = recognized_result(
        [7; 32],
        320,
        "fixture/provider@1".into(),
        "Hello Margret".into(),
    )
    .unwrap();
    round_trip(recognized.clone());
    round_trip(no_speech_result([0; 32], 320, "fixture/provider@1".into()).unwrap());
    round_trip(SpeechRecognitionAttempt::result(recognized).unwrap());
    round_trip(
        SpeechRecognitionAttempt::failed(SpeechRecognitionAudioDigest::new([9; 32]).unwrap())
            .unwrap(),
    );
    round_trip(SpeechRecognitionAttempt::ResourceUnavailable);

    assert!(matches!(
        recognized_result([1; 32], 1, "fixture/provider@1".into(), "hello".into(),).unwrap(),
        SpeechRecognitionResult::Recognized(_)
    ));
    assert!(recognized_result([1; 32], 1, "fixture/provider@1".into(), "x".repeat(257),).is_err());
    assert!(no_speech_result([1; 32], 786_433, "fixture/provider@1".into()).is_err());
}
