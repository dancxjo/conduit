use conduit_audio::{PcmChannelLayout, PcmFrameHeader, PcmSampleRepresentation};
use conduit_form::{
    check_syntax_document, expand_canonical_form_for_authoring, parse_syntax_document,
    ProfileCatalog, StartupCatalog,
};
use conduit_tongues::{
    decode_speech_recognition_result, encode_speech_recognition_result,
    install_speech_recognition_catalog, project_recognized_text, speech_clip_recognition_contract,
    speech_recognition_contract, speech_recognition_to_text_contract, RecognitionTextRefusal,
    RecordedSpeechRecognizer, SpeechRecognitionAttempt, SpeechRecognitionDisposition,
    SpeechRecognitionRefusal, SpeechRecognitionValueError, MAXIMUM_RECOGNITION_AUDIO_BYTES,
    MAXIMUM_RECOGNITION_FIXTURES, MAXIMUM_RECOGNIZED_TEXT_BYTES, SPEECH_RECOGNITION_RESULT_KIND,
    SPEECH_RECOGNITION_TO_TEXT_KIND, SPEECH_RECOGNIZE_KIND, STREAMING_SPEECH_RECOGNIZE_KIND,
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
fn recognition_results_are_canonical_and_project_only_recognized_text() {
    let audio = pcm(&[12, -8, 24, -16]);
    let recognizer = RecordedSpeechRecognizer::new(&[(&audio, "Rosehip House, status")]).unwrap();
    let SpeechRecognitionAttempt::Result(recognized) = recognizer.recognize(&audio).unwrap() else {
        panic!("fixture was not recognized")
    };
    let encoded = encode_speech_recognition_result(&recognized).unwrap();
    assert_eq!(
        decode_speech_recognition_result(&encoded).unwrap(),
        recognized
    );
    assert_eq!(
        project_recognized_text(&encoded).unwrap(),
        b"Rosehip House, status"
    );
    let mut noncanonical = encoded;
    noncanonical.push(b' ');
    assert_eq!(
        decode_speech_recognition_result(&noncanonical),
        Err(SpeechRecognitionValueError::NonCanonical)
    );

    let SpeechRecognitionAttempt::Result(no_speech) =
        recognizer.recognize(&pcm(&[0, 0, 0, 0])).unwrap()
    else {
        panic!("silence did not produce a result")
    };
    assert_eq!(
        project_recognized_text(&encode_speech_recognition_result(&no_speech).unwrap()),
        Err(RecognitionTextRefusal::NotRecognized)
    );
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
    assert_eq!(
        speech_recognition_to_text_contract().kind_id.as_str(),
        SPEECH_RECOGNITION_TO_TEXT_KIND
    );
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

#[test]
fn clip_recognition_is_separate_from_the_accepted_single_frame_contract() {
    let frame = speech_recognition_contract();
    let clip = speech_clip_recognition_contract();
    assert_ne!(clip.kind_id, frame.kind_id);
    assert_ne!(clip.kind_contract_revision, frame.kind_contract_revision);
    assert_eq!(
        clip.inputs[0].value_kind.as_str(),
        conduit_audio::AUDIO_PCM_CLIP_INFO_ID
    );
    assert_eq!(clip.outputs, frame.outputs);
    assert_eq!(
        clip.limits.max_queue_bytes,
        conduit_audio::MAXIMUM_PCM_CLIP_BYTES as u32
    );
}

#[test]
fn ordinary_form_consumes_pcm_and_emits_only_committed_chat_messages_as_flows() {
    let source = r#"
form live-recognized-turn (
    audio: PcmFrames...| > message: ChatMessage...|
) {
    recognize: speech/recognize-stream
    commit: speech/commit-recognized-turn

    audio > recognize.audio
    recognize.events > commit.events
    commit.message > message
}
"#;
    let mut startup = StartupCatalog::new();
    let mut profile = ProfileCatalog::new();
    startup
        .insert_value_kind_alias(
            "PcmFrames",
            conduit_core::kind_id(conduit_audio::AUDIO_PCM_INFO_ID),
        )
        .unwrap();
    startup
        .insert_value_kind_alias(
            "ChatMessage",
            conduit_core::kind_id(conduit_tongues::CHAT_MESSAGE_VALUE_KIND),
        )
        .unwrap();
    install_speech_recognition_catalog(&mut startup, &mut profile).unwrap();
    let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
    let expanded =
        expand_canonical_form_for_authoring(&checked, "live-recognized-turn", &profile).unwrap();
    assert_eq!(expanded.input_bindings.len(), 1);
    assert_eq!(expanded.output_bindings.len(), 1);
    assert!(expanded
        .expanded
        .gears
        .iter()
        .any(|gear| gear.kind_id.as_str() == STREAMING_SPEECH_RECOGNIZE_KIND));
}

#[test]
fn recognition_result_v2_bound_covers_maximum_escaped_semantic_fields() {
    let result = conduit_tongues::SpeechRecognitionResult {
        disposition: conduit_tongues::SpeechRecognitionDisposition::Recognized,
        text: Some("\u{0001}".repeat(conduit_tongues::MAXIMUM_RECOGNIZED_TEXT_BYTES)),
        audio_sha256: [255; 32],
        audio_extent_bytes: conduit_audio::MAXIMUM_PCM_CLIP_BYTES as u32,
        provider_identity: "\u{0002}".repeat(
            conduit_tongues::MAXIMUM_RECOGNITION_PROVIDER_IDENTITY_BYTES,
        ),
    };
    let encoded = conduit_tongues::encode_speech_recognition_result(&result)
        .expect("declared v2 result bound admits every semantically valid field maximum");
    assert!(encoded.len() <= conduit_tongues::MAXIMUM_RECOGNITION_RESULT_BYTES);
    assert_eq!(
        conduit_tongues::decode_speech_recognition_result(&encoded).unwrap(),
        result
    );
}
