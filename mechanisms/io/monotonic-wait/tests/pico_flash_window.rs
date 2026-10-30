//! Qualify the actual Pico adapter's precise clock policy alongside the shared
//! millisecond gate. No hardware, timing or storage grant is synthesized here.
#[path = "../../../../targets/rp2040/proof/environment-usb/src/credential_store/policy.rs"]
mod policy;
#[test]
fn precise_adapter_clock_cannot_inherit_rounded_slack_from_gate() {
    use conduit_monotonic_wait::flash_window::Gate;
    let gate = Gate::new(2000, 100, 750).unwrap();
    let wait = gate.begin_wait(0, 750).unwrap();
    let _permit = wait.try_flash(0, 750).unwrap();
    assert!(!policy::can_start(1, 750_000, 750_000));
    assert!(!policy::completed(0, 800_000, 2_000_000, 6_000_000));
}
