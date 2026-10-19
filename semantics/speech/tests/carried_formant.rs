#![cfg(all(feature = "kernel", feature = "semantic-bindings"))]
#[path = "common/carried_formant_execution.rs"]
mod execution;
#[path = "common/carried_formant_export.rs"]
mod export;
#[path = "common/carried_formant_fixture.rs"]
mod fixture;
#[path = "common/carried_formant_inspection.rs"]
mod inspection;
#[path = "common/shared_projection_model.rs"]
mod model;
use conduit_core::*;
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    PortId as KernelPort, ValueRef,
};
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_speech::{
    carried_formant::{FrozenCarriedFormant, PreparedOfflinePcm},
    carried_formant_kernel::CarriedFormantBack,
    complete_carrier::{CompleteCarrierAcoustics, PreparedCompleteSpeechCarrier},
    semantic::*,
};
use conduit_std_host::carried_formant::CarriedFormantFactory;
use std::sync::Arc;

fn frozen(f: &fixture::Fixture) -> FrozenCarriedFormant {
    let inventory = f.inventory().unwrap();
    let correspondence = f.correspondence(&inventory).unwrap();
    let structure = f.structure();
    let evidence = f.complete_evidence();
    let original = PreparedCompleteSpeechCarrier::prepare(
        &correspondence,
        &structure,
        &evidence.lexical,
        &f.word_phonemes,
        &evidence.request,
        &evidence.result,
        CompleteCarrierAcoustics::not_supplied(),
    )
    .unwrap();
    let anchor = conduit_audio::AudioTrajectoryAnchor::new(
        conduit_audio::AudioOriginIdentity::new(1).unwrap(),
        conduit_audio::AudioTimelineIdentity::new(1).unwrap(),
    )
    .unwrap();
    let basis = conduit_audio::AudioSampleRateBasis::new(
        anchor.clone(),
        conduit_audio::AudioFrameQuantization::Floor,
        8000,
    )
    .unwrap();
    FrozenCarriedFormant::prepare(&original, &f.formant, &f.boundaries, &anchor, &basis).unwrap()
}
fn offline(source: FrozenCarriedFormant) -> PreparedOfflinePcm {
    let digest =
        |bytes: &[u8; 32]| SpeechCarrierDigest::new(fixture::seq(bytes.iter().copied())).unwrap();
    let declaration = SpeechOfflinePcmDeclaration::new(
        fixture::evidence(),
        digest(source.projection_identity()),
        digest(source.source_identity()),
    )
    .unwrap();
    PreparedOfflinePcm::prepare(source, declaration).unwrap()
}

#[test]
fn original_multiword_intent_executes_through_installed_factory_plan_and_play() {
    let f = fixture::Fixture::new();
    let request = fixture::commit_request();
    let commitment = fixture::execute_commitment(&request);
    assert!(*commitment.accepted());
    assert_eq!(*commitment.frontier().committed(), 2);
    let lexical =
        conduit_language::lexical::prepare_lexical_tape(&f.source, &f.lexical_profile, None)
            .unwrap();
    assert_eq!(lexical.tape().tokens().len(), 2);
    assert_eq!(
        request.beam().basis().text(),
        f.source.material().identity()
    );
    assert_eq!(
        request.beam().basis().source_revision(),
        f.source.material().revision()
    );
    let structure = conduit_speech::syllable_structure::PreparedSyllableStructure::prepare(
        &f.intent,
        &f.phones,
        f.source.material(),
        &f.syllables,
        &f.positioned_boundaries,
    )
    .unwrap();
    assert_eq!(structure.syllables().len(), 2);
    let realized = conduit_speech::intent_realization::prepare_intent_realization(
        &f.intent,
        &f.definitions,
        &f.formant,
        &f.boundaries,
    )
    .unwrap();
    let mut renderer = realized.renderer().unwrap();
    let mut expected_pcm = Vec::new();
    let mut block = [0_i16; 128];
    while !renderer.is_complete() {
        let count = renderer.render(&mut block).unwrap();
        for sample in &block[..count] {
            expected_pcm.extend_from_slice(&sample.to_le_bytes());
        }
    }
    let inventory = f.inventory().unwrap();
    let carrier = f.correspondence(&inventory).unwrap();
    let complete = PreparedCompleteSpeechCarrier::prepare(
        &carrier,
        &structure,
        &lexical,
        &f.word_phonemes,
        &request,
        &commitment,
        CompleteCarrierAcoustics::not_supplied(),
    )
    .unwrap();
    let model = model::terminal_evidence(&complete).unwrap();
    let anchor = conduit_audio::AudioTrajectoryAnchor::new(
        conduit_audio::AudioOriginIdentity::new(1).unwrap(),
        conduit_audio::AudioTimelineIdentity::new(1).unwrap(),
    )
    .unwrap();
    let basis = conduit_audio::AudioSampleRateBasis::new(
        anchor.clone(),
        conduit_audio::AudioFrameQuantization::Floor,
        8000,
    )
    .unwrap();
    let source =
        FrozenCarriedFormant::prepare(&complete, &f.formant, &f.boundaries, &anchor, &basis)
            .unwrap();
    assert_eq!(source.original_bytes(), model.original_bytes);
    assert_eq!(source.source_identity(), &model.source_identity);
    let identity = *source.source_identity();
    let projection = *source.projection_identity();
    let projection_bytes = source.projection_bytes().to_vec();
    let sample_basis = source.sample_basis().clone();
    assert_eq!(source.sample_projection_receipts().len(), 7);
    let original = source.original_bytes().to_vec();
    assert_eq!(source.total_frames(), 4000);
    assert_eq!(source.timing_receipts().len(), 7);
    assert_eq!(
        source
            .control_receipts()
            .iter()
            .filter(|(cycle, _)| cycle.is_some())
            .count(),
        5
    );
    let admitted = offline(source);
    let admission_bytes = admitted.admission().clone().encode().unwrap();
    let factory = CarriedFormantFactory::new(admitted, 7).unwrap();
    assert_eq!(factory.source().original_bytes(), original);
    let (kernel, plan) = execution::prepare(factory.clone());
    use conduit_composite::KernelOperationFactory;
    let placement = &plan.fragments[0].placements[0];
    assert!(factory.budget(placement).is_ok());
    for name in ["source", "projection", "offline-authoring"] {
        let mut wrong = placement.clone();
        wrong
            .configuration
            .iter_mut()
            .find(|field| field.key == name)
            .unwrap()
            .value = ConfigurationValue::Text("foreign".into());
        assert!(factory.budget(&wrong).is_err());
    }
    let mut wrong = placement.clone();
    wrong.artifact_id = "foreign/source-profile".into();
    assert!(factory.budget(&wrong).is_err());
    let execution::Run { plays, signs, pcm } = execution::run(kernel, &identity);
    assert_eq!(pcm.len(), 8000);
    assert_eq!(pcm, expected_pcm);
    assert_eq!(
        SpeechUtteranceIntent::decode(&f.intent.clone().encode().unwrap()).unwrap(),
        f.intent
    );
    assert!(pcm
        .as_chunks::<2>()
        .0
        .iter()
        .any(|pair| i16::from_le_bytes(*pair) != 0));
    assert!(signs.values().all(|signs| !signs.is_empty()));
    let packet = export::retain(export::Export {
        f: &f,
        model: &model,
        request: &request,
        result: &commitment,
        original: &original,
        identity: &identity,
        projection: &projection,
        projection_bytes: &projection_bytes,
        sample_basis: &sample_basis,
        admission: &admission_bytes,
        plan: &plan,
        plays: &plays,
        signs: &signs,
        pcm: &pcm,
    });
    let artifact_identity = semantic_digest("proof/carried-formant-artifact@1", &packet);
    let reloaded = inspection::inspect(&packet, &artifact_identity).unwrap();
    assert_eq!(reloaded.source().source_identity(), &identity);
    assert_eq!(reloaded.source().projection_identity(), &projection);
    let mut corrupted = packet.clone();
    corrupted[0] ^= 1;
    assert!(inspection::inspect(&corrupted, &artifact_identity).is_err());
    eprintln!("carried-formant source={identity:?} projection={projection:?} plan={:?} plays={plays:?} PCM-frames=4000", plan.plan_id);
}

#[test]
fn rejected_transaction_pressure_and_cancellation_preserve_original_source() {
    let source = Arc::new(offline(frozen(&fixture::Fixture::new())));
    let identity = source.source().source_identity().to_vec();
    let original = source.source().original_bytes().to_vec();
    let mut back = CarriedFormantBack::prepare(source, 7).unwrap();
    let reference = ValueRef {
        slot: 0,
        generation: 7,
        byte_len: 32,
    };
    let bytes = StepInputBytes::test_frame([Some(identity.as_slice())], None);
    let mut blocked = StepIo::test_frame([Some(reference)], [false], [None], None, 3);
    assert_eq!(back.step(&mut blocked, &bytes), StepOutcome::Await);
    let mut ready = StepIo::test_frame([Some(reference)], [false], [Some(285)], None, 3);
    assert_eq!(back.step(&mut ready, &bytes), StepOutcome::Progress);
    assert_eq!(back.rendered_frames(), 0);
    assert!(!ready.test_consumed(KernelPort(0)));
    let proposed = StepBack::<1>::prepared_output(&back, KernelPort(0))
        .unwrap()
        .to_vec();
    // Reject the transaction, then repeat: no duplicate cursor advance or loss.
    let mut retry = StepIo::test_frame([Some(reference)], [false], [Some(285)], None, 3);
    assert_eq!(back.step(&mut retry, &bytes), StepOutcome::Progress);
    assert_eq!(
        StepBack::<1>::prepared_output(&back, KernelPort(0)),
        Some(proposed.as_slice())
    );
    StepBack::<1>::step_committed(&mut back);
    assert_eq!(back.rendered_frames(), 128);
    assert_eq!(back.source().original_bytes(), original);
    StepBack::<1>::cancel(&mut back);
    assert_eq!(StepBack::<1>::prepared_output(&back, KernelPort(0)), None);
    assert!(matches!(
        back.step(&mut retry, &bytes),
        StepOutcome::Fail(_)
    ));
    assert_eq!(back.rendered_frames(), 128);
}

#[test]
fn foreign_source_and_projection_declaration_refuse_before_pcm() {
    let source = frozen(&fixture::Fixture::new());
    let original = *source.source_identity();
    let digest =
        |bytes: &[u8; 32]| SpeechCarrierDigest::new(fixture::seq(bytes.iter().copied())).unwrap();
    let wrong =
        SpeechOfflinePcmDeclaration::new(fixture::evidence(), digest(&[0; 32]), digest(&original))
            .unwrap();
    assert!(PreparedOfflinePcm::prepare(source, wrong).is_err());
    let source = Arc::new(offline(frozen(&fixture::Fixture::new())));
    let mut back = CarriedFormantBack::prepare(source, 7).unwrap();
    let wrong = [0_u8; 32];
    let bytes = StepInputBytes::test_frame([Some(wrong.as_slice())], None);
    let reference = ValueRef {
        slot: 0,
        generation: 1,
        byte_len: 32,
    };
    let mut io = StepIo::test_frame([Some(reference)], [false], [Some(285)], None, 3);
    assert!(matches!(back.step(&mut io, &bytes), StepOutcome::Fail(_)));
    assert_eq!(StepBack::<1>::prepared_output(&back, KernelPort(0)), None);
    assert_eq!(back.rendered_frames(), 0);
}

#[test]
fn independently_reload_exported_carrier() {
    let root = std::env::var_os("CONDUIT_CARRIED_FORMANT_INSPECT")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../site/evidence/shared-speech-carrier")
        });
    let bytes = std::fs::read(root.join("you-would.native")).unwrap();
    let report: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("you-would.json")).unwrap()).unwrap();
    let expected = report["original_artifact_identity"].as_str().unwrap();
    let digest = semantic_digest("proof/carried-formant-artifact@1", &bytes);
    let hex: String = digest.iter().map(|byte| format!("{byte:02x}")).collect();
    assert_eq!(hex, expected);
    let reloaded = inspection::inspect(&bytes, &digest).unwrap();
    assert_eq!(reloaded.source().total_frames(), 4000);
    let hex = |bytes: &[u8]| {
        bytes
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    };
    let identity = *reloaded.source().source_identity();
    assert_eq!(report["source_identity"], hex(&identity));
    assert_eq!(
        report["projection_identity"],
        hex(reloaded.source().projection_identity())
    );
    assert_eq!(report["compiled_dsp_source"], conduit_speech::SOURCE_ID);
    assert_eq!(
        report["speech_authoring_source"],
        conduit_speech::semantic::IPA_CONSTRUCTOR_SOURCE_ID
    );
    assert_eq!(report["human_listening"], "not performed");
    assert_eq!(report["device_playback"], "not performed");
    assert_eq!(report["model_terminal"]["source_identity"], hex(&identity));
    assert_eq!(
        report["model_terminal"]["refusal"],
        "NoAdmittedAcousticBack"
    );
    assert!(report["model_terminal"]["selected_backend"].is_null());
    assert_eq!(report["model_terminal"]["trained_weights"], false);
    let wav = std::fs::read(root.join("you-would.wav")).unwrap();
    assert_eq!(wav.len(), 8044);
    assert_eq!(&wav[..4], b"RIFF");
    assert_eq!(&wav[8..16], b"WAVEfmt ");
    assert_eq!(&wav[36..40], b"data");
    assert_eq!(u16::from_le_bytes(wav[20..22].try_into().unwrap()), 1);
    assert_eq!(u16::from_le_bytes(wav[22..24].try_into().unwrap()), 1);
    assert_eq!(u32::from_le_bytes(wav[24..28].try_into().unwrap()), 8000);
    assert_eq!(u16::from_le_bytes(wav[34..36].try_into().unwrap()), 16);
    assert_eq!(
        report["wav_identity"],
        hex(&semantic_digest("proof/carried-formant-wav@1", &wav))
    );
    let factory = CarriedFormantFactory::new(reloaded, 7).unwrap();
    let (kernel, plan) = execution::prepare(factory);
    let replayed = execution::run(kernel, &identity);
    assert_eq!(replayed.pcm, wav[44..]);
    assert_eq!(report["plan"], serde_json::to_value(plan).unwrap());
    assert_eq!(
        report["plays"],
        serde_json::to_value(replayed.plays).unwrap()
    );
    assert_eq!(
        report["signs"].as_object().unwrap().len(),
        replayed.signs.len()
    );
    for (host, events) in replayed.signs {
        let rows = report["signs"][host.as_str()].as_array().unwrap();
        assert_eq!(rows.len(), events.len());
        for (row, event) in rows.iter().zip(events) {
            assert_eq!(row["sequence"], event.sequence);
            assert_eq!(row["node"], event.node.0);
            assert_eq!(row["port"], serde_json::json!(event.port.map(|p| p.0)));
            assert_eq!(
                row["request"],
                serde_json::json!(event.request.map(|r| r.0))
            );
            assert_eq!(row["kind"], format!("{:?}", event.kind));
        }
    }
}

#[test]
fn offline_installed_pcm_path_has_finite_admitted_sign_storage() {
    let source = frozen(&fixture::Fixture::new());
    let identity = *source.source_identity();
    let factory = CarriedFormantFactory::new(offline(source), 7).unwrap();
    let (kernel, _) = execution::prepare(factory);
    let result = execution::run(kernel, &identity);
    assert_eq!(result.pcm.len(), 8000);
}
