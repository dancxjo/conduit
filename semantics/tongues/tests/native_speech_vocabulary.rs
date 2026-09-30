use conduit_form::rust_binding::NativeRustBinding;
use conduit_tongues::{
    RecognitionTextRefusal, SpeechCommitReason, SpeechCommitRefusal, SpeechRecognitionDisposition,
    SpeechRecognitionRefusal, SpeechRecognitionValueError, StreamingRecognitionRefusal,
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
