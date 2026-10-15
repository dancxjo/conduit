//! Connected ordinary Plan tests retain their existing semantic assertions.
mod common;
#[path = "common/global_connected_fixture.rs"]
mod global_connected_fixture;
use conduit_core::*;
use conduit_planner::resource_cost::*;
use global_connected_fixture::connected;

#[test]
fn connected_ordinary_plan_adds_scoped_edges_before_join() {
    let result = connected(false, false, false).unwrap();
    assert!(verify_plan(&result.plan));
    assert!(result.includes_transfer);
    assert_eq!(result.score.unknown_operations, 0);
    assert_eq!(result.score.unknown_transfers, 0);
    assert_eq!(
        result.score.critical_path_upper_seconds,
        Some(ExactRational::ratio(14, 1000, &mut ExactNumericWork::new(100_000_000)).unwrap())
    );
}
#[test]
fn missing_local_and_expired_remote_edges_remain_unknown() {
    for (missing_local, stale_remote) in [(true, false), (false, true)] {
        let result = connected(missing_local, stale_remote, false).unwrap();
        assert!(verify_plan(&result.plan));
        assert_eq!(result.score.unknown_operations, 0);
        assert_eq!(result.score.unknown_transfers, 1);
        assert_eq!(result.score.critical_path_upper_seconds, None);
    }
}
#[test]
fn operation_transfer_is_not_counted_twice() {
    assert!(matches!(
        connected(false, false, true),
        Err(GlobalPlacementRefusal::NoFeasibleAssignment { .. })
    ));
}
