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
    custom_registry.register_executable_node(&CELL_CONTRACT, || Box::new(EchoHandler), |_| Ok(()));
    custom_registry.register_executable_node(
        &DEDUPLICATE_CONTRACT,
        || Box::new(EchoHandler),
        |_| Ok(()),
    );

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
    custom_registry.register_executable_node(
        &CIRCUIT_BREAKER_CONTRACT,
        || Box::new(EchoHandler),
        |_| Ok(()),
    );
    custom_registry.register_executable_node(&CACHE_CONTRACT, || Box::new(EchoHandler), |_| Ok(()));

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
