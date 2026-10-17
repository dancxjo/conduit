//! Manual compiler/profile-specific production object extents. This integration
//! test links the ordinary kernel library, without its cfg(test) scan counters.
use conduit_kernel::{
    scheduler::{FixedScheduler, HostCallBack},
    FixedSignLog, FixedValueStore,
};
use std::mem::size_of;

type Kernel<const N: usize> =
    FixedScheduler<HostCallBack, FixedValueStore<N, 1>, FixedSignLog<128>, N, N, 1, N, N, 1, N, N>;

fn observe<const N: usize>() {
    println!(
        "SCHEDULER_PRODUCTION_FOOTPRINT {{\"node_capacity\":{N},\"cord_capacity\":{N},\"pending_request_capacity\":{N},\"payload_width\":1,\"sign_event_capacity\":128,\"scheduler_bytes\":{},\"driver_bytes\":{},\"value_store_bytes\":{},\"sign_log_bytes\":{}}}",
        size_of::<Kernel<N>>(),
        size_of::<HostCallBack>(),
        size_of::<FixedValueStore<N, 1>>(),
        size_of::<FixedSignLog<128>>()
    );
}

#[test]
#[ignore = "manual native production-layout observation; no size threshold"]
fn observe_production_scheduler_footprint() {
    observe::<1>();
    observe::<8>();
    observe::<32>();
}
