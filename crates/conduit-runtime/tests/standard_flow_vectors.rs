use conduit_panel::parse;
use conduit_runtime::{Registry, RunIo};

#[test]
fn tee_node_duplicates_flow_to_multiple_sinks() {
    let panel = parse(
        r#"
            panel 1
            node source : conduit.std/literal { value = "flow data" }
            node splitter : conduit.std/tee
            node sink1 : conduit.std/stdout
            node sink2 : conduit.std/stderr
            cord source.out -> splitter.in
            cord splitter.out1 -> sink1.in
            cord splitter.out2 -> sink2.in
        "#,
    )
    .expect("tee panel parses");

    let registry = Registry::compatibility_demo();
    let resolved = registry.resolve(&panel).expect("tee panel resolves");

    let mut input = &b""[..];
    let mut output = Vec::new();
    let mut error = Vec::new();

    resolved
        .run(&mut RunIo {
            input: &mut input,
            output: &mut output,
            error: &mut error,
        })
        .expect("tee panel executes");

    assert_eq!(output, b"flow data");
    assert_eq!(error, b"flow data");
}

#[test]
fn fallback_node_selects_primary_or_fallback() {
    let panel = parse(
        r#"
            panel 1
            node primary : conduit.std/literal { value = "primary data" }
            node secondary : conduit.std/literal { value = "fallback data" }
            node router : conduit.std/fallback
            node sink : conduit.std/stdout
            cord primary.out -> router.primary
            cord secondary.out -> router.fallback
            cord router.out -> sink.in
        "#,
    )
    .expect("fallback panel parses");

    let registry = Registry::compatibility_demo();
    let resolved = registry.resolve(&panel).expect("fallback panel resolves");

    let mut input = &b""[..];
    let mut output = Vec::new();
    let mut error = Vec::new();

    resolved
        .run(&mut RunIo {
            input: &mut input,
            output: &mut output,
            error: &mut error,
        })
        .expect("fallback panel executes");

    assert_eq!(output, b"primary data");
}

#[test]
fn pass_through_and_merge_nodes_shape_flow() {
    let panel = parse(
        r#"
            panel 1
            node src1 : conduit.std/literal { value = "merged" }
            node src2 : conduit.std/literal { value = "" }
            node pass : conduit.std/pass-through
            node combiner : conduit.std/merge
            node sink : conduit.std/stdout
            cord src1.out -> pass.in
            cord pass.out -> combiner.in1
            cord src2.out -> combiner.in2
            cord combiner.out -> sink.in
        "#,
    )
    .expect("merge panel parses");

    let registry = Registry::compatibility_demo();
    let resolved = registry.resolve(&panel).expect("merge panel resolves");

    let mut input = &b""[..];
    let mut output = Vec::new();
    let mut error = Vec::new();

    resolved
        .run(&mut RunIo {
            input: &mut input,
            output: &mut output,
            error: &mut error,
        })
        .expect("merge panel executes");

    assert_eq!(output, b"merged");
}
