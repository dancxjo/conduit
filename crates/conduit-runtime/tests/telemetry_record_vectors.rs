use conduit_panel::{Node, parse};
use conduit_runtime::{
    ASSERT_CONTRACT, AvailabilityState, Handler, LOG_CONTRACT, PROBE_CONTRACT, RECORD_CONTRACT,
    Registry, RunIo, RuntimeError, Value,
};

struct EchoHandler;
impl Handler for EchoHandler {
    fn run(
        &mut self,
        _node: &Node,
        inputs: &[Value],
        _io: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        let val = inputs.first().cloned().unwrap_or_else(|| Value::text(""));
        Ok(vec![val])
    }
}

#[test]
fn probe_and_log_nodes_pass_through_flow_unmodified() {
    let panel = parse(
        r#"
            panel 1
            node source : conduit/literal { value = "telemetry item" }
            node probe : conduit/probe
            node log : conduit/log
            node sink : conduit/stdout
            cord source.out -> probe.in
            cord probe.out -> log.in
            cord log.out -> sink.in
        "#,
    )
    .expect("telemetry panel parses");

    let registry = Registry::default();

    let avail = registry.node_availability("conduit/probe");
    assert_eq!(avail.state, AvailabilityState::ContractOnly);
    assert_eq!(avail.reason_code, "CND-AVL-001");
    assert_eq!(avail.rejection_reasons, vec!["CND-RES-008"]);

    let err = registry
        .resolve(&panel)
        .expect_err("unsupported probe/log nodes fail resolution");
    assert_eq!(err.code, "CND-IMP-001");

    let mut custom_registry = Registry::default();
    let reg_provider = |reg: &mut Registry,
                        contract: &'static conduit_core::NodeContract<'static>,
                        factory: conduit_runtime::HandlerFactory| {
        let manifest = conduit_runtime::ImplementationManifest {
            implementation_id: format!("{}.test", contract.id.as_str()),
            contract_id: contract.id.as_str().to_owned(),
            contract_hash: conduit_runtime::compute_contract_hash(contract),
        };
        let host_evidence = conduit_runtime::HostResolutionEvidence {
            host_id: "hosted-local".to_owned(),
            time_basis: "clock/monotonic".to_owned(),
            observed_at_tick: 1,
            valid_until_tick: 1000,
            available_memory_bytes: 1_000_000,
            required_memory_bytes: 1_000,
            rejection_reasons: Vec::new(),
        };
        reg.register_executable_provider(contract, manifest, Some(host_evidence), factory, |_| {
            Ok(())
        })
        .unwrap();
    };

    reg_provider(&mut custom_registry, &PROBE_CONTRACT, || {
        Box::new(EchoHandler)
    });
    reg_provider(&mut custom_registry, &LOG_CONTRACT, || {
        Box::new(EchoHandler)
    });

    let resolved = custom_registry
        .resolve(&panel)
        .expect("telemetry panel resolves");

    let mut input = &b""[..];
    let mut output = Vec::new();
    let mut error = Vec::new();

    resolved
        .run(&mut RunIo {
            input: &mut input,
            output: &mut output,
            error: &mut error,
        })
        .expect("telemetry panel runs");

    assert_eq!(output, b"telemetry item");
}

#[test]
fn record_and_assert_nodes_preserve_flow_semantics() {
    let panel = parse(
        r#"
            panel 1
            node source : conduit/literal { value = "tested payload" }
            node recorder : conduit/record
            node assertion : conduit/assert
            node sink : conduit/stdout
            cord source.out -> recorder.in
            cord recorder.out -> assertion.in
            cord assertion.out -> sink.in
        "#,
    )
    .expect("record & assert panel parses");

    let registry = Registry::default();

    let err = registry
        .resolve(&panel)
        .expect_err("unsupported record/assert nodes fail resolution");
    assert_eq!(err.code, "CND-IMP-001");

    let mut custom_registry = Registry::default();
    let reg_provider = |reg: &mut Registry,
                        contract: &'static conduit_core::NodeContract<'static>,
                        factory: conduit_runtime::HandlerFactory| {
        let manifest = conduit_runtime::ImplementationManifest {
            implementation_id: format!("{}.test", contract.id.as_str()),
            contract_id: contract.id.as_str().to_owned(),
            contract_hash: conduit_runtime::compute_contract_hash(contract),
        };
        let host_evidence = conduit_runtime::HostResolutionEvidence {
            host_id: "hosted-local".to_owned(),
            time_basis: "clock/monotonic".to_owned(),
            observed_at_tick: 1,
            valid_until_tick: 1000,
            available_memory_bytes: 1_000_000,
            required_memory_bytes: 1_000,
            rejection_reasons: Vec::new(),
        };
        reg.register_executable_provider(contract, manifest, Some(host_evidence), factory, |_| {
            Ok(())
        })
        .unwrap();
    };

    reg_provider(&mut custom_registry, &RECORD_CONTRACT, || {
        Box::new(EchoHandler)
    });
    reg_provider(&mut custom_registry, &ASSERT_CONTRACT, || {
        Box::new(EchoHandler)
    });

    let resolved = custom_registry
        .resolve(&panel)
        .expect("record & assert panel resolves");

    let mut input = &b""[..];
    let mut output = Vec::new();
    let mut error = Vec::new();

    resolved
        .run(&mut RunIo {
            input: &mut input,
            output: &mut output,
            error: &mut error,
        })
        .expect("record & assert panel runs");

    assert_eq!(output, b"tested payload");
}
