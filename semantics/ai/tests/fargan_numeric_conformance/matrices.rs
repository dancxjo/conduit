//! Every matrix shape required by the checked full-frame graph, without weights.
use super::*;
use conduit_ai::{fixed_neural::FixedNumericRefusal, fixed_tensor::*, fixed_tensor_linear::*};
use std::collections::BTreeSet;

macro_rules! profiles {
    ($apply:ident) => {
        $apply!(32, 64, true);
        $apply!(192, 128, true);
        $apply!(128, 320, true);
        $apply!(80, 1, true);
        $apply!(328, 192, true);
        $apply!(192, 192, true);
        $apply!(192, 4, true);
        $apply!(160, 160, true);
        $apply!(128, 128, true);
        $apply!(688, 128, true);
        $apply!(128, 40, true);
        $apply!(272, 480, false);
        $apply!(160, 480, false);
        $apply!(240, 384, false);
        $apply!(128, 384, false);
        $apply!(208, 384, false);
    };
}
pub(super) fn covered_kinds() -> BTreeSet<String> {
    let mut kinds = BTreeSet::new();
    macro_rules! add {
        ($i:literal,$o:literal,$bias:literal) => {
            kinds.insert(format!(
                "numeric/{}{}x{}",
                if $bias { "dense" } else { "linear" },
                $i,
                $o
            ));
        };
    }
    profiles!(add);
    kinds
}
fn no_growth<T>(work: impl FnOnce() -> T) -> T {
    let (result, counts) = observe(work);
    assert_eq!(
        (
            counts.allocation_attempts,
            counts.allocations,
            counts.reallocations
        ),
        (0, 0, 0)
    );
    result
}
fn compare<const I: usize, const O: usize>(biased: bool) {
    let weights: Vec<f32> = (0..I * O)
        .map(|n| {
            let (column, row) = (n / O, n % O);
            (((row * 19 + column * 7 + 5) % 29) as f32 - 14.0) / 64.0
        })
        .collect();
    let input: [f32; I] = core::array::from_fn(|i| (((i * 11 + 3) % 23) as f32 - 11.0) / 32.0);
    let bias: [f32; O] = core::array::from_fn(|i| (i % 13) as f32 / 64.0);
    let bytes = shared::packed(&weights);
    let bias_bytes = shared::packed(&bias);
    let tensor = shared::tensor(&[I as u64, O as u64], &bytes);
    let bias_tensor = shared::tensor(&[O as u64], &bias_bytes);
    let affine = FixedTensorAffine::<I, O>::prepare(
        &tensor,
        &bytes,
        &bias_tensor,
        &bias_bytes,
        FixedMatrixOrder::InputMajor,
    )
    .unwrap();
    let linear =
        FixedTensorLinear::<I, O>::prepare(&tensor, &bytes, FixedMatrixOrder::InputMajor).unwrap();
    let apply = |input: &[f32; I], output: &mut [f32; O]| {
        if biased {
            affine.apply(input, output)
        } else {
            linear.apply(input, output)
        }
    };
    let expected: [f64; O] = core::array::from_fn(|row| {
        let initial = if biased { f64::from(bias[row]) } else { 0.0 };
        initial
            + input
                .iter()
                .enumerate()
                .map(|(column, x)| f64::from(*x) * f64::from(weights[column * O + row]))
                .sum::<f64>()
    });
    let mut output = [19.0; O];
    no_growth(|| apply(&input, &mut output)).unwrap();
    for (actual, expected) in output.iter().zip(expected) {
        assert!(
            (f64::from(*actual) - expected).abs() <= 2e-6,
            "matrix {I}x{O}"
        );
    }
    let mut invalid = input;
    invalid[I - 1] = f32::NAN;
    output.fill(19.0);
    assert_eq!(
        no_growth(|| apply(&invalid, &mut output)),
        Err(FixedNumericRefusal::NonfiniteInput)
    );
    assert_eq!(output, [19.0; O]);

    // Fail on the last output lane, after preceding results have been staged.
    let mut overflow_weights = vec![0.0; I * O];
    overflow_weights[(I - 1) * O + O - 1] = f32::MAX;
    let overflow_bytes = shared::packed(&overflow_weights);
    let overflow_tensor = shared::tensor(&[I as u64, O as u64], &overflow_bytes);
    let affine = FixedTensorAffine::<I, O>::prepare(
        &overflow_tensor,
        &overflow_bytes,
        &bias_tensor,
        &bias_bytes,
        FixedMatrixOrder::InputMajor,
    )
    .unwrap();
    let linear = FixedTensorLinear::<I, O>::prepare(
        &overflow_tensor,
        &overflow_bytes,
        FixedMatrixOrder::InputMajor,
    )
    .unwrap();
    let mut input = [0.0; I];
    input[I - 1] = 2.0;
    let result = no_growth(|| {
        if biased {
            affine.apply(&input, &mut output)
        } else {
            linear.apply(&input, &mut output)
        }
    });
    assert_eq!(result, Err(FixedNumericRefusal::NonfiniteOutput));
    assert_eq!(output, [19.0; O]);
    eprintln!("matrix {I}x{O} biased={biased}: independent reference and atomic nonfinite/overflow refusal; no allocation attempts");
}
#[test]
fn every_required_matrix_shape_matches_independent_reference_and_refuses_atomically() {
    macro_rules! check {
        ($i:literal,$o:literal,$bias:literal) => {
            compare::<$i, $o>($bias)
        };
    }
    profiles!(check);
    assert_eq!(covered_kinds().len(), 16);
}
