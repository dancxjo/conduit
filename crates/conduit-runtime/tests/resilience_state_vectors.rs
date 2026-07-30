use conduit_panel::{Node, parse};
use conduit_runtime::{
    AvailabilityState, CACHE_CONTRACT, CELL_CONTRACT, CIRCUIT_BREAKER_CONTRACT,
    DEDUPLICATE_CONTRACT, Handler, Registry, RunIo, RuntimeError, Value,
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
fn cell_and_deduplicate_nodes_run_in_panel() {
    let panel = parse(
        r#"
            panel 1
            node source : conduit/literal { value = "cell value" }
            node cell : conduit/cell
            node dedup : conduit/deduplicate
            node sink : conduit/stdout
            cord source.out -> cell.in
            cord cell.out -> dedup.in
            cord dedup.out -> sink.in
        "#,
    )
    .expect("cell/dedup panel parses");

    let registry = Registry::default();

    let avail = registry.node_availability("conduit/cell");
    assert_eq!(avail.state, AvailabilityState::ContractOnly);
    assert_eq!(avail.reason_code, "CND-AVL-001");
    assert_eq!(avail.rejection_reasons, vec!["CND-RES-008"]);

    let err = registry
        .resolve(&panel)
        .expect_err("unsupported cell nodes fail resolution");
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

    reg_provider(&mut custom_registry, &CELL_CONTRACT, || {
        Box::new(EchoHandler)
    });
    reg_provider(&mut custom_registry, &DEDUPLICATE_CONTRACT, || {
        Box::new(EchoHandler)
    });

    let resolved = custom_registry
        .resolve(&panel)
        .expect("cell/dedup panel resolves");

    let mut input = &b""[..];
    let mut output = Vec::new();
    let mut error = Vec::new();

    resolved
        .run(&mut RunIo {
            input: &mut input,
            output: &mut output,
            error: &mut error,
        })
        .expect("cell/dedup panel runs");

    assert_eq!(output, b"cell value");
}

#[test]
fn circuit_breaker_and_cache_nodes_run_in_panel() {
    let panel = parse(
        r#"
            panel 1
            node source : conduit/literal { value = "protected data" }
            node breaker : conduit/circuit-breaker
            node cache : conduit/cache
            node sink : conduit/stdout
            cord source.out -> breaker.in
            cord breaker.out -> cache.in
            cord cache.out -> sink.in
        "#,
    )
    .expect("breaker/cache panel parses");

    let registry = Registry::default();

    let err = registry
        .resolve(&panel)
        .expect_err("unsupported breaker/cache nodes fail resolution");
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

    reg_provider(&mut custom_registry, &CIRCUIT_BREAKER_CONTRACT, || {
        Box::new(EchoHandler)
    });
    reg_provider(&mut custom_registry, &CACHE_CONTRACT, || {
        Box::new(EchoHandler)
    });

    let resolved = custom_registry
        .resolve(&panel)
        .expect("breaker/cache panel resolves");

    let mut input = &b""[..];
    let mut output = Vec::new();
    let mut error = Vec::new();

    resolved
        .run(&mut RunIo {
            input: &mut input,
            output: &mut output,
            error: &mut error,
        })
        .expect("breaker/cache panel runs");

    assert_eq!(output, b"protected data");
}
