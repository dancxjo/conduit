//! Existing installed compact Formant route; never a replacement renderer.
//! Callers must provide an admitted complete carrier for the exact source.
use conduit_audio::*;
use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding};
use conduit_speech::{
    carried_formant::{FrozenCarriedFormant, PreparedOfflinePcm},
    committed_prosody::CommittedSpeechProsody,
    complete_carrier::PreparedCompleteSpeechCarrier,
    semantic::*,
};
use conduit_std_host::carried_formant::CarriedFormantFactory;
#[allow(dead_code)]
#[path = "../carried_formant_execution.rs"]
mod execution;

/// This helper deliberately cannot admit a source by a caller-supplied flag.
/// The Language session borrow stays live through installed execution.
pub fn execute_control(
    prosody: &CommittedSpeechProsody<'_, '_, '_>,
    complete: &PreparedCompleteSpeechCarrier<'_, '_, '_>,
    formant: &SpeechFormantVoiceProfile,
    boundaries: &SpeechFormantBoundaryProfile,
    evidence: SpeechEvidenceProvenance,
    label: &str,
) -> Vec<u8> {
    // Check the complete-carrier source before freezing it into owned bytes.
    assert_eq!(
        complete.correspondence().text(),
        prosody.authority().receipt().lexical().source().material()
    );
    let anchor = AudioTrajectoryAnchor::new(
        AudioOriginIdentity::new(1).unwrap(),
        AudioTimelineIdentity::new(1).unwrap(),
    )
    .unwrap();
    let basis =
        AudioSampleRateBasis::new(anchor.clone(), AudioFrameQuantization::Floor, 8000).unwrap();
    let frozen =
        FrozenCarriedFormant::prepare(complete, formant, boundaries, &anchor, &basis).unwrap();
    let identity = *frozen.source_identity();
    let expected_frames = frozen.total_frames();
    let digest = |bytes: &[u8; 32]| {
        SpeechCarrierDigest::new(BoundedSequence::try_from_iter(bytes.iter().copied()).unwrap())
            .unwrap()
    };
    let declaration = SpeechOfflinePcmDeclaration::new(
        evidence,
        digest(frozen.projection_identity()),
        digest(&identity),
    )
    .unwrap();
    let capture = std::env::var_os("CONDUIT_VOCATIVE_PROOF_DIR").map(|directory| {
        let hex=|bytes:&[u8]| bytes.iter().map(|byte|format!("{byte:02x}")).collect::<String>();
        let receipt=serde_json::json!({
            "source_text":complete.correspondence().text().text(),
            "source_identity":hex(frozen.source_identity()),
            "projection_identity":hex(frozen.projection_identity()),
            "expected_frames":expected_frames,"sample_rate_hz":8000,"channels":1,"sample_bits":16,
            "sample_basis_canonical":hex(&frozen.sample_basis().clone().encode().unwrap()),
            "timing_canonical":frozen.timing_receipts().iter().map(|value|hex(&value.clone().encode().unwrap())).collect::<Vec<_>>(),
            "controls_canonical":frozen.control_receipts().iter().map(|(cycle,amplitude)|(cycle.as_ref().map(|value|hex(&value.clone().encode().unwrap())),amplitude.as_ref().map(|value|hex(&value.clone().encode().unwrap())))).collect::<Vec<_>>(),
            "commitment_request_canonical":hex(&complete.commitment_request().clone().encode().unwrap()),
            "commitment_result_canonical":hex(&complete.commitment_result().clone().encode().unwrap()),
            "committed_prosody_canonical":hex(&prosody.committed().admission().clone().encode().unwrap()),
            "acoustic_profile":"authored/vocative-neutral-extras@1", "variant_label":label,
            "proof_class":"local supplied-structure installed Source Plan Play; authored finite pronunciation"
        });
        (std::path::PathBuf::from(directory),frozen.original_bytes().to_vec(),frozen.projection_bytes().to_vec(),receipt)
    });
    let offline = PreparedOfflinePcm::prepare(frozen, declaration).unwrap();
    let factory = CarriedFormantFactory::new(offline, 7).unwrap();
    let (host, plan) = execution::prepare(factory);
    let result = execution::run_with_frame_count(host, &identity, expected_frames);
    assert!(!result.plays.is_empty());
    assert!(!result.pcm.is_empty());
    assert!(!result.signs.is_empty());
    assert!(result.signs.values().all(|events| !events.is_empty()));
    // Keep the exact borrowed authority in use after Play, rather than dropping
    // it during preparation and pretending frozen bytes confer currentness.
    assert_eq!(
        prosody.authority().receipt().arc(),
        prosody.committed().dependency().arc()
    );
    if let Some((directory, carrier, projection, mut receipt)) = capture {
        // Retain the actual checked/expanded Source IDs and selected Plan from
        // preparation, plus the actual Play map/signs returned by this execution.
        // Never reconstruct them later from an audio filename or a fixture ID.
        receipt["actual_plan"] = serde_json::to_value(&plan).unwrap();
        receipt["actual_plays"] = serde_json::to_value(&result.plays).unwrap();
        receipt["actual_signs"] = serde_json::Value::Array(result.signs.iter().map(|(host,events)|serde_json::json!({"host":host,"count":events.len(),"events_debug":events.iter().map(|event|format!("{event:?}")).collect::<Vec<_>>()})).collect());
        // All artifact I/O occurs after installed Play returned its actual PCM.
        std::fs::create_dir_all(&directory).unwrap();
        std::fs::write(directory.join(format!("{label}.pcm")), &result.pcm).unwrap();
        let size = u32::try_from(result.pcm.len()).unwrap();
        let mut wav = Vec::with_capacity(44 + result.pcm.len());
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(size + 36).to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&8000u32.to_le_bytes());
        wav.extend_from_slice(&16000u32.to_le_bytes());
        wav.extend_from_slice(&2u16.to_le_bytes());
        wav.extend_from_slice(&16u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&size.to_le_bytes());
        wav.extend_from_slice(&result.pcm);
        std::fs::write(directory.join(format!("{label}.wav")), wav).unwrap();
        std::fs::write(directory.join(format!("{label}.carrier")), carrier).unwrap();
        std::fs::write(directory.join(format!("{label}.projection")), projection).unwrap();
        std::fs::write(
            directory.join(format!("{label}.receipt.json")),
            serde_json::to_vec_pretty(&receipt).unwrap(),
        )
        .unwrap();
    }
    result.pcm
}

pub fn execute(
    bound: &conduit_speech::committed_prosody::CommittedSpeechCarrier<'_, '_, '_, '_, '_, '_>,
    formant: &SpeechFormantVoiceProfile,
    boundaries: &SpeechFormantBoundaryProfile,
    evidence: SpeechEvidenceProvenance,
    label: &str,
) -> Vec<u8> {
    execute_control(
        bound.prosody(),
        bound.carrier(),
        formant,
        boundaries,
        evidence,
        label,
    )
}
