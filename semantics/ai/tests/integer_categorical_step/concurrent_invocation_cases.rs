//! Actual concurrent lease claims; fixed barriers/channels, never sleeps.
use super::*;
use conduit_ai::integer_categorical_step::invocation::*;
use std::sync::{mpsc, Barrier};
use std::thread;

#[test]
fn concurrent_readers_have_one_winner_and_held_bytes_survive_pressure_then_reuse() {
    let profile = profile();
    let plan = plan(&profile, true, true).unwrap();
    let (factory, mut outlets) =
        owner::CategoricalOperationFactory::for_plan_with_invocation_outlets(
            &plan,
            &[profile.clone()],
            1,
        )
        .unwrap();
    let outlet = outlets.pop().unwrap();
    let mut values = HostedValueStore::new(8, 4096, 32768).unwrap();
    let mut back = factory.prepare(gear(&plan), &mut values).unwrap();
    // More cycles than capacity; each cycle has different canonical input/output.
    for number in 1..=8 {
        let bytes = indices(&profile, if number % 2 == 0 { &[0, 2] } else { &[1, 1] });
        let (mut io, input) = super::invocation_cases::frame(&bytes);
        assert_eq!(back.step(&mut io, &input), StepOutcome::Progress);
        let expected = back.prepared_output(KPort(0)).unwrap().to_vec();
        back.step_committed();
        let start = Arc::new(Barrier::new(3));
        let attempted = Arc::new(Barrier::new(3));
        let (sender, receiver) = mpsc::channel();
        thread::scope(|scope| {
            for _ in 0..2 {
                let reader = outlet.clone();
                let start = start.clone();
                let attempted = attempted.clone();
                let sender = sender.clone();
                scope.spawn(move || {
                    start.wait();
                    let lease = reader.try_take();
                    attempted.wait();
                    sender.send(lease).unwrap();
                });
            }
            start.wait();
            attempted.wait();
            let first = receiver.recv().unwrap();
            let second = receiver.recv().unwrap();
            assert_eq!(
                usize::from(first.is_some()) + usize::from(second.is_some()),
                1
            );
            let lease = Arc::new(first.or(second).unwrap());
            assert_eq!(lease.invocation(), number);
            assert!(outlet.try_take().is_none());
            let (mut pressure, input) = super::invocation_cases::frame(&bytes);
            assert!(matches!(
                back.step(&mut pressure, &input),
                StepOutcome::Fail(conduit_kernel::Failure {
                    code: conduit_kernel::FailureCode::StorageExhausted,
                    ..
                })
            ));
            assert!(!pressure.test_consumed(KPort(0)));
            // Exercise Sync with simultaneous copies of the SAME held lease.
            let copies = Arc::new(Barrier::new(3));
            let mut joins = Vec::new();
            for _ in 0..2 {
                let lease = lease.clone();
                let copies = copies.clone();
                joins.push(scope.spawn(move || {
                    copies.wait();
                    let mut input = vec![0; lease.input_len()];
                    let mut output = vec![0; lease.output_len()];
                    lease.copy_input_into(&mut input).unwrap();
                    lease.copy_output_into(&mut output).unwrap();
                    (lease.invocation(), input, output)
                }));
            }
            copies.wait();
            for join in joins {
                let (observed, input, output) = join.join().unwrap();
                assert_eq!(observed, number);
                assert_eq!(input, bytes);
                assert_eq!(output, expected);
            }
            // Final Arc drop releases the slot before the next outer iteration.
            drop(lease);
        });
    }
}
