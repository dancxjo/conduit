//! Existing installed compact Formant route; never a replacement renderer.
//! Callers must provide an admitted complete carrier for the exact source.
use conduit_audio::*;
use conduit_plot::rust_binding::BoundedSequence;
use conduit_speech::{
    carried_formant::{FrozenCarriedFormant, PreparedOfflinePcm},
    complete_carrier::PreparedCompleteSpeechCarrier,
    record_pronunciation_commitment::CommittedRecordPronunciation,
    semantic::*,
};
use conduit_std_host::carried_formant::CarriedFormantFactory;
#[allow(dead_code)]
#[path = "../carried_formant_execution.rs"]
mod execution;

/// This helper deliberately cannot admit a source by a caller-supplied flag.
/// The Language session borrow stays live through installed execution.
pub fn execute(
    pronunciation: &CommittedRecordPronunciation<'_, '_, '_, '_>,
    complete: &PreparedCompleteSpeechCarrier<'_, '_, '_>,
    formant: &SpeechFormantVoiceProfile,
    boundaries: &SpeechFormantBoundaryProfile,
    evidence: SpeechEvidenceProvenance,
) -> Vec<u8> {
    // Check the complete-carrier source before freezing it into owned bytes.
    assert_eq!(
        complete.correspondence().text(),
        pronunciation
            .supplied()
            .selection()
            .request()
            .source()
            .material()
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
    let offline = PreparedOfflinePcm::prepare(frozen, declaration).unwrap();
    let factory = CarriedFormantFactory::new(offline, 7).unwrap();
    let (host, _plan) = execution::prepare(factory);
    let result = execution::run_with_frame_count(host, &identity, expected_frames);
    assert!(!result.plays.is_empty());
    assert!(!result.pcm.is_empty());
    assert!(!result.signs.is_empty());
    assert!(result.signs.values().all(|events| !events.is_empty()));
    // Keep the exact borrowed authority in use after Play, rather than dropping
    // it during preparation and pretending frozen bytes confer currentness.
    assert_eq!(
        pronunciation.authority().receipt().arc(),
        pronunciation.supplied().selection().request().basis()
    );
    result.pcm
}
