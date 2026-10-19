//! Optional retained technical evidence selected by the supported native demo.
use super::fixture;
use conduit_core::*;
use conduit_plot::rust_binding::NativeRustBinding;

pub struct Export<'a> {
    pub f: &'a fixture::Fixture,
    pub model: &'a super::model::ModelTerminalEvidence,
    pub request: &'a conduit_language::LanguageParserBeamCommitRequest,
    pub result: &'a conduit_language::LanguageParserBeamCommitResult,
    pub original: &'a [u8],
    pub identity: &'a [u8; 32],
    pub projection: &'a [u8; 32],
    pub sample_basis: &'a conduit_audio::AudioSampleRateBasis,
    pub projection_bytes: &'a [u8],
    pub admission: &'a [u8],
    pub plan: &'a Plan,
    pub plays: &'a std::collections::BTreeMap<HostId, ActivePlayId>,
    pub signs: &'a std::collections::BTreeMap<HostId, Vec<conduit_kernel::KernelEvent>>,
    pub pcm: &'a [u8],
}
pub fn retain(evidence: Export<'_>) -> Vec<u8> {
    let Export {
        f,
        model,
        request,
        result,
        original,
        identity,
        projection,
        projection_bytes,
        sample_basis,
        admission,
        plan,
        plays,
        signs,
        pcm,
    } = evidence;
    let encode_many = |parts: Vec<Vec<u8>>| postcard::to_allocvec(&parts).unwrap();

    let packet = encode_many(vec![
        f.source.clone().encode().unwrap(),
        f.intent.clone().encode().unwrap(),
        f.phonemes.clone().encode().unwrap(),
        f.phones.clone().encode().unwrap(),
        f.definitions.clone().encode().unwrap(),
        f.notation_profile.clone().encode().unwrap(),
        f.alignment.clone().encode().unwrap(),
        encode_many(
            f.syllables
                .iter()
                .map(|v| v.clone().encode().unwrap())
                .collect(),
        ),
        encode_many(
            f.positioned_boundaries
                .iter()
                .map(|v| v.clone().encode().unwrap())
                .collect(),
        ),
        encode_many(
            f.word_phonemes
                .iter()
                .map(|v| v.clone().encode().unwrap())
                .collect(),
        ),
        f.formant.clone().encode().unwrap(),
        f.boundaries.clone().encode().unwrap(),
        request.clone().encode().unwrap(),
        result.clone().encode().unwrap(),
        admission.to_vec(),
        model.correspondence_bytes.clone(),
        projection_bytes.to_vec(),
        f.lexical_profile.clone().encode().unwrap(),
        conduit_language::lexical::prepare_lexical_tape(&f.source, &f.lexical_profile, None)
            .unwrap()
            .tape()
            .clone()
            .encode()
            .unwrap(),
        fixture::commitment_source().into_bytes(),
        sample_basis.clone().encode().unwrap(),
        model.vocabulary_bytes.clone(),
        model.checkpoint_bytes.clone(),
        model.terminal_evidence_identity.to_vec(),
        original.to_vec(),
        identity.to_vec(),
    ]);
    let Some(root) = std::env::var_os("CONDUIT_CARRIED_FORMANT_OUTPUT") else {
        return packet;
    };
    let root = std::path::PathBuf::from(root).join("shared-carrier");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("you-would.native"), &packet).unwrap();
    let mut wav = Vec::new();
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + pcm.len() as u32).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16_u32.to_le_bytes());
    wav.extend_from_slice(&1_u16.to_le_bytes());
    wav.extend_from_slice(&1_u16.to_le_bytes());
    wav.extend_from_slice(&8000_u32.to_le_bytes());
    wav.extend_from_slice(&16000_u32.to_le_bytes());
    wav.extend_from_slice(&2_u16.to_le_bytes());
    wav.extend_from_slice(&16_u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&(pcm.len() as u32).to_le_bytes());
    wav.extend_from_slice(pcm);
    std::fs::write(root.join("you-would.wav"), &wav).unwrap();
    let hex = |bytes: &[u8]| {
        bytes
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    };
    let signs = signs
        .iter()
        .map(|(host, events)| {
            (host.as_str(), events.iter().map(|event| serde_json::json!({
        "sequence": event.sequence, "node": event.node.0,
        "port": event.port.map(|port| port.0), "request": event.request.map(|request| request.0),
        "kind": format!("{:?}", event.kind),
    })).collect::<Vec<_>>())
        })
        .collect::<std::collections::BTreeMap<_, _>>();
    let report = serde_json::json!({
        "text":"you would", "proof_class":"installed factory and execution kernel; offline authored PCM computation",
        "source_identity":hex(identity), "projection_identity":hex(projection),
        "formant_retained_without_realization":["SeparateSyllableStructure", "ScopedFrequencyEnergyAndRate", "LanguageCommitment"],
        "correspondence_subset_identity":hex(&model.correspondence_identity),
        "original_artifact_identity":hex(&semantic_digest("proof/carried-formant-artifact@1", &packet)),
        "wav_identity":hex(&semantic_digest("proof/carried-formant-wav@1", &wav)),
        "compiled_dsp_source":conduit_speech::SOURCE_ID,
        "speech_authoring_source":conduit_speech::semantic::IPA_CONSTRUCTOR_SOURCE_ID,
        "native_schema_digests": {
            "SpeechUtteranceIntent":hex(&conduit_speech::semantic::SpeechUtteranceIntent::semantic_type().unwrap().semantic_digest().unwrap()),
            "SpeechIpaInventoryNotationBasis":hex(&conduit_speech::semantic::SpeechIpaInventoryNotationBasis::semantic_type().unwrap().semantic_digest().unwrap()),
            "SpeechOfflinePcmAdmission":hex(&conduit_speech::semantic::SpeechOfflinePcmAdmission::semantic_type().unwrap().semantic_digest().unwrap()),
            "SpeechModelLinguisticVocabulary":hex(&conduit_speech::semantic::SpeechModelLinguisticVocabulary::semantic_type().unwrap().semantic_digest().unwrap()),
            "SpeechModelCheckpointProjection":hex(&conduit_speech::semantic::SpeechModelCheckpointProjection::semantic_type().unwrap().semantic_digest().unwrap()),
            "AudioSampleRateBasis":hex(&conduit_audio::AudioSampleRateBasis::semantic_type().unwrap().semantic_digest().unwrap()),
        },
        "plan":plan, "plays":plays, "signs":signs,
        "sample_rate_hz":8000, "pcm_frames":pcm.len()/2,
        "human_listening":"not performed", "device_playback":"not performed",
        "model_terminal": {
            "requested_backend":model.requested_backend,
            "selected_backend":model.selected_backend,
            "refusal":format!("{:?}", model.refusal),
            "source_identity":hex(&model.source_identity),
            "terminal_evidence_identity":hex(&model.terminal_evidence_identity),
            "training_phonemes":model.training_phoneme_count,
            "training_phones":model.training_phone_count,
            "inference_phonemes":model.inference_phoneme_count,
            "inference_phones":model.inference_phone_count,
            "trained_weights":false,
            "retained_without_conditioning":model.retained_without_conditioning,

        },
    });
    std::fs::write(
        root.join("you-would.json"),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .unwrap();
    packet
}
