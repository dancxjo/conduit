//! Partial output capacity is ordinary pressure, before any staged consumption.
#![cfg(feature = "native-kernel")]
use conduit_kernel::{self as k, scheduler::*};
use conduit_seven_segment_mask::native_kernel::MaskBack;

#[test]
fn tee_keeps_input_until_the_complete_value_fits() {
    let value = k::ValueRef {
        slot: 0,
        generation: 1,
        byte_len: 96,
    };
    let mut back = MaskBack::Tee;
    let bytes = StepInputBytes::test_frame([None, None], None);
    for maximum in [None, Some(64)] {
        let mut io = StepIo::test_frame([Some(value), None], [false; 2], [maximum, None], None, 8);
        assert_eq!(back.step(&mut io, &bytes), StepOutcome::Await);
        assert!(!io.test_consumed(k::PortId(0)));
        assert_eq!(io.test_output(k::PortId(0)), None);
        assert_eq!(io.test_fault(), None);
    }
    let mut io = StepIo::test_frame([Some(value), None], [false; 2], [Some(96), None], None, 8);
    assert_eq!(back.step(&mut io, &bytes), StepOutcome::Progress);
    assert!(io.test_consumed(k::PortId(0)));
    assert_eq!(io.test_output(k::PortId(0)), Some(value));
}

#[test]
fn renderer_keeps_completion_and_generation_until_the_complete_show_fits() {
    let value = k::ValueRef {
        slot: 0,
        generation: 1,
        byte_len: 96,
    };
    let completion = Some((
        k::RequestId(4),
        k::HostCallOutcome {
            disposition: k::HostCallDisposition::Completed,
            output: Some(k::BoundedValueRef::new(value, 96).unwrap()),
            failure: None,
        },
    ));
    let mut back = MaskBack::Renderer {
        state: 1,
        maximum_input_bytes: 96,
        next_request: 4,
    };
    let bytes = StepInputBytes::test_frame([None, None], None);
    for maximum in [None, Some(64)] {
        let mut io = StepIo::test_frame([None; 2], [false; 2], [maximum, None], completion, 8);
        assert_eq!(back.step(&mut io, &bytes), StepOutcome::Await);
        assert!(!io.test_host_completion_consumed());
        assert_eq!(io.test_output(k::PortId(0)), None);
        assert_eq!(io.test_fault(), None);
        assert!(matches!(
            back,
            MaskBack::Renderer {
                state: 1,
                next_request: 4,
                ..
            }
        ));
    }
    let mut io = StepIo::test_frame([None; 2], [false; 2], [Some(96), None], completion, 8);
    assert_eq!(back.step(&mut io, &bytes), StepOutcome::Progress);
    assert!(io.test_host_completion_consumed());
    assert_eq!(io.test_output(k::PortId(0)), Some(value));
    assert!(matches!(
        back,
        MaskBack::Renderer {
            state: 2,
            next_request: 5,
            ..
        }
    ));
}
