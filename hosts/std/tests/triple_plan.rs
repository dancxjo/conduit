use conduit_plan_lowering::lowering::{lower_plan_fragment, RemoteCordDirection};
use conduit_signal_conformance::triple;

#[test]
fn source_fragment_has_one_local_and_two_remote_atomic_fanout_branches() {
    let exact = triple::exact_plan().expect("triple plan resolves");
    let fragment = exact
        .plan
        .fragments
        .iter()
        .find(|fragment| fragment.host_id == exact.source_advertisement.host_id)
        .expect("source fragment");
    let lowered = lower_plan_fragment(fragment).expect("source lowers");
    assert_eq!(lowered.nodes.len(), 2);
    assert_eq!(lowered.cords.len(), 3);
    assert_eq!(lowered.routes.len(), 1);
    assert_eq!(lowered.routes[0].targets.len(), 3);
    assert_eq!(lowered.remote_endpoints.len(), 2);
    assert!(lowered
        .remote_endpoints
        .iter()
        .all(|remote| remote.direction == RemoteCordDirection::Egress));
    assert_eq!(lowered.host_operations.len(), 2);
    assert_eq!(lowered.cord_value_slots, 3);
}
