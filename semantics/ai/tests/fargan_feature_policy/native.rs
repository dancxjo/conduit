//! Private handoff replay: acoustic admission is separate from joint linguistic commit.
use super::*;
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_speech::{
    intent_realization::prepare_intent_realization,
    pitch_trajectory::{prepare_utterance_pitch, OfferedSegmentPitch},
    semantic::*,
};
use sha2::{Digest, Sha256};

fn bytes(v: &serde_json::Value) -> Vec<u8> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|n| u8::try_from(n.as_u64().unwrap()).unwrap())
        .collect()
}
#[test]
#[ignore = "requires private exact shared native handoff; set CONDUIT_FARGAN_SHARED_HANDOFF"]
fn retained_shared_native_intent_replays_exact_formant_and_pitch_basis() {
    std::thread::Builder::new()
        .stack_size(32 * 1024 * 1024)
        .spawn(replay)
        .unwrap()
        .join()
        .unwrap();
}
fn replay() {
    let path = std::env::var("CONDUIT_FARGAN_SHARED_HANDOFF").unwrap();
    let document = std::fs::read(&path).unwrap();
    assert_eq!(
        format!("{:x}", Sha256::digest(&document)),
        "f8d5a958ec0751eccedbbcdfb9dd2d3445b53ee45512399b1d081532eba1c467"
    );
    let json: serde_json::Value = serde_json::from_slice(&document).unwrap();
    let h = &json["shared_realization_handoff"];
    let intent = SpeechUtteranceIntent::decode(&bytes(&h["utterance_intent_bytes"])).unwrap();
    let voice = SpeechFormantVoiceProfile::decode(&bytes(&h["voice_profile_bytes"])).unwrap();
    let inventory = SpeechInventory::decode(&bytes(&h["inventory_bytes"])).unwrap();
    let boundaries =
        SpeechFormantBoundaryProfile::decode(&bytes(&h["boundary_profile_bytes"])).unwrap();
    let admissions: Vec<_> = h["pitch_admissions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| {
            let raw = p.get("pitch_admission_bytes").unwrap_or(p);
            SpeechSegmentPitchAdmission::decode(&bytes(raw)).unwrap()
        })
        .collect();
    for (event, offered) in h["pitch_admissions"].as_array().unwrap().iter().enumerate() {
        assert_eq!(offered["event"].as_u64().unwrap(), event as u64);
    }
    let offers: Vec<_> = admissions
        .iter()
        .enumerate()
        .map(|(event, admission)| OfferedSegmentPitch { event, admission })
        .collect();
    let pitch = prepare_utterance_pitch(&intent, &offers).unwrap();
    let realized = prepare_intent_realization(&intent, &inventory, &voice, &boundaries).unwrap();
    assert_eq!(pitch.source(), realized.source());
    assert_eq!(
        realized.compiled_source_id(),
        h["compiled_formant_source_id"].as_str().unwrap()
    );
    assert_eq!(
        realized.timing().spans().len(),
        h["event_span_bytes"].as_array().unwrap().len()
    );
    for (span, offered) in realized
        .timing()
        .spans()
        .iter()
        .zip(h["event_span_bytes"].as_array().unwrap())
    {
        assert_eq!(span.clone().encode().unwrap(), bytes(offered));
    }
    for receipt in h["pitch_cadence"].as_array().unwrap() {
        let event = receipt["event"].as_u64().unwrap() as usize;
        let frame = receipt["local_frame_8k"].as_u64().unwrap();
        assert_eq!(
            receipt["utterance_frame_8k"].as_u64().unwrap(),
            realized.timing().spans()[event].start_frame() + frame
        );
        assert_eq!(
            pitch
                .at_frame(event, frame, 8000)
                .unwrap()
                .encode()
                .unwrap(),
            bytes(&receipt["formant_cycle_bytes"])
        );
        assert_eq!(
            pitch
                .at_frame(event, frame * 2, 16000)
                .unwrap()
                .encode()
                .unwrap(),
            bytes(&receipt["fargan_cycle_bytes"])
        );
    }
    let mut renderer = pitch.renderer(&realized).unwrap();
    let mut pcm = Vec::new();
    loop {
        let mut block = [0; 80];
        let n = renderer.render(&mut block).unwrap();
        if n == 0 {
            break;
        }
        pcm.extend_from_slice(&block[..n]);
    }
    assert_eq!(pcm.len(), 16240);
    let wav = std::fs::read(std::path::Path::new(&path).with_extension("wav")).unwrap();
    let mut offset = 12;
    let mut actual = None;
    while offset + 8 <= wav.len() {
        let size = u32::from_le_bytes(wav[offset + 4..offset + 8].try_into().unwrap()) as usize;
        if &wav[offset..offset + 4] == b"data" {
            actual = Some(
                wav[offset + 8..offset + 8 + size]
                    .as_chunks::<2>()
                    .0
                    .iter()
                    .map(|s| i16::from_le_bytes(*s))
                    .collect::<Vec<_>>(),
            );
            break;
        }
        offset += 8 + size + (size % 2);
    }
    assert_eq!(actual.unwrap(), pcm);
    // Execute authored causal resampling and preemphasis over the first four
    // real epochs, then the ordinary Source feature graph over their history.
    let source = format!(
        "type FarganPeriod = U16 in 32..=255\n{}\n{}",
        include_str!("../../../speech/fargan_feature_policy.conduit"),
        include_str!("../../../speech/fargan_feature_correlation.conduit")
    );
    let mut startup = StartupCatalog::new();
    let mut profiles = ProfileCatalog::new();
    install_fixed_numeric_catalogs(&mut startup, &mut profiles).unwrap();
    let checked = check_syntax_document(&parse_syntax_document(&source), &startup).unwrap();
    let resample = pure(&checked, &profiles, "speech/fargan-formant-linear2x");
    let preemphasis = pure(&checked, &profiles, "speech/fargan-feature-preemphasis");
    let append = pure(&checked, &profiles, "speech/fargan-feature-wave-append");
    let mut history = vec![0f32; 640];
    let mut previous_raw = 0f32;
    let mut previous_normalized = 0f32;
    for samples in pcm[..320].as_chunks::<80>().0 {
        let input = record(
            &resample.input_type,
            &[
                (
                    "samples_raw",
                    samples.iter().map(|s| f32::from(*s)).collect(),
                ),
                ("previous_raw", vec![previous_raw]),
            ],
        )
        .canonical_bytes()
        .unwrap();
        let result =
            StructuredInfoValue::from_canonical_bytes(&resample.evaluate(&input).unwrap()).unwrap();
        previous_raw = field(&result, "next_previous_raw")[0];
        let input = record(
            &preemphasis.input_type,
            &[
                ("samples", field(&result, "samples")),
                ("previous_normalized", vec![previous_normalized]),
            ],
        )
        .canonical_bytes()
        .unwrap();
        let result =
            StructuredInfoValue::from_canonical_bytes(&preemphasis.evaluate(&input).unwrap())
                .unwrap();
        previous_normalized = field(&result, "next_previous_normalized")[0];
        let input = record(
            &append.input_type,
            &[("history", history), ("samples", field(&result, "samples"))],
        )
        .canonical_bytes()
        .unwrap();
        history = floats(
            &StructuredInfoValue::from_canonical_bytes(&append.evaluate(&input).unwrap()).unwrap(),
        );
    }
    assert!(history.iter().any(|x| *x != 0.));
    let entry = "speech/fargan-feature-frame";
    let schema = plan::SourceSchema::for_entry(&source, entry);
    let load = |raw: &[u8]| {
        raw.as_chunks::<4>()
            .0
            .iter()
            .map(|v| f32::from_le_bytes(*v))
            .collect::<Vec<_>>()
    };
    let resources: shared::Resources = [
        (
            "band_weights",
            vec![161, 18],
            load(include_bytes!(
                "../../../../proof/fargan/feature-profile/bands161x18.bin"
            )),
        ),
        (
            "band_bias",
            vec![18],
            load(include_bytes!(
                "../../../../proof/fargan/feature-profile/band_bias18.bin"
            )),
        ),
        (
            "dct_weights",
            vec![18, 18],
            load(include_bytes!(
                "../../../../proof/fargan/feature-profile/dct18x18.bin"
            )),
        ),
    ]
    .into_iter()
    .map(|(n, d, v)| (n.into(), shared::Resource::new(schema.ty(n).clone(), &d, v)))
    .collect();
    let cycle = pitch.at_frame(0, 240 * 2, 16000).unwrap();
    let q8 = *cycle.whole_q8();
    let rounded = (q8.clamp(8192, 65280) + 128) / 256;
    // The numerical expression is replayed below from the exact authored law.
    let period_source = include_str!("../../../speech/fargan_epoch_policy.conduit")
        .split("plot speech/flow-fargan-period-q8-round")
        .next()
        .unwrap();
    let period_checked =
        check_syntax_document(&parse_syntax_document(period_source), &startup).unwrap();
    let period_program = pure(&period_checked, &profiles, "speech/fargan-period-q8-round");
    let mut admitted_round = PreparedPortableExpressionEvaluator::new(&period_program).unwrap();
    let q8_input = q8.to_le_bytes();
    let rounded_value = period_program.evaluate(&q8_input).unwrap();
    assert_eq!(admitted_round.evaluate(&q8_input).unwrap(), rounded_value);
    assert_eq!(
        u64::from_le_bytes(rounded_value.try_into().unwrap()),
        rounded
    );
    let StructuredInfoTypeShape::Nominal { representation, .. } = schema.ty("period").shape()
    else {
        panic!("period")
    };
    let period = StructuredInfoValue::nominal(
        schema.ty("period").clone(),
        StructuredInfoValue::leaf(
            representation.clone(),
            u16::try_from(rounded).unwrap().to_le_bytes().to_vec(),
        )
        .unwrap(),
    )
    .unwrap();
    let input = std::collections::BTreeMap::from([
        (
            "waveform".into(),
            value(schema.ty("waveform"), &mut history.into_iter())
                .canonical_bytes()
                .unwrap(),
        ),
        ("period".into(), period.canonical_bytes().unwrap()),
    ]);
    let selected = plan::prepare_entry_plan(&schema, &source, entry);
    let feature =
        runtime::run_entry_plan(selected, &resources, input, runtime::ExecutionMode::Normal)
            .unwrap();
    let actual = floats(&feature.value);
    assert_eq!(actual.len(), 20);
    assert!(actual.iter().all(|x| x.is_finite()));
    if let Ok(path) = std::env::var("CONDUIT_FARGAN_FEATURE_EVIDENCE") {
        let evidence = serde_json::json!({
            "profile":"speech/fargan-formant-spectral-approximation@1",
            "shared_handoff_sha256":format!("{:x}",Sha256::digest(&document)),
            "source_sha256":format!("{:x}",Sha256::digest(source.as_bytes())),
            "period_source_sha256":format!("{:x}",Sha256::digest(period_source.as_bytes())),
            "feature_event":0,"feature_local_frame_8k":240,"feature_period_q8":q8,"feature_period_samples_16k":rounded,
            "retained_linguistic_graph_receipt":json["graph_receipt"],
            "native_handoff":h,
            "formant_exact_samples":pcm.len(),"pitch_epochs_replayed":200,
            "source_preprocessed_epochs":4,"feature_graph_executed_frames":1,
            "feature20":actual,"graph_nodes":feature.nodes,"graph_cords":feature.cords,
            "owner_preparation_ns":feature.preparation.as_nanos().to_string(),
            "execution_ns":feature.execution.as_nanos().to_string(),
            "joint_committed_session":false,"neural_waveform":false
        });
        std::fs::write(path, serde_json::to_vec_pretty(&evidence).unwrap()).unwrap();
    }
    eprintln!("actual shared native Source feature: {} nodes {} cords prepare={:?} execute={:?} feature20={actual:?}",feature.nodes,feature.cords,feature.preparation,feature.execution);
    eprintln!("exact shared native basis: {} samples, {} pitch epochs; joint linguistic commit not asserted",pcm.len(),h["pitch_cadence"].as_array().unwrap().len());
}

fn pure(
    checked: &CheckedSyntaxDocument,
    profiles: &ProfileCatalog,
    entry: &str,
) -> PortableExpressionProgram {
    let expanded = expand_canonical_plot_for_authoring(checked, entry, profiles).unwrap();
    assert_eq!(expanded.expanded.gears.len(), 1);
    let ConfigurationValue::Text(hex) = &expanded.expanded.gears[0].configuration[0].value else {
        panic!("expression")
    };
    PortableExpressionProgram::from_canonical_hex(hex).unwrap()
}
fn record(ty: &StructuredInfoType, fields: &[(&str, Vec<f32>)]) -> StructuredInfoValue {
    match ty.shape() {
        StructuredInfoTypeShape::Nominal { representation, .. } => {
            StructuredInfoValue::nominal(ty.clone(), record(representation, fields)).unwrap()
        }
        StructuredInfoTypeShape::Record { fields: shape, .. } => StructuredInfoValue::record(
            ty.clone(),
            shape
                .iter()
                .map(|f| {
                    StructuredFieldValue::new(
                        f.name(),
                        value(
                            f.value_type(),
                            &mut fields
                                .iter()
                                .find(|(n, _)| *n == f.name())
                                .unwrap()
                                .1
                                .iter()
                                .copied(),
                        ),
                    )
                    .unwrap()
                })
                .collect(),
        )
        .unwrap(),
        _ => panic!("record"),
    }
}
fn field(v: &StructuredInfoValue, name: &str) -> Vec<f32> {
    let StructuredInfoValueShape::Record(fields) = v.shape() else {
        panic!("record")
    };
    floats(fields.iter().find(|f| f.name() == name).unwrap().value())
}
