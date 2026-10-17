#[cfg(test)]
use super::on_large_stack;
#[cfg(test)]
use super::{
    pinned,
    plan::{prepare_entry_plan, source},
    runtime::*,
    shared::*,
    state::floats,
};
use super::{plan::SourceSchema, state::vector};
use conduit_core::*;
use std::collections::BTreeMap;

pub fn input(
    schema: &SourceSchema,
    features: &[f32; 20],
    history: &[f32; 128],
    period: u16,
    previous: u16,
) -> BTreeMap<String, Vec<u8>> {
    let mut result = BTreeMap::new();
    for (name, values) in [
        ("features", features.as_slice()),
        ("history", history.as_slice()),
    ] {
        result.insert(
            name.into(),
            vector(schema.ty(name), values).canonical_bytes().unwrap(),
        );
    }
    for (name, value) in [("period", period), ("previous_period", previous)] {
        let leaf = StructuredInfoValue::leaf(
            StructuredInfoType::leaf(kind_id("value/u16")).unwrap(),
            value.to_le_bytes().to_vec(),
        )
        .unwrap();
        result.insert(
            name.into(),
            StructuredInfoValue::nominal(schema.ty(name).clone(), leaf)
                .unwrap()
                .canonical_bytes()
                .unwrap(),
        );
    }
    result
}
fn period(value: &StructuredInfoValue) -> u16 {
    match value.shape() {
        StructuredInfoValueShape::Leaf(bytes) => u16::from_le_bytes(bytes.try_into().unwrap()),
        _ => panic!("period"),
    }
}
#[test]
fn conditioner_executes_with_explicit_history_and_delayed_period() {
    on_large_stack(|| {
        let schema = SourceSchema::for_entry(&source(), "speech/fargan-conditioning");
        let resources: Resources = schema
            .resource_shapes
            .iter()
            .map(|(name, shape)| {
                (
                    name.clone(),
                    Resource::new(
                        schema.ty(name).clone(),
                        shape,
                        vec![0.; shape.iter().product::<u64>() as usize],
                    ),
                )
            })
            .collect();
        let history = std::array::from_fn(|i| i as f32 / 128.);
        let plan = prepare_entry_plan(&schema, &source(), "speech/fargan-conditioning");
        let result = run_entry_plan(
            plan,
            &resources,
            input(&schema, &[0.; 20], &history, 80, 160),
            ExecutionMode::Normal,
        )
        .unwrap();
        assert_eq!(floats(&result.outputs["condition"]), [0.; 320]);
        let next = floats(&result.outputs["next_history"]);
        assert_eq!(&next[..64], &history[64..]);
        assert_eq!(&next[64..], &[0.; 64]);
        assert_eq!(period(&result.outputs["conditioned_period"]), 160);
        assert_eq!(period(&result.outputs["next_period"]), 80);
    });
}
#[test]
#[ignore = "requires explicit pinned local model and conditioning oracle"]
fn pinned_conditioning_differential() {
    on_large_stack(|| {
        use sha2::{Digest, Sha256};
        let root = std::path::PathBuf::from(
            std::env::var_os("CONDUIT_FARGAN_DEVELOPMENT_FIXTURE").expect("explicit fixture"),
        );
        let schema = SourceSchema::for_entry(&source(), "speech/fargan-conditioning");
        let resources = pinned::conditioning_resources(&root, &schema);
        let plan = prepare_entry_plan(&schema, &source(), "speech/fargan-conditioning");
        let inputs = std::fs::read(root.join("conditions.f32le")).unwrap();
        let oracle = std::fs::read(root.join("conditioning.trace")).unwrap();
        assert_eq!(inputs.len(), 24 * 80);
        assert_eq!(oracle.len(), 24 * 1800);
        assert_eq!(
            format!("{:x}", Sha256::digest(&oracle)),
            "ca634e3e31af975329ed3db41d38cf1d8c05a164ed0d8b16bcebcd270e5faacd"
        );
        let mut history = [0.; 128];
        let mut previous = 69;
        let mut condition_error = 0f32;
        let mut history_error = 0f32;
        for (frame, bytes) in inputs.as_chunks::<80>().0.iter().enumerate() {
            let features = std::array::from_fn(|i| {
                f32::from_le_bytes(bytes[i * 4..i * 4 + 4].try_into().unwrap())
            });
            let record = &oracle[frame * 1800..(frame + 1) * 1800];
            let current =
                u16::try_from(i32::from_le_bytes(record[1796..].try_into().unwrap())).unwrap();
            let result = run_entry_plan(
                plan.clone(),
                &resources,
                input(&schema, &features, &history, current, previous),
                ExecutionMode::Normal,
            )
            .unwrap();
            assert_eq!(period(&result.outputs["conditioned_period"]), previous);
            assert_eq!(period(&result.outputs["next_period"]), current);
            for (i, value) in floats(&result.outputs["condition"]).iter().enumerate() {
                condition_error = condition_error.max(
                    (value - f32::from_le_bytes(record[i * 4..i * 4 + 4].try_into().unwrap()))
                        .abs(),
                );
            }
            let next = floats(&result.outputs["next_history"]);
            for (i, value) in next.iter().enumerate() {
                let start = 1280 + i * 4;
                history_error = history_error.max(
                    (value - f32::from_le_bytes(record[start..start + 4].try_into().unwrap()))
                        .abs(),
                );
            }
            history = next.try_into().unwrap();
            previous = current;
        }
        eprintln!("24-frame conditioning: condition max_abs={condition_error}, history max_abs={history_error}");
        assert!(condition_error < 0.002 && history_error < 0.001);
    });
}
