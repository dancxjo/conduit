//! Measured predicate cost and the unchanged direct exact-integer path.
use super::*;
use conduit_plot::{
    PortableExpressionOperation as Op, PreparedPortableExpressionEvaluator as Evaluator,
};

#[test]
fn prepared_validation_and_refusal_keep_capacity_and_report_component_cost() {
    let source = "plot accept (\n >> value: Frequency near 0.01Hz\n result: Frequency near 0.1Hz >>\n) = .\n";
    let (_, plan) = prepared_fore(source, None).unwrap();
    let qualified = crate::installed_std::pure_expression_back::program_from_placement(
        &plan.fragments[0].placements[0],
    )
    .unwrap();
    let mut plain = qualified.clone();
    plain.root = match plain.root.operation {
        Op::ValueContract { value, .. } => *value,
        _ => panic!("qualified program"),
    };
    let packet = projected_packet("Hz");
    for (label, program) in [
        ("structural-identity", &plain),
        ("qualified-identity", &qualified),
    ] {
        for trial in 0..8 {
            let start = std::time::Instant::now();
            let allocations = crate::allocation_probe::begin();
            let mut evaluator = Evaluator::new(program).unwrap();
            let prep_allocations = allocations.finish();
            let preparation = start.elapsed();
            let capacity = evaluator.output_capacity();
            let start = std::time::Instant::now();
            let allocations = crate::allocation_probe::begin();
            for _ in 0..16 {
                assert_eq!(
                    evaluator.evaluate(std::hint::black_box(&packet)).unwrap(),
                    packet
                );
            }
            let hot_allocations = allocations.finish();
            let warm = start.elapsed();
            assert_eq!(hot_allocations, 0);
            assert_eq!(evaluator.output_capacity(), capacity);
            println!("FORE_COST profile={label} trial={trial} preparation_wall_ns={} preparation_allocations={prep_allocations} calls=16 warm_wall_ns={} hot_allocations={hot_allocations}", preparation.as_nanos(), warm.as_nanos());
        }
    }
    let tight = source.replace("0.1Hz", "0.000000001Hz");
    let (_, plan) = prepared_fore(&tight, None).unwrap();
    let program = crate::installed_std::pure_expression_back::program_from_placement(
        &plan.fragments[0].placements[0],
    )
    .unwrap();
    let mut evaluator = Evaluator::new(&program).unwrap();
    let capacity = evaluator.output_capacity();
    let allocations = crate::allocation_probe::begin();
    for _ in 0..16 {
        assert_eq!(
            evaluator.evaluate(&packet),
            Err(
                conduit_plot::PortableExpressionEvaluationRefusal::OutputValueConstraint(
                    ValueConstraintRefusal::NearQuantity(
                        NearQuantityEvidenceRefusal::InsufficientTolerance
                    )
                )
            )
        );
    }
    assert_eq!(allocations.finish(), 0);
    assert_eq!(evaluator.output_capacity(), capacity);
    // Retain exact canonical fixtures for an independently optimized component
    // capture. This does not turn the unvalidated identity into an equivalent
    // certificate-checking realization.
    println!("FORE_COST_QUALIFIED={}", qualified.canonical_hex().unwrap());
    println!("FORE_COST_PLAIN={}", plain.canonical_hex().unwrap());
    println!(
        "FORE_COST_PACKET={}",
        packet
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    );
}

#[test]
fn bounded_exact_integer_still_selects_tape_and_plays_without_allocation() {
    let source = "plot accept (\n >> value: U64\n result: U64 >>\n) = (. + 1)\n";
    let (mut host, plan) = prepared_fore(source, None).unwrap();
    assert_eq!(
        plan.fragments[0].placements[0]
            .execution_profile_id
            .as_str(),
        conduit_std_offers::PURE_SCALAR_TAPE_STD_PROFILE
    );
    let mut outputs = Outputs(Vec::with_capacity(1));
    let report = host
        .run_external_plot_to(
            plan.fragments[0].clone(),
            &[ExternalForeInput {
                front_port_id: "value".into(),
                track: ConnectionTrack::Payload,
                bytes: 1_u64.to_le_bytes().to_vec(),
            }],
            &mut outputs,
            &mut Vec::new(),
            &mut NoTimer,
        )
        .unwrap();
    assert_eq!(outputs.0, vec![2_u64.to_le_bytes().to_vec()]);
    let kernel = report.kernel.as_ref().unwrap();
    assert_eq!(kernel.post_play_start_allocations, 0);
    assert_eq!(
        kernel.value_allocation_capacity_before,
        kernel.value_allocation_capacity_after
    );
}
