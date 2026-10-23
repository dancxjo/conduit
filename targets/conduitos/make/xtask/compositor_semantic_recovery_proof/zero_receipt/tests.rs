//! Synthetic contract controls, never relabeled actual disposal evidence.
use super::*;
fn good() -> String {
    super::super::tests::fixture()
}
fn check(serial: &str) -> bool {
    super::super::validate(serial, "profile", "build", "image", [7; 32]).is_ok()
}
fn change(f: impl FnOnce(&mut Value)) -> String {
    let text = good();
    let mut output = String::new();
    let mut f = Some(f);
    for line in text.split_inclusive('\n') {
        if let Some(payload) = line.strip_prefix(PREFIX) {
            let mut value: Value = serde_json::from_str(payload).unwrap();
            f.take().unwrap()(&mut value);
            output.push_str(&format!("{PREFIX}{value}\n"));
        } else {
            output.push_str(line);
        }
    }
    output
}
#[test]
fn mandatory_single_whole_witness_is_after_eight_snapshots_before_summary() {
    let good = good();
    assert!(check(&good));
    let witness = good.lines().find(|line| line.starts_with(PREFIX)).unwrap();
    let missing = good.replace(&format!("{witness}\n"), "");
    assert!(!check(&missing));
    assert!(!check(&good.replace(
        &format!("{witness}\n"),
        &format!("{witness}\n{witness}\n")
    )));
    assert!(!check(&format!("{witness}\n{missing}")));
    assert!(!check(&format!("{missing}{witness}\n")));
    assert!(!check(&good.replace(&format!("{witness}\n"), witness)));
    assert!(!check(&good.replace(
        "compositor-stopped-lifecycle-result@2",
        "compositor-stopped-lifecycle-result@1"
    )));
    assert!(!check(&good.replace(
        "\"whole_zero_audited\":true",
        "\"whole_zero_audited\":false"
    )));
}
#[test]
fn counts_nonzero_partial_regions_and_spare_capacity_overflow_are_not_whole_zero() {
    for field in ["storage_bytes", "checked_bytes"] {
        assert!(!check(&change(
            |v| v["private"][field] = serde_json::json!(1)
        )));
    }
    for field in ["counts_valid", "all_zero"] {
        assert!(!check(&change(
            |v| v["private"][field] = serde_json::json!(false)
        )));
    }
    for index in 0..12 {
        assert!(!check(&change(
            |v| v["private"]["regions"][index]["all_zero"] = serde_json::json!(false)
        )));
        assert!(!check(&change(
            |v| v["private"]["regions"][index]["checked_bytes"] = serde_json::json!(0)
        )));
    }
    assert!(!check(&change(|v| {
        v["private"]["regions"].as_array_mut().unwrap().pop();
    })));
    assert!(!check(&change(|v| v["private"]["regions"]
        .as_array_mut()
        .unwrap()
        .swap(0, 1))));
    for field in ["initialized_words", "checked_words", "capacity_words"] {
        assert!(!check(&change(
            |v| v["staging"][field] = serde_json::json!(1)
        )));
    }
    assert!(!check(&change(
        |v| v["staging"]["nonzero_words"] = serde_json::json!(1)
    )));
    assert!(!check(&change(
        |v| v["staging"]["capacity_words"] = serde_json::json!(u64::MAX)
    )));
}
#[test]
fn scope_identity_release_joins_unknown_duplicate_and_missing_fields_are_strict() {
    for field in ["instance", "namespace", "release_ordinal"] {
        assert!(!check(&change(|v| v[field] = serde_json::json!(0))));
        assert!(!check(&change(
            |v| v["private"][field] = serde_json::json!(2)
        )));
    }
    assert!(!check(&change(
        |v| v["staging"]["release_ordinal"] = serde_json::json!(8)
    )));
    assert!(!check(&change(
        |v| v["staging"]["instance"] = serde_json::json!(2)
    )));
    assert!(!check(&change(
        |v| v["identity"]["guest_image_sha256"][0] = serde_json::json!(8)
    )));
    assert!(!check(&change(
        |v| v["identity"]["base"] = serde_json::json!("foreign")
    )));
    assert!(!check(&change(
        |v| v["profile"] = serde_json::json!("runtime")
    )));
    assert!(!check(&change(|v| {
        v.as_object_mut().unwrap().remove("private");
    })));
    assert!(!check(&change(|v| v["unknown"] = serde_json::json!(true))));
    assert!(!check(&change(
        |v| v["backend_teardown_ticks_include_scan"] = serde_json::json!(true)
    )));
    let text = good();
    let needle = "\"release_ordinal\":7";
    let duplicated = text.replacen(needle, "\"release_ordinal\":7,\"release_ordinal\":7", 1);
    assert!(!check(&duplicated));
}
