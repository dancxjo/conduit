//! Actual graph fixture receipts, with physical and heldout boundaries explicit.
use super::{intent::Composite, language::Case};
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_speech::{
    lexical_pronunciation::PreparedPronunciation, playback_basis::PreparedSpeechPlaybackTape,
};
use serde_json::Value;
fn native<T: NativeRustBinding + Clone>(value: &T) -> Vec<u8> {
    value.clone().encode().unwrap()
}
#[allow(clippy::too_many_arguments)]
pub fn retain(
    position: usize,
    graph: &Value,
    case: &Case,
    pronunciation: &[PreparedPronunciation<'_>],
    composite: &Composite,
    tape: &PreparedSpeechPlaybackTape<'_>,
    pcm: &[u8],
    epoch: Value,
) {
    let Ok(directory) = std::env::var("CONDUIT_LEARNED_PLAYBACK_PROOF_DIR") else {
        return;
    };
    let directory = std::path::Path::new(&directory);
    std::fs::create_dir_all(directory).unwrap();
    let mut wav = Vec::with_capacity(44 + pcm.len());
    wav.extend_from_slice(b"RIFF");
    wav.extend_from_slice(&(36 + pcm.len() as u32).to_le_bytes());
    wav.extend_from_slice(b"WAVEfmt ");
    wav.extend_from_slice(&16u32.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&1u16.to_le_bytes());
    wav.extend_from_slice(&8000u32.to_le_bytes());
    wav.extend_from_slice(&16000u32.to_le_bytes());
    wav.extend_from_slice(&2u16.to_le_bytes());
    wav.extend_from_slice(&16u16.to_le_bytes());
    wav.extend_from_slice(b"data");
    wav.extend_from_slice(&(pcm.len() as u32).to_le_bytes());
    wav.extend_from_slice(pcm);
    std::fs::write(
        directory.join(format!("learned-vocative-{position}.wav")),
        wav,
    )
    .unwrap();
    let receipts = pronunciation.iter().enumerate().map(|(index, receipt)| serde_json::json!({
        "selection_request_bytes":native(receipt.selection().request()),"candidate_bytes":native(receipt.selection().candidate()),
        "candidate_selection_bytes":native(receipt.selection().candidate_selection()),"pronunciation_request_bytes":native(receipt.request()),
        "pronunciation_result_bytes":native(receipt.result()),"row_selection_bytes":native(receipt.row_selection()),
        "original_word_intent_bytes":native(&composite.words[index])
    })).collect::<Vec<_>>();
    let correspondence = composite.correspondence.iter().map(|(old,current)| serde_json::json!({"previous_bytes":native(old),"current_bytes":native(current)})).collect::<Vec<_>>();
    let participation = case.participation.iter().map(|role| serde_json::json!({"request_bytes":native(role.request()),"result_bytes":native(role.result())})).collect::<Vec<_>>();
    let evidence = serde_json::json!({"proof":"actual-native-graph-playback@1","position":position,
        "text":case.lexical.tape().source().material().text(),"heldout_accuracy":false,"general_parser_accuracy":false,
        "physical_playback":false,"renderer":"formant","fargan_neural_waveform":false,"scheduler_delivery_played_frames":0,
        "pcm_frames":pcm.len()/2,"native_encoding":"canonical-bytes@1","playback_occurrence_limit":32,
        "graph_receipt":graph,"playback_basis_bytes":native(tape.basis()),"pronunciation":receipts,"token_participation":participation,"spoken_ordinals":case.spoken_ordinals,
        "phonetic_correspondence":correspondence,"epoch":epoch,"atomic_pressure_preserves_pcm":true,"cancel_stops_future_queue":true});
    std::fs::write(
        directory.join(format!("learned-vocative-{position}.json")),
        serde_json::to_vec_pretty(&evidence).unwrap(),
    )
    .unwrap();
}
