//! The owner's deadline wrapper preserves the real admitted timer basis.
use super::*;

#[test]
fn deadline_timer_forwards_preparation_bound_and_exact_basis_sampling() {
    let host = conduit_core::HostId::from("fixture/startup-owner");
    let boot = conduit_core::BootId::from("boot/startup-owner");
    let mut timer = DeadlineTimer {
        until: Instant::now() + Duration::from_secs(1),
        stop_request: "owner/run-deadline",
        control: RunControl::default(),
    };
    assert_eq!(
        timer.monotonic_identity_backing_bound(&host, &boot),
        ThreadTimer.monotonic_identity_backing_bound(&host, &boot)
    );
    let initial = timer.monotonic_observation(&host, &boot).unwrap();
    let bound = timer
        .monotonic_identity_backing_bound(&host, &boot)
        .unwrap();
    assert!(initial.clock().retained_backing_bytes().unwrap() <= bound);
    assert!(timer.monotonic_ticks_on(initial.clock()).unwrap() >= initial.ticks());
    assert!(!timer.control.stop_requested());
}
