//! Full configuration Source runs through the same admitted kernel/carrier.
use super::descriptor_probe_execution::{Descriptor, run};

#[test]
fn configuration_request_and_complete_walk_close_through_production_host_calls() {
    run(
        Descriptor::Configuration,
        Some(25),
        "frame",
        Some("configuration"),
        1,
    );
}

#[test]
fn short_configuration_and_stalled_transfer_preserve_distinct_outcomes() {
    run(
        Descriptor::Configuration,
        Some(0),
        "frame",
        Some("short"),
        1,
    );
    run(
        Descriptor::Configuration,
        Some(18),
        "frame",
        Some("short"),
        1,
    );
    run(Descriptor::Configuration, None, "stalled", None, 1);
}

#[test]
fn full_configuration_exchange_reuses_value_slots_across_64_calls() {
    run(
        Descriptor::Configuration,
        Some(25),
        "frame",
        Some("configuration"),
        64,
    );
}
