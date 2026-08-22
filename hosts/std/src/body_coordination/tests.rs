use super::{run_in_process, CoordinationEndpoint};
use conduit_core::BootId;
use conduit_std_catalog::{exact_body_coordination_plan, FOREBRAIN_HOST, MOTHERBRAIN_HOST};

#[test]
fn two_production_kernel_fragments_exchange_message_and_reply() {
    let exact = exact_body_coordination_plan(
        BootId::from("forebrain/boot-1"),
        BootId::from("motherbrain/boot-1"),
        "wifi/interbrain-1",
    )
    .unwrap();
    let mut forebrain =
        CoordinationEndpoint::prepare(&exact, &conduit_core::HostId::from(FOREBRAIN_HOST)).unwrap();
    let mut motherbrain =
        CoordinationEndpoint::prepare(&exact, &conduit_core::HostId::from(MOTHERBRAIN_HOST))
            .unwrap();
    run_in_process(&mut forebrain, &mut motherbrain).unwrap();
    assert_eq!(
        motherbrain.received(),
        "coordinate issue 1633; evidence follows"
    );
    assert_eq!(
        forebrain.received(),
        "received issue 1633; evidence acknowledged"
    );
    assert_ne!(
        forebrain.fragment().fragment_id,
        motherbrain.fragment().fragment_id
    );
}
