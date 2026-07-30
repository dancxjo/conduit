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

#[test]
fn cross_contract_semantic_impersonation_is_rejected() {
    use conduit_panel::Node;
    use conduit_runtime::{
        FILE_READ_CONTRACT, Handler, HostResolutionEvidence, ImplementationManifest, RunIo,
        RuntimeError, Value,
    };

    struct DummyHandler;
    impl Handler for DummyHandler {
        fn run(
            &mut self,
            _: &Node,
            _: &[Value],
            _: &mut RunIo<'_>,
        ) -> Result<Vec<Value>, RuntimeError> {
            Ok(Vec::new())
        }
    }

    let mut registry = Registry::default();

    // Impersonator manifest claims to be "conduit.std/literal" while attempting to implement "conduit.std/file-read"
    let fake_manifest = ImplementationManifest {
        implementation_id: "fake.literal.impersonator".to_owned(),
        contract_id: "conduit.std/literal".to_owned(),
        contract_hash: None,
    };

    let err = registry
        .register_executable_provider(
            &FILE_READ_CONTRACT,
            fake_manifest,
            Some(HostResolutionEvidence {
                host_id: "local-host".to_owned(),
                is_capable: true,
                rejection_reasons: Vec::new(),
            }),
            || Box::new(DummyHandler),
            |_| Ok(()),
        )
        .expect_err("cross-contract impersonation must be rejected during provider registration");

    assert_eq!(err.code, "CND-REG-004");
    assert!(
        err.message
            .contains("cross-contract semantic impersonation rejected")
    );

    // Ensure node availability remains ContractOnly despite the failed attempt
    let avail = registry.node_availability("conduit/file-read");
    assert_eq!(avail.state, AvailabilityState::ContractOnly);
}

#[test]
fn provider_available_state_when_host_evidence_absent_or_incapable() {
    use conduit_panel::Node;
    use conduit_runtime::{
        FILE_READ_CONTRACT, Handler, HostResolutionEvidence, ImplementationManifest, RunIo,
        RuntimeError, Value,
    };

    struct DummyHandler;
    impl Handler for DummyHandler {
        fn run(
            &mut self,
            _: &Node,
            _: &[Value],
            _: &mut RunIo<'_>,
        ) -> Result<Vec<Value>, RuntimeError> {
            Ok(Vec::new())
        }
    }

    let mut registry = Registry::default();

    let valid_manifest = ImplementationManifest {
        implementation_id: "test.file-read.native".to_owned(),
        contract_id: FILE_READ_CONTRACT.id.as_str().to_owned(),
        contract_hash: None,
    };

    // Case 1: Manifest present, but host facts/evidence absent
    registry
        .register_executable_provider(
            &FILE_READ_CONTRACT,
            valid_manifest.clone(),
            None,
            || Box::new(DummyHandler),
            |_| Ok(()),
        )
        .expect("registration succeeds");

    let avail_no_evidence = registry.node_availability("conduit/file-read");
    assert_eq!(
        avail_no_evidence.state,
        AvailabilityState::ProviderAvailable
    );
    assert_eq!(avail_no_evidence.reason_code, "CND-AVL-002");
    assert_eq!(
        avail_no_evidence.implementation_id.as_deref(),
        Some("test.file-read.native")
    );
    assert_eq!(avail_no_evidence.host_id, None);

    // Case 2: Manifest present, host facts present but host is not capable
    registry
        .register_executable_provider(
            &FILE_READ_CONTRACT,
            valid_manifest,
            Some(HostResolutionEvidence {
                host_id: "remote-pico".to_owned(),
                is_capable: false,
                rejection_reasons: vec!["CND-RES-015".to_owned()],
            }),
            || Box::new(DummyHandler),
            |_| Ok(()),
        )
        .expect("registration succeeds");

    let avail_incapable = registry.node_availability("conduit/file-read");
    assert_eq!(avail_incapable.state, AvailabilityState::ProviderAvailable);
    assert_eq!(avail_incapable.reason_code, "CND-AVL-002");
    assert_eq!(avail_incapable.host_id.as_deref(), Some("remote-pico"));
    assert_eq!(avail_incapable.rejection_reasons, vec!["CND-RES-015"]);
}

#[test]
fn bound_in_plan_and_running_availability_states() {
    let registry = Registry::default();
    let avail = registry.node_availability("conduit/literal");

    let bound = avail.clone().bound_in_plan("sha256:plan-123");
    assert_eq!(bound.state, AvailabilityState::BoundInThisPlan);
    assert_eq!(bound.reason_code, "CND-AVL-004");
    assert_eq!(bound.plan_identity.as_deref(), Some("sha256:plan-123"));

    let running = avail.with_run("run/99");
    assert_eq!(running.state, AvailabilityState::Running);
    assert_eq!(running.reason_code, "CND-AVL-005");
    assert_eq!(running.run_id.as_deref(), Some("run/99"));
}

#[test]
fn patchbay_snapshot_carries_truthful_node_availabilities() {
    let src = "panel 1\nnode greeting : conduit/literal { value = \"hello\" }\nnode output : conduit/stdout\n";
    let workspace = conduit_patchbay::Workspace::new("doc-1", src).expect("parses");
    let snapshot = workspace.semantic();

    assert_eq!(snapshot.availabilities.len(), 2);

    let greeting_avail = &snapshot.availabilities[0];
    assert_eq!(greeting_avail.contract_id, "conduit.std/literal");
    assert_eq!(greeting_avail.availability_state, "resolvable-on-this-host");
    assert_eq!(greeting_avail.reason_code, "CND-AVL-003");
}
