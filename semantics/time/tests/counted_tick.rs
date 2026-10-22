#![cfg(feature = "kernel-step")]
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    HostCallDisposition, HostCallOutcome, RequestId, ValueRef,
};
use conduit_time::TickBack;
fn wait() -> ValueRef {
    ValueRef {
        slot: 0,
        generation: 1,
        byte_len: 8,
    }
}
#[test]
fn finite_ticks_reuse_one_wait_close_exactly_and_honor_pressure() {
    for count in [0, 1, 4, 960] {
        let mut back = TickBack::counted(wait(), count).unwrap();
        assert_eq!(back.allocation_capacity(), 1);
        let mut io = StepIo::test_frame([None], [false], [Some(8)], None, 8);
        assert_eq!(
            back.step(&mut io, &StepInputBytes::test_frame([None], None)),
            if count == 0 {
                StepOutcome::Complete
            } else {
                StepOutcome::Progress
            }
        );
        for sequence in 0..count {
            let completion = Some((
                RequestId(sequence as u32),
                HostCallOutcome {
                    disposition: HostCallDisposition::Completed,
                    output: None,
                    failure: None,
                },
            ));
            let mut pressured = StepIo::test_frame([None], [false], [None], completion, 8);
            assert_eq!(
                back.step(&mut pressured, &StepInputBytes::test_frame([None], None)),
                StepOutcome::Await
            );
            let mut io = StepIo::test_frame([None], [false], [Some(8)], completion, 8);
            assert_eq!(
                back.step(&mut io, &StepInputBytes::test_frame([None], None)),
                if sequence + 1 == count {
                    StepOutcome::Complete
                } else {
                    StepOutcome::Progress
                }
            );
            let output = io.test_canonical_output().unwrap();
            assert_eq!(output.0, conduit_kernel::PortId(0));
            assert_eq!(output.1.as_slice(), &sequence.to_le_bytes());
            assert_eq!(back.allocation_capacity(), 1);
        }
    }
    assert!(TickBack::counted(wait(), 4097).is_none());
}
