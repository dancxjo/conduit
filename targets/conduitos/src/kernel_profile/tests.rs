use super::*;
use conduit_kernel::{KernelEventKind, SignError, SignQuery};

fn reach_timer_request(profile: &mut KernelProfile) -> HostOperationRequest {
    assert!(matches!(
        profile.step().unwrap(),
        SchedulerStatus::Progress { .. }
    ));
    profile.next_host_request().unwrap()
}

#[test]
fn production_kernel_owns_timer_to_serial_progress() {
    let mut profile = KernelProfile::new().unwrap();
    let timer = reach_timer_request(&mut profile);
    let interest = KernelProfile::timer_interest(timer).unwrap();
    profile.complete_timer(interest).unwrap();

    let mut serial = None;
    for _ in 0..8 {
        let status = profile.step().unwrap();
        if let Some(request) = profile.next_host_request() {
            serial = Some(request);
            break;
        }
        assert!(!matches!(status, SchedulerStatus::Complete));
    }
    let serial = serial.expect("serial presentation request");
    assert_eq!(profile.host_value(serial.input.value).unwrap(), TIMER_VALUE);
    profile.complete_serial(serial).unwrap();
    for _ in 0..4 {
        if profile.step().unwrap() == SchedulerStatus::Complete {
            break;
        }
    }
    assert_eq!(profile.pending_host_operations(), 0);
    assert!(
        profile
            .scheduler
            .signs()
            .contains_kind(KernelEventKind::HostOperationCompleted)
    );
    assert!(
        profile
            .scheduler
            .signs()
            .contains_kind(KernelEventKind::OperationCompleted)
    );
}

#[test]
fn cancellation_rejects_late_machine_wake() {
    let mut profile = KernelProfile::new().unwrap();
    let timer = reach_timer_request(&mut profile);
    let interest = KernelProfile::timer_interest(timer).unwrap();
    profile.cancel().unwrap();
    assert_eq!(
        profile.complete_timer(interest),
        Err(SchedulerError::HostOperationCompletionRejected)
    );
    assert_eq!(profile.step(), Ok(SchedulerStatus::Cancelled));
}

#[test]
fn base_failure_remains_failure_and_sign_full_never_becomes_success() {
    let mut profile = KernelProfile::new().unwrap();
    let timer = reach_timer_request(&mut profile);
    let interest = KernelProfile::timer_interest(timer).unwrap();
    profile.fail_timer(interest).unwrap();
    assert!(matches!(
        profile.step(),
        Ok(SchedulerStatus::Progress { node: NodeId(1) })
    ));
    assert_eq!(
        profile.step(),
        Err(SchedulerError::OperationFailed(conduit_kernel::Failure {
            code: conduit_kernel::FailureCode::HostOperationFailed,
            detail: 11
        }))
    );

    let event_bytes = core::mem::size_of::<KernelEvent>() as u32;
    let mut signs = FixedSignLog::<1>::new(event_bytes).unwrap();
    signs
        .record(NodeId(0), None, None, KernelEventKind::Decision)
        .unwrap();
    assert_eq!(
        signs.record(NodeId(0), None, None, KernelEventKind::Decision),
        Err(SignError::ItemCapacityExceeded)
    );
}
