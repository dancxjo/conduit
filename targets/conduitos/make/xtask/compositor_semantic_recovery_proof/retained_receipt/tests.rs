//! Synthetic decoder controls, never real QEMU measurement.
use super::*;
fn cost(entries: u64, copies: u64) -> Value {
    let mut value = json!({});
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
        value[field] = json!(0);
    }
    value["entries"] = json!(entries);
    value["copied_bytes"] = json!(copies);
    value["reserved_bytes"] = json!(4096);
    value["setup_copied_bytes"] = json!(100);
    value["setup_ticks"] = json!(100);
    value
}
fn fixture() -> String {
    let mut text = String::new();
    let identity = json!({"host":"host","boot":"boot","offer_generation":1,"base":"display",
        "width":640,"height":480,"pitch":2560,"bits_per_pixel":32,"red_shift":16,"green_shift":8,"blue_shift":0,
        "make_profile":"profile","make_build":"build","make_image_binding":"image","guest_image_sha256":vec![7_u8;32]});
    for trial in 0..3 {
        for kind in 0..3 {
            let instance = trial + 1;
            let prepared = kind == 0;
            let (before, after) = match kind {
                0 => (cost(0, 0), cost(1, 10)),
                1 => (cost(1, 10), cost(10, 100)),
                _ => (cost(11, 110), cost(22, 230)),
            };
            let ack = if prepared {
                Value::Null
            } else {
                json!({"instance":instance,"namespace":kind,"frame_sequence":1,"guest_sequence":1})
            };
            if !prepared {
                let entries = after["entries"].as_u64().unwrap();
                text.push_str(&format!("CONDUIT_COMPOSITOR_SERVICE_FRAME service_instance={instance} namespace={kind} frame_sequence=1 guest_sequence=1 pixels_written=200 damage_count=1 entries={entries} privilege_transitions=0 address_space_switches=0 scheduler_returns=0\n"));
            }
            let arm = ["prepared", "cold", "retained"][kind];
            let record = json!({"schema":"conduit.conduitos/compositor-retained-pair@1","trial":trial,
                "arm":arm,"identity":identity,"instance":instance,"ack":ack,
                "before":before,"after":after,"elapsed_raw_ticks":10,
                "installation_raw_ticks":if prepared {json!(1)} else {Value::Null},
                "initialization_raw_ticks":if prepared {json!(1)} else {Value::Null},
                "activation_raw_ticks":if prepared {Value::Null} else {json!(1)},
                "activation_entries":if prepared {Value::Null} else {json!(2)},
                "face":"actual-face-id","scene_commands":3,"raster_commands":if prepared {0} else {3},
                "raster_pixels":if prepared {0} else {100},"target_writes":if prepared {0} else {200},
                "scene_equal_within_pair":true,"zero_audit_armed":false,"wall_nanoseconds":null});
            text.push_str(&format!("{PREFIX}{record}\n"));
        }
    }
    text
}
fn check(text: &str) -> bool {
    validate(text, "profile", "build", "image", [7; 32]).is_ok()
}
#[test]
fn three_pairs_keep_real_control_cost_differences_and_refuse_incomplete_or_relabelled_records() {
    let good = fixture();
    let report = validate(&good, "profile", "build", "image", [7; 32]).unwrap();
    assert_ne!(
        report["pairs"][0]["cold_control_cost"],
        report["pairs"][0]["retained_control_cost"]
    );
    assert!(!check(good.trim_end()));
    assert!(!check(&format!("{good}{good}")));
    assert!(!check(&good.replace("\"trial\":2", "\"trial\":1")));
    assert!(!check(&good.replace("\"activation_entries\":null,", "")));
    assert!(!check(&good.replace(
        "\"zero_audit_armed\":false",
        "\"zero_audit_armed\":true"
    )));
    assert!(validate(&good, "profile", "build", "image", [8; 32]).is_err());
}
#[test]
fn mismatched_render_copy_scope_and_false_retained_identity_are_refused() {
    let good = fixture();
    assert!(!check(&good.replace("namespace=2", "namespace=1")));
    assert!(!check(
        &good.replace("\"target_writes\":200", "\"target_writes\":201")
    ));
    assert!(!check(&good.replace(
        "\"scene_equal_within_pair\":true",
        "\"scene_equal_within_pair\":false"
    )));
    assert!(!check(&good.replace(
        "\"zero_audit_armed\":false",
        "\"zero_audit_armed\":false,\"unrequested\":true"
    )));
    assert!(!check(
        &good.replace("\"arm\":\"retained\"", "\"arm\":\"cold\"")
    ));
}
