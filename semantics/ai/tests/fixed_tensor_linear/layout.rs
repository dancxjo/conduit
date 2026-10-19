//! Compare contiguous input-major traversal against the prior ordered scalar loop.
use super::*;
fn reference<const I: usize, const O: usize>(
    weights: &[f32],
    input: &[f32; I],
    order: FixedMatrixOrder,
) -> Result<[f32; O], FixedNumericRefusal> {
    if input.iter().any(|value| !value.is_finite()) {
        return Err(FixedNumericRefusal::NonfiniteInput);
    }
    let mut output = [0.; O];
    for (row, result) in output.iter_mut().enumerate() {
        let mut sum = 0.0_f32;
        for (column, value) in input.iter().enumerate() {
            let index = match order {
                FixedMatrixOrder::InputMajor => column * O + row,
                FixedMatrixOrder::OutputMajor => row * I + column,
            };
            sum += weights[index] * value;
        }
        if !sum.is_finite() {
            return Err(FixedNumericRefusal::NonfiniteOutput);
        }
        *result = sum;
    }
    Ok(output)
}
fn compare<const I: usize, const O: usize>(
    weights: &[f32],
    input: &[f32; I],
    order: FixedMatrixOrder,
) {
    let packed = bytes(weights);
    let shape = match order {
        FixedMatrixOrder::InputMajor => [I as u64, O as u64],
        FixedMatrixOrder::OutputMajor => [O as u64, I as u64],
    };
    let tensor = resource_tensor(&shape, &packed);
    let linear = FixedTensorLinear::<I, O>::prepare(&tensor, &packed, order).unwrap();
    let mut actual = [f32::from_bits(0x41200001); O];
    let sentinel = actual.map(f32::to_bits);
    let expected = reference::<I, O>(weights, input, order);
    let result = linear.apply(input, &mut actual);
    match expected {
        Ok(expected) => {
            assert_eq!(result, Ok(()));
            assert_eq!(actual.map(f32::to_bits), expected.map(f32::to_bits));
        }
        Err(error) => {
            assert_eq!(result, Err(error));
            assert_eq!(actual.map(f32::to_bits), sentinel);
        }
    }
}
fn shapes<const I: usize, const O: usize>() {
    let mut seed = 0x913a_6579_u32;
    let mut next = || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        let exponent = 102 + (seed >> 25) % 40;
        f32::from_bits((seed & 0x807f_ffff) | (exponent << 23))
    };
    for _ in 0..16 {
        let weights: Vec<_> = (0..I * O).map(|_| next()).collect();
        let input = core::array::from_fn(|_| next());
        compare::<I, O>(&weights, &input, FixedMatrixOrder::InputMajor);
        compare::<I, O>(&weights, &input, FixedMatrixOrder::OutputMajor);
    }
}
#[test]
fn real_matrix_shapes_match_scalar_reference_bit_for_bit_for_both_orders() {
    shapes::<1, 1>();
    shapes::<3, 2>();
    shapes::<20, 40>();
    shapes::<40, 128>();
    shapes::<128, 160>();
    shapes::<160, 128>();
    shapes::<192, 160>();
    shapes::<320, 160>();
}
#[test]
fn ordered_cancellation_subnormals_signed_zero_and_atomic_refusal_are_preserved() {
    for order in [FixedMatrixOrder::InputMajor, FixedMatrixOrder::OutputMajor] {
        // A fused multiply-add would retain the product's discarded low bits.
        let a = f32::from_bits(0x3f800001);
        let b = f32::from_bits(0x3f7ffffe);
        assert_eq!(
            reference::<2, 1>(&[-1., a], &[1., b], order).unwrap()[0].to_bits(),
            0
        );
        compare::<2, 1>(&[-1., a], &[1., b], order);
        compare::<4, 1>(&[1e20, 1., -1e20, 3.], &[1.; 4], order);
        compare::<2, 2>(
            &[0., -0., f32::from_bits(1), -f32::from_bits(1)],
            &[-0., 1.],
            order,
        );
        compare::<2, 2>(&[f32::MAX, 0., 0., 1.], &[2., 1.], order);
        compare::<2, 2>(&[1., 0., 0., f32::MAX], &[1., 2.], order);
        compare::<2, 2>(&[1., 2., 3., 4.], &[f32::NAN, 1.], order);
        compare::<2, 2>(&[1., 2., 3., 4.], &[1., f32::INFINITY], order);
    }
}

#[test]
fn contiguous_path_retains_rounding_subnormals_and_atomic_failure() {
    const I: usize = 40;
    const O: usize = 128;
    for order in [FixedMatrixOrder::InputMajor, FixedMatrixOrder::OutputMajor] {
        let index = |column: usize, row: usize| match order {
            FixedMatrixOrder::InputMajor => column * O + row,
            FixedMatrixOrder::OutputMajor => row * I + column,
        };
        let mut weights = vec![0.; I * O];
        let mut input = [0.; I];
        input[0] = 1.;
        input[1] = f32::from_bits(0x3f7ffffe);
        for row in 0..O {
            weights[index(0, row)] = -1.;
            weights[index(1, row)] = f32::from_bits(0x3f800001);
        }
        assert!(reference::<I, O>(&weights, &input, order)
            .unwrap()
            .iter()
            .all(|value| value.to_bits() == 0));
        compare::<I, O>(&weights, &input, order);
        weights.fill(0.);
        input.fill(0.);
        input[0] = 1.;
        for row in 0..O {
            weights[index(0, row)] = if row % 2 == 0 {
                f32::from_bits(1)
            } else {
                -f32::from_bits(1)
            };
        }
        compare::<I, O>(&weights, &input, order);
        weights[index(0, O - 1)] = f32::MAX;
        input[0] = 2.;
        compare::<I, O>(&weights, &input, order);
        input[I - 1] = f32::NAN;
        compare::<I, O>(&weights, &input, order);
    }
}
