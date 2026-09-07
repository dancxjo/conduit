use conduit_audio::{PcmChannelLayout, PcmFrameHeader, PcmSampleRepresentation};
use conduit_form::{ProfileCatalog, StartupCatalog};
use conduit_tongues::{
    install_speech_recognition_catalog, speech_recognition_contract, RecordedSpeechRecognizer,
    SpeechRecognitionAttempt, SpeechRecognitionDisposition, SpeechRecognitionRefusal,
    MAXIMUM_RECOGNITION_AUDIO_BYTES, MAXIMUM_RECOGNITION_FIXTURES, MAXIMUM_RECOGNIZED_TEXT_BYTES,
    SPEECH_RECOGNITION_RESULT_KIND, SPEECH_RECOGNIZE_KIND,
};

fn pcm(samples: &[i16]) -> Vec<u8> {
    let payload = samples
        .iter()
        .flat_map(|sample| sample.to_le_bytes())
        .collect::<Vec<_>>();
    PcmFrameHeader::new(
        PcmSampleRepresentation::Signed16LittleEndian,
        16_000,
        PcmChannelLayout::Mono,
        samples.len() as u16,
        1,
        0,
        false,
    )
    .unwrap()
    .encode_frame(&payload)
    .unwrap()
}

#[test]
fn recorded_audio_recognition_no_speech_and_failure_remain_distinct() {
    let addressed = pcm(&[12, -8, 24, -16]);
    let recognizer = RecordedSpeechRecognizer::new(&[(&addressed, "Rosehip House, status")])
        .expect("one exact recorded fixture is admitted");
    let SpeechRecognitionAttempt::Result(recognized) = recognizer.recognize(&addressed).unwrap()
    else {
        panic!("recorded fixture was not recognized")
    };
    assert_eq!(
        recognized.disposition,
        SpeechRecognitionDisposition::Recognized
    );
    assert_eq!(recognized.text.as_deref(), Some("Rosehip House, status"));

    let SpeechRecognitionAttempt::Result(no_speech) =
        recognizer.recognize(&pcm(&[0, 0, 0, 0])).unwrap()
    else {
        panic!("silence did not produce a semantic result")
    };
    assert_eq!(
        no_speech.disposition,
        SpeechRecognitionDisposition::NoSpeech
    );
    assert_eq!(no_speech.text, None);

    let SpeechRecognitionAttempt::Failed { audio_sha256 } =
        recognizer.recognize(&pcm(&[1, 2, 3, 4])).unwrap()
    else {
        panic!("unknown audio did not retain a failed attempt")
    };
    assert_ne!(recognized.audio_sha256, audio_sha256);

    let unavailable = RecordedSpeechRecognizer::resource_unavailable();
    assert_eq!(unavailable, SpeechRecognitionAttempt::ResourceUnavailable);
}

#[test]
fn fixture_and_audio_bounds_refuse_before_recognition() {
    assert_eq!(
        RecordedSpeechRecognizer::new(&[]),
        Err(SpeechRecognitionRefusal::EmptyFixtures)
    );
    let audio = pcm(&[1]);
    let too_many = vec![(&audio[..], "x"); MAXIMUM_RECOGNITION_FIXTURES + 1];
    assert_eq!(
        RecordedSpeechRecognizer::new(&too_many),
        Err(SpeechRecognitionRefusal::TooManyFixtures)
    );
    assert_eq!(
        RecordedSpeechRecognizer::new(&[(&audio, &"x".repeat(MAXIMUM_RECOGNIZED_TEXT_BYTES + 1))]),
        Err(SpeechRecognitionRefusal::TranscriptTooLarge)
    );
    assert_eq!(
        RecordedSpeechRecognizer::new(&[(&audio, "one"), (&audio, "two")]),
        Err(SpeechRecognitionRefusal::DuplicateAudio)
    );
    let recognizer = RecordedSpeechRecognizer::new(&[(&audio, "one")]).unwrap();
    assert_eq!(
        recognizer.recognize(&vec![0; MAXIMUM_RECOGNITION_AUDIO_BYTES + 1]),
        Err(SpeechRecognitionRefusal::AudioTooLarge)
    );
    assert_eq!(
        recognizer.recognize(b"not-pcm"),
        Err(SpeechRecognitionRefusal::InvalidPcm)
    );
}

#[test]
fn portable_contract_contains_no_engine_device_or_host_facts() {
    let contract = speech_recognition_contract();
    assert_eq!(contract.kind_id.as_str(), SPEECH_RECOGNIZE_KIND);
    assert_eq!(contract.inputs[0].value_kind.as_str(), "audio/pcm-frames@1");
    assert_eq!(
        contract.outputs[0].value_kind.as_str(),
        SPEECH_RECOGNITION_RESULT_KIND
    );
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    install_speech_recognition_catalog(&mut startup, &mut profile).unwrap();
    assert!(profile.get(&contract.kind_id).is_some());
    let debug = format!("{contract:?}");
    for forbidden in ["Whisper", "browser", "microphone", "Host", "HTTP"] {
        assert!(!debug.contains(forbidden), "found {forbidden}");
    }
}
