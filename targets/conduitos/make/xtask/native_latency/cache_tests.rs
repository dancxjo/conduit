use super::*;
fn row(sequence: usize, mode: CacheMode, delta: [u64; 7], retained: [u64; 2]) -> Value {
    let plot = candidate_inventory()[0];
    json!({"schema":SCHEMA,"scope":"native-root-boot","sequence":sequence,
        "mode":mode.as_str(),"limits":if mode == CacheMode::Retained { json!([NATIVE_CANDIDATE_CAPACITY,1,8192]) } else { json!([0,0,0]) },
        "plot":plot.name(),"source_sha256":format!("{:x}",Sha256::digest(plot.source().as_bytes())),
        "path":"shared","accepted":true,"delta":delta,"retained":retained})
}
fn serial(rows: &[Value]) -> String {
    let mut result = String::from(
        "CONDUIT_NATIVE_FACE {\"show\":{\"host_id\":\"host\",\"boot_id\":\"boot\"}}\n",
    );
    for row in rows {
        result.push_str(&format!("{PREFIX}{row}\n"));
    }
    result
}
fn retained() -> Vec<Value> {
    vec![
        row(0, CacheMode::Retained, [1, 0, 1, 0, 0, 0, 0], [1, 1]),
        row(1, CacheMode::Retained, [0, 1, 0, 1, 0, 0, 0], [1, 1]),
    ]
}
#[test]
fn actual_cold_hit_and_private_fallback_deltas_do_not_reset_totals() {
    let mut rows = retained();
    let mut private = row(2, CacheMode::Retained, [1, 0, 1, 0, 0, 0, 0], [1, 1]);
    private["path"] = json!("private-contention");
    rows.push(private);
    rows.push(row(3, CacheMode::Retained, [0, 1, 0, 1, 0, 0, 0], [1, 1]));
    let receipt = parse(&serial(&rows), CacheMode::Retained, "host", "boot").unwrap();
    assert_eq!(receipt["totals"]["checks"], 2);
    assert_eq!(receipt["totals"]["check_hits"], 2);
    assert_eq!(receipt["private_contention_calls"], 1);
    assert_eq!(receipt["final_shared_retained"], json!([1, 1]));
}
#[test]
fn zero_retention_requires_real_unretained_cold_work_and_exact_guest_mode() {
    let rows = vec![
        row(0, CacheMode::ZeroRetention, [1, 0, 1, 0, 0, 0, 1], [0, 0]),
        row(1, CacheMode::ZeroRetention, [1, 0, 1, 0, 0, 0, 1], [0, 0]),
    ];
    let receipt = parse(&serial(&rows), CacheMode::ZeroRetention, "host", "boot").unwrap();
    assert_eq!(receipt["totals"]["checks"], 2);
    assert_eq!(receipt["totals"]["check_hits"], 0);
    assert!(parse(&serial(&rows), CacheMode::Retained, "host", "boot").is_err());
    for (field, value) in [
        ("retained", json!([1, 1])),
        ("delta", json!([0, 1, 0, 1, 0, 0, 0])),
        ("limits", json!([1, 1, 8192])),
    ] {
        let mut changed = rows.clone();
        changed[1][field] = value;
        assert!(
            parse(&serial(&changed), CacheMode::ZeroRetention, "host", "boot").is_err(),
            "{field}"
        );
    }
}
#[test]
fn refuses_foreign_stale_reordered_omitted_and_authority_upgraded_records() {
    let good = retained();
    for (index, field, value) in [
        (1, "source_sha256", json!("f".repeat(64))),
        (1, "sequence", json!(0)),
        (1, "path", json!("private-contention")),
        (1, "retained", json!([0, 0])),
        (1, "delta", json!([u64::MAX, 1, 0, 1, 0, 0, 0])),
        (1, "plan_authority", json!(true)),
    ] {
        let mut changed = good.clone();
        changed[index][field] = value;
        assert!(
            parse(&serial(&changed), CacheMode::Retained, "host", "boot").is_err(),
            "{field}"
        );
    }
    assert!(parse(&serial(&good[1..]), CacheMode::Retained, "host", "boot").is_err());
    assert!(parse(&serial(&good[..1]), CacheMode::Retained, "host", "boot").is_err());
    assert!(parse(&serial(&good), CacheMode::Retained, "host", "foreign").is_err());
    assert!(parse(
        serial(&good).trim_end(),
        CacheMode::Retained,
        "host",
        "boot"
    )
    .is_err());
    assert!(parse(
        &(serial(&good) + "CONDUIT_NATIVE_CACHE_REFUSAL observation-bound\n"),
        CacheMode::Retained,
        "host",
        "boot"
    )
    .is_err());
}
