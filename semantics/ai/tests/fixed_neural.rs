use conduit_ai::fixed_neural::*;

#[test]
fn affine_row_order_matches_independent_double_precision_oracle() {
    let weights = [[1.25, -2.0, 0.5], [-3.0, 0.25, 2.0]];
    let bias = [0.125, -0.375];
    let input = [2.0, -0.5, 3.0];
    let mut output = [99.0; 2];
    FixedAffine::prepare(&weights, &bias)
        .unwrap()
        .apply(&input, &mut output)
        .unwrap();
    for row in 0..2 {
        let oracle = f64::from(bias[row])
            + (0..3)
                .map(|column| f64::from(weights[row][column]) * f64::from(input[column]))
                .sum::<f64>();
        assert_eq!(f64::from(output[row]), oracle);
    }
    assert_eq!(output, [5.125, -0.5]);
}

#[test]
fn quantized_scales_are_per_output_and_missing_or_bad_metadata_refuses() {
    let weights = [[4i8, -8, 2], [-12, 1, 8]];
    let scales = [0.25, 0.125];
    let bias = [0.125, -0.375];
    let input = [2.0, -0.5, 3.0];
    let decoded = [[1.0, -2.0, 0.5], [-1.5, 0.125, 1.0]];
    let mut compact = [0.0; 2];
    let mut reference = [0.0; 2];
    FixedAffineI8::prepare(&weights, &scales, &bias)
        .unwrap()
        .apply(&input, &mut compact)
        .unwrap();
    FixedAffine::prepare(&decoded, &bias)
        .unwrap()
        .apply(&input, &mut reference)
        .unwrap();
    assert_eq!(compact, reference);
    for invalid in [0.0, -1.0, f32::NAN, f32::INFINITY] {
        assert!(matches!(
            FixedAffineI8::prepare(&weights, &[invalid, 1.0], &bias),
            Err(FixedNumericRefusal::InvalidScale)
        ));
    }
}

#[test]
fn overflow_and_nonfinite_inputs_do_not_publish_partial_output() {
    let weights = [[1.0], [f32::MAX]];
    let affine = FixedAffine::prepare(&weights, &[0.0; 2]).unwrap();
    let mut output = [7.0, 11.0];
    assert_eq!(
        affine.apply(&[2.0], &mut output),
        Err(FixedNumericRefusal::NonfiniteOutput)
    );
    assert_eq!(output, [7.0, 11.0]);
    assert_eq!(
        affine.apply(&[f32::NAN], &mut output),
        Err(FixedNumericRefusal::NonfiniteInput)
    );
    assert_eq!(output, [7.0, 11.0]);
    assert!(matches!(
        FixedAffine::prepare(&[[f32::INFINITY]], &[0.0]),
        Err(FixedNumericRefusal::NonfiniteWeight)
    ));
}

#[test]
fn activation_profiles_match_independent_f64_functions_and_saturate() {
    let input = [-1000.0f32, -3.0, 0.0, 2.0, 1000.0];
    let mut sigmoid = [0.0; 5];
    let mut tanh = [0.0; 5];
    fixed_sigmoid(&input, &mut sigmoid).unwrap();
    fixed_tanh(&input, &mut tanh).unwrap();
    for index in 0..5 {
        let x = f64::from(input[index]);
        assert!((f64::from(sigmoid[index]) - 1.0 / (1.0 + (-x).exp())).abs() <= 1e-7);
        assert!((f64::from(tanh[index]) - x.tanh()).abs() <= 1e-7);
    }
    assert_eq!(sigmoid[0], 0.0);
    assert_eq!(sigmoid[4], 1.0);
    let prior = sigmoid;
    assert_eq!(
        fixed_sigmoid(&[f32::INFINITY; 5], &mut sigmoid),
        Err(FixedNumericRefusal::NonfiniteInput)
    );
    assert_eq!(sigmoid, prior);
}

#[test]
fn embedding_refuses_outside_fixed_table_without_touching_output() {
    let table = [[1.0, 2.0], [3.0, 4.0]];
    let mut output = [0.0; 2];
    fixed_embedding(&table, 1, &mut output).unwrap();
    assert_eq!(output, [3.0, 4.0]);
    assert_eq!(
        fixed_embedding(&table, 2, &mut output),
        Err(FixedNumericRefusal::Index)
    );
    assert_eq!(output, [3.0, 4.0]);
}
