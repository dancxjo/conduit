use conduit_panel::parse;
use conduit_runtime::{Registry, RunIo};

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
    let resolved = registry.resolve(&panel).expect("boundary panel resolves");

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
    let resolved = registry.resolve(&panel).expect("kv/proc panel resolves");

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
