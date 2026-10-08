use super::common::*;
use conduit_audio::*;
use conduit_plot::rust_binding::NativeRustBinding;
#[test]
fn real_formant_step_values_and_sample_time_queries_execute_exactly() {
    for rate in [8000_u64, 16000, 48000] {
        for value in [1500_u64, 2500, u64::MAX] {
            let original = trajectory(vec![segment(
                time(0, 1),
                time(1, 10),
                hz(value, 1),
                hz(value, u64::MAX),
                AudioTrajectoryInterpolation::Step,
            )]);
            let prepared = prepare(&original).unwrap();
            for frame in [1, rate / 20] {
                let query = query(time(frame, rate));
                let receipt = prepared.evaluate(&query).unwrap();
                assert_eq!(ratio(receipt.result()), (u128::from(value), 1));
                assert_eq!(
                    receipt.original_canonical(),
                    original.clone().encode().unwrap()
                );
                assert_eq!(receipt.query_canonical(), query);
                for e in receipt.executions() {
                    assert_eq!(
                        conduit_plot::PortableExpressionProgram::from_canonical_hex(
                            e.source_program_hex()
                        )
                        .unwrap()
                        .evaluate(e.input_canonical())
                        .unwrap(),
                        e.output_canonical()
                    );
                }
            }
            assert_eq!(
                ratio(prepared.evaluate(&query(time(1, 10))).unwrap().result()),
                (u128::from(value), u128::from(u64::MAX))
            );
        }
    }
}
#[test]
fn original_full_u64_step_fractions_and_u32_cross_products_remain_exact() {
    let top = u64::from(u32::MAX);
    let authored = trajectory(vec![segment(
        time(top - 1, top),
        time(top, top - 1),
        hz(u64::MAX, u64::MAX - 1),
        hz(1, u64::MAX),
        AudioTrajectoryInterpolation::Step,
    )]);
    let p = prepare(&authored).unwrap();
    let q = time(top, top);
    let receipt = p.evaluate(&query(q)).unwrap();
    assert_eq!(
        ratio(receipt.result()),
        (u128::from(u64::MAX), u128::from(u64::MAX - 1))
    );
    let start_product = u128::from(top) * u128::from(top);
    assert!(start_product <= u128::from(u64::MAX));
    assert!(p.evaluate(&query(time(top + 1, top + 1))).is_err());
    let linear = trajectory(vec![segment(
        time(0, 1),
        time(1, 1),
        hz(1500, 1),
        hz(2500, 1),
        AudioTrajectoryInterpolation::Linear,
    )]);
    assert!(prepare(&linear).is_err());
    let small_linear = trajectory(vec![segment(
        time(0, 1),
        time(1, 1),
        hz(100, 1),
        hz(200, 1),
        AudioTrajectoryInterpolation::Linear,
    )]);
    assert!(prepare(&small_linear)
        .unwrap()
        .evaluate(&query(time(1, 8000)))
        .is_err());
}
