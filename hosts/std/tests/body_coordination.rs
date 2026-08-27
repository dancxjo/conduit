use conduit_core::{BootId, HostId};
use conduit_std_catalog::{exact_body_coordination_plan, FOREBRAIN_HOST, MOTHERBRAIN_HOST};
use conduit_std_host::body_coordination::CoordinationEndpoint;

#[test]
fn stale_motherbrain_boot_refuses_before_kernel_delivery() {
    let exact = exact_body_coordination_plan(
        BootId::from("forebrain/boot-1"),
        BootId::from("motherbrain/boot-1"),
        "wifi/interbrain-1",
    )
    .unwrap();
    let forebrain = CoordinationEndpoint::prepare(&exact, &HostId::from(FOREBRAIN_HOST)).unwrap();
    let stale = exact_body_coordination_plan(
        BootId::from("forebrain/boot-1"),
        BootId::from("motherbrain/boot-stale"),
        "wifi/interbrain-1",
    )
    .unwrap();
    let motherbrain =
        CoordinationEndpoint::prepare(&stale, &HostId::from(MOTHERBRAIN_HOST)).unwrap();
    assert_ne!(
        forebrain.binding(conduit_plan_lowering::lowering::RemoteCordDirection::Egress),
        motherbrain.binding(conduit_plan_lowering::lowering::RemoteCordDirection::Ingress)
    );
}
