use super::*;
fn row(seq: u64, span: u64, parent: u64, phase: &str, state: &str, ticks: u64) -> Value {
    json!({"schema":"conduit.conduitos/native-progress@1","scope":"native-root-boot",
        "sequence":seq,"span":span,"parent":parent,"phase":phase,"context":"native-plot",
        "state":state,"clock":"x86-tsc-lfence","frequency_hz":null,"ticks":ticks})
}
fn family(rows: &[Value]) -> String {
    rows.iter()
        .map(|r| format!("{PREFIX}{r}\n"))
        .collect::<String>()
        + "CONDUIT_NATIVE_FACE {\"show\":{\"host_id\":\"host\",\"boot_id\":\"boot\"}}\n"
}
fn nested() -> Vec<Value> {
    vec![
        row(0, 1, 0, "plot-build", "started", 10),
        row(1, 2, 1, "source-check", "started", 20),
        row(2, 2, 1, "source-check", "refused", 30),
        row(3, 1, 0, "plot-build", "finished", 40),
    ]
}
#[test]
fn validated_nested_progress_retains_refusal_and_unknown_frequency() {
    let p = parse(&family(&nested()), "host", "boot").unwrap();
    assert_eq!(p["phases"]["source-check"]["refused"], 1);
    assert_eq!(p["phases"]["plot-build"]["aggregate_nested_ticks"], 30);
    assert!(p["frequency_hz"].is_null());
    assert_eq!(p["optional_network_exercised"], false);
    assert!(require_product_phases(&p).is_err());
}
#[test]
fn dropped_truncated_reordered_and_relabelled_families_do_not_pass() {
    for (index, key, value) in [
        (0, "sequence", json!(1)),
        (0, "phase", json!("invented")),
        (1, "parent", json!(0)),
        (1, "span", json!(1)),
        (2, "phase", json!("source-parse")),
        (2, "state", json!("abandoned")),
        (2, "context", json!("other")),
        (3, "ticks", json!(29)),
        (0, "frequency_hz", json!(1)),
        (0, "context", json!("bad\"context")),
    ] {
        let mut rows = nested();
        rows[index][key] = value;
        assert!(parse(&family(&rows), "host", "boot").is_err(), "{key}");
    }
    let rows = nested();
    assert!(parse(&family(&rows[..3]), "host", "boot").is_err());
    assert!(parse(&(family(&rows) + PREFIX + "{\"pending"), "host", "boot").is_err());
    assert!(parse(&family(&rows), "host", "wrong-boot").is_err());
    assert!(parse(&(family(&rows) + REFUSAL + "span-bound\n"), "host", "boot").is_err());
}
#[test]
fn all_required_local_product_phases_must_be_observed_but_network_is_optional() {
    let mut rows = Vec::new();
    for phase in Phase::ALL.into_iter().filter(|p| *p != Phase::NetworkSetup) {
        let span = rows.len() as u64 / 2 + 1;
        let seq = rows.len() as u64;
        rows.push(row(seq, span, 0, phase.as_str(), "started", seq));
        rows.push(row(seq + 1, span, 0, phase.as_str(), "finished", seq + 1));
    }
    require_product_phases(&parse(&family(&rows), "host", "boot").unwrap()).unwrap();
    assert!(require_product_phases(
        &parse(
            &family(
                &rows[2..]
                    .iter()
                    .enumerate()
                    .map(|(i, r)| {
                        let mut r = r.clone();
                        r["sequence"] = json!(i);
                        r["span"] = json!(i / 2 + 1);
                        r
                    })
                    .collect::<Vec<_>>()
            ),
            "host",
            "boot"
        )
        .unwrap()
    )
    .is_err());
}
