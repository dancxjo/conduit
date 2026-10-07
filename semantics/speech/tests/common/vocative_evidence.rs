//! Optional inspectable proof output; ordinary tests leave the filesystem alone.
use super::{intent::Composite, language::Case};
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_speech::{
    lexical_pronunciation::PreparedPronunciation, playback_basis::PreparedSpeechPlaybackTape,
};
use std::fmt::Write;
fn hex<T: NativeRustBinding + Clone>(value: &T) -> String {
    value
        .clone()
        .encode()
        .unwrap()
        .iter()
        .fold(String::new(), |mut text, byte| {
            write!(&mut text, "{byte:02x}").unwrap();
            text
        })
}
pub fn retain(
    position: usize,
    case: &Case,
    pronunciation: &[PreparedPronunciation<'_>],
    composite: &Composite,
    tape: &PreparedSpeechPlaybackTape<'_>,
    pcm: &[u8],
    replacement: &PreparedSpeechPlaybackTape<'_>,
) {
    let Ok(directory) = std::env::var("CONDUIT_VOCATIVE_PROOF_DIR") else {
        return;
    };
    std::fs::create_dir_all(&directory).unwrap();
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
        std::path::Path::new(&directory).join(format!("vocative-{position}.wav")),
        wav,
    )
    .unwrap();
    let correction =
        conduit_speech::playback_revision::PreparedPlaybackChange::correction(tape, replacement)
            .unwrap();
    let correction = correction.correction_data().unwrap();
    let mut json = format!("{{\"proof\":\"supplied-canonical-graph@1\",\"parser_accuracy\":false,\"physical_playback\":false,\"playback_occurrence_limit\":32,\"position\":{position},\"text\":{:?},\"epoch_played_frames\":128,\"epoch_queued_frames\":128,\"played_ack_provenance\":\"manual-effect-owner-fixture\",\"scheduler_delivery_played_frames\":0,\"receipt_encoding\":\"canonical-hex@1\",\"basis\":{:?},\"replacement_basis\":{:?},\"lexical_profile\":{:?},\"correction\":{:?},\"lineage\":{:?},\"pronunciation\":[", case.lexical.tape().source().material().text(), hex(tape.basis()), hex(replacement.basis()), hex(case.lexical.tape().profile()), hex(correction), hex(correction.lineage()));
    for (index, receipt) in pronunciation.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        write!(&mut json, "{{\"selection_request\":{:?},\"candidate\":{:?},\"candidate_index\":{:?},\"request\":{:?},\"result\":{:?},\"row_index\":{:?},\"original_word_intent\":{:?}}}",hex(receipt.selection().request()),hex(receipt.selection().candidate()),hex(receipt.selection().candidate_selection()),hex(receipt.request()),hex(receipt.result()),hex(receipt.row_selection()),hex(&composite.words[index])).unwrap();
    }
    json.push_str("],\"phonetic_correspondence\":[");
    for (index, (old, current)) in composite.correspondence.iter().enumerate() {
        if index > 0 {
            json.push(',');
        }
        write!(
            &mut json,
            "{{\"previous\":{:?},\"current\":{:?}}}",
            hex(old),
            hex(current)
        )
        .unwrap();
    }
    json.push_str("],\"asserted_transitions\":[\"stable-does-not-commit\",\"preplay-revise-requires-replan\",\"withdraw-before-queue\",\"staged-does-not-queue\",\"queued-does-not-attest-played\",\"owned-played-ack-freezes-epoch\",\"stale-correction-refused\",\"correction-retains-old-receipt\",\"atomic-pressure-preserves-pcm\",\"cancel-stops-future-queue\"]}");
    std::fs::write(
        std::path::Path::new(&directory).join(format!("vocative-{position}.json")),
        json,
    )
    .unwrap();
}
