//! Optional Linux thread-CPU/wall capture; ordinary conformance does not time code.
use super::*;
use std::{hint::black_box, time::Instant};
fn cpu_ns() -> u128 {
    let mut time = libc::timespec {
        tv_sec: 0,
        tv_nsec: 0,
    };
    assert_eq!(
        unsafe { libc::clock_gettime(libc::CLOCK_THREAD_CPUTIME_ID, &mut time) },
        0
    );
    u128::try_from(time.tv_sec).unwrap() * 1_000_000_000 + u128::try_from(time.tv_nsec).unwrap()
}
fn request() -> MathematicalEnclosureRequirements {
    let ty = conduit_semantic_catalog::mathematical_enclosure_request_type();
    let leaf = StructuredInfoType::leaf(kind_id(MATHEMATICAL_VALUE_ID)).unwrap();
    let q = ExactRational::ratio(
        1,
        100_000_000_000_000_000,
        &mut ExactNumericWork::new(ENCLOSURE_WORK),
    )
    .unwrap();
    let mut bytes = [0; 2 + EXACT_RATIONAL_MAX_ENCODED_BYTES];
    let n = MathematicalValue::rational(q).encode(&mut bytes).unwrap();
    let value = StructuredInfoValue::record(
        ty.clone(),
        vec![StructuredFieldValue::new(
            "maximum-width",
            StructuredInfoValue::leaf(leaf, bytes[..n].to_vec()).unwrap(),
        )
        .unwrap()],
    )
    .unwrap();
    let configuration = [ConfigurationEntry {
        key: "request".into(),
        value: ConfigurationValue::Structured(
            StructuredConfigurationValue::new(
                ty.profile().unwrap().value_kind().clone(),
                value.canonical_bytes().unwrap(),
            )
            .unwrap(),
        ),
    }];
    conduit_semantic_catalog::mathematical_enclose_requirements(&configuration).unwrap()
}
fn specimen(which: u8) -> Vec<u8> {
    let mut work = ExactNumericWork::new(ENCLOSURE_WORK);
    let value = if which == 0 {
        MathematicalValue::rational(ExactRational::ratio(1, 3, &mut work).unwrap())
    } else {
        let mut builder = MathematicalValueBuilder::new();
        let root = if which == 1 {
            let pi = builder.insert(MathematicalNode::Pi, &mut work).unwrap();
            let n = builder
                .insert(
                    MathematicalNode::Rational(ExactRational::from_i128(180)),
                    &mut work,
                )
                .unwrap();
            builder
                .insert(MathematicalNode::Divide(pi, n), &mut work)
                .unwrap()
        } else {
            let two = builder
                .insert(
                    MathematicalNode::Rational(ExactRational::from_i128(2)),
                    &mut work,
                )
                .unwrap();
            builder
                .insert(MathematicalNode::PrincipalSquareRoot(two), &mut work)
                .unwrap()
        };
        builder.finish(root, &mut work).unwrap()
    };
    let mut bytes = vec![0; MATHEMATICAL_VALUE_MAX_ENCODED_BYTES];
    let n = value.encode(&mut bytes).unwrap();
    bytes.truncate(n);
    bytes
}
#[test]
#[ignore = "optional component cost capture; not a timing conformance gate"]
fn capture_preparation_and_repeated_interval_back_costs() {
    let requirements = request();
    const CALLS: u32 = 4;
    println!("ENCLOSURE_BACK_COST_PROFILE debug_assertions={} trials=16 calls_per_trial={CALLS} scope=back_preparation_and_evaluate_only", cfg!(debug_assertions));
    for (which, name) in [(0, "third"), (1, "pi_over_180"), (2, "sqrt_two")] {
        let input = specimen(which);
        for trial in 0..16 {
            let start = Instant::now();
            let cpu = cpu_ns();
            let guard = crate::allocation_probe::begin();
            let mut back = MathematicalEnclosureBack::new(requirements).unwrap();
            let prepare_allocations = guard.finish();
            let prepare_cpu_ns = cpu_ns() - cpu;
            let prepare_wall_ns = start.elapsed().as_nanos();
            let retained = back.retained_capacity_bytes();
            back.evaluate(&input).unwrap();
            let expected = semantic_digest(
                "numeric/enclose-cost-result@1",
                back.state.encoding.encoded(),
            );
            let start = Instant::now();
            let cpu = cpu_ns();
            let guard = crate::allocation_probe::begin();
            for _ in 0..CALLS {
                back.evaluate(black_box(&input)).unwrap();
                black_box(back.state.encoding.encoded());
            }
            let warm_allocations = guard.finish();
            let warm_cpu_ns = cpu_ns() - cpu;
            let warm_wall_ns = start.elapsed().as_nanos();
            assert_eq!(warm_allocations, 0);
            assert_eq!(retained, back.retained_capacity_bytes());
            assert_eq!(
                semantic_digest(
                    "numeric/enclose-cost-result@1",
                    back.state.encoding.encoded()
                ),
                expected
            );
            println!("ENCLOSURE_BACK_COST case={name} trial={trial} prepare_cpu_ns={prepare_cpu_ns} prepare_wall_ns={prepare_wall_ns} prepare_allocations={prepare_allocations} warm_cpu_ns={warm_cpu_ns} warm_wall_ns={warm_wall_ns} warm_allocations={warm_allocations} calls={CALLS} retained_payload_bytes={retained}");
        }
    }
}
