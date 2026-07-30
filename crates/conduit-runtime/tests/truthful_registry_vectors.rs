use conduit_panel::parse;
use conduit_runtime::{AvailabilityState, Registry};

#[test]
fn delay_filter_assert_record_file_process_gpio_serial_wifi_fail_without_provider() {
    let unverified_node_kinds = [
        "conduit/delay",
        "conduit/debounce",
        "conduit/throttle",
        "conduit/take",
        "conduit/skip",
        "conduit/filter",
        "conduit/probe",
        "conduit/log",
        "conduit/assert",
        "conduit/record",
        "conduit/replay",
        "conduit/fault-source",
        "conduit/file-read",
        "conduit/file-write",
        "conduit/blob-store",
        "conduit/kv-store",
        "conduit/process-spawn",
        "conduit/gpio-pin",
        "conduit/serial-port",
        "conduit/cell",
        "conduit/counter",
        "conduit/deduplicate",
        "conduit/cache",
        "conduit/circuit-breaker",
        "conduit/health-gate",
        "conduit/backoff",
        "conduit/wifi-station",
        "conduit/wifi-ap",
        "conduit/network-interface",
        "conduit/tcp-socket",
        "conduit/udp-socket",
        "conduit/dns-resolver",
    ];

    let registry = Registry::default();

    for kind in unverified_node_kinds {
        let avail = registry.node_availability(kind);
        assert_eq!(
            avail.state,
            AvailabilityState::ContractOnly,
            "kind {kind} should be ContractOnly"
        );
        assert_eq!(avail.reason_code, "CND-AVL-001");
        assert_eq!(avail.rejection_reasons, vec!["CND-RES-008"]);

        let panel_src = format!("panel 1\nnode n : {kind}\n");
        if let Ok(panel) = parse(&panel_src) {
            let err = registry
                .resolve(&panel)
                .expect_err(&format!("{kind} resolution should fail"));
            assert_eq!(err.code, "CND-IMP-001");
        }
    }
}

#[test]
fn contract_present_but_executable_absent_fails_resolution() {
    let registry = Registry::default();
    let avail = registry.node_availability("conduit/file-read");
    assert_eq!(avail.state, AvailabilityState::ContractOnly);
    assert_eq!(avail.reason_code, "CND-AVL-001");
    assert_eq!(avail.rejection_reasons, vec!["CND-RES-008"]);

    let panel = parse("panel 1\nnode r : conduit/file-read\n").unwrap();
    let err = registry
        .resolve(&panel)
        .expect_err("file-read resolution fails");
    assert_eq!(err.code, "CND-IMP-001");
}

#[test]
fn honest_primitives_remain_resolvable() {
    let registry = Registry::default();
    let honest_kinds = [
        "conduit/literal",
        "conduit/stdin",
        "conduit/uppercase",
        "conduit/stdout",
        "conduit/stderr",
        "conduit/supervisor",
        "conduit/pass-through",
        "conduit/tee",
        "conduit/merge",
        "conduit/fallback",
    ];

    for kind in honest_kinds {
        let avail = registry.node_availability(kind);
        assert_eq!(avail.state, AvailabilityState::ResolvableOnThisHost);
        assert_eq!(avail.reason_code, "CND-AVL-003");
        assert!(avail.rejection_reasons.is_empty());
    }
}

#[test]
fn unknown_node_returns_unsupported_availability() {
    let registry = Registry::default();
    let avail = registry.node_availability("conduit/nonexistent");
    assert_eq!(avail.state, AvailabilityState::Unsupported);
    assert_eq!(avail.reason_code, "CND-AVL-006");
}
