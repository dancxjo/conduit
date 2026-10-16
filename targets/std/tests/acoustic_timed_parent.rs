//! Real parent Flow retains exact DSP state across delivery schedules.
#[path = "common_acoustic_timed/assertions.rs"]
mod assertions;
#[path = "common_acoustic_timed/parent_kernel.rs"]
mod parent_kernel;
#[path = "common_acoustic_timed/parent_plan.rs"]
mod parent_plan;
#[test]
fn timed_parent_retains_states_under_delayed_delivery_and_pressure() {
    std::thread::Builder::new()
        .stack_size(8 * 1024 * 1024)
        .spawn(|| {
            let immediate = check_scan(0, 1);
            let pressured = check_scan(7, 4);
            assert_eq!(immediate, pressured);
        })
        .unwrap()
        .join()
        .unwrap();
}
fn check_scan(gap: usize, hold: usize) -> Vec<Vec<u8>> {
    let prepared = parent_plan::prepare();
    let states = parent_kernel::run(
        prepared.plan,
        &prepared.front,
        prepared.registry,
        &prepared.items,
        gap,
        hold,
    );
    assertions::check_states(&states);
    states
}
