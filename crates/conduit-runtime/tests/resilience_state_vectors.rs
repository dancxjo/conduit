use conduit_panel::parse;
use conduit_runtime::{Registry, RunIo};

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
    let resolved = registry.resolve(&panel).expect("cell/dedup panel resolves");

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
    let resolved = registry
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
