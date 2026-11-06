//! Detect gate permutations and reset-before drift through actual checked Source.
use super::*;
fn sigmoid(x: f64) -> f64 {
    1.0 / (1.0 + (-x).exp())
}
fn oracle(input: &[f32], prior: &[f32], weights: &[f32], recurrent: &[f32]) -> Vec<f64> {
    let h = prior.len();
    (0..h)
        .map(|lane| {
            let mut gates = [0.0; 3];
            let mut history = [0.0; 3];
            for gate in 0..3 {
                gates[gate] = input
                    .iter()
                    .enumerate()
                    .map(|(i, x)| f64::from(*x) * f64::from(weights[i * 3 * h + gate * h + lane]))
                    .sum();
                history[gate] = prior
                    .iter()
                    .enumerate()
                    .map(|(i, x)| f64::from(*x) * f64::from(recurrent[i * 3 * h + gate * h + lane]))
                    .sum();
            }
            let z = sigmoid(gates[0] + history[0]);
            let r = sigmoid(gates[1] + history[1]);
            z * f64::from(prior[lane]) + (1.0 - z) * (gates[2] + r * history[2]).tanh()
        })
        .collect()
}
fn permute(section: &str, hidden: usize, left: (&str, usize), right: (&str, usize)) -> String {
    let mut changed = section.to_owned();
    for (gate, old, new) in [(left.0, left.1, right.1), (right.0, right.1, left.1)] {
        for side in ["input", "recurrent"] {
            let before = format!("prior >> ({}) >> {gate}_{side}.start", old * hidden);
            let after = format!("prior >> ({}) >> {gate}_{side}.start", new * hidden);
            assert_eq!(changed.matches(&before).count(), 1);
            changed = changed.replace(&before, &after);
        }
    }
    changed
}
fn reset_before(section: &str, hidden: usize) -> String {
    let mut changed = section.replace(
        "    recurrent_affine: ",
        &format!(
            "    candidate_recurrent_affine: numeric/linear{hidden}x{}\n    recurrent_affine: ",
            3 * hidden
        ),
    );
    for (before,after) in [
        ("recurrent_affine.result >> h_recurrent.value", "candidate_recurrent_affine.result >> h_recurrent.value"),
        ("h_recurrent.result >> reset.right", "prior >> reset.right"),
        ("reset.result >> candidate_sum.right", "h_recurrent.result >> candidate_sum.right\n    reset.result >> candidate_recurrent_affine.value\n    recurrent_weights >> candidate_recurrent_affine.weights"),
    ] {
        assert_eq!(changed.matches(before).count(),1);
        changed=changed.replace(before,after);
    }
    changed
}
fn exercise(width: usize, hidden: usize) {
    let source = plan::source();
    let entry = format!("speech/fargan-gru{width}x{hidden}");
    let start = source.find(&format!("plot {entry} ")).unwrap();
    let end = source[start + 1..]
        .find("\nplot ")
        .map_or(source.len(), |n| start + 1 + n);
    let section = &source[start..end];
    let schema = plan::SourceSchema::for_entry(&source, &entry);
    let mut input = vec![0.0f32; width];
    input[..3].copy_from_slice(&[0.75, -0.5, 0.25]);
    let prior: Vec<f32> = (0..hidden).map(|i| -0.2 + i as f32 * 0.005).collect();
    let mut w = vec![0.0f32; width * 3 * hidden];
    let mut r = vec![0.0f32; hidden * 3 * hidden];
    for j in 0..hidden {
        w[j] = 0.2 + (j % 7) as f32 * 0.01;
        w[3 * hidden + hidden + j] = -0.3 + (j % 5) as f32 * 0.01;
        w[6 * hidden + 2 * hidden + j] = 0.4 + (j % 3) as f32 * 0.02;
        r[j * 3 * hidden + j] = -0.15;
        r[j * 3 * hidden + hidden + j] = 0.25;
        r[((j + 1) % hidden) * 3 * hidden + 2 * hidden + j] = 0.5;
    }
    let expected = oracle(&input, &prior, &w, &r);
    let resources: BTreeMap<_, _> = [
        ("input_weights", vec![width as u64, (3 * hidden) as u64], w),
        (
            "recurrent_weights",
            vec![hidden as u64, (3 * hidden) as u64],
            r,
        ),
    ]
    .into_iter()
    .map(|(name, shape, values)| {
        (
            name.to_owned(),
            shared::Resource::new(schema.ty(name).clone(), &shape, values),
        )
    })
    .collect();
    let values: BTreeMap<_, _> = [("value", input), ("prior", prior)]
        .into_iter()
        .map(|(name, vector)| {
            (
                name.to_owned(),
                state::vector(schema.ty(name), &vector)
                    .canonical_bytes()
                    .unwrap(),
            )
        })
        .collect();
    let variants = [
        section.to_owned(),
        permute(section, hidden, ("z", 0), ("r", 1)),
        permute(section, hidden, ("r", 1), ("h", 2)),
        reset_before(section, hidden),
    ];
    let mut identity = None;
    for (mutation, variant) in variants.iter().enumerate() {
        let changed = format!("{}{}{}", &source[..start], variant, &source[end..]);
        let plan = plan::prepare_entry_plan(&schema, &changed, &entry);
        assert!(conduit_core::verify_plan(&plan));
        if mutation == 0 {
            identity = Some(plan.source_document_id.clone());
        } else {
            assert_ne!(identity.as_ref().unwrap(), &plan.source_document_id);
        }
        let (result, counts) = run_measured(
            plan,
            &resources,
            values.clone(),
            runtime::ExecutionMode::Normal,
        );
        let result = result.unwrap();
        assert_eq!(
            (counts.allocations, counts.reallocations),
            (0, 0),
            "Play growth: {width}/{hidden} mutation {mutation}"
        );
        let actual = state::floats(&result.outputs["next_state"]);
        let error = actual
            .iter()
            .zip(&expected)
            .map(|(a, b)| (f64::from(*a) - b).abs())
            .fold(0.0, f64::max);
        if mutation == 0 {
            assert!(error <= 2e-6, "reference error {error}");
        } else {
            assert!(error > 1e-4, "mutation {mutation} escaped oracle: {error}");
        }
        eprintln!("GRU {width}x{hidden} mutation={mutation} max_error={error} Play_allocations={} Play_reallocations={}",counts.allocations,counts.reallocations);
    }
}
#[test]
fn authored_gate_order_and_reset_after_match_independent_oracle_and_detect_mutations() {
    on_large_stack(|| {
        for (input, hidden) in [(272, 160), (240, 128), (208, 128)] {
            exercise(input, hidden);
        }
    });
}
