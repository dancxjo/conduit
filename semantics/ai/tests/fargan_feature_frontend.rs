#![cfg(feature = "kernel-operation-owners")]
//! Public numerical feature approximation only, not canonical-carrier/neural PCM proof.
#[path = "fargan_feature_frontend/oracle.rs"]
mod oracle;
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
use conduit_core::*;
use sha2::{Digest, Sha256};
use std::{collections::BTreeMap, f64::consts::PI, path::PathBuf};
const PROFILE: &str = "speech/fargan-formant-spectral-approximation@1";
fn coefficients(schema: &plan::SourceSchema) -> shared::Resources {
    let manifest: serde_json::Value = serde_json::from_str(include_str!(
        "../../../proof/fargan/feature-profile/manifest.json"
    ))
    .unwrap();
    assert_eq!(manifest["profile"], PROFILE);
    assert_eq!(manifest["element"], "f32-le");
    assert_eq!(manifest["matrix_order"], "input-major");
    let directory = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../proof/fargan/feature-profile/generated");
    [
        ("band_weights", "bands161x18", vec![161_u64, 18]),
        ("band_bias", "band_bias18", vec![18]),
        ("dct_weights", "dct18x18", vec![18, 18]),
    ]
    .into_iter()
    .map(|(port, name, dims)| {
        let bytes = std::fs::read(directory.join(format!("{name}.bin")))
            .expect("materialize public coefficients using the pinned generator first");
        let spec = &manifest["arrays"][name];
        assert_eq!(spec["shape"], serde_json::json!(dims));
        assert_eq!(spec["bytes"].as_u64().unwrap(), bytes.len() as u64);
        assert_eq!(bytes.len() as u64, dims.iter().product::<u64>() * 4);
        assert_eq!(
            format!("{:x}", Sha256::digest(&bytes)),
            spec["sha256"].as_str().unwrap()
        );
        let values: Vec<_> = bytes
            .chunks_exact(4)
            .map(|b| f32::from_le_bytes(b.try_into().unwrap()))
            .collect();
        assert!(values.iter().all(|v| v.is_finite()));
        (
            port.into(),
            shared::Resource::new(schema.ty(port).clone(), &dims, values),
        )
    })
    .collect()
}
fn compare(name: &str, actual: &[f32], expected: &[f64], absolute: f64, relative: f64) {
    assert_eq!(actual.len(), expected.len());
    for (i, (&a, &e)) in actual.iter().zip(expected).enumerate() {
        assert!(a.is_finite() && e.is_finite());
        assert!(
            (f64::from(a) - e).abs() <= absolute + relative * e.abs(),
            "{name}[{i}]: actual={a} expected={e}"
        );
    }
}
#[test]
fn nonzero_pcm_executes_checked_feature_source_and_generic_dsp_with_public_coefficients() {
    // Existing fixture has large fixed scheduler arrays; explicit hosted stack,
    // not a device-memory or real-time claim. No separate execution engine.
    std::thread::Builder::new().stack_size(32*1024*1024).spawn(|| {
        let source = [include_str!("../../speech/fargan_conditioning.conduit"),
            include_str!("../../speech/fargan_feature_policy.conduit"),
            include_str!("../../speech/fargan_feature_correlation.conduit"),
            include_str!("fixtures/fargan_feature_probe.conduit")].join("\n");
        let schema = plan::SourceSchema::for_entry(&source, "speech-test/fargan-feature-probe");
        let resources = coefficients(&schema);
        // Synthetic signed-PCM excitation: 125Hz fundamental and nonzero phase/
        // harmonic content, never produced from spelling or a claimed speech model.
        let raw: [i16;320] = core::array::from_fn(|n|
            (900.*(2.*PI*n as f64/64.+0.37).sin()
             + 300.*(2.*PI*n as f64/32.+0.11).cos()).round() as i16);
        let history = preprocess::prepare(&source, &raw);
        assert_eq!(history.samples.len(), 640);
        assert_eq!(history.executions, 12);
        assert_eq!(history.programs.len(), 3);
        assert_eq!(history.receipts.len(), 12);
        assert_eq!(history.admission_receipts.len(), 8);
        assert!(history.samples.iter().any(|v| *v != 0.));
        // Independently verify executed causal interpolation/preemphasis. This
        // oracle is a test equation; the product proposal executes Speech Source.
        let (mut previous_raw, mut previous_normalized) = (0_f64,0_f64);
        let mut expected_history = Vec::new();
        for sample in raw {
            let sample=f64::from(sample);
            for normalized in [(previous_raw+sample)/65536., sample/32768.] {
                expected_history.push(normalized - f64::from(f32::from_bits(0x3f59999a))*previous_normalized);
                previous_normalized=normalized;
            }
            previous_raw=sample;
        }
        compare("preprocessing", &history.samples, &expected_history, 1e-7, 1e-6);
        let period = 128_u16; // exact 16kHz/125Hz numeric fixture; no clamping.
        let period_value = StructuredInfoValue::nominal(schema.ty("period").clone(),
            StructuredInfoValue::leaf(StructuredInfoType::leaf(kind_id("value/u16")).unwrap(),
                period.to_le_bytes().to_vec()).unwrap()).unwrap().canonical_bytes().unwrap();
        let inputs = BTreeMap::from([
            ("waveform".into(), state::vector(schema.ty("waveform"), &history.samples).canonical_bytes().unwrap()),
            ("period".into(), period_value),
        ]);
        let selected = plan::prepare_entry_plan(&schema, &source, "speech-test/fargan-feature-probe");
        let plan_id = selected.plan_id.clone();
        let result = runtime::run_entry_plan(selected, &resources, inputs,
            runtime::ExecutionMode::Normal).unwrap();
        let expected = oracle::evaluate(&history.samples, usize::from(period));
        compare("phase", &state::floats(&result.outputs["phase"]), &expected.phase, 2e-6, 2e-5);
        compare("power", &state::floats(&result.outputs["power"]), &expected.power, 1e-7, 3e-5);
        compare("bands", &state::floats(&result.outputs["bands"]), &expected.bands, 0.05, 3e-5);
        compare("features", &state::floats(&result.outputs["features"]), &expected.features, 0.002, 0.);
        assert!(!result.signs.is_empty());
        eprintln!("{PROFILE}: Source={:x} Plan={plan_id:?} nodes={} cords={} owner_prepare={:?} execution={:?}; synthetic nonzero PCM only; no carrier/startup/neural-waveform claim",
            Sha256::digest(source.as_bytes()), result.nodes, result.cords, result.preparation, result.execution);
    }).unwrap().join().unwrap();
}
