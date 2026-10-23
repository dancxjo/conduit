//! Strict negative controls; synthetic bytes never claim native execution.
use super::*;
pub(super) fn fixture() -> String {
    let identity = json!({"host":"host","boot":"boot","offer_generation":1,"base":"display",
        "width":640,"height":480,"pitch":2560,"bits_per_pixel":32,"red_shift":16,"green_shift":8,"blue_shift":0,
        "make_profile":"profile","make_build":"build","make_image_binding":"image","guest_image_sha256":vec![7_u8;32]});
    let mut text = String::new();
    for (index, phase) in PHASES.into_iter().enumerate() {
        let entries = [0, 1, 10, 15, 15, 15, 1, 10][index];
        let mut cost = json!({});
        for field in [
            "entries",
            "interrupt_entries",
            "source_timer_interrupts",
            "privilege_transitions",
            "gate_transitions",
            "copied_bytes",
            "setup_copied_bytes",
            "base_gate_transitions",
            "tlb_flushes",
            "setup_ticks",
            "teardown_ticks",
            "teardown_zeroed_bytes",
            "shared_peak_bytes",
            "root_metadata_bytes",
            "address_space_switches",
            "scheduler_returns",
            "preemptions",
        ] {
            cost[field] = json!(0);
        }
        cost["entries"] = json!(entries);
        cost["reserved_bytes"] = json!(19349504);
        cost["teardown_zeroed_bytes"] = json!(if matches!(index, 4 | 5) { 19349504 } else { 0 });
        let instance = if index < 6 { 1 } else { 2 };
        let ack = if matches!(index, 2 | 7) {
            text.push_str(&format!("CONDUIT_COMPOSITOR_SERVICE_FRAME service_instance={instance} namespace=1 frame_sequence=1 guest_sequence=1 pixels_written=1 damage_count=1 entries={entries} privilege_transitions=0 address_space_switches=0 scheduler_returns=0\n"));
            json!({"instance":instance,"namespace":1,"frame_sequence":1,"guest_sequence":1})
        } else {
            Value::Null
        };
        let record = json!({"schema":"conduit.conduitos/compositor-stopped-lifecycle@1","part":index,"phase":phase,
            "identity":identity,"instance":instance,"namespace":if ack.is_null() {Value::Null} else {json!(1)},
            "ack":ack,"cost":cost,"raw_ticks":if index==3 {Value::Null} else {json!(1)},
            "staging_live_bytes":if matches!(index,4|5) {0} else {8388608},"staging_prepared_bytes":8388608});
        text.push_str(&format!("{PREFIX}{record}\n"));
    }
    let counts = [
        4096_u64, 4096, 4096, 49152, 2097152, 4096, 65536, 65536, 262144, 8388608, 8388608, 16384,
    ];
    let names = [
        "pml4", "pdpt", "pd", "pt", "image", "exchange", "output", "stack", "metadata", "pixels",
        "scratch", "trap",
    ];
    let regions = names.into_iter().zip(counts).map(|(name,count)|json!({"name":name,"expected_bytes":count,"checked_bytes":count,"all_zero":true})).collect::<Vec<_>>();
    let zero = json!({"schema":"conduit.conduitos/compositor-stopped-zero-witness@1","identity":identity,"instance":1,"namespace":1,"release_ordinal":7,"profile":"compositor",
        "instance_label_source":"root-exclusive-disposal-window","private_observed_at":"before-lease-release","staging_observed_at":"after-volatile-wipe-before-free",
        "backend_teardown_ticks_include_scan":false,"service_disposal_ticks_include_scan":true,"owner_disposal_ticks_include_scan":true,"observer_storage_bytes":400,
        "private":{"instance":1,"release_ordinal":7,"storage_bytes":19349504,"checked_bytes":19349504,"counts_valid":true,"all_zero":true,"regions":regions},
        "staging":{"instance":1,"release_ordinal":7,"initialized_words":2097152,"checked_words":2097152,"capacity_words":2097152,"nonzero_words":0}});
    text.push_str(&format!("{}{zero}\n", zero_receipt::PREFIX));
    text.push_str(&format!("{RESULT}{}\n",json!({"schema":"conduit.conduitos/compositor-stopped-lifecycle-result@2","status":"completed",
        "stopped_owner_cancellation":true,"unfinished_upload_fenced":true,"staging_released":true,"old_clients_refused":true,
        "stale_drop_preserves_replacement":true,"same_retained_grant":true,"fresh_semantic_show_and_progress":true,
        "whole_zero_audited":true,"asynchronous_cancel":false,"guest_cpu_fault":false,"malformed_reply":false,"physical_scanout":false})));
    text
}
fn check(serial: &str) -> bool {
    validate(serial, "profile", "build", "image", [7; 32]).is_ok()
}
#[test]
fn requires_exact_complete_records_and_independent_image_binding() {
    let good = fixture();
    assert!(check(&good));
    assert!(!check(good.trim_end()));
    assert!(!check(&format!("{good}{good}")));
    assert!(!check(&good.replace("\"part\":3", "\"part\":2")));
    assert!(!check(&good.replace(
        "\"whole_zero_audited\":true",
        "\"whole_zero_audited\":false"
    )));
    assert!(!check(&good.replace("\"raw_ticks\":null,", "")));
    assert!(validate(&good, "profile", "other-build", "image", [7; 32]).is_err());
    assert!(validate(&good, "profile", "build", "image", [8; 32]).is_err());
}
#[test]
fn ack_stale_generation_and_partial_disposal_are_not_success() {
    let good = fixture();
    assert!(!check(
        &good.replace("service_instance=2", "service_instance=3")
    ));
    assert!(!check(&good.replace(
        "\"teardown_zeroed_bytes\":19349504",
        "\"teardown_zeroed_bytes\":1"
    )));
    assert!(!check(&good.replace(
        "\"staging_live_bytes\":0",
        "\"staging_live_bytes\":8388608"
    )));
    assert!(!check(&good.replace(
        "\"schema\":\"conduit.conduitos/compositor-stopped-lifecycle@1\"",
        "\"part\":0,\"schema\":\"conduit.conduitos/compositor-stopped-lifecycle@1\""
    )));
}
