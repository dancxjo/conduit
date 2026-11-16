//! Shared canonical input for scalar admission and whole-workset tests.
use crate::kernel_multivalue::{advertisement, plan_local, profile_catalog};
use conduit_core::{HostAdvertisement, OfferGeneration, Plan};
use conduit_host_make::HostBounds;
pub(super) const SOURCE: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../proof/fixtures/plots/kernel-multivalue.conduit"
));

pub(super) fn bounds() -> HostBounds {
    HostBounds {
        static_memory_bytes: 16 * 1024 * 1024,
        heap_arena_bytes: 64 * 1024 * 1024,
        queue_items: 4096,
        buffered_bytes: 16 * 1024 * 1024,
        active_instances: 512,
        operation_slots: 256,
        timer_slots: 128,
        line_sessions: 64,
        evidence_items: 4096,
    }
}
pub(super) fn fixture() -> (HostAdvertisement, Plan) {
    let host = advertisement(
        "image-budget-host".into(),
        "image-budget-boot".into(),
        OfferGeneration(1),
    );
    let plot = conduit_plot::parse(SOURCE, &profile_catalog()).unwrap();
    let plan = plan_local(&plot, &host).unwrap();
    (host, plan)
}
