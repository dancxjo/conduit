//! Complete configuration exchanges share one admitted, reusable proof kernel.
use super::descriptor_probe_execution::{Descriptor, run_script};

#[test]
fn complete_walk_reuses_slots_across_64_calls_then_preserves_short_and_stalled_outcomes() {
    let mut cases = vec![(Some(25), "frame", Some("configuration")); 64];
    cases.extend([
        (Some(0), "frame", Some("short")),
        (Some(18), "frame", Some("short")),
        (None, "stalled", None),
    ]);
    run_script(Descriptor::Configuration, &cases);
}
