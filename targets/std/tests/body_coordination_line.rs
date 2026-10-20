//! Actual Body coordination Lines retain kernel admission and terminal semantics.
#[path = "../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
use conduit_core::{BootId, HostId};
use conduit_semantic_catalog::{
    exact_body_coordination_plan, BODY_COORDINATION_MAXIMUM_FRAME_BYTES, FOREBRAIN_HOST,
    MOTHERBRAIN_HOST,
};
use conduit_std_host::{
    body_coordination::{run_forebrain, run_motherbrain, CoordinationEndpoint},
    websocket::NativeWebSocketListener,
};

fn exchange(stale: bool) -> (CoordinationEndpoint, CoordinationEndpoint, bool, bool) {
    let exact = exact_body_coordination_plan(
        BootId::from("forebrain/line-test"),
        BootId::from("motherbrain/line-test"),
        "wifi/body-line-test",
    )
    .unwrap();
    let sink_exact = if stale {
        exact_body_coordination_plan(
            BootId::from("forebrain/line-test"),
            BootId::from("motherbrain/stale"),
            "wifi/body-line-test",
        )
        .unwrap()
    } else {
        exact.clone()
    };
    let mut forebrain =
        CoordinationEndpoint::prepare(&exact, &HostId::from(FOREBRAIN_HOST)).unwrap();
    let mut motherbrain =
        CoordinationEndpoint::prepare(&sink_exact, &HostId::from(MOTHERBRAIN_HOST)).unwrap();
    let outward =
        NativeWebSocketListener::bind_loopback(BODY_COORDINATION_MAXIMUM_FRAME_BYTES).unwrap();
    let returned =
        NativeWebSocketListener::bind_loopback(BODY_COORDINATION_MAXIMUM_FRAME_BYTES).unwrap();
    let outward_address = outward.local_addr().unwrap();
    let return_address = returned.local_addr().unwrap();
    let peer = std::thread::spawn(move || {
        let (result, allocation) = allocation_probe::observe(|| {
            run_motherbrain(&mut motherbrain, returned, outward_address)
        });
        (motherbrain, result, allocation)
    });
    let (result, allocation) =
        allocation_probe::observe(|| run_forebrain(&mut forebrain, outward, return_address));
    let (motherbrain, peer_result, peer_allocation) = peer.join().unwrap();
    println!(
        "body-line-allocation stale={stale} forebrain={}/{} motherbrain={}/{}",
        allocation.allocations,
        allocation.reallocations,
        peer_allocation.allocations,
        peer_allocation.reallocations
    );
    (forebrain, motherbrain, result.is_ok(), peer_result.is_ok())
}

#[test]
fn exact_body_round_trip_over_two_real_websocket_lines() {
    for _ in 0..3 {
        let (forebrain, motherbrain, source_ok, sink_ok) = exchange(false);
        assert!(source_ok && sink_ok);
        assert_eq!(
            forebrain.received(),
            "received issue 1633; evidence acknowledged"
        );
        assert_eq!(
            motherbrain.received(),
            "coordinate issue 1633; evidence follows"
        );
        assert_eq!(forebrain.fragment().plan_id, motherbrain.fragment().plan_id);
        assert_ne!(
            forebrain.fragment().fragment_id,
            motherbrain.fragment().fragment_id
        );
    }
}

#[test]
fn stale_boot_is_refused_before_either_body_presents_a_value() {
    let (forebrain, motherbrain, source_ok, sink_ok) = exchange(true);
    assert!(!source_ok && !sink_ok);
    assert!(forebrain.received().is_empty());
    assert!(motherbrain.received().is_empty());
}
