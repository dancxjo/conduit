//! Bind the operator corpus to the actual expanded frame and observe all Play.
use super::*;
use conduit_core::*;
use std::collections::BTreeSet;

fn inputs(schema: &plan::SourceSchema, prior: &state::State) -> BTreeMap<String, Vec<u8>> {
    let mut values = BTreeMap::new();
    for (name, value) in [("features", &[0.0; 20][..]), ("history", &[0.0; 128][..])] {
        values.insert(
            name.into(),
            state::vector(schema.ty(name), value)
                .canonical_bytes()
                .unwrap(),
        );
    }
    for name in ["period", "previous_period"] {
        let value = StructuredInfoValue::leaf(
            StructuredInfoType::leaf(kind_id("value/u16")).unwrap(),
            80u16.to_le_bytes().to_vec(),
        )
        .unwrap();
        values.insert(
            name.into(),
            StructuredInfoValue::nominal(schema.ty(name).clone(), value)
                .unwrap()
                .canonical_bytes()
                .unwrap(),
        );
    }
    values.insert("state".into(), prior.encode(schema.ty("state")));
    values
}
#[test]
fn full_frame_required_profiles_drain_without_growth_or_partial_pressure_commit() {
    on_large_stack(|| {
        let source = plan::source();
        let schema = plan::SourceSchema::for_entry(&source, "speech/fargan-frame");
        let plan = plan::prepare_entry_plan(&schema, &source, "speech/fargan-frame");
        assert!(verify_plan(&plan));
        let fragment = &plan.fragments[0];
        let numeric_nodes: Vec<_> = fragment
            .placements
            .iter()
            .enumerate()
            .filter(|(_, p)| p.kind_id.as_str().starts_with("numeric/"))
            .map(|(index, p)| {
                (
                    conduit_kernel::NodeId(u16::try_from(index).unwrap()),
                    p.kind_id.as_str(),
                )
            })
            .collect();
        let kinds: BTreeSet<_> = fragment
            .placements
            .iter()
            .map(|p| p.kind_id.as_str())
            .filter(|k| k.starts_with("numeric/"))
            .collect();
        let matrix_kinds: BTreeSet<_> = kinds
            .iter()
            .filter(|k| k.starts_with("numeric/dense") || k.starts_with("numeric/linear"))
            .map(|k| k.to_string())
            .collect();
        assert_eq!(
            matrix_kinds,
            matrices::covered_kinds(),
            "every newly required matrix shape needs an independent reference case"
        );
        let families: BTreeSet<_> = kinds
            .iter()
            .map(|k| k[8..].split(|c: char| c.is_ascii_digit()).next().unwrap())
            .collect();
        assert_eq!(
            families,
            BTreeSet::from([
                "add",
                "clamp",
                "complement",
                "concatenate",
                "dense",
                "embedding",
                "exp",
                "gather",
                "history",
                "linear",
                "multiply",
                "one-pole",
                "pair",
                "reciprocal-offset",
                "scale",
                "sigmoid",
                "slice",
                "tanh"
            ]),
            "a new operator family needs its own reference and refusal corpus"
        );
        assert_eq!(kinds.len(), 72);
        let resources: shared::Resources = schema
            .resource_shapes
            .iter()
            .map(|(name, shape)| {
                let mut values = vec![0.0; shape.iter().product::<u64>() as usize];
                if name == "signal_output_bias" {
                    values.fill(0.25);
                }
                (
                    name.clone(),
                    shared::Resource::new(schema.ty(name).clone(), shape, values),
                )
            })
            .collect();
        let prior = state::State {
            conv: [0.1; 164],
            gru1: [0.2; 160],
            gru2: [-0.4; 128],
            gru3: [0.6; 128],
            pitch: [0.1; 256],
            deemphasis: 0.0,
        };
        let retained = prior.flattened();
        let values = inputs(&schema, &prior);
        let (result, _) = run_measured(
            plan.clone(),
            &resources,
            values.clone(),
            runtime::ExecutionMode::Normal,
        );
        let result = result.unwrap();
        for (node, kind) in numeric_nodes {
            assert_eq!(
                result
                    .signs
                    .iter()
                    .filter(|sign| sign.node == node
                        && sign.kind == conduit_kernel::KernelEventKind::BackCompleted)
                    .count(),
                1,
                "required {kind} must actually complete in the observed Play"
            );
        }
        let next = state::State::from_result(&result.value);
        assert_eq!(next.gru1, [0.2 / 16.0; 160]);
        assert_eq!(next.gru2, [-0.4 / 16.0; 128]);
        assert_eq!(next.gru3, [0.6 / 16.0; 128]);
        let mut pcm = Vec::new();
        for name in ["pcm0", "pcm1", "pcm2"] {
            pcm.extend(state::floats(&result.outputs[name]));
        }
        pcm.extend(state::floats(state::field(&result.value, "pcm_f32")));
        assert_eq!(pcm.len(), 160);
        let mut expected = 0.0f64;
        for actual in pcm {
            expected = 0.25f64.tanh() + f64::from(0.85f32) * expected;
            assert!((f64::from(actual) - expected).abs() < 2e-6);
        }
        assert!((f64::from(next.deemphasis) - expected).abs() < 2e-6);
        assert_eq!(state::floats(&result.outputs["next_history"]), [0.0; 128]);
        assert!(result
            .signs
            .iter()
            .enumerate()
            .all(|(i, s)| s.sequence == i as u32));
        assert!(!result
            .signs
            .iter()
            .any(|s| matches!(s.kind, conduit_kernel::KernelEventKind::BackFailed)));
        for mode in [
            runtime::ExecutionMode::StoragePressure,
            runtime::ExecutionMode::CancelFirstExpression,
        ] {
            assert!(run_measured(plan.clone(), &resources, values.clone(), mode)
                .0
                .is_none());
            assert_eq!(prior.flattened(), retained);
        }
        eprintln!("Full frame: 72 required numeric kinds, 16 independently checked matrix profiles, 160 reference samples; zero Play allocation attempts on success, pressure and cancellation");
    });
}
