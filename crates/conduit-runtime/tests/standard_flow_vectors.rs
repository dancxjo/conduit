use conduit_panel::parse;
use conduit_runtime::{Registry, RunIo};

#[test]
fn tee_node_duplicates_flow_to_multiple_sinks() {
    let panel = parse(
        r#"
            panel 1
            node source : conduit/literal { value = "flow data" }
            node splitter : conduit/tee
            node sink1 : conduit/stdout
            node sink2 : conduit/stderr
            cord source.out -> splitter.in
            cord splitter.out1 -> sink1.in
            cord splitter.out2 -> sink2.in
        "#,
    )
    .expect("tee panel parses");

    let registry = Registry::default();
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
            node primary : conduit/literal { value = "primary data" }
            node secondary : conduit/literal { value = "fallback data" }
            node router : conduit/fallback
            node sink : conduit/stdout
            cord primary.out -> router.primary
            cord secondary.out -> router.fallback
            cord router.out -> sink.in
        "#,
    )
    .expect("fallback panel parses");

    let registry = Registry::default();
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
            node src1 : conduit/literal { value = "merged" }
            node src2 : conduit/literal { value = "" }
            node pass : conduit/pass-through
            node combiner : conduit/merge
            node sink : conduit/stdout
            cord src1.out -> pass.in
            cord pass.out -> combiner.in1
            cord src2.out -> combiner.in2
            cord combiner.out -> sink.in
        "#,
    )
    .expect("merge panel parses");

    let registry = Registry::default();
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
