#![cfg(feature = "kernel-operation-owners")]
//! Actual consecutive carrier epochs enter the existing checked numerical frontend.
#[path = "../../speech/tests/common/carried_formant_fixture.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "fargan_signal_graph/plan.rs"]
mod plan;
#[path = "fargan_feature_frontend/preprocess.rs"]
mod preprocess;
#[allow(dead_code)]
#[path = "fargan_signal_graph/runtime.rs"]
mod runtime;
#[allow(dead_code)]
#[path = "fargan_signal_graph/shared.rs"]
mod shared;
#[allow(dead_code)]
#[path = "fargan_signal_graph/state.rs"]
mod state;
use conduit_audio::*;
use conduit_core::*;
use conduit_speech::{
    carried_formant::FrozenCarriedFormant,
    complete_carrier::{CompleteCarrierAcoustics, PreparedCompleteSpeechCarrier},
    fargan_carrier_epochs::*,
};
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, path::PathBuf};
fn coefficients(schema: &plan::SourceSchema) -> shared::Resources {
    let manifest: serde_json::Value = serde_json::from_str(include_str!(
        "../../../proof/fargan/feature-profile/manifest.json"
    ))
    .unwrap();
    assert_eq!(
        manifest["profile"],
        "speech/fargan-formant-spectral-approximation@1"
    );
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../proof/fargan/feature-profile/generated");
    [
        ("band_weights", "bands161x18", vec![161u64, 18]),
        ("band_bias", "band_bias18", vec![18]),
        ("dct_weights", "dct18x18", vec![18, 18]),
    ]
    .into_iter()
    .map(|(port, name, dims)| {
        let bytes = std::fs::read(directory.join(format!("{name}.bin")))
            .expect("materialize pinned public coefficients");
        assert_eq!(manifest["arrays"][name]["shape"], serde_json::json!(dims));
        assert_eq!(
            manifest["arrays"][name]["sha256"],
            format!("{:x}", Sha256::digest(&bytes))
        );
        assert_eq!(bytes.len() as u64, dims.iter().product::<u64>() * 4);
        let values = bytes
            .chunks_exact(4)
            .map(|v| f32::from_le_bytes(v.try_into().unwrap()))
            .collect();
        (
            port.into(),
            shared::Resource::new(schema.ty(port).clone(), &dims, values),
        )
    })
    .collect()
}
#[test]
fn first_four_consecutive_actual_carrier_epochs_feed_checked_frontend_without_gap_filtering() {
    std::thread::Builder::new().stack_size(32*1024*1024).spawn(||{
        let f=fixture::Fixture::new();let inventory=f.inventory().unwrap();
        let original=f.correspondence(&inventory).unwrap();let evidence=f.complete_evidence();let structure=f.structure();
        let complete=PreparedCompleteSpeechCarrier::prepare(&original,&structure,&evidence.lexical,
            &f.word_phonemes,&evidence.request,&evidence.result,CompleteCarrierAcoustics::not_supplied()).unwrap();
        let anchor=AudioTrajectoryAnchor::new(AudioOriginIdentity::new(1).unwrap(),AudioTimelineIdentity::new(1).unwrap()).unwrap();
        let basis=AudioSampleRateBasis::new(anchor.clone(),AudioFrameQuantization::Floor,8000).unwrap();
        let frozen=FrozenCarriedFormant::prepare(&complete,&f.formant,&f.boundaries,&anchor,&basis).unwrap();
        let original_bytes=complete.canonical_bytes().to_vec();
        let epochs=prepare_carrier_epochs(&complete,&frozen,4096).unwrap();
        assert!(epochs.epochs().len()>=4);
        // Index the first four, never filter_map ready epochs across unsupported gaps.
        let prefix=&epochs.epochs()[..4];
        let receipt=prefix[0].period().expect("actual first epoch must support this finite fixture");
        let period=receipt.period();
        let mut raw=Vec::new();
        for (i,epoch) in prefix.iter().enumerate() {
            assert_eq!(epoch.start_frame(),(i*80) as u64);
            assert!(epoch.blockers().is_empty());assert_eq!(epoch.pcm().len(),80);
            assert_eq!(epoch.period().unwrap().projection().input(),receipt.projection().input());
            assert_eq!(epoch.period().unwrap().consumer_ledger(),receipt.consumer_ledger());
            raw.extend_from_slice(epoch.pcm());
        }
        let raw:[i16;320]=raw.try_into().unwrap();
        assert!(raw.iter().any(|sample|*sample!=0));
        let source=[include_str!("../../speech/fargan_conditioning.conduit"),
            include_str!("../../speech/fargan_feature_policy.conduit"),
            include_str!("../../speech/fargan_feature_correlation.conduit"),
            include_str!("fixtures/fargan_feature_probe.conduit")].join("\n");
        let schema=plan::SourceSchema::for_entry(&source,"speech-test/fargan-feature-probe");
        let resources=coefficients(&schema);
        let history=preprocess::prepare(&source,&raw);
        assert_eq!(history.executions,12);assert_eq!(history.admission_receipts.len(),4);
        let waveform=state::vector(schema.ty("waveform"),&history.samples).canonical_bytes().unwrap();
        // Re-encoding retains exact previously executed finite-copy output bytes.
        assert_eq!(format!("{:x}",Sha256::digest(&waveform)),history.receipts.last().unwrap().1);
        let period_value=StructuredInfoValue::nominal(schema.ty("period").clone(),
            StructuredInfoValue::leaf(StructuredInfoType::leaf(kind_id("value/u16")).unwrap(),period.to_le_bytes().to_vec()).unwrap()).unwrap().canonical_bytes().unwrap();
        let selected=plan::prepare_entry_plan(&schema,&source,"speech-test/fargan-feature-probe");
        let plan_id=selected.plan_id.clone();
        let result=runtime::run_entry_plan(selected,&resources,BTreeMap::from([
            ("waveform".into(),waveform),("period".into(),period_value)]),runtime::ExecutionMode::Normal).unwrap();
        let features=state::floats(&result.outputs["features"]);
        assert_eq!(features.len(),20);assert!(features.iter().all(|v|v.is_finite()));
        assert!(state::floats(&result.outputs["power"]).iter().any(|v|*v>0.));
        assert!(!result.signs.is_empty());
        assert_eq!(complete.canonical_bytes(),original_bytes);
        assert_eq!(epochs.source().source_identity(),&complete.identity());
        let represented:usize=epochs.epochs().iter().map(|e|e.pcm().len()).sum();
        assert_eq!(represented as u64,frozen.total_frames());
        let blocked:Vec<_>=epochs.epochs().iter().enumerate().filter(|(_,e)|!e.blockers().is_empty()).map(|(i,e)|(i,e.blockers())).collect();
        eprintln!("actual carrier={:x?} projection={:x?} PCMprefixSHA={:x} SourceSHA={:x} Plan={plan_id:?} period={period} epoch_count={} later_blockers={blocked:?};320actual8k samples -> one feature20 observation; no whole-utterance/neural/model/quality claim",
            frozen.source_identity(),frozen.projection_identity(),Sha256::digest(raw.iter().flat_map(|v|v.to_le_bytes()).collect::<Vec<_>>()),Sha256::digest(source.as_bytes()),epochs.epochs().len());
    }).unwrap().join().unwrap();
}
