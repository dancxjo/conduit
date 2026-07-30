use conduit_panel::{Node, parse};
use conduit_runtime::{
    AvailabilityState, FILE_READ_CONTRACT, FILE_WRITE_CONTRACT, Handler, KV_STORE_CONTRACT,
    PROCESS_SPAWN_CONTRACT, Registry, RunIo, RuntimeError, Value,
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

struct ConsumerHandler;
impl Handler for ConsumerHandler {
    fn run(
        &mut self,
        _node: &Node,
        inputs: &[Value],
        io: &mut RunIo<'_>,
    ) -> Result<Vec<Value>, RuntimeError> {
        if let Some(val) = inputs.first() {
            let _ = io.output.write_all(&val.bytes);
        }
        Ok(vec![])
    }
}

#[test]
fn boundary_file_read_and_write_nodes_execute_within_panel() {
    let panel = parse(
        r#"
            panel 1
            node source : conduit/literal { value = "file payload" }
            node reader : conduit/file-read
            node writer : conduit/file-write
            cord source.out -> reader.in
            cord reader.out -> writer.in
        "#,
    )
    .expect("boundary panel parses");

    let registry = Registry::default();

    // Default registry returns contract-only availability and fails resolution
    let avail = registry.node_availability("conduit/file-read");
    assert_eq!(avail.state, AvailabilityState::ContractOnly);
    assert_eq!(avail.reason_code, "CND-AVL-001");
    assert_eq!(avail.rejection_reasons, vec!["CND-RES-008"]);

    let err = registry
        .resolve(&panel)
        .expect_err("unsupported file nodes fail resolution");
    assert_eq!(err.code, "CND-IMP-001");

    // Register concrete implementation providers
    let mut custom_registry = Registry::default();
    custom_registry.register_executable_node(
        &FILE_READ_CONTRACT,
        || Box::new(EchoHandler),
        |_| Ok(()),
    );
    custom_registry.register_executable_node(
        &FILE_WRITE_CONTRACT,
        || Box::new(ConsumerHandler),
        |_| Ok(()),
    );

    let resolved = custom_registry
        .resolve(&panel)
        .expect("boundary panel resolves");

    let mut input = &b""[..];
    let mut output = Vec::new();
    let mut error = Vec::new();

    resolved
        .run(&mut RunIo {
            input: &mut input,
            output: &mut output,
            error: &mut error,
        })
        .expect("boundary panel runs");

    assert_eq!(output, b"file payload");
}

#[test]
fn kv_store_and_process_spawn_nodes_resolve_and_execute() {
    let panel = parse(
        r#"
            panel 1
            node key_in : conduit/literal { value = "config_key" }
            node store : conduit/kv-store
            node proc : conduit/process-spawn
            node sink : conduit/stdout
            cord key_in.out -> store.in
            cord store.out -> proc.in
            cord proc.out -> sink.in
        "#,
    )
    .expect("kv/proc panel parses");

    let registry = Registry::default();

    let err = registry
        .resolve(&panel)
        .expect_err("unsupported kv/proc nodes fail resolution");
    assert_eq!(err.code, "CND-IMP-001");

    let mut custom_registry = Registry::default();
    custom_registry.register_executable_node(
        &KV_STORE_CONTRACT,
        || Box::new(EchoHandler),
        |_| Ok(()),
    );
    custom_registry.register_executable_node(
        &PROCESS_SPAWN_CONTRACT,
        || Box::new(EchoHandler),
        |_| Ok(()),
    );

    let resolved = custom_registry
        .resolve(&panel)
        .expect("kv/proc panel resolves");

    let mut input = &b""[..];
    let mut output = Vec::new();
    let mut error = Vec::new();

    resolved
        .run(&mut RunIo {
            input: &mut input,
            output: &mut output,
            error: &mut error,
        })
        .expect("kv/proc panel runs");

    assert_eq!(output, b"config_key");
}
