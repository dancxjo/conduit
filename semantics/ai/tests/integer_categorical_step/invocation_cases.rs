//! Uses the unchanged categorical model/Plan fixture; acknowledgements here are
//! diagnostic StepBack protocol proof, not installed Plan/Play acceptance.
use super::*;
use conduit_ai::integer_categorical_step::invocation::*;

fn frame(bytes: &[u8]) -> (StepIo<PORTS>, StepInputBytes<'_, PORTS>) {
    let reference = ValueRef {
        slot: 0,
        generation: 1,
        byte_len: bytes.len() as u32,
    };
    (
        StepIo::test_frame(
            core::array::from_fn(|i| (i == 0).then_some(reference)),
            [false; PORTS],
            core::array::from_fn(|i| (i == 0).then_some(4096)),
            None,
            2,
        ),
        StepInputBytes::test_frame(core::array::from_fn(|i| (i == 0).then_some(bytes)), None),
    )
}

#[test]
fn opaque_outlet_publishes_only_after_ack_and_holds_exact_bytes_under_pressure() {
    fn send_sync<T: Send + Sync>() {}
    send_sync::<CategoricalInvocationOutlet>();
    send_sync::<CommittedCategoricalInvocation>();
    let profile = profile();
    let plan = plan(&profile, true, true).unwrap();
    let (factory, mut readers) =
        owner::CategoricalOperationFactory::for_plan_with_invocation_outlets(
            &plan,
            &[profile.clone()],
            1,
        )
        .unwrap();
    let outlet = readers.pop().unwrap();
    assert!(readers.is_empty());
    assert_eq!(outlet.capacity(), 1);
    let mut values = HostedValueStore::new(8, 4096, 32768).unwrap();
    let mut back = factory.prepare(gear(&plan), &mut values).unwrap();
    // An observed outlet is never rebound to another instance / Play.
    assert!(factory.prepare(gear(&plan), &mut values).is_err());
    let bytes = indices(&profile, &[1, 1]);
    let (mut io, input) = frame(&bytes);
    let (outcome, allocations) = allocation_probe::observe(|| back.step(&mut io, &input));
    assert_eq!(outcome, StepOutcome::Progress);
    assert_eq!(allocations.allocations, 0);
    assert_eq!(allocations.reallocations, 0);
    assert!(outlet.try_take().is_none());
    let output = back.prepared_output(KPort(0)).unwrap().to_vec();
    let (_, allocations) = allocation_probe::observe(|| back.step_committed());
    assert_eq!(allocations.allocations, 0);
    let receipt = outlet.try_take().unwrap();
    assert!(outlet.try_take().is_none());
    assert!(outlet.contains(&receipt));
    assert!(outlet.clone().contains(&receipt));
    let (_, foreign_readers) =
        owner::CategoricalOperationFactory::for_plan_with_invocation_outlets(
            &plan,
            &[profile.clone()],
            1,
        )
        .unwrap();
    // Identical Plan/profile/invocation origin does not confer outlet membership.
    assert!(!foreign_readers[0].contains(&receipt));
    assert_eq!(receipt.invocation(), 1);
    assert_eq!(receipt.placement(), gear(&plan));
    assert_eq!(receipt.plan(), &plan);
    assert!(receipt.flow());
    assert_eq!(
        receipt.profile().resource().artifact(),
        profile.resource().artifact()
    );
    let mut retained_input = vec![0; receipt.input_len()];
    let mut retained_output = vec![0; receipt.output_len()];
    receipt.copy_input_into(&mut retained_input).unwrap();
    receipt.copy_output_into(&mut retained_output).unwrap();
    assert_eq!(retained_input, bytes);
    assert_eq!(retained_output, output);
    assert_eq!(scores(&retained_output), [4, -4]);
    assert!(receipt.copy_input_into(&mut []).is_err());
    assert_eq!(
        outlet.retained_payload_bytes(),
        bytes.len() + output.len() + 8
    );
    let (mut pressure, input) = frame(&bytes);
    assert_eq!(
        back.step(&mut pressure, &input),
        StepOutcome::Fail(conduit_kernel::Failure {
            code: conduit_kernel::FailureCode::StorageExhausted,
            detail: 3106,
        })
    );
    assert!(back.prepared_output(KPort(0)).is_none());
    // A held receipt is immutable even after a refused subsequent invocation.
    receipt.copy_output_into(&mut retained_output).unwrap();
    assert_eq!(retained_output, output);
    drop(receipt);
    let (mut io, input) = frame(&bytes);
    assert_eq!(back.step(&mut io, &input), StepOutcome::Progress);
    back.step_committed();
    assert_eq!(outlet.try_take().unwrap().invocation(), 2);
}

#[test]
fn cancellation_and_bad_input_never_publish_and_capacity_is_admitted_before_use() {
    let profile = profile();
    let plan = plan(&profile, true, true).unwrap();
    for capacity in [0, MAX_INVOCATION_SLOTS + 1] {
        assert!(
            owner::CategoricalOperationFactory::for_plan_with_invocation_outlets(
                &plan,
                &[profile.clone()],
                capacity
            )
            .is_err()
        );
    }
    let (factory, readers) = owner::CategoricalOperationFactory::for_plan_with_invocation_outlets(
        &plan,
        &[profile.clone()],
        1,
    )
    .unwrap();
    let mut values = HostedValueStore::new(8, 4096, 32768).unwrap();
    let mut wrong = gear(&plan).clone();
    wrong.kind_contract_revision = "foreign".into();
    assert!(factory.prepare(&wrong, &mut values).is_err());
    // Failed descriptor admission did not consume exclusive writer custody.
    let mut back = factory.prepare(gear(&plan), &mut values).unwrap();
    let malformed = indices(&profile, &[0, 3]);
    let (mut io, input) = frame(&malformed);
    assert!(matches!(back.step(&mut io, &input), StepOutcome::Fail(_)));
    back.step_committed();
    assert!(readers[0].try_take().is_none());
    let bytes = indices(&profile, &[0, 2]);
    let (mut io, input) = frame(&bytes);
    assert_eq!(back.step(&mut io, &input), StepOutcome::Progress);
    back.cancel();
    back.step_committed();
    assert!(readers[0].try_take().is_none());
}
