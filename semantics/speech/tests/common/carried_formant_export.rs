//! Optional retained technical evidence selected by the supported native demo.
use super::fixture;
use conduit_core::*;
use conduit_plot::rust_binding::NativeRustBinding;

pub struct Export<'a> {
    pub f: &'a fixture::Fixture,
    pub original: &'a [u8],
    pub identity: &'a [u8; 32],
    pub projection: &'a [u8; 32],
    pub projection_bytes: &'a [u8],
    pub admission: &'a [u8],
    pub plan: &'a Plan,
    pub plays: &'a std::collections::BTreeMap<HostId, ActivePlayId>,
    pub signs: &'a std::collections::BTreeMap<HostId, Vec<conduit_kernel::KernelEvent>>,
    pub pcm: &'a [u8],
}
pub fn retain(evidence: Export<'_>) {
    let Export {
        f,
        original,
        identity,
        projection,
        projection_bytes,
        admission,
        plan,
        plays,
        signs,
        pcm,
    } = evidence;
    let Some(root) = std::env::var_os("CONDUIT_CARRIED_FORMANT_OUTPUT") else {
        return;
    };
    let root = std::path::PathBuf::from(root).join("shared-carrier");
    std::fs::create_dir_all(&root).unwrap();
    let encode_many = |parts: Vec<Vec<u8>>| postcard::to_allocvec(&parts).unwrap();
    let request = fixture::commit_request();
    let result = fixture::execute_commitment(&request);
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
        request.encode().unwrap(),
        result.encode().unwrap(),
        admission.to_vec(),
        original.to_vec(),
        projection_bytes.to_vec(),
        f.lexical_profile.clone().encode().unwrap(),
        conduit_language::lexical::prepare_lexical_tape(&f.source, &f.lexical_profile, None)
            .unwrap()
            .tape()
            .clone()
            .encode()
            .unwrap(),
        fixture::commitment_source().into_bytes(),
    ]);
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
    let report = serde_json::json!({
        "text":"you would", "proof_class":"installed factory and execution kernel; offline authored PCM computation",
        "source_identity":hex(identity), "projection_identity":hex(projection),
        "original_artifact_identity":hex(&semantic_digest("proof/carried-formant-artifact@1", &packet)),
        "wav_identity":hex(&semantic_digest("proof/carried-formant-wav@1", &wav)),
        "compiled_dsp_source":conduit_speech::SOURCE_ID,
        "plan":plan, "plays":plays, "signs":signs,
        "sample_rate_hz":8000, "pcm_frames":pcm.len()/2,
        "human_listening":"not performed", "device_playback":"not performed",
        "neural_realization":"not established by this formant-only receipt",
    });
    std::fs::write(
        root.join("you-would.json"),
        serde_json::to_vec_pretty(&report).unwrap(),
    )
    .unwrap();
}
