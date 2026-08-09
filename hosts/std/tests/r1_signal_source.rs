use conduit_core::{BootId, ConnectionBase, HostId};
use conduit_std_host::pico_usb_source::PicoUsbSource;
use conduit_system_continuity::{exact_r1_signal_plan, R1SignalRouteSet};

#[test]
fn one_production_source_executes_both_exact_r1_recovery_plans() {
    let pico_boot = BootId::from("r1/test-pico-runtime-boot");
    let source_host = HostId::from(conduit_net::R1_STD_HOST_ID);
    let plan_a = exact_r1_signal_plan(pico_boot.clone(), R1SignalRouteSet::WebSocketOnly)
        .expect("WebSocket-only Plan A");
    let plan_b =
        exact_r1_signal_plan(pico_boot, R1SignalRouteSet::UsbOnly).expect("USB-only Plan B");

    let source_a = PicoUsbSource::prepare_plan(plan_a.plan, &source_host)
        .expect("production source prepares Plan A");
    let source_b = PicoUsbSource::prepare_plan(plan_b.plan, &source_host)
        .expect("production source prepares Plan B");

    assert_eq!(
        source_a.binding().attachment.base,
        ConnectionBase::WebSocket
    );
    assert_eq!(source_b.binding().attachment.base, ConnectionBase::UsbCdc);
    assert_eq!(source_a.source_host_id(), source_b.source_host_id());
    assert_eq!(source_a.binding().sink, source_b.binding().sink);
    assert_ne!(source_a.binding().plan_id, source_b.binding().plan_id);
    assert_ne!(
        source_a.binding().source_active_play_id,
        source_b.binding().source_active_play_id
    );
    assert_ne!(
        source_a.binding().sink_active_play_id,
        source_b.binding().sink_active_play_id
    );
}
