//! Original warm-start tests retain their established names and assertions.
use super::warm_start_support::*;
use super::{frame, on_large_stack, plan, runtime, state};
use sha2::{Digest, Sha256};
use std::path::PathBuf;
#[test]
#[ignore = "requires explicitly supplied private pinned FARGAN tensors/full-frame oracle"]
fn pinned_warm_start_executes_five_conditioners_and_four_teacher_forced_subframes() {
    on_large_stack(|| {
        let root = PathBuf::from(
            std::env::var_os("CONDUIT_FARGAN_DEVELOPMENT_FIXTURE").expect("private fixture"),
        );
        let oracle = frame::oracle(&root); // verifies exact9c676 trace and initial-state geometry
        let source = source();
        let schema = plan::SourceSchema::for_entry(&source, ENTRY);
        let resources = frame::resources(&root, &schema);
        let prepared = plan::prepare_entry_plan(&schema, &source, ENTRY);
        let features = [oracle.cases[0].features; 5];
        let input = inputs(&schema, &[0.; 320], &features, [80; 5]);
        let result = runtime::run_entry_plan(
            prepared.clone(),
            &resources,
            input.clone(),
            runtime::ExecutionMode::Normal,
        )
        .unwrap();
        compare(&result, &oracle.state, &oracle.history, 80);
        assert!(state::State::from_state(&result.outputs["next_state"])
            .flattened()
            .iter()
            .any(|v| *v != 0.));
        for mode in [
            runtime::ExecutionMode::StoragePressure,
            runtime::ExecutionMode::CancelFirstExpression,
        ] {
            assert!(
                runtime::run_entry_plan(prepared.clone(), &resources, input.clone(), mode)
                    .is_none()
            );
        }
        eprintln!(
            "warm SourceSHA={:x}; only full-float numerical warm-start proof, no carrier/quality/startup-trim claim",
            Sha256::digest(source.as_bytes())
        );
    });
}

fn f32s(bytes: &[u8]) -> Vec<f32> {
    assert_eq!(bytes.len() % 4, 0);
    bytes
        .chunks_exact(4)
        .map(|b| {
            let v = f32::from_le_bytes(b.try_into().unwrap());
            assert!(v.is_finite());
            v
        })
        .collect()
}
#[test]
#[ignore = "requires private pinned tensors and independently built supplied-context C warm fixture"]
fn pinned_varied_warm_start_retains_teacher_pcm_and_period3_not_period4() {
    on_large_stack(|| {
        let model = PathBuf::from(
            std::env::var_os("CONDUIT_FARGAN_DEVELOPMENT_FIXTURE").expect("private model fixture"),
        );
        let fixture = PathBuf::from(
            std::env::var_os("CONDUIT_FARGAN_WARM_ORACLE_FIXTURE").expect("private C warm fixture"),
        );
        let manifest: serde_json::Value =
            serde_json::from_slice(&std::fs::read(fixture.join("manifest.json")).unwrap()).unwrap();
        assert_eq!(
            manifest["upstream_sha"],
            "503d81b138d76621aae4b12786e90de48aa8db3a"
        );
        assert_eq!(
            manifest["model_f32_sha256"],
            "d35f0510da476716183a33caafe45cc095e48d496f06de2db7655e780a385e47"
        );
        assert_eq!(
            manifest["periods"],
            serde_json::json!([64, 80, 96, 128, 160])
        );
        assert_eq!(
            manifest["oracle_source_sha256"],
            format!(
                "{:x}",
                Sha256::digest(include_bytes!(
                    "../../../../proof/fargan/development-reference/warm_start_oracle.c"
                ))
            )
        );
        let raw = std::fs::read(fixture.join("warm-input.bin")).unwrap();
        assert_eq!(raw.len(), 1680);
        assert_eq!(
            format!("{:x}", Sha256::digest(&raw)),
            "824f35c1cce745c085c792c195e206dfb7e94dcbacf8e208225a2b3030b47665"
        );
        assert_eq!(
            manifest["input_sha256"],
            format!("{:x}", Sha256::digest(&raw))
        );
        let expected = std::fs::read(fixture.join("warm-state.bin")).unwrap();
        assert_eq!(expected.len(), 3864);
        assert_eq!(
            manifest["state_sha256"],
            format!("{:x}", Sha256::digest(&expected))
        );
        let context: [f32; 320] = f32s(&raw[..1280]).try_into().unwrap();
        let features: [[f32; 20]; 5] =
            core::array::from_fn(|i| f32s(&raw[1280 + i * 80..1360 + i * 80]).try_into().unwrap());
        let expected_state = state::State::from_oracle(&f32s(&expected[..3348]));
        let expected_history: [f32; 128] = f32s(&expected[3348..3860]).try_into().unwrap();
        assert_eq!(
            i32::from_le_bytes(expected[3860..].try_into().unwrap()),
            128
        );
        assert_eq!(expected_state.deemphasis.to_bits(), context[319].to_bits());
        for (j, &actual) in expected_state.pitch.iter().enumerate() {
            let i = j + 64;
            let teacher = context[i] - 0.85f32 * context[i - 1];
            assert_eq!(actual.to_bits(), teacher.to_bits());
        }
        let source = source();
        let schema = plan::SourceSchema::for_entry(&source, ENTRY);
        let resources = frame::resources(&model, &schema);
        let prepared = plan::prepare_entry_plan(&schema, &source, ENTRY);
        // Only raw context/features and explicit periods enter product execution.
        // The independent C warm state remains comparison data throughout.
        let result = runtime::run_entry_plan(
            prepared,
            &resources,
            inputs(&schema, &context, &features, [64, 80, 96, 128, 160]),
            runtime::ExecutionMode::Normal,
        )
        .unwrap();
        compare(&result, &expected_state, &expected_history, 128);
    });
}
