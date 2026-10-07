//! Preparation-only retained native tape; control and acoustic equations remain Source.
use super::*;
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_speech::{
    intent_realization::prepare_intent_realization,
    pitch_trajectory::{prepare_utterance_pitch, OfferedSegmentPitch},
    semantic::*,
};

pub(super) struct NativeEpoch {
    pub samples: [i16; 80],
    pub event: usize,
    pub cycle: Option<Vec<u8>>,
    pub q8: u64,
}
pub(super) struct NativeTape {
    pub epochs: Vec<NativeEpoch>,
    pub pcm: Vec<i16>,
    pub immutable_material: Vec<u8>,
}

#[test]
#[ignore = "complete private committed native basis and33 shared trained tensors"]
fn committed_native_first_feature_drives_source_warm_startup_with_retained_model() {
    std::thread::Builder::new().stack_size(32 * 1024 * 1024).spawn(|| {
        let basis = super::committed_lineage::admit_shared_basis();
        let tape = prepare_native_tape(&basis);
        let observations = tape.epochs.iter().map(|epoch| (epoch.cycle.is_none(), epoch.q8)).collect::<Vec<_>>();
        let periods = super::runtime::run_native_period_controls(&observations);
        assert_eq!(periods.len(), 63);
        let proposal = super::runtime::run_native_first_feature(&tape.epochs[0].samples, &periods[0]);
        let root = std::path::PathBuf::from(std::env::var("CONDUIT_FARGAN_MODEL_FIXTURE").unwrap());
        let mut model = super::custody::RetainedSignalModel::load(&root);
        let (_, conditioning) = model.conditioning_resources();
        for (name, resource) in conditioning { assert!(model.resources.insert(format!("conditioning_{name}"), resource).is_none()); }
        let warm = super::runtime::run_native_warm_startup(&model, &proposal);
        assert!(warm.iter().any(|value| matches!(value.shape(), StructuredInfoValueShape::Record(_))));
        assert!(warm.iter().any(|value| matches!(value.shape(), StructuredInfoValueShape::Collection(_))));
        eprintln!("complete committed native basis→63 Source period admissions→first native feature→33 shared-resource Source warm startup; no native utterance PCM claim");
    }).unwrap().join().unwrap();
}
fn bytes(value: &serde_json::Value) -> Vec<u8> {
    serde_json::from_value(value.clone()).unwrap()
}

#[test]
#[ignore = "complete private committed native basis, model custody and full trained three-cell Source utterance"]
fn committed_native_utterance_executes_all_three_feedback_cells_and_source_eof() {
    std::thread::Builder::new().stack_size(32 * 1024 * 1024).spawn(|| {
        let basis = super::committed_lineage::admit_shared_basis();
        let tape = prepare_native_tape(&basis);
        let observations = tape.epochs.iter().map(|epoch| (epoch.cycle.is_none(), epoch.q8)).collect::<Vec<_>>();
        let periods = super::runtime::run_native_period_controls(&observations);
        let proposal = super::runtime::run_native_first_feature(&tape.epochs[0].samples, &periods[0]);
        let root = std::path::PathBuf::from(std::env::var("CONDUIT_FARGAN_MODEL_FIXTURE").unwrap());
        let mut model = super::custody::RetainedSignalModel::load(&root);
        let (_, conditioning) = model.conditioning_resources();
        for (name, resource) in conditioning { assert!(model.resources.insert(format!("conditioning_{name}"), resource).is_none()); }
        let warm = super::runtime::run_native_warm_startup(&model, &proposal);
        let epochs = super::runtime::run_native_trained_utterance(&model, &tape, &periods, &proposal, &warm);
        for (value, epoch) in epochs.iter().zip(1u64..=64) {
            let StructuredInfoValueShape::Leaf(bytes) = super::case_state::field(value, "epoch").shape() else { panic!("epoch") };
            assert_eq!(u64::from_le_bytes(bytes.try_into().unwrap()), epoch);
            let StructuredInfoValueShape::Collection(pcm) = super::case_state::field(value, "pcm_i16").shape() else { panic!("PCM160") };
            assert_eq!(pcm.len(), 160);
        }
        if let Ok(directory) = std::env::var("CONDUIT_FARGAN_NATIVE_OUTPUT") {
            let (pcm, alignment_source) = super::native_startup::align_native_pcm(&epochs);
            assert_eq!(pcm.len(), tape.pcm.len() * 2);
            let directory = std::path::PathBuf::from(directory);
            let raw = epochs.iter().map(|epoch| epoch.canonical_bytes().unwrap()).collect::<Vec<_>>();
            std::fs::write(directory.join("raw-epochs.json"), serde_json::to_vec(&raw).unwrap()).unwrap();
            std::fs::write(directory.join("alignment-source.conduit"), &alignment_source).unwrap();
            write_pcm_wav(&directory.join("neural.wav"), 16000, &pcm);
            write_pcm_wav(&directory.join("formant.wav"), 8000, &tape.pcm);
            use sha2::{Digest, Sha256};
            let hash = |path: &str| format!("{:x}", Sha256::digest(std::fs::read(directory.join(path)).unwrap()));
            let receipt = serde_json::json!({
                "committed_handoff_sha256":"d4be97a0c8350c72df496cae35f817dd617e449157dbe6a25e5dbda79eee2c89",
                "complete_committed_dependency_admission_bytes":basis.receipt["graph_receipt"]["committed_dependency_admission_bytes"],
                "full_native_basis_material":tape.immutable_material,
                "raw_epochs":64,"native_epochs":63,"source_continuation_epochs":2,
                "neural_samples":pcm.len(),"neural_sample_rate":16000,"neural_wav_sha256":hash("neural.wav"),
                "formant_samples":tape.pcm.len(),"formant_sample_rate":8000,"formant_wav_sha256":hash("formant.wav"),
                "raw_epochs_sha256":hash("raw-epochs.json"),"session_basis_sha256":hash("session-basis.bin"),
                "bound_source_sha256":hash("bound-epoch-source.conduit"),"alignment_source_sha256":hash("alignment-source.conduit"),
                "model_raw_blob_sha256":model.raw_blob_sha256,"signal_only_model_artifact_identity":model.model.artifact().content_identity(),
                "signal_only_model_descriptor_identity":model.model.descriptor_identity(),
                "same_native_intent_and_pitch_readmitted":true,"ordinary_source_epoch_scheduler_drained":true,
                "alignment_execution":"checked Source portable reference/prepared evaluator parity on actual committed canonical PCM blocks",
                "plan_retention":"complete debug diagnostic; bound Source and exact session basis retained separately",
                "model_compute_lifecycle_admitted":false,"booted_target":false,"human_listening":false,"physical_playback":false,"realtime":false
            });
            std::fs::write(directory.join("manifest.json"), serde_json::to_vec_pretty(&receipt).unwrap()).unwrap();
            eprintln!("actual Source aligned native pair retained:10080mono16k/5040mono8k samples; no listening or boot claim");
        }
        eprintln!("complete committed native utterance→actual canonical PCM16 and three causal feedback cells; Source startup/continuation chronology verified");
    }).unwrap().join().unwrap();
}

fn write_pcm_wav(path: &std::path::Path, rate: u32, pcm: &[i16]) {
    let size = u32::try_from(pcm.len() * 2).unwrap();
    let mut bytes = Vec::with_capacity(44 + size as usize);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + size).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16u32.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&1u16.to_le_bytes());
    bytes.extend_from_slice(&rate.to_le_bytes());
    bytes.extend_from_slice(&(rate * 2).to_le_bytes());
    bytes.extend_from_slice(&2u16.to_le_bytes());
    bytes.extend_from_slice(&16u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&size.to_le_bytes());
    for sample in pcm {
        bytes.extend_from_slice(&sample.to_le_bytes());
    }
    std::fs::write(path, bytes).unwrap();
}

pub(super) fn prepare_native_tape(
    basis: &super::committed_lineage::RetainedCommittedBasis,
) -> NativeTape {
    let shared = &basis.receipt["shared_realization_handoff"];
    let intent = SpeechUtteranceIntent::decode(&bytes(&shared["utterance_intent_bytes"])).unwrap();
    let voice = SpeechFormantVoiceProfile::decode(&bytes(&shared["voice_profile_bytes"])).unwrap();
    let inventory = SpeechInventory::decode(&bytes(&shared["inventory_bytes"])).unwrap();
    let boundary =
        SpeechFormantBoundaryProfile::decode(&bytes(&shared["boundary_profile_bytes"])).unwrap();
    let admissions: Vec<_> = shared["pitch_admissions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| {
            SpeechSegmentPitchAdmission::decode(&bytes(&row["pitch_admission_bytes"])).unwrap()
        })
        .collect();
    let offers: Vec<_> = admissions
        .iter()
        .enumerate()
        .map(|(event, admission)| OfferedSegmentPitch { event, admission })
        .collect();
    let realized = prepare_intent_realization(&intent, &inventory, &voice, &boundary).unwrap();
    let pitch = prepare_utterance_pitch(&intent, &offers).unwrap();
    assert_eq!(pitch.source(), realized.source());
    assert_eq!(
        realized.compiled_source_id(),
        shared["compiled_formant_source_id"].as_str().unwrap()
    );
    for (span, offered) in realized
        .timing()
        .spans()
        .iter()
        .zip(shared["event_span_bytes"].as_array().unwrap())
    {
        assert_eq!(span.clone().encode().unwrap(), bytes(offered));
    }
    let mut renderer = pitch.renderer(&realized).unwrap();
    let mut pcm = Vec::new();
    loop {
        let mut block = [0; 80];
        let count = renderer.render(&mut block).unwrap();
        if count == 0 {
            break;
        }
        pcm.extend_from_slice(&block[..count]);
    }
    assert_eq!(
        pcm.len(),
        5040,
        "original committed handoff pin keeps its exact duration"
    );
    let epochs = pcm
        .as_chunks::<80>()
        .0
        .iter()
        .enumerate()
        .map(|(epoch, samples)| {
            let frame = u64::try_from(epoch * 80).unwrap();
            let (event, span) = realized
                .timing()
                .spans()
                .iter()
                .enumerate()
                .find(|(_, span)| {
                    *span.start_frame() <= frame && frame < span.start_frame() + span.frame_count()
                })
                .unwrap();
            let cycle = if let Some(row) = shared["pitch_cadence"]
                .as_array()
                .unwrap()
                .iter()
                .find(|row| row["utterance_frame_8k"].as_u64() == Some(frame))
            {
                let cycle = pitch
                    .at_frame(event, (frame - span.start_frame()) * 2, 16000)
                    .unwrap();
                let encoded = cycle.clone().encode().unwrap();
                assert_eq!(encoded, bytes(&row["fargan_cycle_bytes"]));
                Some((encoded, *cycle.whole_q8()))
            } else {
                let native = super::interface::admit_retained_session_native(
                    &basis.document,
                    "SpeechUtteranceIntent",
                    &bytes(&shared["utterance_intent_bytes"]),
                )
                .unwrap();
                let StructuredInfoValueShape::Record(fields) = native.shape() else {
                    panic!("intent")
                };
                let StructuredInfoValueShape::Collection(events) = fields
                    .iter()
                    .find(|field| field.name() == "events")
                    .unwrap()
                    .value()
                    .shape()
                else {
                    panic!("events")
                };
                let StructuredInfoValueShape::Variant { tag, .. } = events[event].shape() else {
                    panic!("event")
                };
                assert_eq!(
                    tag, "boundary",
                    "missing phone pitch cannot become a boundary"
                );
                assert!(pitch
                    .at_frame(event, (frame - span.start_frame()) * 2, 16000)
                    .is_err());
                None
            };
            NativeEpoch {
                samples: *samples,
                event,
                q8: cycle.as_ref().map_or(0, |(_, q8)| *q8),
                cycle: cycle.map(|(encoded, _)| encoded),
            }
        })
        .collect::<Vec<_>>();
    assert_eq!(epochs.len(), 63);
    let mut immutable_material = basis.encoded.clone();
    immutable_material.extend_from_slice(&basis.language_source);
    immutable_material.extend_from_slice(&basis.speech_source);
    immutable_material.extend_from_slice(&basis.committed.canonical_bytes().unwrap());
    NativeTape {
        epochs,
        pcm,
        immutable_material,
    }
}

#[test]
#[ignore = "exact private committed handoff and complete retained Source closures"]
fn committed_native_tape_retains_all_epochs_and_matches_formant_waveform() {
    let basis = super::committed_lineage::admit_shared_basis();
    let tape = prepare_native_tape(&basis);
    assert_eq!(tape.epochs.len(), 63);
    assert_eq!(
        tape.epochs
            .iter()
            .filter(|epoch| epoch.cycle.is_some())
            .count(),
        60
    );
    for (index, epoch) in tape.epochs.iter().enumerate() {
        assert_eq!(&epoch.samples, &tape.pcm[index * 80..(index + 1) * 80]);
        if epoch.cycle.is_some() {
            assert!(epoch.event < 6);
            assert!(epoch.q8 > 0);
        } else {
            assert_eq!(epoch.event, 6);
            assert_eq!(epoch.q8, 0);
        }
    }
    assert!(tape.immutable_material.starts_with(&basis.encoded));
    assert!(tape.immutable_material.len() > basis.encoded.len());
    let handoff =
        std::path::PathBuf::from(std::env::var("CONDUIT_COMMITTED_SPEECH_HANDOFF").unwrap());
    let wav = std::fs::read(handoff.with_extension("wav")).unwrap();
    assert_eq!(&wav[..4], b"RIFF");
    assert_eq!(&wav[8..12], b"WAVE");
    assert_eq!(&wav[36..40], b"data");
    assert_eq!(wav.len(), 10124);
    let samples = wav[44..]
        .as_chunks::<2>()
        .0
        .iter()
        .map(|bytes| i16::from_le_bytes(*bytes))
        .collect::<Vec<_>>();
    assert_eq!(tape.pcm, samples);
}

#[test]
fn native_boundary_control_is_source_owned_and_never_requires_fake_phone_pitch() {
    let source = control_source();
    let (startup, profiles) = catalogs(true);
    let checked = check_syntax_document(&parse_syntax_document(&source), &startup).unwrap();
    check_control_programs(&checked, &profiles);
}

pub(super) fn control_source() -> String {
    "type FarganPeriod = U16 in 32..=255\n".to_owned()
        + include_str!("../../../speech/fargan_epoch_policy.conduit")
            .split("# Source chooses normalized PCM saturation")
            .next()
            .unwrap()
        + include_str!("../../../speech/fargan_native_control.conduit")
}

fn check_control_programs(checked: &CheckedSyntaxDocument, profiles: &ProfileCatalog) {
    let expanded =
        expand_canonical_plot_for_authoring(checked, "speech/fargan-native-control-q8", profiles)
            .unwrap();
    let ConfigurationValue::Text(hex) = &expanded.expanded.gears[0].configuration[0].value else {
        panic!("Source control")
    };
    let program = PortableExpressionProgram::from_canonical_hex(hex).unwrap();
    let mut prepared = PreparedPortableExpressionEvaluator::new(&program).unwrap();
    for (boundary, q8, expected) in [
        (true, 0, 16384u64),
        (true, u64::MAX, 16384),
        (false, 20480, 20480),
    ] {
        let StructuredInfoTypeShape::Record { fields, .. } = program.input_type.shape() else {
            panic!("observation")
        };
        let value = StructuredInfoValue::record(
            program.input_type.clone(),
            fields
                .iter()
                .map(|field| {
                    StructuredFieldValue::new(
                        field.name(),
                        StructuredInfoValue::leaf(
                            field.value_type().clone(),
                            if field.name() == "boundary" {
                                vec![u8::from(boundary)]
                            } else {
                                q8.to_le_bytes().to_vec()
                            },
                        )
                        .unwrap(),
                    )
                    .unwrap()
                })
                .collect(),
        )
        .unwrap()
        .canonical_bytes()
        .unwrap();
        let reference = program.evaluate(&value).unwrap();
        assert_eq!(prepared.evaluate(&value).unwrap(), reference);
        assert_eq!(reference, expected.to_le_bytes());
    }
    let round =
        expand_canonical_plot_for_authoring(checked, "speech/fargan-period-q8-round", profiles)
            .unwrap();
    let ConfigurationValue::Text(hex) = &round.expanded.gears[0].configuration[0].value else {
        panic!("Source rounding")
    };
    let round = PortableExpressionProgram::from_canonical_hex(hex).unwrap();
    let mut prepared = PreparedPortableExpressionEvaluator::new(&round).unwrap();
    for (q8, expected) in [(0u64, 32u64), (16384, 64), (20480, 80), (u64::MAX, 255)] {
        let input = q8.to_le_bytes();
        let reference = round.evaluate(&input).unwrap();
        assert_eq!(prepared.evaluate(&input).unwrap(), reference);
        assert_eq!(reference, expected.to_le_bytes());
    }
}
