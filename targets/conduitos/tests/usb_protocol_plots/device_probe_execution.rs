//! Device Source retains the existing descriptor proof contract.
use super::descriptor_probe_execution::{Descriptor, run};

#[test]
fn checked_probe_constructs_request_decodes_reply_and_closes_in_one_kernel() {
    run(Descriptor::Device, Some(18), "frame", Some("device"), 1);
}

#[test]
fn short_or_stalled_transfer_remains_observed_and_never_decodes_or_retries() {
    run(Descriptor::Device, Some(0), "short", None, 1);
    run(Descriptor::Device, Some(17), "short", None, 1);
    run(Descriptor::Device, None, "stalled", None, 1);
}

#[test]
fn descriptor_exchange_reuses_value_slots_across_64_calls() {
    run(Descriptor::Device, Some(18), "frame", Some("device"), 64);
}
