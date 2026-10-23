//! Pure collector controls. These synthetic lines are not product execution proof.
#[path = "../../../targets/conduitos/make/xtask/compositor_lifecycle_proof.rs"]
mod proof;
use proof::{ExpectedBinding, Identity, Refusal};
use serde_json::{json, Value};

fn identity() -> Value {
    json!({"host":"host", "boot":"boot", "offer_generation":1,"base":"display",
        "width":1280,"height":800,"pitch":5120,"bits_per_pixel":32,
        "red_shift":16,"green_shift":8,"blue_shift":0,
        "make_profile":"profile","make_build":"build","make_image_binding":"binding",
        "guest_image_sha256":[7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7,7]})
}
fn expected() -> ExpectedBinding {
    ExpectedBinding {
        source_commit: "a".repeat(40),
        root_elf_sha256: "b".repeat(64),
        iso_sha256: "c".repeat(64),
        identity: serde_json::from_value::<Identity>(identity()).unwrap(),
        preparation: 1,
        service_reserved_bytes: 1000,
        staging_prepared_bytes: 800,
    }
}
fn cost(n: u64) -> Value {
    json!({"entries":n,"interrupt_entries":n,"source_timer_interrupts":n,
        "privilege_transitions":n,"gate_transitions":n,"copied_bytes":n,
        "setup_copied_bytes":99,"base_gate_transitions":n,"tlb_flushes":n,
        "setup_ticks":12,"teardown_ticks":0,"teardown_zeroed_bytes":0,
        "shared_peak_bytes":800,"root_metadata_bytes":50,
        "address_space_switches":n,"scheduler_returns":n,"preemptions":0,"reserved_bytes":1000})
}
fn fixture() -> Vec<Value> {
    let mut output = Vec::new();
    for (ordinal, checkpoint) in ["prepared", "initial-hub", "first-creche", "playing-hub"]
        .iter()
        .enumerate()
    {
        let prepared = ordinal == 0;
        let shape = if prepared {
            vec![
                ("preparation", "context"),
                ("preparation", "installed"),
                ("preparation", "before"),
                ("preparation", "after"),
                ("preparation", "current"),
            ]
        } else {
            vec![
                ("activation", "context"),
                ("activation", "before"),
                ("activation", "after"),
                ("request", "before"),
                ("request", "after"),
                ("request", "current"),
            ]
        };
        let count = shape.len();
        for (part, (stage, kind)) in shape.iter().enumerate() {
            let paired = *kind == "before" || *kind == "after";
            let mut record = json!({"record_kind":"snapshot","schema":"conduit.conduitos/compositor-lifecycle@1","complete":true,
                "snapshot":{"position":{"preparation":1,"checkpoint":ordinal,"part":part,"parts":count,"instance":9,"namespace":null},
                "checkpoint":checkpoint,"ack":null,"stage":stage,"kind":kind,"identity":identity(),"phase":null,"sample":null,"cost":null,
                "staging_prepared_bytes":800,"staging_live_bytes":800,"installation_raw_ticks":null,"staging_preparation_raw_ticks":null,
                "preparation_raw_ticks":null,"disposal_raw_ticks":null}});
            let s = &mut record["snapshot"];
            if !prepared {
                // Intermediate real activations need not appear as selected batches.
                s["position"]["namespace"] = json!(ordinal * 3);
                s["ack"] = json!({"instance":9,"namespace":ordinal*3,"frame_sequence":ordinal,"guest_sequence":ordinal+20});
            }
            if paired {
                s["sample"] = json!({"succeeded":true,"refused":false,"raw_ticks":null});
                if !prepared {
                    s["phase"] = json!({"count":ordinal*10 + usize::from(*stage=="request"),"overflowed":false});
                }
            }
            if *kind != "context" {
                let n = if prepared {
                    u64::from(part >= 3)
                } else {
                    ordinal as u64 * 10 + part.min(4) as u64
                };
                s["cost"] = cost(n);
            }
            output.push(record);
        }
    }
    output
}
fn lines(records: &[Value]) -> String {
    let mut result = String::from("unrelated serial output\n");
    for record in records {
        let s = &record["snapshot"];
        if s["position"]["part"] == 0 && !s["ack"].is_null() {
            let a = &s["ack"];
            let n = s["position"]["checkpoint"].as_u64().unwrap() * 10 + 4;
            result.push_str(&format!("CONDUIT_COMPOSITOR_SERVICE_FRAME service_instance=9 namespace={} frame_sequence={} guest_sequence={} pixels_written=100 damage_count=1 entries={n} privilege_transitions={n} address_space_switches={n} scheduler_returns={n}\n",a["namespace"],a["frame_sequence"],a["guest_sequence"]));
        }
        result.push_str(proof::PREFIX);
        result.push_str(&record.to_string());
        result.push('\n');
    }
    result
}
fn refuses(records: &[Value]) {
    assert!(proof::parse_terminal(&lines(records), &expected()).is_err());
}
#[test]
fn complete_selected_batches_join_real_frame_tuples_without_claiming_artifact_custody() {
    let report = proof::parse_terminal(&lines(&fixture()), &expected()).unwrap();
    assert!(proof::preparation_context(&lines(&fixture()))
        .unwrap()
        .ack
        .0
        .is_none());
    assert_eq!(report.instance, 9);
    assert_eq!(report.lifecycle_records, 23);
    assert_eq!(report.checkpoints.len(), 4);
    assert!(!report.artifact_custody_verified);
    assert_eq!(report.source_commit, "a".repeat(40));
    assert_eq!(report.root_elf_sha256, "b".repeat(64));
    assert_eq!(report.iso_sha256, "c".repeat(64));
    assert_eq!(
        report.checkpoints[0].checkpoint,
        proof::Checkpoint::Prepared
    );
    assert!(report.checkpoints[0].ack.is_none());
    assert_eq!(report.checkpoints[2].ack.unwrap().namespace, 6);
    assert_eq!(report.checkpoints[3].activation_count, Some(30));
    assert_eq!(report.checkpoints[3].request_count, Some(31));
}
#[test]
fn all_required_nullable_fields_refuse_omission_but_accept_explicit_null() {
    for field in [
        "ack",
        "phase",
        "sample",
        "cost",
        "installation_raw_ticks",
        "staging_preparation_raw_ticks",
        "preparation_raw_ticks",
        "disposal_raw_ticks",
    ] {
        let mut r = fixture();
        r[0]["snapshot"].as_object_mut().unwrap().remove(field);
        refuses(&r);
    }
    let mut r = fixture();
    r[0]["snapshot"]["position"]
        .as_object_mut()
        .unwrap()
        .remove("namespace");
    refuses(&r);
    let mut r = fixture();
    r[2]["snapshot"]["sample"]
        .as_object_mut()
        .unwrap()
        .remove("raw_ticks");
    refuses(&r);
}
#[test]
fn missing_duplicate_reordered_or_wrong_shape_records_refuse() {
    let mut r = fixture();
    r.remove(3);
    refuses(&r);
    let mut r = fixture();
    r[3] = r[2].clone();
    refuses(&r);
    let mut r = fixture();
    r.swap(5, 11);
    refuses(&r);
    let mut r = fixture();
    r[0]["snapshot"]["position"]["parts"] = json!(6);
    refuses(&r);
    let mut r = fixture();
    r[1]["snapshot"]["stage"] = json!("request");
    refuses(&r);
}
#[test]
fn foreign_scope_image_and_disposal_cannot_satisfy_normal_proof() {
    for field in [
        "host",
        "boot",
        "base",
        "make_profile",
        "make_build",
        "make_image_binding",
    ] {
        let mut r = fixture();
        r[5]["snapshot"]["identity"][field] = json!("foreign");
        refuses(&r);
    }
    let mut r = fixture();
    r[5]["snapshot"]["identity"]["guest_image_sha256"][0] = json!(8);
    refuses(&r);
    let mut r = fixture();
    r[5]["snapshot"]["position"]["instance"] = json!(99);
    refuses(&r);
    let mut r = fixture();
    r[0]["snapshot"]["disposal_raw_ticks"] = json!(0);
    refuses(&r);
}
#[test]
fn real_ack_requires_unique_preceding_exact_tuple_and_matching_cost() {
    let s = lines(&fixture());
    assert!(
        proof::parse_terminal(&s.replace("namespace=3 ", "namespace=4 "), &expected()).is_err()
    );
    let frame = s
        .lines()
        .find(|x| x.starts_with("CONDUIT_COMPOSITOR_SERVICE_FRAME "))
        .unwrap();
    assert!(proof::parse_terminal(&(s.clone() + frame + "\n"), &expected()).is_err());
    let delayed = s
        .lines()
        .filter(|l| *l != frame)
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
        + frame
        + "\n";
    assert!(proof::parse_terminal(&delayed, &expected()).is_err());
    assert!(proof::parse_terminal(&s.replace("entries=14 ", "entries=15 "), &expected()).is_err());
    let mut r = fixture();
    r[5]["snapshot"]["ack"] = Value::Null;
    refuses(&r);
}
#[test]
fn unknown_duplicate_json_fields_and_numeric_overflow_refuse() {
    let mut r = fixture();
    r[0]["unexpected"] = json!(true);
    refuses(&r);
    let mut r = fixture();
    r[0]["snapshot"]["identity"]["unexpected"] = json!(1);
    refuses(&r);
    let s = lines(&fixture());
    let duplicate = s.replacen(
        "\"complete\":true",
        "\"complete\":true,\"complete\":true",
        1,
    );
    assert!(proof::parse_terminal(&duplicate, &expected()).is_err());
    let overflow = s.replacen(
        "\"preparation\":1",
        "\"preparation\":18446744073709551616",
        1,
    );
    assert!(proof::parse_terminal(&overflow, &expected()).is_err());
    let mut r = fixture();
    r[6]["snapshot"]["phase"]["overflowed"] = json!(true);
    refuses(&r);
}
#[test]
fn terminal_partial_required_line_oversize_and_record_flood_refuse() {
    let s = lines(&fixture());
    assert_eq!(
        proof::parse_terminal(&(s.clone() + proof::PREFIX.trim_end()), &expected()).unwrap_err(),
        Refusal::Truncated
    );
    assert_eq!(
        proof::parse_terminal(s.trim_end_matches('\n'), &expected()).unwrap_err(),
        Refusal::Truncated
    );
    let long = format!("{}{}\n", proof::PREFIX, " ".repeat(proof::MAX_LINE_BYTES));
    assert_eq!(
        proof::parse_terminal(&long, &expected()).unwrap_err(),
        Refusal::Oversize
    );
    let records = fixture();
    let flood = vec![records[0].clone(); proof::MAX_RECORDS + 1];
    assert_eq!(
        proof::parse_terminal(&lines(&flood), &expected()).unwrap_err(),
        Refusal::RecordCount
    );
    assert!(proof::parse_terminal(&(s + proof::PREFIX + "{broken}\n"), &expected()).is_err());
}
#[test]
fn counter_reversal_and_false_success_refuse_without_conflating_gauges() {
    let mut r = fixture();
    r[7]["snapshot"]["cost"]["entries"] = json!(0);
    refuses(&r);
    let mut r = fixture();
    r[6]["snapshot"]["sample"]["succeeded"] = json!(false);
    r[7]["snapshot"]["sample"]["succeeded"] = json!(false);
    refuses(&r);
    let mut r = fixture();
    r[6]["snapshot"]["cost"]["setup_ticks"] = json!(999);
    r[7]["snapshot"]["cost"]["setup_ticks"] = json!(1);
    assert!(proof::parse_terminal(&lines(&r), &expected()).is_ok());
}

#[test]
fn actual_encoder_emergency_wire_shape_refuses_with_retained_reason() {
    // Exact static FALLBACK bytes from lifecycle_sign::encode, whose serializer
    // fallback branch is otherwise not triggerable by valid bounded metadata.
    let marker = concat!(
        "CONDUIT_COMPOSITOR_LIFECYCLE ",
        "{\"record_kind\":\"incomplete\",\"schema\":\"conduit.conduitos/compositor-lifecycle@1\",",
        "\"complete\":false,\"reason\":\"incomplete-marker-serialization\"}\n"
    );
    assert_eq!(
        proof::parse_terminal(marker, &expected()).unwrap_err(),
        Refusal::Incomplete(proof::IncompleteReason::IncompleteMarkerSerialization)
    );
    let missing_details = marker.replace("incomplete-marker-serialization", "counter-overflow");
    assert_eq!(
        proof::parse_terminal(&missing_details, &expected()).unwrap_err(),
        Refusal::Malformed
    );
}
