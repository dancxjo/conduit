//! Local transfer observations cannot stand in for replacement-Boot evidence.
#[allow(dead_code, unused_imports)]
#[path = "../examples/resource_replacement_cost/fixture.rs"]
mod actual_cases;
use conduit_core::ExactNumericWork;
use conduit_planner::resource_cost::{ReplacementCostFacts, ReplacementCostRefusal};
#[test]
fn independently_planned_boots_require_another_transfer_profile() {
    let old = actual_cases::Cases::new().unwrap();
    let new = actual_cases::Cases::new().unwrap();
    assert_eq!(old.previous.checked_plot_id, new.previous.checked_plot_id);
    assert_ne!(old.host.boot_id, new.host.boot_id);
    assert!(matches!(
        ReplacementCostFacts::admit(
            &old.previous,
            &old.previous.fragments[0].placements[0],
            &new.previous,
            &new.previous.fragments[0].placements[0],
            [None; 5],
            &[],
            &[],
            &mut ExactNumericWork::new(100_000_000),
        ),
        Err(ReplacementCostRefusal::UnsupportedTransfer)
    ));
}
